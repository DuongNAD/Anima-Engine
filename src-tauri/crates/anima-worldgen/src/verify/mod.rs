//! Unified verification orchestrator for World v2 (`G-SHAPE`, `G-COMP v2`, `G-SIM` raw checks,
//! `sim_256.anmw3` projection, `index.json`, and `gates.json`).

pub mod g_comp;
pub mod g_shape;

use crate::anmw_v3::{
    compute_sim_world_identity, compute_strahler_order, project_raw_dir_to_sim_256, sha256_to_hex,
    AnmwV3Sim, ANMW_V3_SIM_SIZE,
};
use crate::caves::CaveNetwork;
use crate::pipeline::{read_bin_f32, read_bin_u8, read_bin_u8x4, PipelineError, RawWorldMeta};
use crate::{normalize_elevation, WaterKind, SEA_LEVEL_M};
use g_comp::{evaluate_c4_c5_and_flags, finalize_g_comp_v2, GCompV2Report};
use g_shape::{evaluate_g_shape, GShapeInput, GShapeReport};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::io::Read;
use std::path::Path;

const DEEP_OCEAN_THRESHOLD_M: f32 = -220.0;
const DX8: [isize; 8] = [1, 1, 0, -1, -1, -1, 0, 1];
const DZ8: [isize; 8] = [0, 1, 1, 1, 0, -1, -1, -1];

#[inline]
fn pipeline_err(message: impl Into<String>) -> PipelineError {
    PipelineError {
        message: message.into(),
    }
}

#[inline]
fn is_oceanic_fine(wk: u8, h_m: f32) -> bool {
    wk == WaterKind::Ocean as u8 || (wk == WaterKind::DeltaTurbid as u8 && h_m <= SEA_LEVEL_M + 0.1)
}

#[inline]
fn is_lake_fine(wk: u8) -> bool {
    wk == WaterKind::FreshwaterLake as u8 || wk == WaterKind::EndorheicLake as u8
}

/// Static `GS-1` / `GS-2` raw simulation-projection checks emitted by `anima-worldgen verify`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GSimRawReport {
    pub passed: bool,
    pub sim_seed_u32: u32,
    pub sim_world_identity: String,
    pub sim_256_byte_len: usize,
    pub max_mean_norm_elev_diff: f64,
    pub all_biomes_in_range: bool,
    pub deep_ocean_cells: usize,
    pub ocean_cells: usize,
    pub water_mask_mismatches: usize,
    pub strahler_ge4_blocks: usize,
    pub strahler_ge4_block_mismatches: usize,
    pub strahler_ge4_disconnected_stems: usize,
    pub walkable_cells: usize,
    pub largest_walkable_component: usize,
    pub largest_walkable_fraction: f64,
    pub walkable_shoreline_cells: usize,
    pub round_trip_ok: bool,
}

/// Top-level machine-readable verification gate report (`artifacts/world-v2/gates.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationReport {
    pub schema_version: u32,
    pub mode: String,
    pub seed: Option<u64>,
    pub schema_tag: Option<String>,
    pub width: usize,
    pub height: usize,
    pub dx_m: f32,
    pub extent_m: f32,
    pub world_sha256: Option<String>,
    pub sim_world_identity: Option<String>,
    pub overall_pass: bool,
    pub failed_gates: Vec<String>,
    pub g_shape: GShapeReport,
    pub g_comp_v2: Option<GCompV2Report>,
    pub g_sim_raw: Option<GSimRawReport>,
}

/// Deterministic `artifacts/world-v2/index.json` skeleton (sorted keys via `BTreeMap` fields).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldV2IndexJson {
    pub extent_m: u32,
    pub format_version: u32,
    pub generator_version: u32,
    pub grid_size: u32,
    pub release_archive: Option<String>,
    pub release_archive_sha256: Option<String>,
    pub schema_tag: String,
    pub seed: u64,
    pub sim_256_byte_len: usize,
    pub sim_256_file: String,
    pub sim_world_identity: String,
    pub tiles: Vec<serde_json::Value>,
}

impl WorldV2IndexJson {
    /// Serializes `index.json` with deterministically sorted JSON keys at every object level.
    pub fn to_canonical_json_bytes(&self) -> Result<Vec<u8>, PipelineError> {
        let val = serde_json::to_value(self)?;
        let sorted = canonicalize_json_value(val);
        let mut s = serde_json::to_string_pretty(&sorted)?;
        s.push('\n');
        Ok(s.into_bytes())
    }
}

fn canonicalize_json_value(val: serde_json::Value) -> serde_json::Value {
    match val {
        serde_json::Value::Object(map) => {
            let mut btree = BTreeMap::new();
            for (k, v) in map {
                btree.insert(k, canonicalize_json_value(v));
            }
            serde_json::Value::Object(btree.into_iter().collect())
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(canonicalize_json_value).collect())
        }
        other => other,
    }
}

struct SimBlockGroundTruth {
    block: usize,
    expected_norm_mean: Vec<f64>,
    expected_elev_m_mean: Vec<f64>,
    ocean_raw_counts: Vec<usize>,
    lake_raw_counts: Vec<usize>,
    major_fine_starts: Vec<u32>,
    major_trace_dirs: Vec<u8>,
    fine_is_water_body: Vec<bool>,
}

fn build_sim_ground_truth(
    w: usize,
    h: usize,
    total_h_m: &[f32],
    water_kind: &[u8],
    d8_dir: &[u8],
    river_w_m: &[f32],
) -> Option<SimBlockGroundTruth> {
    let sim = ANMW_V3_SIM_SIZE as usize;
    if w < sim || h != w || !w.is_multiple_of(sim) {
        return None;
    }
    let block = w / sim;
    let sim_n = sim * sim;
    let mut expected_norm_mean = vec![0.0_f64; sim_n];
    let mut expected_elev_m_mean = vec![0.0_f64; sim_n];
    let mut ocean_raw_counts = vec![0_usize; sim_n];
    let mut lake_raw_counts = vec![0_usize; sim_n];

    let block_cells = (block * block) as f64;
    for by in 0..sim {
        for bx in 0..sim {
            let mut sum_norm = 0.0_f64;
            let mut sum_m = 0.0_f64;
            let mut oc = 0_usize;
            let mut lk = 0_usize;
            for dy in 0..block {
                let r = by * block + dy;
                let row = r * w;
                for dx in 0..block {
                    let sidx = row + bx * block + dx;
                    let m = total_h_m[sidx];
                    sum_m += f64::from(m);
                    sum_norm += f64::from(normalize_elevation(m));
                    let wk = water_kind[sidx];
                    if is_oceanic_fine(wk, m) {
                        oc += 1;
                    } else if is_lake_fine(wk) {
                        lk += 1;
                    }
                }
            }
            let bi = by * sim + bx;
            expected_norm_mean[bi] = sum_norm / block_cells;
            expected_elev_m_mean[bi] = sum_m / block_cells;
            ocean_raw_counts[bi] = oc;
            lake_raw_counts[bi] = lk;
        }
    }

    let n = w * h;
    let sim_is_channel: Vec<bool> = river_w_m
        .iter()
        .zip(water_kind.iter())
        .zip(total_h_m.iter())
        .map(|((&rw, &wk), &hm)| {
            !is_oceanic_fine(wk, hm)
                && !is_lake_fine(wk)
                && (rw >= 0.5 || wk == WaterKind::River as u8 || wk == WaterKind::Waterfall as u8)
        })
        .collect();
    let sim_strahler = compute_strahler_order(d8_dir, &sim_is_channel, w, h);

    let mut major_fine_starts = Vec::new();
    let mut fine_is_water_body = vec![false; n];
    for i in 0..n {
        let wk = water_kind[i];
        let hm = total_h_m[i];
        let water_body = is_oceanic_fine(wk, hm) || is_lake_fine(wk);
        fine_is_water_body[i] = water_body;
        if !water_body && sim_is_channel[i] && (sim_strahler[i] >= 4 || river_w_m[i] >= 8.0) {
            major_fine_starts.push(i as u32);
        }
    }
    drop(sim_is_channel);
    drop(sim_strahler);

    Some(SimBlockGroundTruth {
        block,
        expected_norm_mean,
        expected_elev_m_mean,
        ocean_raw_counts,
        lake_raw_counts,
        major_fine_starts,
        major_trace_dirs: d8_dir.to_vec(),
        fine_is_water_body,
    })
}

fn verify_gsim_on_raw_projection(
    raw_dir: &Path,
    width: usize,
    generator_version: u32,
    gt: &SimBlockGroundTruth,
) -> Result<(GSimRawReport, Vec<u8>), PipelineError> {
    let sim = project_raw_dir_to_sim_256(raw_dir, generator_version)
        .map_err(|e| pipeline_err(format!("ANMW v3 projection failed: {e}")))?;
    let sim_bytes = sim
        .to_bytes()
        .map_err(|e| pipeline_err(format!("ANMW v3 encode failed: {e}")))?;
    let decoded = AnmwV3Sim::from_bytes(&sim_bytes)
        .map_err(|e| pipeline_err(format!("ANMW v3 round-trip decode failed: {e}")))?;
    let peek_seed = AnmwV3Sim::peek_seed(&sim_bytes)
        .map_err(|e| pipeline_err(format!("ANMW v3 peek_seed failed: {e}")))?;
    let round_trip_ok = decoded == sim && peek_seed == sim.sim_seed_u32;

    let digest = compute_sim_world_identity(&sim_bytes);
    let sim_world_identity = sha256_to_hex(&digest);

    let sim_dim = ANMW_V3_SIM_SIZE as usize;
    let sim_n = sim_dim * sim_dim;
    let backend = sim.backend_biomes();
    let all_biomes_in_range = backend.len() == sim_n && backend.iter().all(|&b| b <= 10);

    let mut max_diff = 0.0_f64;
    let mut deep_ocean_cells = 0_usize;
    let mut ocean_cells = 0_usize;
    let mut water_mask_mismatches = 0_usize;
    let block_cells = gt.block * gt.block;

    for (bi, &b_val) in backend.iter().enumerate().take(sim_n) {
        let diff = (f64::from(decoded.elevation[bi]) - gt.expected_norm_mean[bi]).abs();
        max_diff = max_diff.max(diff);

        let block_is_ocean = gt.ocean_raw_counts[bi] * 2 >= block_cells;
        let block_is_lake = !block_is_ocean && gt.lake_raw_counts[bi] * 2 >= block_cells;
        let expect_water = block_is_ocean || block_is_lake;
        let actual_ocean = b_val == 0 || b_val == 1;
        if actual_ocean != expect_water {
            water_mask_mismatches += 1;
        }
        if block_is_ocean && gt.expected_elev_m_mean[bi] < f64::from(DEEP_OCEAN_THRESHOLD_M) {
            if b_val != 0 {
                water_mask_mismatches += 1;
            }
            deep_ocean_cells += 1;
        } else if actual_ocean {
            if b_val != 1 {
                water_mask_mismatches += 1;
            }
            ocean_cells += 1;
        }
    }

    // Check Strahler >= 4 / >= 8 m block coverage & 8-connected downstream trace on 256^2.
    let mut strahler_ge4_blocks = 0_usize;
    let mut seen_block = vec![false; sim_n];
    let mut strahler_ge4_block_mismatches = 0_usize;
    for &s_u32 in &gt.major_fine_starts {
        let su = s_u32 as usize;
        let bx = (su % width) / gt.block;
        let by = (su / width) / gt.block;
        let bi = by * sim_dim + bx;
        if !seen_block[bi] {
            seen_block[bi] = true;
            strahler_ge4_blocks += 1;
            if backend[bi] != 0 && backend[bi] != 1 && backend[bi] != 3 {
                strahler_ge4_block_mismatches += 1;
            }
        }
    }

    let mut disconnected_stems = 0_usize;
    let mut checked_fine = vec![false; width * width];
    let stem_stride = (gt.major_fine_starts.len() / 256).max(1);
    for &s_u32 in gt.major_fine_starts.iter().step_by(stem_stride) {
        let mut cur = s_u32 as usize;
        let mut prev_b: Option<(usize, usize)> = None;
        let mut steps = 0_usize;
        while steps < width * width {
            steps += 1;
            if gt.fine_is_water_body[cur] {
                break;
            }
            let bx = (cur % width) / gt.block;
            let by = (cur / width) / gt.block;
            let bi = by * sim_dim + bx;
            if backend[bi] == 0 || backend[bi] == 1 {
                break;
            }
            if let Some((px, py)) = prev_b {
                if bx.abs_diff(px) > 1 || by.abs_diff(py) > 1 {
                    disconnected_stems += 1;
                    break;
                }
            }
            if backend[bi] != 3 {
                disconnected_stems += 1;
                break;
            }
            if checked_fine[cur] {
                break;
            }
            checked_fine[cur] = true;
            prev_b = Some((bx, by));
            let d = gt.major_trace_dirs[cur];
            if d >= 8 {
                break;
            }
            let nr = (cur / width) as isize + DZ8[usize::from(d)];
            let nc = (cur % width) as isize + DX8[usize::from(d)];
            if nr < 0 || nc < 0 || nr >= width as isize || nc >= width as isize {
                break;
            }
            let nxt = (nr as usize) * width + (nc as usize);
            if nxt == cur {
                break;
            }
            cur = nxt;
        }
    }

    // GS-2 Static LAND1 Walkability & Connectivity on `256^2` (`>= 80%` component, `>= 500` shore).
    let mut walkable_cells = 0_usize;
    let mut largest_walkable_component = 0_usize;
    let mut walkable_shoreline_cells = 0_usize;
    let mut visited = vec![false; sim_n];
    let mut q = VecDeque::new();
    for by in 0..sim_dim {
        for bx in 0..sim_dim {
            let bi = by * sim_dim + bx;
            if backend[bi] < 2 {
                continue;
            }
            walkable_cells += 1;
            let next_to_water = [
                (bx > 0, bi.wrapping_sub(1)),
                (bx + 1 < sim_dim, bi + 1),
                (by > 0, bi.wrapping_sub(sim_dim)),
                (by + 1 < sim_dim, bi + sim_dim),
            ]
            .iter()
            .any(|&(ok, nb)| ok && (backend[nb] <= 1 || backend[nb] == 3));
            if next_to_water {
                walkable_shoreline_cells += 1;
            }
            if visited[bi] {
                continue;
            }
            visited[bi] = true;
            q.push_back((bx, by));
            let mut comp = 0_usize;
            while let Some((cx, cy)) = q.pop_front() {
                comp += 1;
                for (dc, dr) in [(-1isize, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = cx as isize + dc;
                    let ny = cy as isize + dr;
                    if (0..sim_dim as isize).contains(&nx) && (0..sim_dim as isize).contains(&ny) {
                        let nb = (ny as usize) * sim_dim + (nx as usize);
                        if !visited[nb] && backend[nb] >= 2 {
                            visited[nb] = true;
                            q.push_back((nx as usize, ny as usize));
                        }
                    }
                }
            }
            largest_walkable_component = largest_walkable_component.max(comp);
        }
    }

    let largest_walkable_fraction = if walkable_cells > 0 {
        largest_walkable_component as f64 / walkable_cells as f64
    } else {
        0.0
    };

    let gs1_ok = round_trip_ok
        && max_diff <= 1e-5
        && all_biomes_in_range
        && water_mask_mismatches == 0
        && strahler_ge4_blocks > 0
        && strahler_ge4_block_mismatches == 0
        && disconnected_stems == 0;
    let gs2_ok = largest_walkable_fraction >= 0.80 && walkable_shoreline_cells >= 500;

    Ok((
        GSimRawReport {
            passed: gs1_ok && gs2_ok,
            sim_seed_u32: sim.sim_seed_u32,
            sim_world_identity,
            sim_256_byte_len: sim_bytes.len(),
            max_mean_norm_elev_diff: max_diff,
            all_biomes_in_range,
            deep_ocean_cells,
            ocean_cells,
            water_mask_mismatches,
            strahler_ge4_blocks,
            strahler_ge4_block_mismatches,
            strahler_ge4_disconnected_stems: disconnected_stems,
            walkable_cells,
            largest_walkable_component,
            largest_walkable_fraction,
            walkable_shoreline_cells,
            round_trip_ok,
        },
        sim_bytes,
    ))
}

fn collect_failed_gates(
    g_shape: &GShapeReport,
    g_comp: Option<&GCompV2Report>,
    g_sim: Option<&GSimRawReport>,
) -> Vec<String> {
    let mut failed = Vec::new();
    if !g_shape.metric_1_strahler_horton.passed {
        failed.push("G-SHAPE.1.StrahlerHorton".to_owned());
    }
    if !g_shape.metric_2_junction_angles.passed {
        failed.push("G-SHAPE.2.JunctionAngles".to_owned());
    }
    if !g_shape.metric_3_drainage_density.passed {
        failed.push("G-SHAPE.3.DrainageDensity".to_owned());
    }
    if !g_shape.metric_4_hack_law.passed {
        failed.push("G-SHAPE.4.HackLaw".to_owned());
    }
    if !g_shape.metric_5_valleys_flint.passed {
        failed.push("G-SHAPE.5.ValleysFlint".to_owned());
    }
    if !g_shape.metric_6_endorheic_spillways.passed {
        failed.push("G-SHAPE.6.EndorheicSpillways".to_owned());
    }
    if !g_shape.metric_7_hypsometry_spikes.passed {
        failed.push("G-SHAPE.7.HypsometrySpikes".to_owned());
    }
    if !g_shape.metric_4_2_shoreline_fractal.passed {
        failed.push("G-SHAPE.4_2.ShorelineFractal".to_owned());
    }
    if let Some(c) = g_comp {
        if !c.c1_partition.passed {
            failed.push("G-COMP-v2.C1.Partition".to_owned());
        }
        if !c.c2_ecotone.passed {
            failed.push("G-COMP-v2.C2.Ecotone".to_owned());
        }
        if !c.c3_lakes.passed {
            failed.push("G-COMP-v2.C3.Lakes".to_owned());
        }
        if !c.c4_rivers_fans_deltas.passed {
            failed.push("G-COMP-v2.C4.RiversFansDeltas".to_owned());
        }
        if !c.c5_coastal_sightlines.passed {
            failed.push("G-COMP-v2.C5.CoastalSightlines".to_owned());
        }
    }
    if let Some(s) = g_sim {
        if !s.round_trip_ok
            || s.max_mean_norm_elev_diff > 1e-5
            || !s.all_biomes_in_range
            || s.water_mask_mismatches > 0
            || s.strahler_ge4_blocks == 0
            || s.strahler_ge4_block_mismatches > 0
            || s.strahler_ge4_disconnected_stems > 0
        {
            failed.push("G-SIM.GS-1.GridParity".to_owned());
        }
        if s.largest_walkable_fraction < 0.80 || s.walkable_shoreline_cells < 500 {
            failed.push("G-SIM.GS-2.Walkability".to_owned());
        }
    }
    failed
}

/// Verifies a full `anima-worldgen generate` output directory (`raw_meta.json` + spilled layers)
/// sequentially in memory to keep `4096^2` verification peak RAM under `~1 GiB`.
pub fn verify_raw_dir(
    raw_dir: &Path,
    generator_version: u32,
    out_sim_path: Option<&Path>,
    out_index_path: Option<&Path>,
) -> Result<VerificationReport, PipelineError> {
    let meta_bytes = fs::read(raw_dir.join("raw_meta.json"))?;
    let meta: RawWorldMeta = serde_json::from_slice(&meta_bytes)?;
    let w = meta.width as usize;
    let h = meta.height as usize;
    let n = w * h;

    // Phase A: Terrain, Climate, Hydrology & Coastal/River Gates (`G-SHAPE`, `C4`, `C5`, `GS-1` summary).
    let total_h_m = read_bin_f32(&raw_dir.join("total_h_m.bin"), n)?;
    let water_h_m = read_bin_f32(&raw_dir.join("water_h_m.bin"), n)?;
    let water_kind = read_bin_u8(&raw_dir.join("water_kind.bin"), n)?;
    let river_w_m = read_bin_f32(&raw_dir.join("river_w_m.bin"), n)?;
    let glacier_h_m = read_bin_f32(&raw_dir.join("glacier_h_m.bin"), n)?;
    let lithology_id = read_bin_u8(&raw_dir.join("lithology_id.bin"), n)?;
    let precip_m_yr = read_bin_f32(&raw_dir.join("precip_m_yr.bin"), n)?;
    let pet_m_yr = read_bin_f32(&raw_dir.join("pet_m_yr.bin"), n)?;

    let shape_in = GShapeInput {
        width: w,
        height: h,
        dx_m: meta.dx_m,
        sea_level_m: meta.sea_level_m,
        total_h_m: &total_h_m,
        precip_m_yr: Some(&precip_m_yr),
        pet_m_yr: Some(&pet_m_yr),
        river_w_m: Some(&river_w_m),
        water_kind: Some(&water_kind),
        glacier_h_m: Some(&glacier_h_m),
        lithology_id: Some(&lithology_id),
        basins: Some(&meta.basins),
        waterfalls: Some(&meta.waterfalls),
        reported_unmodeled_micro_pits: Some(meta.stats.unmodeled_micro_pits),
        reported_severed_humid_spillways: Some(meta.stats.severed_humid_spillways),
        reported_endorheic_land_fraction: Some(meta.stats.endorheic_land_fraction),
        reported_endorheic_humid_fraction: Some(meta.stats.endorheic_humid_fraction),
        min_hack_samples: if n >= 512 * 512 { 500 } else { 20 },
        has_climate_fields: true,
    };
    let shape_out = evaluate_g_shape(&shape_in);
    drop(precip_m_yr);
    drop(pet_m_yr);
    drop(lithology_id);

    let c4_c5_summary = evaluate_c4_c5_and_flags(
        w,
        h,
        meta.dx_m,
        meta.sea_level_m,
        &total_h_m,
        &water_h_m,
        &water_kind,
        &glacier_h_m,
        &river_w_m,
        &shape_out.is_channel,
        &shape_out.mean_width_by_order,
        &shape_out.report.metric_4_hack_law,
        meta.stats.fan_count,
        meta.stats.delta_count,
    );
    drop(glacier_h_m);
    drop(water_h_m);

    let sim_gt =
        build_sim_ground_truth(w, h, &total_h_m, &water_kind, &shape_out.d8_dir, &river_w_m);
    drop(total_h_m);
    drop(river_w_m);
    drop(shape_out.d8_dir);
    drop(shape_out.is_channel);
    drop(shape_out.strahler);
    let g_shape = shape_out.report;

    // Phase B: Biomes, Lakes & Caves (`G-COMP v2` `C1..C3`).
    let dominant_biome = read_bin_u8(&raw_dir.join("dominant_biome.bin"), n)?;
    let biome_ids = read_bin_u8x4(&raw_dir.join("biome_ids.bin"), n)?;
    let biome_weights = read_bin_u8x4(&raw_dir.join("biome_weights.bin"), n)?;
    let cave_bytes = fs::read(raw_dir.join("cave_network.bin"))?;
    let cave_network = CaveNetwork::from_bytes(&cave_bytes)
        .map_err(|e| pipeline_err(format!("cave_network.bin decode failed: {e}")))?;
    drop(cave_bytes);

    let g_comp_v2 = finalize_g_comp_v2(
        w,
        h,
        meta.dx_m,
        meta.extent_m,
        &water_kind,
        &dominant_biome,
        &biome_ids,
        &biome_weights,
        &meta.waterfalls,
        &meta.basins,
        meta.stats.micro_pond_count,
        &cave_network,
        c4_c5_summary,
    );
    drop(dominant_biome);
    drop(biome_ids);
    drop(biome_weights);
    drop(water_kind);
    drop(cave_network);

    // Phase C: `sim_256.anmw3` projection & static `GS-1`/`GS-2` check (`§5.2`).
    let (g_sim_raw, sim_bytes_opt) = if let Some(gt) = &sim_gt {
        let (rep, bytes) = verify_gsim_on_raw_projection(raw_dir, w, generator_version, gt)?;
        (Some(rep), Some(bytes))
    } else if out_sim_path.is_some() || out_index_path.is_some() {
        return Err(pipeline_err(format!(
            "cannot project sim_256.anmw3 from non-divisible grid {w}x{h}"
        )));
    } else {
        (None, None)
    };

    if let (Some(sim_path), Some(sim_bytes)) = (out_sim_path, sim_bytes_opt.as_ref()) {
        if let Some(parent) = sim_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(sim_path, sim_bytes)?;
    }

    if let (Some(idx_path), Some(sim_rep)) = (out_index_path, g_sim_raw.as_ref()) {
        if let Some(parent) = idx_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let idx = WorldV2IndexJson {
            extent_m: meta.extent_m.round() as u32,
            format_version: 1,
            generator_version,
            grid_size: meta.width,
            release_archive: None,
            release_archive_sha256: None,
            schema_tag: meta.schema_tag.clone(),
            seed: meta.seed,
            sim_256_byte_len: sim_rep.sim_256_byte_len,
            sim_256_file: "sim_256.anmw3".to_owned(),
            sim_world_identity: sim_rep.sim_world_identity.clone(),
            tiles: Vec::new(),
        };
        let bytes = idx.to_canonical_json_bytes()?;
        fs::write(idx_path, bytes)?;
    }

    let sim_world_identity = g_sim_raw.as_ref().map(|s| s.sim_world_identity.clone());
    let failed_gates = collect_failed_gates(&g_shape, Some(&g_comp_v2), g_sim_raw.as_ref());
    let overall_pass = failed_gates.is_empty();

    Ok(VerificationReport {
        schema_version: 1,
        mode: "raw".to_owned(),
        seed: Some(meta.seed),
        schema_tag: Some(meta.schema_tag),
        width: w,
        height: h,
        dx_m: meta.dx_m,
        extent_m: meta.extent_m,
        world_sha256: Some(meta.world_sha256),
        sim_world_identity,
        overall_pass,
        failed_gates,
        g_shape,
        g_comp_v2: Some(g_comp_v2),
        g_sim_raw,
    })
}

/// Reads a flat little-endian `f32` DEM patch (`--dem-input` mode, §4.2) and runs `G-SHAPE`.
pub fn verify_dem_file(
    dem_path: &Path,
    width: usize,
    height: usize,
    dx_m: f32,
    sea_level_m: f32,
) -> Result<VerificationReport, PipelineError> {
    if width < 16 || height < 16 || !dx_m.is_finite() || dx_m <= 0.0 {
        return Err(pipeline_err(format!(
            "invalid DEM geometry {width}x{height} dx={dx_m}"
        )));
    }
    let expected_bytes = width * height * 4;
    let mut f = fs::File::open(dem_path)?;
    let meta = f.metadata()?;
    if meta.len() != expected_bytes as u64 {
        return Err(pipeline_err(format!(
            "DEM file size mismatch: expected {expected_bytes} bytes for {width}x{height}, got {}",
            meta.len()
        )));
    }
    let mut raw = vec![0_u8; expected_bytes];
    f.read_exact(&mut raw)?;
    let mut total_h_m = Vec::with_capacity(width * height);
    for &chunk in raw.as_chunks::<4>().0 {
        let v = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        if !v.is_finite() {
            return Err(pipeline_err("DEM contains NaN or Inf values"));
        }
        total_h_m.push(v);
    }
    drop(raw);

    let g_shape_out = evaluate_g_shape(&GShapeInput::from_dem_only(
        &total_h_m,
        width,
        height,
        dx_m,
        sea_level_m,
    ));
    let failed_gates = collect_failed_gates(&g_shape_out.report, None, None);
    let overall_pass = failed_gates.is_empty();

    Ok(VerificationReport {
        schema_version: 1,
        mode: "dem_input".to_owned(),
        seed: None,
        schema_tag: None,
        width,
        height,
        dx_m,
        extent_m: width as f32 * dx_m,
        world_sha256: None,
        sim_world_identity: None,
        overall_pass,
        failed_gates,
        g_shape: g_shape_out.report,
        g_comp_v2: None,
        g_sim_raw: None,
    })
}
