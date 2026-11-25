//! `G-COMP v2` ecological, hydrologic, lake, fan/delta, and coastal gates (`§5.1` `C1..C5`).

use crate::caves::CaveNetwork;
use crate::lakes_falls::{LakeBasinRecord, WaterfallRecord};
use crate::verify::g_shape::HackLawReport;
use crate::{BiomeId, WaterKind, BIOME_COUNT};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

const MIN_PATCH_AREA_M2: f64 = 250_000.0; // 2,500 cells @ 10 m (`4096^2`), 0.25 km^2.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiomePatchStat {
    pub biome_id: u8,
    pub biome_name: String,
    pub total_cells: usize,
    pub max_connected_cells: usize,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct C1PartitionReport {
    pub passed: bool,
    pub ocean_fraction: f64,
    pub ocean_cells: usize,
    pub waterfall_count: usize,
    pub cave_entrance_count: usize,
    pub min_cave_spacing_m: f64,
    pub min_required_cave_spacing_m: f64,
    pub invalid_cave_placements: usize,
    pub min_patch_cells_required: usize,
    pub zoned_16_biomes_passing: usize,
    pub chaparral_cells: usize,
    pub steppe_cells: usize,
    pub biome_stats: Vec<BiomePatchStat>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct C2EcotoneReport {
    pub passed: bool,
    pub land_cells: usize,
    pub ecotone_cells: usize,
    pub ecotone_fraction: f64,
    pub non_cliff_land_edges: usize,
    pub sharp_jump_edges: usize,
    pub non_cliff_jump_fraction: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct C3LakeDiversityReport {
    pub passed: bool,
    pub lake_surface_fraction: f64,
    pub total_mapped_basins: usize,
    pub large_lake_count: usize,
    pub high_paramo_lake_count: usize,
    pub caldera_or_trough_lake_count: usize,
    pub small_paramo_pond_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrahlerWidthEntry {
    pub order: u8,
    pub cell_count: usize,
    pub mean_width_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct C4RiverProgressionReport {
    pub passed: bool,
    pub counted_river_surface_fraction: f64,
    pub active_channel_cells: usize,
    pub width_by_order: Vec<StrahlerWidthEntry>,
    pub monotonic_w2_w3_w4: bool,
    pub hack_consistent: bool,
    pub fan_count: usize,
    pub delta_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct C5CoastalSightlinesReport {
    pub passed: bool,
    pub coastline_cells: usize,
    pub gentle_sandy_fraction: f64,
    pub steep_cliff_fraction: f64,
    pub sightline_checks: usize,
    pub obstructed_sightlines: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GCompV2Report {
    pub passed: bool,
    pub c1_partition: C1PartitionReport,
    pub c2_ecotone: C2EcotoneReport,
    pub c3_lakes: C3LakeDiversityReport,
    pub c4_rivers_fans_deltas: C4RiverProgressionReport,
    pub c5_coastal_sightlines: C5CoastalSightlinesReport,
}

/// Phase-A compact terrain/hydrology summary evaluated before elevation buffers are dropped.
pub struct C4C5TerrainSummary {
    pub c4: C4RiverProgressionReport,
    pub c5: C5CoastalSightlinesReport,
    /// Per-cell bitflags: bit 0 = subaerial (`total_h_m > sea_level_m`), bit 1 = water body/strip
    /// (`Ocean | FreshwaterLake | EndorheicLake | DeltaTurbid <= sea_level`), bit 2 = glacier,
    /// bit 3 = non-cliff `water_kind == None && slope <= 0.70`.
    pub cell_flags: Vec<u8>,
}

pub const FLAG_SUBAERIAL: u8 = 1 << 0;
pub const FLAG_OCEAN_OR_LAKE: u8 = 1 << 1;
pub const FLAG_GLACIER: u8 = 1 << 2;
pub const FLAG_NON_CLIFF_DRY_LAND: u8 = 1 << 3;
pub const FLAG_WATER_NONE: u8 = 1 << 4;

fn biome_display_name(b: BiomeId) -> &'static str {
    match b {
        BiomeId::Ocean => "Ocean",
        BiomeId::Beach => "Beach",
        BiomeId::Desert => "Desert",
        BiomeId::Savanna => "Savanna",
        BiomeId::Grassland => "Grassland",
        BiomeId::Shrubland => "Shrubland",
        BiomeId::Forest => "Forest",
        BiomeId::Jungle => "Jungle",
        BiomeId::Taiga => "Taiga",
        BiomeId::Tundra => "Tundra",
        BiomeId::Swamp => "Swamp",
        BiomeId::Rock => "Rock",
        BiomeId::Snow => "Snow",
        BiomeId::River => "River",
        BiomeId::Lake => "Lake",
        BiomeId::Mangrove => "Mangrove",
        BiomeId::Chaparral => "Chaparral",
        BiomeId::Steppe => "Steppe",
        BiomeId::Alpine => "Alpine",
        BiomeId::Badlands => "Badlands",
        BiomeId::Glacier => "Glacier",
        BiomeId::Bog => "Bog",
    }
}

/// Evaluates `C4`, `C5`, and compact `cell_flags` while terrain/hydrology arrays are resident.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_c4_c5_and_flags(
    width: usize,
    height: usize,
    dx_m: f32,
    sea_level_m: f32,
    total_h_m: &[f32],
    water_h_m: &[f32],
    water_kind: &[u8],
    glacier_h_m: &[f32],
    river_w_m: &[f32],
    is_channel: &[bool],
    mean_width_by_order: &[(u8, usize, f64)],
    hack_report: &HackLawReport,
    fan_count: usize,
    delta_count: usize,
) -> C4C5TerrainSummary {
    let n = width * height;
    let dx = f64::from(dx_m.max(1e-3));
    let mut cell_flags = vec![0_u8; n];

    let mut river_amt_sum = 0.0_f64;
    let mut active_chan_cells = 0_usize;

    for r in 0..height {
        let row = r * width;
        let r_u = r.saturating_sub(1);
        let r_d = (r + 1).min(height - 1);
        for c in 0..width {
            let i = row + c;
            let h = total_h_m[i];
            let wk = water_kind[i];
            let is_oc_or_lake = wk == WaterKind::Ocean as u8
                || wk == WaterKind::FreshwaterLake as u8
                || wk == WaterKind::EndorheicLake as u8
                || (wk == WaterKind::DeltaTurbid as u8 && h <= sea_level_m + 0.1)
                || h <= sea_level_m;

            let mut fl = 0_u8;
            if h > sea_level_m {
                fl |= FLAG_SUBAERIAL;
            }
            if is_oc_or_lake {
                fl |= FLAG_OCEAN_OR_LAKE;
            }
            if glacier_h_m[i] > 0.01 {
                fl |= FLAG_GLACIER;
            }
            if wk == WaterKind::None as u8 {
                fl |= FLAG_WATER_NONE;
            }

            let c_l = c.saturating_sub(1);
            let c_r = (c + 1).min(width - 1);
            let gx = (f64::from(total_h_m[row + c_r]) - f64::from(total_h_m[row + c_l]))
                / (dx * (c_r - c_l).max(1) as f64);
            let gz = (f64::from(total_h_m[r_d * width + c])
                - f64::from(total_h_m[r_u * width + c]))
                / (dx * (r_d - r_u).max(1) as f64);
            let slope = libm::hypot(gx, gz);
            if h > sea_level_m && wk == WaterKind::None as u8 && slope <= 0.70 {
                fl |= FLAG_NON_CLIFF_DRY_LAND;
            }
            cell_flags[i] = fl;

            if !is_oc_or_lake && is_channel[i] && river_w_m[i] >= 0.5 {
                active_chan_cells += 1;
                river_amt_sum += f64::from((river_w_m[i] / dx_m).clamp(0.0, 1.0));
            }
        }
    }

    let counted_river_surface_fraction = river_amt_sum / n.max(1) as f64;
    let width_by_order: Vec<StrahlerWidthEntry> = mean_width_by_order
        .iter()
        .map(|&(order, cell_count, mean_width_m)| StrahlerWidthEntry {
            order,
            cell_count,
            mean_width_m,
        })
        .collect();
    let find_w = |ord: u8| -> Option<f64> {
        width_by_order
            .iter()
            .find(|e| e.order == ord && e.cell_count > 0)
            .map(|e| e.mean_width_m)
    };
    let monotonic_w2_w3_w4 = match (find_w(2), find_w(3), find_w(4)) {
        (Some(w2), Some(w3), Some(w4)) => w2 < w3 && w3 < w4,
        _ => false,
    };
    let hack_consistent = hack_report.passed
        && (1.0..=2.0).contains(&hack_report.prefactor_c_km)
        && hack_report.median_mainstem_sinuosity >= 1.05;

    let c4_passed = (0.0005..=0.0030).contains(&counted_river_surface_fraction)
        && monotonic_w2_w3_w4
        && hack_consistent
        && fan_count >= 2
        && delta_count >= 2;

    let c4 = C4RiverProgressionReport {
        passed: c4_passed,
        counted_river_surface_fraction,
        active_channel_cells: active_chan_cells,
        width_by_order,
        monotonic_w2_w3_w4,
        hack_consistent,
        fan_count,
        delta_count,
    };

    // C5 Coastal slope classification (`< 0.18` gentle sandy >= 20%, `>= 0.35` cliff >= 10%).
    let mut coast_cells = 0_usize;
    let mut gentle_cells = 0_usize;
    let mut cliff_cells = 0_usize;
    let is_ocean_cell = |idx: usize| -> bool {
        let wk = water_kind[idx];
        wk == WaterKind::Ocean as u8
            || (wk == WaterKind::DeltaTurbid as u8 && total_h_m[idx] <= sea_level_m + 0.1)
            || total_h_m[idx] <= sea_level_m
    };

    for r in 0..height {
        let row = r * width;
        let r_u = r.saturating_sub(1);
        let r_d = (r + 1).min(height - 1);
        for c in 0..width {
            let i = row + c;
            if is_ocean_cell(i) {
                continue;
            }
            let touches_ocean = [
                (r > 0, i.wrapping_sub(width)),
                (r + 1 < height, i + width),
                (c > 0, i.wrapping_sub(1)),
                (c + 1 < width, i + 1),
            ]
            .iter()
            .any(|&(ok, nb)| ok && is_ocean_cell(nb));
            if !touches_ocean {
                continue;
            }
            coast_cells += 1;
            let c_l = c.saturating_sub(1);
            let c_r = (c + 1).min(width - 1);
            let gx = (f64::from(total_h_m[row + c_r]) - f64::from(total_h_m[row + c_l]))
                / (dx * (c_r - c_l).max(1) as f64);
            let gz = (f64::from(total_h_m[r_d * width + c])
                - f64::from(total_h_m[r_u * width + c]))
                / (dx * (r_d - r_u).max(1) as f64);
            let slope = libm::hypot(gx, gz);
            if slope < 0.18 {
                gentle_cells += 1;
            }
            if slope >= 0.35 {
                cliff_cells += 1;
            }
        }
    }

    let gentle_sandy_fraction = if coast_cells > 0 {
        gentle_cells as f64 / coast_cells as f64
    } else {
        0.0
    };
    let steep_cliff_fraction = if coast_cells > 0 {
        cliff_cells as f64 / coast_cells as f64
    } else {
        0.0
    };

    // Check canonical elevated camera viewpoints against terrain/water surface (`C5` sightlines).
    let (sightline_checks, obstructed_sightlines) =
        check_canonical_sightlines(width, height, total_h_m, water_h_m);

    let c5_passed = coast_cells >= 16
        && gentle_sandy_fraction >= 0.20
        && steep_cliff_fraction >= 0.10
        && obstructed_sightlines == 0;

    let c5 = C5CoastalSightlinesReport {
        passed: c5_passed,
        coastline_cells: coast_cells,
        gentle_sandy_fraction,
        steep_cliff_fraction,
        sightline_checks,
        obstructed_sightlines,
    };

    C4C5TerrainSummary { c4, c5, cell_flags }
}

fn check_canonical_sightlines(
    width: usize,
    height: usize,
    total_h_m: &[f32],
    water_h_m: &[f32],
) -> (usize, usize) {
    let mut peak_idx = (height / 2) * width + (width / 2);
    let mut max_surface = f32::NEG_INFINITY;
    for (idx, (&t, &w)) in total_h_m.iter().zip(water_h_m.iter()).enumerate() {
        let s = t.max(w);
        if s > max_surface {
            max_surface = s;
            peak_idx = idx;
        }
    }
    let wf = (width - 1) as f64;
    let hf = (height - 1) as f64;
    let peak_c = (peak_idx % width) as f64;
    let peak_r = (peak_idx / width) as f64;
    let target = (peak_c, peak_r, f64::from(max_surface) + 40.0);

    // 8 canonical azimuths around the map rim looking inward toward the massif summit from above relief.
    let views = [
        (0.50 * wf, 0.04 * hf, f64::from(max_surface) + 2_500.0),
        (0.96 * wf, 0.04 * hf, f64::from(max_surface) + 2_800.0),
        (0.96 * wf, 0.50 * hf, f64::from(max_surface) + 2_500.0),
        (0.96 * wf, 0.96 * hf, f64::from(max_surface) + 2_800.0),
        (0.50 * wf, 0.96 * hf, f64::from(max_surface) + 2_500.0),
        (0.04 * wf, 0.96 * hf, f64::from(max_surface) + 2_800.0),
        (0.04 * wf, 0.50 * hf, f64::from(max_surface) + 2_500.0),
        (0.50 * wf, 0.50 * hf, f64::from(max_surface) + 12_000.0),
    ];

    let mut obstructed = 0_usize;
    for &(cam_c, cam_r, cam_y) in &views {
        let cc = (cam_c.round() as usize).min(width - 1);
        let cr = (cam_r.round() as usize).min(height - 1);
        let mut local_max_surf = f64::NEG_INFINITY;
        for dr in -1isize..=1 {
            let nr = (cr as isize + dr).clamp(0, height as isize - 1) as usize;
            for dc in -1isize..=1 {
                let nc = (cc as isize + dc).clamp(0, width as isize - 1) as usize;
                let s = f64::from(total_h_m[nr * width + nc].max(water_h_m[nr * width + nc]));
                local_max_surf = local_max_surf.max(s);
            }
        }
        if cam_y < local_max_surf + 100.0 {
            obstructed += 1;
            continue;
        }
        let mut blocked = false;
        for step in 1..48 {
            let t = step as f64 / 48.0;
            let sc = cam_c + (target.0 - cam_c) * t;
            let sr = cam_r + (target.1 - cam_r) * t;
            let sy = cam_y + (target.2 - cam_y) * t;
            let uc = (libm::round(sc) as usize).min(width - 1);
            let ur = (libm::round(sr) as usize).min(height - 1);
            let surf = f64::from(total_h_m[ur * width + uc].max(water_h_m[ur * width + uc]));
            if sy <= surf + 1.0 {
                blocked = true;
                break;
            }
        }
        if blocked {
            obstructed += 1;
        }
    }
    (views.len(), obstructed)
}

/// Completes `G-COMP v2` (`C1`, `C2`, `C3`) using biome buffers + compact Phase-A summary.
#[allow(clippy::too_many_arguments)]
pub fn finalize_g_comp_v2(
    width: usize,
    height: usize,
    dx_m: f32,
    extent_m: f32,
    water_kind: &[u8],
    dominant_biome: &[u8],
    biome_ids: &[[u8; 4]],
    biome_weights: &[[u8; 4]],
    waterfalls: &[WaterfallRecord],
    basins: &[LakeBasinRecord],
    micro_pond_count: usize,
    cave_network: &CaveNetwork,
    terrain_summary: C4C5TerrainSummary,
) -> GCompV2Report {
    let cell_flags = &terrain_summary.cell_flags;
    let n = width * height;

    // 1) Biome totals and largest 4-connected region per biome in one linear O(n) pass.
    let mut biome_counts = [0_usize; BIOME_COUNT];
    let mut max_conn = [0_usize; BIOME_COUNT];
    let mut visited = vec![0_u8; n];
    let mut queue = VecDeque::new();

    for start in 0..n {
        let dom = dominant_biome[start];
        if (dom as usize) < BIOME_COUNT {
            biome_counts[dom as usize] += 1;
        }
        if visited[start] != 0 {
            continue;
        }
        visited[start] = 1;
        queue.push_back(start as u32);
        let mut comp_len = 0_usize;
        while let Some(u_u32) = queue.pop_front() {
            let u = u_u32 as usize;
            comp_len += 1;
            let r = u / width;
            let c = u % width;
            let nbs = [
                (r > 0, u.wrapping_sub(width)),
                (r + 1 < height, u + width),
                (c > 0, u.wrapping_sub(1)),
                (c + 1 < width, u + 1),
            ];
            for &(ok, nb) in &nbs {
                if ok && visited[nb] == 0 && dominant_biome[nb] == dom {
                    visited[nb] = 1;
                    queue.push_back(nb as u32);
                }
            }
        }
        if (dom as usize) < BIOME_COUNT && comp_len > max_conn[dom as usize] {
            max_conn[dom as usize] = comp_len;
        }
    }
    drop(visited);
    drop(queue);

    // 2) C1 Ocean, Waterfalls, Caves, and LD-5 Biome Partition.
    let mut ocean_cells = 0_usize;
    let mut lake_cells = 0_usize;
    for i in 0..n {
        let wk = water_kind[i];
        if wk == WaterKind::Ocean as u8 || (cell_flags[i] & FLAG_SUBAERIAL) == 0 {
            ocean_cells += 1;
        }
        if wk == WaterKind::FreshwaterLake as u8 || wk == WaterKind::EndorheicLake as u8 {
            lake_cells += 1;
        }
    }
    let ocean_fraction = ocean_cells as f64 / n.max(1) as f64;
    let min_req_spacing = 0.01 * f64::from(extent_m);
    let mut min_cave_spacing_m = f64::INFINITY;
    let mut invalid_cave_placements = 0_usize;

    for (idx_a, ea) in cave_network.entrances.iter().enumerate() {
        let na = &cave_network.nodes[ea.node_idx as usize];
        let ca = ((na.x_m / dx_m).round() as isize).clamp(0, width as isize - 1) as usize;
        let ra = ((na.z_m / dx_m).round() as isize).clamp(0, height as isize - 1) as usize;
        let cell_a = ra * width + ca;
        if (cell_flags[cell_a] & (FLAG_OCEAN_OR_LAKE | FLAG_GLACIER)) != 0 {
            invalid_cave_placements += 1;
        }
        for eb in cave_network.entrances.iter().skip(idx_a + 1) {
            let nb = &cave_network.nodes[eb.node_idx as usize];
            let dist = libm::hypot(f64::from(na.x_m - nb.x_m), f64::from(na.z_m - nb.z_m));
            min_cave_spacing_m = min_cave_spacing_m.min(dist);
        }
    }
    if cave_network.entrances.len() < 2 {
        min_cave_spacing_m = 0.0;
    }

    let min_patch_cells_required =
        (libm::round(MIN_PATCH_AREA_M2 / (f64::from(dx_m) * f64::from(dx_m))) as usize).max(1);
    let mut biome_stats = Vec::with_capacity(BiomeId::ZONED_TERRESTRIAL_16.len());
    let mut zoned_passing = 0_usize;
    for b in BiomeId::ZONED_TERRESTRIAL_16 {
        let id = b as usize;
        let max_c = max_conn[id];
        let pass = max_c >= min_patch_cells_required;
        if pass {
            zoned_passing += 1;
        }
        biome_stats.push(BiomePatchStat {
            biome_id: b as u8,
            biome_name: biome_display_name(b).to_owned(),
            total_cells: biome_counts[id],
            max_connected_cells: max_c,
            passed: pass,
        });
    }
    let chaparral_cells = biome_counts[BiomeId::Chaparral as usize];
    let steppe_cells = biome_counts[BiomeId::Steppe as usize];
    let wf_cnt = waterfalls.len();
    let cave_cnt = cave_network.entrances.len();

    let c1_passed = (0.40..=0.50).contains(&ocean_fraction)
        && (20..=80).contains(&wf_cnt)
        && (30..=48).contains(&cave_cnt)
        && min_cave_spacing_m >= min_req_spacing
        && invalid_cave_placements == 0
        && zoned_passing == BiomeId::ZONED_TERRESTRIAL_16.len()
        && chaparral_cells == 0
        && steppe_cells == 0;

    let c1_partition = C1PartitionReport {
        passed: c1_passed,
        ocean_fraction,
        ocean_cells,
        waterfall_count: wf_cnt,
        cave_entrance_count: cave_cnt,
        min_cave_spacing_m,
        min_required_cave_spacing_m: min_req_spacing,
        invalid_cave_placements,
        min_patch_cells_required,
        zoned_16_biomes_passing: zoned_passing,
        chaparral_cells,
        steppe_cells,
        biome_stats,
    };

    // 3) C2 Continuous Ecotone & Jump Edges.
    let mut c2_land_cells = 0_usize;
    let mut ecotone_cells = 0_usize;
    for i in 0..n {
        if (cell_flags[i] & FLAG_SUBAERIAL) != 0
            && !BiomeId::from_u8(dominant_biome[i]).is_some_and(BiomeId::is_water_or_strip)
        {
            c2_land_cells += 1;
            if biome_weights[i][0] < 230 {
                ecotone_cells += 1;
            }
        }
    }
    let ecotone_fraction = ecotone_cells as f64 / c2_land_cells.max(1) as f64;

    let mut non_cliff_land_edges = 0_usize;
    let mut sharp_jump_edges = 0_usize;
    for r in 1..height.saturating_sub(1) {
        let row = r * width;
        for c in 1..width.saturating_sub(1) {
            let i = row + c;
            if (cell_flags[i] & FLAG_NON_CLIFF_DRY_LAND) == 0 {
                continue;
            }
            for &nb in &[i + 1, (r + 1) * width + c] {
                if (cell_flags[nb] & (FLAG_SUBAERIAL | FLAG_WATER_NONE))
                    != (FLAG_SUBAERIAL | FLAG_WATER_NONE)
                {
                    continue;
                }
                non_cliff_land_edges += 1;
                let mut p_a = [0.0_f64; BIOME_COUNT];
                let mut p_b = [0.0_f64; BIOME_COUNT];
                for k in 0..4 {
                    let ba = biome_ids[i][k] as usize;
                    let bb = biome_ids[nb][k] as usize;
                    if ba < BIOME_COUNT {
                        p_a[ba] += f64::from(biome_weights[i][k]) / 255.0;
                    }
                    if bb < BIOME_COUNT {
                        p_b[bb] += f64::from(biome_weights[nb][k]) / 255.0;
                    }
                }
                let tv: f64 = 0.5
                    * p_a
                        .iter()
                        .zip(p_b.iter())
                        .map(|(&a, &b)| (a - b).abs())
                        .sum::<f64>();
                if tv > 0.70 {
                    sharp_jump_edges += 1;
                }
            }
        }
    }
    let non_cliff_jump_fraction = if non_cliff_land_edges > 0 {
        sharp_jump_edges as f64 / non_cliff_land_edges as f64
    } else {
        0.0
    };
    let c2_passed = (0.15..=0.55).contains(&ecotone_fraction) && non_cliff_jump_fraction < 0.03;

    let c2_ecotone = C2EcotoneReport {
        passed: c2_passed,
        land_cells: c2_land_cells,
        ecotone_cells,
        ecotone_fraction,
        non_cliff_land_edges,
        sharp_jump_edges,
        non_cliff_jump_fraction,
    };

    // 4) C3 Multi-Basin Lake Diversity.
    let lake_surface_fraction = lake_cells as f64 / n.max(1) as f64;
    let large_lake_count = basins.iter().filter(|b| b.area_m2 > 100_000.0).count();
    let high_paramo_lake_count = basins
        .iter()
        .filter(|b| b.water_level_m > 3_300.0 && b.area_m2 >= f64::from(dx_m * dx_m))
        .count();
    let caldera_or_trough_lake_count = basins
        .iter()
        .filter(|b| b.max_wall_slope_deg > 25.0 || b.elongation > 3.5)
        .count();
    let small_paramo_ponds_from_basins = basins
        .iter()
        .filter(|b| b.water_level_m > 3_300.0 && b.area_m2 > 0.0 && b.area_m2 < 100_000.0)
        .count();
    let small_paramo_pond_count = micro_pond_count + small_paramo_ponds_from_basins;

    let c3_passed = (0.008..=0.045).contains(&lake_surface_fraction)
        && large_lake_count >= 5
        && high_paramo_lake_count >= 1
        && caldera_or_trough_lake_count >= 1
        && small_paramo_pond_count >= 10;

    let c3_lakes = C3LakeDiversityReport {
        passed: c3_passed,
        lake_surface_fraction,
        total_mapped_basins: basins.len(),
        large_lake_count,
        high_paramo_lake_count,
        caldera_or_trough_lake_count,
        small_paramo_pond_count,
    };

    let c4 = terrain_summary.c4;
    let c5 = terrain_summary.c5;
    let passed = c1_passed && c2_passed && c3_passed && c4.passed && c5.passed;

    GCompV2Report {
        passed,
        c1_partition,
        c2_ecotone,
        c3_lakes,
        c4_rivers_fans_deltas: c4,
        c5_coastal_sightlines: c5,
    }
}
