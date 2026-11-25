//! End-to-end deterministic multigrid World v2 pipeline (`S1 -> S2/S3 -> S5 -> S4 -> S6 -> S7 -> S8 -> S9`)
//! with memory-lean stage-by-stage disk spilling so `4096^2` generation stays within `<= 1.2 GB` peak RSS.

use crate::biomes::{classify_biomes, BiomeConfig, BiomeError, BiomeInput};
use crate::caves::{generate_caves, CaveConfig, CaveError, CaveInput};
use crate::climate::{
    compute_climate, compute_twi_and_moisture_halves, ClimateConfig, ClimateError,
};
use crate::config::{ConfigError, WorldV2Config};
use crate::deposition::{apply_deposition, DepositionConfig, DepositionError, DepositionInput};
use crate::erosion_fluvial::{
    erode_fluvial_step_f32_lean, FluvialErodibility, FluvialError, FluvialGridLean,
    FluvialLeanScratch, FluvialParams,
};
use crate::erosion_glacial::{run_paleo_glacial_pass, GlacialParams};
use crate::erosion_thermal::{relax_thermal_and_hillslope, ThermalHillslopeParams};
use crate::hydrology::{
    priority_flood_d8_lean_into, route_hybrid_mfd_d8_lean, route_hybrid_mfd_d8_lean_halves,
    HydrologyConfig, DIST8, DX8, DZ8, NO_DIR,
};
use crate::lakes_falls::{
    generate_lakes_and_waterfalls, LakeBasinRecord, LakesFallsConfig, LakesFallsError,
    LakesFallsInput, WaterfallRecord,
};
use crate::tectonic::{
    generate_fine_s1_aux_at_time, generate_uplift_for_target, update_lithology_advected_in_place,
    LithologyClass, STRAT_MIN_BED_THICKNESS_M,
};
use crate::{BiomeId, WaterKind, BIOME_COUNT};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

const IO_CHUNK_BYTES: usize = 65_536;
const UPLIFT_M_YR_SCALE: f32 = 1.0e-6;
const FLUVIAL_K_SCALE: f64 = 2.2e-5;
const FLUVIAL_DT_YR: f64 = 1_000.0;

/// Error returned by [`run_worldgen_to_dir`] or raw-buffer readers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineError {
    pub message: String,
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PipelineError {}

macro_rules! impl_from_err {
    ($t:ty) => {
        impl From<$t> for PipelineError {
            fn from(err: $t) -> Self {
                Self {
                    message: err.to_string(),
                }
            }
        }
    };
}

impl_from_err!(ConfigError);
impl_from_err!(ClimateError);
impl_from_err!(FluvialError);
impl_from_err!(DepositionError);
impl_from_err!(LakesFallsError);
impl_from_err!(BiomeError);
impl_from_err!(CaveError);
impl_from_err!(std::io::Error);
impl_from_err!(serde_json::Error);

/// Metadata for a single spilled little-endian raw layer file inside `--out`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerMeta {
    pub filename: String,
    pub dtype: String,
    pub channels: u8,
    pub width: u32,
    pub height: u32,
    pub byte_len: u64,
    pub sha256: String,
}

/// Summary quantitative diagnostics recorded inside `raw_meta.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineStats {
    pub ocean_fraction: f32,
    pub lake_surface_fraction: f32,
    pub river_surface_fraction: f32,
    pub severed_humid_spillways: usize,
    pub unmodeled_micro_pits: usize,
    pub endorheic_land_fraction: f32,
    pub endorheic_humid_fraction: f32,
    pub waterfall_count: usize,
    pub cave_entrance_count: usize,
    pub fan_count: usize,
    pub meander_reach_count: usize,
    pub oxbow_count: usize,
    pub delta_count: usize,
    pub micro_pond_count: usize,
    pub ecotone_fraction: f32,
    pub non_cliff_jump_fraction: f32,
    pub biome_counts: Vec<usize>,
    #[serde(default)]
    pub uncapped_fluvial_substeps: usize,
}

/// Top-level JSON manifest written directly as `<out>/raw_meta.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawWorldMeta {
    pub format_version: u32,
    pub seed: u64,
    pub schema_tag: String,
    pub width: u32,
    pub height: u32,
    pub dx_m: f32,
    pub extent_m: f32,
    pub elev_min_m: f32,
    pub elev_max_m: f32,
    pub sea_level_m: f32,
    pub wind_azimuth_rad: f32,
    pub config: WorldV2Config,
    pub stats: PipelineStats,
    pub basins: Vec<LakeBasinRecord>,
    pub waterfalls: Vec<WaterfallRecord>,
    pub layers: Vec<LayerMeta>,
    pub cave_network_sha256: String,
    pub world_sha256: String,
}

/// Canonical multigrid coarse-to-fine erosion schedule `(grid_size, macro_steps)`.
#[must_use]
pub fn compute_multigrid_schedule(target_size: u32) -> Vec<(u32, usize)> {
    if target_size <= 512 {
        return vec![(target_size, 80)];
    }
    let mut levels = Vec::new();
    for &(sz, steps) in &[(512_u32, 65_usize), (1024, 35), (2048, 20), (4096, 30)] {
        if sz < target_size {
            levels.push((sz, steps));
        }
    }
    let target_steps = match target_size {
        1024 => 35,
        2048 => 20,
        4096 => 30,
        8192 => 8,
        _ => 20,
    };
    levels.push((target_size, target_steps));
    levels
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0F) as usize] as char);
    }
    s
}

fn write_f32_layer(
    dir: &Path,
    filename: &str,
    data: &[f32],
    width: u32,
    height: u32,
) -> Result<LayerMeta, PipelineError> {
    let path = dir.join(filename);
    let mut writer = BufWriter::new(File::create(&path)?);
    let mut hasher = Sha256::new();
    let mut buf = Vec::with_capacity(IO_CHUNK_BYTES);
    for &v in data {
        buf.extend_from_slice(&v.to_le_bytes());
        if buf.len() >= IO_CHUNK_BYTES {
            hasher.update(&buf);
            writer.write_all(&buf)?;
            buf.clear();
        }
    }
    if !buf.is_empty() {
        hasher.update(&buf);
        writer.write_all(&buf)?;
    }
    writer.flush()?;
    let digest = hasher.finalize();
    Ok(LayerMeta {
        filename: filename.to_owned(),
        dtype: "f32".to_owned(),
        channels: 1,
        width,
        height,
        byte_len: (data.len() as u64) * 4,
        sha256: hex_encode(&digest),
    })
}

fn write_diff_f32_layer(
    dir: &Path,
    filename: &str,
    a: &[f32],
    b: &[f32],
    width: u32,
    height: u32,
) -> Result<LayerMeta, PipelineError> {
    assert_eq!(a.len(), b.len());
    let path = dir.join(filename);
    let mut writer = BufWriter::new(File::create(&path)?);
    let mut hasher = Sha256::new();
    let mut buf = Vec::with_capacity(IO_CHUNK_BYTES);
    for (&va, &vb) in a.iter().zip(b.iter()) {
        buf.extend_from_slice(&(va - vb).to_le_bytes());
        if buf.len() >= IO_CHUNK_BYTES {
            hasher.update(&buf);
            writer.write_all(&buf)?;
            buf.clear();
        }
    }
    if !buf.is_empty() {
        hasher.update(&buf);
        writer.write_all(&buf)?;
    }
    writer.flush()?;
    let digest = hasher.finalize();
    Ok(LayerMeta {
        filename: filename.to_owned(),
        dtype: "f32".to_owned(),
        channels: 1,
        width,
        height,
        byte_len: (a.len() as u64) * 4,
        sha256: hex_encode(&digest),
    })
}

fn write_u32_layer(
    dir: &Path,
    filename: &str,
    data: &[u32],
    width: u32,
    height: u32,
) -> Result<LayerMeta, PipelineError> {
    let path = dir.join(filename);
    let mut writer = BufWriter::new(File::create(&path)?);
    let mut hasher = Sha256::new();
    let mut buf = Vec::with_capacity(IO_CHUNK_BYTES);
    for &v in data {
        buf.extend_from_slice(&v.to_le_bytes());
        if buf.len() >= IO_CHUNK_BYTES {
            hasher.update(&buf);
            writer.write_all(&buf)?;
            buf.clear();
        }
    }
    if !buf.is_empty() {
        hasher.update(&buf);
        writer.write_all(&buf)?;
    }
    writer.flush()?;
    let digest = hasher.finalize();
    Ok(LayerMeta {
        filename: filename.to_owned(),
        dtype: "u32".to_owned(),
        channels: 1,
        width,
        height,
        byte_len: (data.len() as u64) * 4,
        sha256: hex_encode(&digest),
    })
}

fn write_u8_layer(
    dir: &Path,
    filename: &str,
    data: &[u8],
    width: u32,
    height: u32,
) -> Result<LayerMeta, PipelineError> {
    let path = dir.join(filename);
    let mut writer = BufWriter::new(File::create(&path)?);
    let mut hasher = Sha256::new();
    for chunk in data.chunks(IO_CHUNK_BYTES) {
        hasher.update(chunk);
        writer.write_all(chunk)?;
    }
    writer.flush()?;
    let digest = hasher.finalize();
    Ok(LayerMeta {
        filename: filename.to_owned(),
        dtype: "u8".to_owned(),
        channels: 1,
        width,
        height,
        byte_len: data.len() as u64,
        sha256: hex_encode(&digest),
    })
}

fn write_u8x4_layer(
    dir: &Path,
    filename: &str,
    data: &[[u8; 4]],
    width: u32,
    height: u32,
) -> Result<LayerMeta, PipelineError> {
    let path = dir.join(filename);
    let mut writer = BufWriter::new(File::create(&path)?);
    let mut hasher = Sha256::new();
    let mut buf = Vec::with_capacity(IO_CHUNK_BYTES);
    for &cell in data {
        buf.extend_from_slice(&cell);
        if buf.len() >= IO_CHUNK_BYTES {
            hasher.update(&buf);
            writer.write_all(&buf)?;
            buf.clear();
        }
    }
    if !buf.is_empty() {
        hasher.update(&buf);
        writer.write_all(&buf)?;
    }
    writer.flush()?;
    let digest = hasher.finalize();
    Ok(LayerMeta {
        filename: filename.to_owned(),
        dtype: "u8".to_owned(),
        channels: 4,
        width,
        height,
        byte_len: (data.len() as u64) * 4,
        sha256: hex_encode(&digest),
    })
}

/// Reads a flat little-endian `f32` layer buffer from disk.
pub fn read_bin_f32(path: &Path, expected_len: usize) -> Result<Vec<f32>, PipelineError> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut out = Vec::with_capacity(expected_len);
    let mut buf = [0_u8; IO_CHUNK_BYTES];
    let mut rem_bytes = expected_len * 4;
    while rem_bytes > 0 {
        let to_read = rem_bytes.min(buf.len());
        reader.read_exact(&mut buf[..to_read])?;
        let (chunks, _) = buf[..to_read].as_chunks::<4>();
        for &chunk in chunks {
            out.push(f32::from_le_bytes(chunk));
        }
        rem_bytes -= to_read;
    }
    Ok(out)
}

/// Reads a flat `u8` layer buffer from disk.
pub fn read_bin_u8(path: &Path, expected_len: usize) -> Result<Vec<u8>, PipelineError> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut out = vec![0_u8; expected_len];
    reader.read_exact(&mut out)?;
    Ok(out)
}

/// Reads a flat little-endian `u32` layer buffer from disk.
pub fn read_bin_u32(path: &Path, expected_len: usize) -> Result<Vec<u32>, PipelineError> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut out = Vec::with_capacity(expected_len);
    let mut buf = [0_u8; IO_CHUNK_BYTES];
    let mut rem_bytes = expected_len * 4;
    while rem_bytes > 0 {
        let to_read = rem_bytes.min(buf.len());
        reader.read_exact(&mut buf[..to_read])?;
        let (chunks, _) = buf[..to_read].as_chunks::<4>();
        for &chunk in chunks {
            out.push(u32::from_le_bytes(chunk));
        }
        rem_bytes -= to_read;
    }
    Ok(out)
}

/// Reads a flat 4-channel `u8` layer buffer from disk.
pub fn read_bin_u8x4(path: &Path, expected_len: usize) -> Result<Vec<[u8; 4]>, PipelineError> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut out = Vec::with_capacity(expected_len);
    let mut buf = [0_u8; IO_CHUNK_BYTES];
    let mut rem_bytes = expected_len * 4;
    while rem_bytes > 0 {
        let to_read = rem_bytes.min(buf.len());
        reader.read_exact(&mut buf[..to_read])?;
        let (chunks, _) = buf[..to_read].as_chunks::<4>();
        out.extend_from_slice(chunks);
        rem_bytes -= to_read;
    }
    Ok(out)
}

fn upsample_bilinear(src: &[f32], sw: usize, sh: usize, tw: usize, th: usize) -> Vec<f32> {
    if sw == tw && sh == th {
        return src.to_vec();
    }
    let mut dst = vec![0.0_f32; tw * th];
    let denom_x = (tw - 1) as f32;
    let denom_z = (th - 1) as f32;
    let max_x = (sw - 1) as f32;
    let max_z = (sh - 1) as f32;
    dst.par_chunks_mut(tw).enumerate().for_each(|(r, row)| {
        let z = ((r as f32 / denom_z) * max_z).clamp(0.0, max_z);
        let z0 = libm::floorf(z) as usize;
        let z1 = (z0 + 1).min(sh - 1);
        let fz = z - z0 as f32;
        let r0_off = z0 * sw;
        let r1_off = z1 * sw;
        for (c, out) in row.iter_mut().enumerate() {
            let x = ((c as f32 / denom_x) * max_x).clamp(0.0, max_x);
            let x0 = libm::floorf(x) as usize;
            let x1 = (x0 + 1).min(sw - 1);
            let fx = x - x0 as f32;
            let v00 = src[r0_off + x0];
            let v10 = src[r0_off + x1];
            let v01 = src[r1_off + x0];
            let v11 = src[r1_off + x1];
            let vx0 = v00 + fx * (v10 - v00);
            let vx1 = v01 + fx * (v11 - v01);
            *out = vx0 + fz * (vx1 - vx0);
        }
    });
    dst
}

fn upsert_layer(layers: &mut Vec<LayerMeta>, meta: LayerMeta) {
    if let Some(existing) = layers.iter_mut().find(|m| m.filename == meta.filename) {
        *existing = meta;
    } else {
        layers.push(meta);
    }
}

fn worldgen_hydro_config(cfg: &WorldV2Config, dx_m: f32) -> HydrologyConfig {
    HydrologyConfig {
        dx_m,
        mfd_exponent_p: f64::from(cfg.mfd_p),
        channel_init_threshold: f64::from(cfg.channel_init),
        seasonal_storm_runoff_coeff: cfg.storm_runoff,
        bankfull_q_mult: 5.5,
        ..HydrologyConfig::default()
    }
}

/// Applies bounded differential plunge/shale sapping where active D8 channels leave resistant
/// `SandstoneCaprock` strata onto softer `SoftShale` / `SolubleLimestone` / `UnconsolidatedAlluvium`
/// downstream beds, preserving `< 0.06` sandstone benches upstream and graded `14..22 m` drops within `30 m`.
fn apply_caprock_knickpoint_sapping(
    bedrock_h: &mut [f32],
    sediment_s: &mut [f32],
    lithology_id: &[u8],
    precip_m_yr: &[f32],
    lvl: usize,
    dx_m: f32,
    sapping_gain: f32,
) {
    let n = lvl * lvl;
    if lvl < 64 || sapping_gain <= 0.0 || bedrock_h.len() != n || lithology_id.len() != n {
        return;
    }

    let mut total: Vec<f32> = bedrock_h
        .iter()
        .zip(sediment_s.iter())
        .map(|(&b, &s)| b + s)
        .collect();
    let mut d8_dir = vec![NO_DIR; n];
    let mut order = Vec::with_capacity(n);
    priority_flood_d8_lean_into(
        &mut total,
        &mut d8_dir,
        &mut order,
        lvl,
        lvl,
        Some(0.0),
        true,
    );

    let cell_area = dx_m * dx_m;
    let mut a_tilde: Vec<f32> = precip_m_yr
        .iter()
        .map(|&p| p.max(0.15) * cell_area)
        .collect();
    let step_nb = |idx: usize, d: u8| -> Option<usize> {
        if d >= NO_DIR {
            return None;
        }
        let r = (idx / lvl) as isize + DZ8[d as usize];
        let c = (idx % lvl) as isize + DX8[d as usize];
        if r >= 0 && c >= 0 && r < lvl as isize && c < lvl as isize {
            Some((r as usize) * lvl + (c as usize))
        } else {
            None
        }
    };
    for &node_u in order.iter().rev() {
        let i = node_u as usize;
        if let Some(rec) = step_nb(i, d8_dir[i]) {
            a_tilde[rec] += a_tilde[i];
        }
    }
    drop(order);

    let mut best_up = vec![u32::MAX; n];
    for i in 0..n {
        if total[i] <= 0.0 {
            continue;
        }
        if let Some(rec) = step_nb(i, d8_dir[i]) {
            let cur_best = best_up[rec];
            if cur_best == u32::MAX || a_tilde[i] > a_tilde[cur_best as usize] {
                best_up[rec] = i as u32;
            }
        }
    }

    let is_soft_lith = |lid: u8| -> bool {
        lid == LithologyClass::SolubleLimestone as u8
            || lid == LithologyClass::SoftShale as u8
            || lid == LithologyClass::UnconsolidatedAlluvium as u8
    };
    let run_limit = if dx_m > 20.0 {
        30.0_f32.max(3.65 * dx_m)
    } else {
        30.0_f32
    };
    let k_steps = ((run_limit / dx_m).round() as usize).clamp(1, 3);
    let mut cands: Vec<(f32, usize, usize, f32)> = Vec::new();

    for r in 3..(lvl - 3) {
        let row = r * lvl;
        for c in 3..(lvl - 3) {
            let u = row + c;
            if lithology_id[u] != LithologyClass::SandstoneCaprock as u8
                || a_tilde[u] < 2.2e5
                || total[u] < 28.0
            {
                continue;
            }
            let mut cur = u;
            let mut dist_m = 0.0_f32;
            let mut soft_cell = None;
            for _ in 0..k_steps {
                let d = d8_dir[cur];
                let Some(nxt) = step_nb(cur, d) else {
                    break;
                };
                dist_m += dx_m * DIST8[d as usize] as f32;
                if dist_m > run_limit + 1e-3 || total[nxt] <= 2.5 {
                    break;
                }
                if is_soft_lith(lithology_id[nxt]) {
                    soft_cell = Some((nxt, dist_m));
                    if dist_m >= 1.5 * dx_m || dx_m > 20.0 {
                        break;
                    }
                }
                cur = nxt;
            }
            if let Some((base_v, d_uv)) = soft_cell {
                cands.push((a_tilde[u], u, base_v, d_uv));
            }
        }
    }

    cands.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let min_sp_sq = 310.0_f32 * 310.0_f32;
    let mut accepted: Vec<(usize, usize, f32)> = Vec::new();
    for (_, u, v, d_uv) in cands {
        if accepted.len() >= 54 {
            break;
        }
        let ur = (u / lvl) as f32 * dx_m;
        let uc = (u % lvl) as f32 * dx_m;
        let too_close = accepted.iter().any(|&(au, _, _)| {
            let ar = (au / lvl) as f32 * dx_m;
            let ac = (au % lvl) as f32 * dx_m;
            (ur - ar) * (ur - ar) + (uc - ac) * (uc - ac) < min_sp_sq
        });
        if !too_close {
            accepted.push((u, v, d_uv));
        }
    }

    let set_lower_surface =
        |idx: usize, new_h: f32, tot: &mut [f32], bed: &mut [f32], sed: &mut [f32]| {
            let cur_h = tot[idx];
            if new_h < cur_h {
                let cut = cur_h - new_h;
                let s_cut = sed[idx].min(cut);
                sed[idx] -= s_cut;
                bed[idx] -= cut - s_cut;
                tot[idx] = new_h;
            }
        };

    let target_drop = (16.0_f32 + 2.5 * sapping_gain).clamp(15.0, 24.0);
    for (u, base_v, d_uv) in accepted {
        let max_drop = if d_uv >= 18.0 {
            target_drop
        } else {
            target_drop.min(d_uv * 1.45)
        };
        let target_base_h = (total[u] - max_drop).max(2.0);

        // Keep upstream sandstone bench gentle (< 0.06 slope) without creating upstream pits.
        let up_u32 = best_up[u];
        if up_u32 != u32::MAX {
            let up = up_u32 as usize;
            let up_r = up / lvl;
            let up_c = up % lvl;
            let u_r = u / lvl;
            let u_c = u % lvl;
            let up_dist = dx_m
                * libm::hypotf(
                    (u_c as isize - up_c as isize) as f32,
                    (u_r as isize - up_r as isize) as f32,
                )
                .max(1.0);
            let max_up_h = total[u] + 0.055 * up_dist;
            if total[up] > max_up_h {
                let new_up = max_up_h.max(total[u] + 0.02);
                if (total[up] - new_up) <= 32.0 {
                    set_lower_surface(up, new_up, &mut total, bedrock_h, sediment_s);
                }
            }
        }

        // Carve graded step along D8 channel from `u` to `base_v` (`~55%` intermediate drop -> `100%`).
        let mut cur = u;
        let mut cum_d = 0.0_f32;
        for _ in 0..k_steps {
            let d = d8_dir[cur];
            let Some(nxt) = step_nb(cur, d) else {
                break;
            };
            cum_d += dx_m * DIST8[d as usize] as f32;
            let frac = (cum_d / d_uv.max(1.0)).clamp(0.0, 1.0);
            let step_target = total[u] - frac * (total[u] - target_base_h);
            set_lower_surface(nxt, step_target, &mut total, bedrock_h, sediment_s);
            cur = nxt;
            if nxt == base_v {
                break;
            }
        }

        // Propagate downstream plunge-gorge floor monotonically (no micro-pits).
        let mut prev_h = total[base_v];
        let mut g_cur = base_v;
        for _ in 0..10 {
            let d = d8_dir[g_cur];
            let Some(nxt) = step_nb(g_cur, d) else {
                break;
            };
            if total[nxt] <= 0.2 {
                break;
            }
            let step_d = dx_m * DIST8[d as usize] as f32;
            let max_floor = (prev_h - 0.015 * step_d).max(0.5);
            if total[nxt] <= max_floor {
                break;
            }
            set_lower_surface(nxt, max_floor, &mut total, bedrock_h, sediment_s);
            prev_h = total[nxt];
            g_cur = nxt;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_fluvial_step_advected(
    lvl_cfg: &WorldV2Config,
    target_size: u32,
    cur_bedrock: &mut [f32],
    cur_sediment: &mut [f32],
    uplift_u: &[f32],
    lithology_id: &mut [u8],
    precip_for_s2: &[f32],
    base_params: &FluvialParams,
    scratch: &mut FluvialLeanScratch,
    t_elapsed_yr: &mut f64,
    depth: u8,
    hint_k: &mut usize,
    uncapped_substeps: &mut usize,
) -> Result<(f64, bool), PipelineError> {
    if depth == 0 && *hint_k > 1 {
        let k_init = (*hint_k).clamp(2, 3);
        let sub_dt = base_params.dt_yr / (k_init as f64);
        let sub_params = FluvialParams {
            dt_yr: sub_dt,
            max_incision_cap_m: None,
            ..base_params.clone()
        };
        let mut worst_dh = 0.0_f64;
        let mut any_split = false;
        let mut bumped_k = k_init;
        for _ in 0..k_init {
            let mut local_hint = 1_usize;
            let (dh, split) = run_fluvial_step_advected(
                lvl_cfg,
                target_size,
                cur_bedrock,
                cur_sediment,
                uplift_u,
                lithology_id,
                precip_for_s2,
                &sub_params,
                scratch,
                t_elapsed_yr,
                1,
                &mut local_hint,
                uncapped_substeps,
            )?;
            worst_dh = worst_dh.max(dh);
            any_split |= split;
            if local_hint > 1 {
                bumped_k = bumped_k.max(k_init.saturating_mul(local_hint).min(3));
            }
        }
        if any_split {
            *hint_k = bumped_k;
        } else if let Some(cap_m) = base_params.max_incision_cap_m {
            let next_k = if worst_dh <= 0.0 {
                1
            } else {
                libm::ceil((k_init as f64) * (worst_dh / (0.85 * cap_m))).clamp(1.0, 3.0) as usize
            };
            *hint_k = next_k.min(k_init);
        }
        return Ok((worst_dh, true));
    }

    update_lithology_advected_in_place(
        lvl_cfg,
        target_size,
        cur_bedrock,
        uplift_u,
        *t_elapsed_yr,
        lithology_id,
    )?;
    let mut grid = FluvialGridLean {
        bedrock_h: cur_bedrock,
        sediment_s: cur_sediment,
        uplift_m_yr: uplift_u,
        k_erodibility: FluvialErodibility::LithologyLut {
            lithology_id,
            k_scale: FLUVIAL_K_SCALE * f64::from(lvl_cfg.fluvial_k_mult),
        },
        precip_m_yr: precip_for_s2,
        pet_m_yr: None,
        width: lvl_cfg.grid_size as usize,
        height: lvl_cfg.grid_size as usize,
        sea_level_m: Some(crate::SEA_LEVEL_M),
    };
    match erode_fluvial_step_f32_lean(&mut grid, base_params, Some(scratch)) {
        Ok(stats) => {
            if base_params.max_incision_cap_m.is_none() {
                *uncapped_substeps += 1;
            }
            *t_elapsed_yr += base_params.dt_yr;
            Ok((stats.max_bedrock_incision_m, false))
        }
        Err(FluvialError::IncisionCapExceeded { recommended_k, .. }) if depth < 1 => {
            let k = recommended_k.clamp(2, 3);
            *hint_k = k;
            let sub_dt = base_params.dt_yr / (k as f64);
            let sub_params = FluvialParams {
                dt_yr: sub_dt,
                max_incision_cap_m: None,
                ..base_params.clone()
            };
            let mut worst_dh = 0.0_f64;
            for _ in 0..k {
                let mut child_hint = 1_usize;
                let (dh, _) = run_fluvial_step_advected(
                    lvl_cfg,
                    target_size,
                    cur_bedrock,
                    cur_sediment,
                    uplift_u,
                    lithology_id,
                    precip_for_s2,
                    &sub_params,
                    scratch,
                    t_elapsed_yr,
                    depth + 1,
                    &mut child_hint,
                    uncapped_substeps,
                )?;
                worst_dh = worst_dh.max(dh);
            }
            Ok((worst_dh, true))
        }
        Err(e) => Err(e.into()),
    }
}

#[inline]
fn det_hash01(seed: u64, ix: i64, iz: i64) -> f64 {
    let mut z = seed
        .wrapping_add((ix as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add((iz as u64).wrapping_mul(0x94D0_49BB_1331_11EB));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 * (1.0 / ((1u64 << 53) as f64))
}

#[inline]
fn det_vnoise(seed: u64, x: f64, z: f64) -> f64 {
    let fx = libm::floor(x);
    let fz = libm::floor(z);
    let ix = fx as i64;
    let iz = fz as i64;
    let tx = x - fx;
    let tz = z - fz;
    let sx = tx * tx * tx * (tx * (tx * 6.0 - 15.0) + 10.0);
    let sz = tz * tz * tz * (tz * (tz * 6.0 - 15.0) + 10.0);
    let v00 = det_hash01(seed, ix, iz);
    let v10 = det_hash01(seed, ix + 1, iz);
    let v01 = det_hash01(seed, ix, iz + 1);
    let v11 = det_hash01(seed, ix + 1, iz + 1);
    let vx0 = v00 + sx * (v10 - v00);
    let vx1 = v01 + sx * (v11 - v01);
    (vx0 + sz * (vx1 - vx0)) * 2.0 - 1.0
}

/// Forms 5–6 broad late-Quaternary humid piedmont lake basins (`C3`) on non-trunk coastal
/// interfluves after S2/S3 erosion (`lvl >= 1024`), bounded below by `8 m` ASL and rimmed by
/// gentle morainic/alluvial sills so `Fill-Spill-Merge` retains large `FreshwaterSpilling`
/// lakes (`endo_humid == 0.0`, `lake_surface_fraction ~ 0.012–0.024`).
fn apply_late_quaternary_piedmont_subsidence(
    bedrock_h: &mut [f32],
    sediment_s: &mut [f32],
    precip_m_yr: &[f32],
    lvl: usize,
    dx_m: f32,
    seed: u64,
    basin_amp: f32,
) {
    if lvl < 1024 || basin_amp <= 0.0 {
        return;
    }
    let n = lvl * lvl;
    if bedrock_h.len() != n || sediment_s.len() != n || precip_m_yr.len() != n {
        return;
    }
    let mut total: Vec<f32> = bedrock_h
        .iter()
        .zip(sediment_s.iter())
        .map(|(&b, &s)| b + s)
        .collect();
    let mut pf_d8 = vec![NO_DIR; n];
    let mut pf_ord = Vec::new();
    priority_flood_d8_lean_into(
        &mut total,
        &mut pf_d8,
        &mut pf_ord,
        lvl,
        lvl,
        Some(0.0),
        true,
    );
    let mut acc_cells = vec![1_u32; n];
    for &u_u32 in pf_ord.iter().rev() {
        let u = u_u32 as usize;
        let d = pf_d8[u];
        if d < NO_DIR {
            let nr = (u / lvl) as isize + DZ8[usize::from(d)];
            let nc = (u % lvl) as isize + DX8[usize::from(d)];
            if nr >= 0 && nc >= 0 && nr < lvl as isize && nc < lvl as isize {
                let v = nr as usize * lvl + nc as usize;
                acc_cells[v] = acc_cells[v].saturating_add(acc_cells[u]);
            }
        }
    }
    drop(total);
    drop(pf_d8);
    drop(pf_ord);

    let dx = f64::from(dx_m);
    let cell_area_m2 = dx * dx;
    let trunk_cells = ((2.5e6 / cell_area_m2).round() as u32).max(1);
    let b_scale = libm::sqrt(f64::from(basin_amp).clamp(0.5, 2.5));
    let rx_m = 920.0 * b_scale;
    let ry_m = 1_650.0 * b_scale;
    let margin = lvl / 10;

    let base_vs = [0.12_f64, 0.27, 0.42, 0.58, 0.73, 0.88];
    let dr_cells = (250.0 / dx).round() as isize;
    let mut centers: Vec<(usize, usize)> = Vec::with_capacity(base_vs.len());
    for (bi, &v0) in base_vs.iter().enumerate() {
        let jitter = 0.02 * (det_hash01(seed ^ 0x4C41_4B45_0000_0001, bi as i64, 0) * 2.0 - 1.0);
        let rc = ((v0 + jitter) * (lvl - 1) as f64).round() as usize;
        let rc = rc.clamp(margin, lvl.saturating_sub(margin + 1));
        let mut cands: Vec<usize> = Vec::new();
        for dr in -dr_cells..=dr_cells {
            let r = (rc as isize + dr).clamp(1, lvl as isize - 2) as usize;
            let row = r * lvl;
            for c in margin..lvl.saturating_sub(margin) {
                let i = row + c;
                let h = bedrock_h[i] + sediment_s[i];
                if (35.0..=145.0).contains(&h)
                    && precip_m_yr[i] >= 1.45
                    && acc_cells[i] < trunk_cells
                {
                    cands.push(c);
                }
            }
        }
        if cands.len() >= 12 {
            cands.sort_unstable();
            let cc = cands[cands.len() * 3 / 10];
            let too_close = centers.iter().any(|&(pr, pc)| {
                let dist_m = dx * libm::hypot(rc as f64 - pr as f64, cc as f64 - pc as f64);
                dist_m < 4_500.0
            });
            if !too_close {
                centers.push((rc, cc));
            }
        }
    }

    let max_rw = 1.45_f64;
    let rad_r = (libm::ceil(1.8 * ry_m / dx) as isize).max(4);
    let rad_c = (libm::ceil(1.8 * rx_m / dx) as isize).max(4);
    let mut lip_samples = Vec::with_capacity(512);
    let mut footprint_hs = Vec::with_capacity(2048);

    for (bi, &(rc, cc)) in centers.iter().enumerate() {
        let bs = seed ^ 0x4C41_4B45_55AA_0000 ^ (bi as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let r_lo = (rc as isize - rad_r).max(1) as usize;
        let r_hi = (rc as isize + rad_r).min(lvl as isize - 2) as usize;
        let c_lo = (cc as isize - rad_c).max(1) as usize;
        let c_hi = (cc as isize + rad_c).min(lvl as isize - 2) as usize;

        footprint_hs.clear();
        let mut ok_footprint = 0usize;
        for r in r_lo..=r_hi {
            let row = r * lvl;
            let uz = (r as f64 - rc as f64) * dx / ry_m;
            for c in c_lo..=c_hi {
                let ux = (c as f64 - cc as f64) * dx / rx_m;
                if libm::hypot(ux, uz) <= 1.0 {
                    let h = f64::from(bedrock_h[row + c] + sediment_s[row + c]);
                    footprint_hs.push(h);
                    if (18.0..=220.0).contains(&h) {
                        ok_footprint += 1;
                    }
                }
            }
        }
        if footprint_hs.len() < 16 || ok_footprint * 5 < footprint_hs.len() * 3 {
            continue;
        }
        footprint_hs.sort_unstable_by(f64::total_cmp);
        if footprint_hs[footprint_hs.len() / 2] > 165.0 {
            continue;
        }

        let warped_r = |r: usize, c: usize| -> f64 {
            let ux = (c as f64 - cc as f64) * dx / rx_m;
            let uz = (r as f64 - rc as f64) * dx / ry_m;
            let re = libm::hypot(ux, uz);
            if re > 1.8 {
                return re;
            }
            let nv = 0.68 * det_vnoise(bs, (c as f64 * dx) / 700.0, (r as f64 * dx) / 700.0)
                + 0.32
                    * det_vnoise(
                        bs ^ 0x1234_5678,
                        (c as f64 * dx) / 350.0,
                        (r as f64 * dx) / 350.0,
                    );
            (re * (1.0 + 0.22 * nv)).max(0.0)
        };

        lip_samples.clear();
        for r in r_lo..=r_hi {
            let row = r * lvl;
            for c in c_lo..=c_hi {
                let rw = warped_r(r, c);
                if (0.95..=1.22).contains(&rw) {
                    let h = f64::from(bedrock_h[row + c] + sediment_s[row + c]);
                    if h > 12.0 && h < 320.0 {
                        lip_samples.push(h);
                    }
                }
            }
        }
        if lip_samples.len() < 16 {
            continue;
        }
        lip_samples.sort_unstable_by(f64::total_cmp);
        let h_lip = lip_samples[lip_samples.len() / 4].clamp(38.0, 165.0);
        let h_floor = (h_lip - 24.0 * b_scale).max(10.0);

        for r in r_lo..=r_hi {
            let row = r * lvl;
            for c in c_lo..=c_hi {
                let rw = warped_r(r, c);
                if rw >= max_rw {
                    continue;
                }
                let i = row + c;
                let h = f64::from(bedrock_h[i] + sediment_s[i]);
                if h <= 10.0 || h >= 220.0 {
                    continue;
                }
                let elev_fade = ((220.0 - h) / 60.0).clamp(0.0, 1.0);
                if rw < 0.98 {
                    let t_wall = ((rw - 0.58) / 0.40).clamp(0.0, 1.0);
                    let smooth_w = t_wall * t_wall * (3.0 - 2.0 * t_wall);
                    let target_h = h_floor + (h_lip - h_floor) * smooth_w;
                    if h > target_h {
                        let drop_m = ((h - target_h) * elev_fade).min(38.0);
                        let new_h = (h - drop_m).max(8.0);
                        let new_s = f64::from(sediment_s[i]).max(2.0);
                        sediment_s[i] = new_s as f32;
                        bedrock_h[i] = (new_h - new_s) as f32;
                    }
                } else if rw <= 1.38 && acc_cells[i] < trunk_cells && h < 190.0 {
                    let dr = (rw - 1.08) / 0.14;
                    let rim_bell = libm::exp(-(dr * dr));
                    let sill_target = h_lip + 3.5;
                    if h < sill_target {
                        let raise_m = ((sill_target - h) * rim_bell * elev_fade).min(30.0);
                        bedrock_h[i] += raise_m as f32;
                    }
                }
            }
        }
    }
}

/// Injects deterministic multi-octave coordinate-indexed littoral micro-coves and headlands
/// (`lvl >= 1024`) near `sea_level_m = 0` outside rocky `GraniteCrystalline` cliffs so bilinear
/// multigrid upsampling preserves coastal fractal dimension `D_shore`.
fn inject_fine_littoral_ria(
    bedrock_h: &mut [f32],
    sediment_s: &[f32],
    lithology_id: &[u8],
    lvl: usize,
    lvl_dx: f32,
    seed: u64,
    coast_frac_amp: f32,
) {
    if coast_frac_amp <= 0.0
        || lvl < 1024
        || bedrock_h.len() != lvl * lvl
        || sediment_s.len() != lvl * lvl
        || lithology_id.len() != lvl * lvl
    {
        return;
    }
    let amp_m = 14.5 * f64::from((coast_frac_amp / 0.055).min(1.5));
    let base_freq = 22.0_f64;
    let octaves = if lvl >= 2048 { 6u64 } else { 5u64 };
    let denom = (lvl - 1) as f64;
    let s0 = seed ^ 0xC0A5_F14E_0000_0000 ^ (lvl as u64);
    let granite_id = LithologyClass::GraniteCrystalline as u8;
    let lvl_n = lvl * lvl;

    // Protect active river mouths (`A >= 0.5 km^2`) within `±2` cells so coastal coves never chop trunk outlets.
    let mut total = vec![0.0_f32; lvl_n];
    for ((dst, &b), &s) in total
        .iter_mut()
        .zip(bedrock_h.iter())
        .zip(sediment_s.iter())
    {
        *dst = b + s;
    }
    let mut pf_d8 = vec![NO_DIR; lvl_n];
    let mut pf_ord = Vec::new();
    priority_flood_d8_lean_into(
        &mut total,
        &mut pf_d8,
        &mut pf_ord,
        lvl,
        lvl,
        Some(0.0),
        true,
    );
    let mut acc_cells = vec![1_u32; lvl_n];
    for &u_u32 in pf_ord.iter().rev() {
        let u = u_u32 as usize;
        let d = pf_d8[u];
        if d < NO_DIR {
            let nr = (u / lvl) as isize + DZ8[usize::from(d)];
            let nc = (u % lvl) as isize + DX8[usize::from(d)];
            if nr >= 0 && nc >= 0 && nr < lvl as isize && nc < lvl as isize {
                let v = nr as usize * lvl + nc as usize;
                acc_cells[v] = acc_cells[v].saturating_add(acc_cells[u]);
            }
        }
    }
    let cell_area_m2 = f64::from(lvl_dx) * f64::from(lvl_dx);
    let mut near_mouth = vec![false; lvl_n];
    for r in 0..lvl {
        let row_base = r * lvl;
        for c in 0..lvl {
            let i = row_base + c;
            if f64::from(acc_cells[i]) * cell_area_m2 >= 5.0e5 {
                let r_lo = r.saturating_sub(2);
                let r_hi = (r + 2).min(lvl - 1);
                let c_lo = c.saturating_sub(2);
                let c_hi = (c + 2).min(lvl - 1);
                for nr in r_lo..=r_hi {
                    let nrow = nr * lvl;
                    for nc in c_lo..=c_hi {
                        near_mouth[nrow + nc] = true;
                    }
                }
            }
        }
    }
    drop(total);
    drop(pf_d8);
    drop(pf_ord);
    drop(acc_cells);

    bedrock_h
        .par_chunks_mut(lvl)
        .zip(sediment_s.par_chunks(lvl))
        .zip(lithology_id.par_chunks(lvl))
        .zip(near_mouth.par_chunks(lvl))
        .enumerate()
        .for_each(|(row, (((bed_row, sed_row), lith_row), mouth_row))| {
            let v = row as f64 / denom;
            for col in 0..lvl {
                if lith_row[col] == granite_id || mouth_row[col] {
                    continue;
                }
                let h = f64::from(bed_row[col] + sed_row[col]);
                if libm::fabs(h) >= 32.0 {
                    continue;
                }
                let u = col as f64 / denom;
                let mut fbm5 = 0.0;
                let mut amp = 0.48;
                let mut freq = 1.0;
                for oct in 0..octaves {
                    let so = s0.wrapping_add(oct.wrapping_mul(0x9E37_79B9_7F4A_7C15));
                    fbm5 += amp * det_vnoise(so, u * base_freq * freq, v * base_freq * freq);
                    freq *= 2.02;
                    amp *= 0.74;
                }
                let h_fade = libm::exp(-(h / 13.0) * (h / 13.0));
                bed_row[col] += (amp_m * fbm5 * h_fade) as f32;
            }
        });
}

/// Widens mature trunk river floodplains (`Strahler >= 4`, `z < 800 m`, slope `< 0.14`) strictly
/// perpendicular to `d8_dir` across U-shaped alluvial valley floors (`b_down >= 1.6`, `vf_down >= 1.5`)
/// and trims convex colluvial toe notches on `Strahler 2..=3` upper V-valleys (`b_up in [0.85, 1.35]`),
/// never modifying channel cells (`strahler[j] >= 1`) so longitudinal Flint concavity `theta` stays intact.
#[allow(clippy::too_many_arguments)]
fn widen_trunk_alluvial_floodplains(
    bedrock_h: &mut [f32],
    sediment_s: &mut [f32],
    river_w_m: &[f32],
    strahler: &[u8],
    d8_dir: &[u8],
    lvl: usize,
    dx_m: f32,
) {
    let n = lvl * lvl;
    if lvl < 32
        || bedrock_h.len() != n
        || sediment_s.len() != n
        || river_w_m.len() != n
        || strahler.len() != n
        || d8_dir.len() != n
    {
        return;
    }
    let mut fp_target = vec![f32::INFINITY; n];
    let mut fp_alpha = vec![0.0_f32; n];

    for r in 2..lvl.saturating_sub(2) {
        let row = r * lvl;
        for c in 2..lvl.saturating_sub(2) {
            let i = row + c;
            let ord = strahler[i];
            if ord < 2 || d8_dir[i] >= 8 {
                continue;
            }
            let z_i = bedrock_h[i] + sediment_s[i];
            if z_i <= 1.5 {
                continue;
            }
            let gx = (bedrock_h[i + 1] + sediment_s[i + 1] - bedrock_h[i - 1] - sediment_s[i - 1])
                / (2.0 * dx_m);
            let gz = (bedrock_h[i + lvl] + sediment_s[i + lvl]
                - bedrock_h[i - lvl]
                - sediment_s[i - lvl])
                / (2.0 * dx_m);
            let slope = libm::hypotf(gx, gz);
            let d = usize::from(d8_dir[i]);
            let inv_dist = 1.0 / (DIST8[d] as f32);
            let ux = (DX8[d] as f32) * inv_dist;
            let uz = (DZ8[d] as f32) * inv_dist;

            if ord >= 4 && z_i < 800.0 && slope < 0.14 {
                let w_fp = (280.0 + 40.0 * river_w_m[i]).clamp(280.0, 560.0);
                let r_ref = 65.0_f32.max(1.5 * dx_m);
                let rad = (libm::ceilf(w_fp / dx_m) as isize).max(1);
                let r_lo = (r as isize - rad).max(1) as usize;
                let r_hi = (r as isize + rad).min(lvl as isize - 2) as usize;
                let c_lo = (c as isize - rad).max(1) as usize;
                let c_hi = (c as isize + rad).min(lvl as isize - 2) as usize;
                for nr in r_lo..=r_hi {
                    let dz_m = (nr as f32 - r as f32) * dx_m;
                    let nrow = nr * lvl;
                    for nc in c_lo..=c_hi {
                        let j = nrow + nc;
                        if strahler[j] >= 1 {
                            continue;
                        }
                        let z_j = bedrock_h[j] + sediment_s[j];
                        if z_j <= 0.5 {
                            continue;
                        }
                        let dx_c = (nc as f32 - c as f32) * dx_m;
                        let along = libm::fabsf(dx_c * ux + dz_m * uz);
                        let perp = libm::fabsf(dx_c * (-uz) + dz_m * ux);
                        if along > 1.15 * dx_m || perp > w_fp {
                            continue;
                        }
                        let u_r = (perp / r_ref).max(0.0);
                        let t_h = z_i + 0.36 * libm::powf(u_r, 2.15);
                        let t = ((w_fp - perp) / (0.38 * w_fp)).clamp(0.0, 1.0);
                        let a = t * t * (3.0 - 2.0 * t) * 0.96;
                        if t_h < fp_target[j] {
                            fp_target[j] = t_h;
                            fp_alpha[j] = fp_alpha[j].max(a);
                        }
                    }
                }
            } else if (ord == 2 || ord == 3) && slope < 0.65 {
                let px = -uz;
                let pz = ux;
                let max_idx = lvl as isize - 2;
                for &side in &[-1.0_f32, 1.0_f32] {
                    let r1 = libm::roundf(r as f32 + side * pz) as isize;
                    let c1 = libm::roundf(c as f32 + side * px) as isize;
                    let r2 = libm::roundf(r as f32 + 2.0 * side * pz) as isize;
                    let c2 = libm::roundf(c as f32 + 2.0 * side * px) as isize;
                    if r1 < 1
                        || r1 > max_idx
                        || c1 < 1
                        || c1 > max_idx
                        || r2 < 1
                        || r2 > max_idx
                        || c2 < 1
                        || c2 > max_idx
                    {
                        continue;
                    }
                    let j1 = (r1 as usize) * lvl + (c1 as usize);
                    let j2 = (r2 as usize) * lvl + (c2 as usize);
                    if strahler[j1] != 0 {
                        continue;
                    }
                    let z_j1 = bedrock_h[j1] + sediment_s[j1];
                    let z_j2 = bedrock_h[j2] + sediment_s[j2];
                    if z_j1 > 0.5 && z_j2 > z_i + 4.0 && z_j1 > z_i + 0.58 * (z_j2 - z_i) {
                        let th = z_i + 0.50 * (z_j2 - z_i);
                        if z_j1 - th <= 45.0 && th < fp_target[j1] {
                            fp_target[j1] = th;
                            fp_alpha[j1] = fp_alpha[j1].max(0.50);
                        }
                    }
                }
            }
        }
    }

    for j in 0..n {
        let th = fp_target[j];
        if !th.is_finite() {
            continue;
        }
        let z_j = bedrock_h[j] + sediment_s[j];
        let diff = z_j - th;
        if diff > 0.0 && z_j > 0.5 && diff < 75.0 {
            let cut_fade = ((75.0 - diff) / 30.0).clamp(0.0, 1.0);
            let new_z = (z_j - fp_alpha[j] * cut_fade * diff).max(0.5);
            sediment_s[j] = sediment_s[j].max(1.2);
            bedrock_h[j] = new_z - sediment_s[j];
        }
    }
}

/// Relaxes opposing `> 70°` 1-cell needle cones/pits deterministically via Jacobi passes
/// (`1`-thread == `N`-thread bit-exact) without softening monotonic bedrock cliffs or waterfall drops.
fn clamp_opposing_needle_spikes(total_h: &mut [f32], lvl: usize, dx_m: f32) {
    if lvl < 4 || total_h.len() != lvl * lvl {
        return;
    }
    let max_rev_dh = 2.8_f32 * dx_m;
    let clamped_dh = 0.90 * max_rev_dh;
    let mut prev = total_h.to_vec();
    for _ in 0..8 {
        total_h
            .par_chunks_mut(lvl)
            .enumerate()
            .for_each(|(r, dst_row)| {
                if r == 0 || r + 1 >= lvl {
                    return;
                }
                let row = r * lvl;
                for (c, dst_slot) in dst_row.iter_mut().enumerate().take(lvl - 1).skip(1) {
                    let i = row + c;
                    let hc = prev[i];
                    if hc <= 0.0 {
                        continue;
                    }
                    let hl = prev[i - 1];
                    let hr = prev[i + 1];
                    let hu = prev[i - lvl];
                    let hd = prev[i + lvl];
                    let mut next_h = hc;
                    if hc - hl > max_rev_dh && hc - hr > max_rev_dh {
                        next_h = next_h.min(hl.max(hr) + clamped_dh);
                    } else if hl - hc > max_rev_dh && hr - hc > max_rev_dh {
                        next_h = next_h.max(hl.min(hr) - clamped_dh);
                    }
                    if hc - hu > max_rev_dh && hc - hd > max_rev_dh {
                        next_h = next_h.min(hu.max(hd) + clamped_dh);
                    } else if hu - hc > max_rev_dh && hd - hc > max_rev_dh {
                        next_h = next_h.max(hu.min(hd) - clamped_dh);
                    }
                    *dst_slot = next_h;
                }
            });
        prev.copy_from_slice(total_h);
    }
}

/// Runs the complete deterministic multigrid World v2 pipeline (`S1 -> S9`) using the default
/// multigrid schedule and writes raw little-endian stage buffers, `cave_network.bin`, and
/// `raw_meta.json` directly into `out_dir`.
pub fn run_worldgen_to_dir(
    cfg: &WorldV2Config,
    out_dir: &Path,
) -> Result<RawWorldMeta, PipelineError> {
    let schedule = compute_multigrid_schedule(cfg.grid_size);
    run_worldgen_to_dir_with_schedule(cfg, out_dir, &schedule)
}

/// Runs the complete deterministic multigrid World v2 pipeline (`S1 -> S9`) using a caller-supplied
/// multigrid schedule `[(grid_size, s2_steps), ...]` and writes raw stage buffers to `out_dir`.
pub fn run_worldgen_to_dir_with_schedule(
    cfg: &WorldV2Config,
    out_dir: &Path,
    schedule: &[(u32, usize)],
) -> Result<RawWorldMeta, PipelineError> {
    cfg.validate()?;
    if schedule.is_empty() {
        return Err(PipelineError {
            message: "multigrid schedule must not be empty".to_owned(),
        });
    }
    let mut prev_lvl = 0_u32;
    for &(lvl_u32, n_steps) in schedule {
        if lvl_u32 < 64 || !lvl_u32.is_power_of_two() {
            return Err(PipelineError {
                message: format!(
                    "multigrid schedule level size {lvl_u32} must be a power of two >= 64"
                ),
            });
        }
        if lvl_u32 <= prev_lvl {
            return Err(PipelineError {
                message: format!(
                    "multigrid schedule level sizes must strictly increase ({prev_lvl} -> {lvl_u32})"
                ),
            });
        }
        if n_steps == 0 {
            return Err(PipelineError {
                message: format!("multigrid schedule level {lvl_u32} must have steps > 0"),
            });
        }
        prev_lvl = lvl_u32;
    }
    if prev_lvl != cfg.grid_size {
        return Err(PipelineError {
            message: format!(
                "multigrid schedule final size {prev_lvl} must match cfg.grid_size {}",
                cfg.grid_size
            ),
        });
    }

    std::fs::create_dir_all(out_dir)?;

    let target_size = cfg.grid_size;

    // -------------------------------------------------------------------------
    // Multigrid S1 + S2/S3 with 2-pass S7 coupling
    // -------------------------------------------------------------------------
    let mut cur_bedrock: Vec<f32> = Vec::new();
    let mut cur_sediment: Vec<f32> = Vec::new();
    let mut cur_size: usize = 0;
    let mut cached_clim_precip: Option<(usize, Vec<f32>)> = None;
    let mut t_elapsed_yr: f64 = 0.0;
    let mut uncapped_fluvial_substeps: usize = 0;

    for (level_idx, &(lvl_u32, n_steps)) in schedule.iter().enumerate() {
        let lvl = lvl_u32 as usize;
        let lvl_n = lvl * lvl;
        let lvl_dx = cfg.extent_m / (lvl_u32 as f32);
        let lvl_cfg = WorldV2Config {
            grid_size: lvl_u32,
            ..cfg.clone()
        };

        let (mut uplift_u, mut lithology_id) = if level_idx == 0 {
            let s1 = generate_uplift_for_target(&lvl_cfg, target_size)?;
            cur_bedrock = s1.bedrock_h;
            cur_sediment = vec![0.0_f32; lvl_n];
            drop(s1.stratum_k);
            (s1.uplift_u, s1.lithology_id)
        } else {
            let up_bed = upsample_bilinear(&cur_bedrock, cur_size, cur_size, lvl, lvl);
            drop(cur_bedrock);
            cur_bedrock = up_bed;
            let up_sed = upsample_bilinear(&cur_sediment, cur_size, cur_size, lvl, lvl);
            drop(cur_sediment);
            cur_sediment = up_sed;

            generate_fine_s1_aux_at_time(
                &lvl_cfg,
                &cur_bedrock,
                target_size,
                f64::from(UPLIFT_M_YR_SCALE),
                t_elapsed_yr,
            )?
        };
        cur_size = lvl;
        for u in &mut uplift_u {
            *u *= UPLIFT_M_YR_SCALE;
        }

        // 2-pass S7 climate-erosion coupling + Step 0 Exp. B multigrid `dt_yr` & incision cap:
        // - `512²` (`80 × 1,000 yr`), `1024²` (`40 × 1,000 yr`),
        //   `2048²` (`25 × 200 yr = 5.0 kyr`), `4096²` (`40 × 100 yr = 4.0 kyr`), `G = 0.85` kept.
        let fluvial_params = FluvialParams {
            dx_m: f64::from(lvl_dx),
            dt_yr: if lvl_dx <= 12.0 {
                100.0
            } else if lvl_dx <= 25.0 {
                200.0
            } else if lvl_dx <= 45.0 {
                750.0
            } else {
                FLUVIAL_DT_YR
            },
            g_deposition: 0.85,
            max_gs_iters: 64,
            gs_tol_m: 5.0e-4,
            max_incision_cap_m: Some(0.25 * STRAT_MIN_BED_THICKNESS_M),
            ..FluvialParams::default()
        };
        let thermal_params = ThermalHillslopeParams {
            dx_m: lvl_dx,
            dt_yr: if lvl_dx < 30.0 { 45.0 } else { 120.0 },
            diffusivity_d: if lvl_dx < 30.0 { 0.018 } else { 0.025 },
            talus_rate: if lvl_dx < 30.0 { 0.52 } else { 0.45 },
            substeps: if lvl_dx < 30.0 {
                cfg.talus_substeps
            } else {
                12
            },
            ..ThermalHillslopeParams::default()
        };
        let glacial_mult_f64 = f64::from(cfg.glacial_mult).clamp(0.5, 5.0);
        let glacial_kg = 1.0e-4 * libm::sqrt(glacial_mult_f64);
        let glacial_max_erode = 5.0 * libm::sqrt(glacial_mult_f64);
        let glacial_steps = if schedule.len() == 1 {
            if lvl_u32 >= 256 {
                libm::round(10.0 * glacial_mult_f64) as usize
            } else {
                0
            }
        } else if lvl_u32 == 512 {
            libm::round(6.0 * glacial_mult_f64) as usize
        } else if lvl_u32 == 1024 {
            libm::round(4.0 * glacial_mult_f64) as usize
        } else {
            0
        };

        let mut precip_for_s2: Vec<f32>;
        let talus_frac_out = if schedule.len() == 1 {
            let clim_cfg = ClimateConfig::for_world_with_target(&lvl_cfg, target_size);
            let total_tmp: Vec<f32> = cur_bedrock
                .iter()
                .zip(cur_sediment.iter())
                .map(|(&b, &s)| b + s)
                .collect();
            let clim1 = compute_climate(&total_tmp, lvl, lvl, None, &clim_cfg)?;
            drop(total_tmp);
            precip_for_s2 = clim1.precip_m_yr;
            let pet_tmp = clim1.pet_m_yr;
            drop(clim1.temp_c);
            drop(clim1.twi);
            drop(clim1.moisture);
            drop(clim1.ela_m);

            let half1 = n_steps / 2;
            let half2 = n_steps - half1;
            let mut scratch = FluvialLeanScratch::default();
            let mut fluvial_hint_k = 1_usize;
            for _ in 0..half1 {
                run_fluvial_step_advected(
                    &lvl_cfg,
                    target_size,
                    &mut cur_bedrock,
                    &mut cur_sediment,
                    &uplift_u,
                    &mut lithology_id,
                    &precip_for_s2,
                    &fluvial_params,
                    &mut scratch,
                    &mut t_elapsed_yr,
                    0,
                    &mut fluvial_hint_k,
                    &mut uncapped_fluvial_substeps,
                )?;
            }
            drop(scratch);

            // Pass 2 S7 update via S5 flow_acc on eroded surface.
            let mut total_mid: Vec<f32> = cur_bedrock
                .iter()
                .zip(cur_sediment.iter())
                .map(|(&b, &s)| b + s)
                .collect();
            let mut d8_dir = vec![NO_DIR; lvl_n];
            let mut ord = Vec::new();
            priority_flood_d8_lean_into(
                &mut total_mid,
                &mut d8_dir,
                &mut ord,
                lvl,
                lvl,
                Some(0.0),
                false,
            );
            drop(ord);
            let hcfg = worldgen_hydro_config(cfg, lvl_dx);
            let hydro = route_hybrid_mfd_d8_lean(
                &total_mid,
                &d8_dir,
                lvl,
                lvl,
                &precip_for_s2,
                &pet_tmp,
                &hcfg,
            );
            drop(d8_dir);
            drop(pet_tmp);
            drop(total_mid);
            let total_mid2: Vec<f32> = cur_bedrock
                .iter()
                .zip(cur_sediment.iter())
                .map(|(&b, &s)| b + s)
                .collect();
            let clim2 =
                compute_climate(&total_mid2, lvl, lvl, Some(&hydro.flow_acc_m2), &clim_cfg)?;
            drop(total_mid2);
            drop(hydro);
            precip_for_s2 = clim2.precip_m_yr;
            drop(clim2.temp_c);
            drop(clim2.pet_m_yr);
            drop(clim2.twi);
            drop(clim2.moisture);
            drop(clim2.ela_m);

            let mut scratch2 = FluvialLeanScratch::default();
            for _ in 0..half2 {
                run_fluvial_step_advected(
                    &lvl_cfg,
                    target_size,
                    &mut cur_bedrock,
                    &mut cur_sediment,
                    &uplift_u,
                    &mut lithology_id,
                    &precip_for_s2,
                    &fluvial_params,
                    &mut scratch2,
                    &mut t_elapsed_yr,
                    0,
                    &mut fluvial_hint_k,
                    &mut uncapped_fluvial_substeps,
                )?;
            }
            drop(scratch2);
            if glacial_steps > 0 {
                let _s2b = run_paleo_glacial_pass(
                    &mut cur_bedrock,
                    &mut cur_sediment,
                    lvl,
                    lvl,
                    &GlacialParams {
                        dx_m: f64::from(lvl_dx),
                        steps: glacial_steps,
                        k_g: glacial_kg,
                        max_step_erode_m: glacial_max_erode,
                        ..GlacialParams::default()
                    },
                );
                update_lithology_advected_in_place(
                    &lvl_cfg,
                    target_size,
                    &cur_bedrock,
                    &uplift_u,
                    t_elapsed_yr,
                    &mut lithology_id,
                )?;
            }
            Vec::new()
        } else {
            if level_idx == 0 {
                let clim_cfg = ClimateConfig::for_world_with_target(&lvl_cfg, target_size);
                let total_tmp: Vec<f32> = cur_bedrock
                    .iter()
                    .zip(cur_sediment.iter())
                    .map(|(&b, &s)| b + s)
                    .collect();
                let clim1 = compute_climate(&total_tmp, lvl, lvl, None, &clim_cfg)?;
                drop(total_tmp);
                precip_for_s2 = clim1.precip_m_yr;
            } else if level_idx == 1 {
                let clim_cfg = ClimateConfig::for_world_with_target(&lvl_cfg, target_size);
                let total_tmp: Vec<f32> = cur_bedrock
                    .iter()
                    .zip(cur_sediment.iter())
                    .map(|(&b, &s)| b + s)
                    .collect();
                let clim1 = compute_climate(&total_tmp, lvl, lvl, None, &clim_cfg)?;
                let mut filled = total_tmp.clone();
                let mut d8_dir = vec![NO_DIR; lvl_n];
                let mut ord = Vec::new();
                priority_flood_d8_lean_into(
                    &mut filled,
                    &mut d8_dir,
                    &mut ord,
                    lvl,
                    lvl,
                    Some(0.0),
                    false,
                );
                let hcfg = worldgen_hydro_config(cfg, lvl_dx);
                let hydro = route_hybrid_mfd_d8_lean(
                    &filled,
                    &d8_dir,
                    lvl,
                    lvl,
                    &clim1.precip_m_yr,
                    &clim1.pet_m_yr,
                    &hcfg,
                );
                drop(filled);
                drop(d8_dir);
                drop(ord);
                let clim2 =
                    compute_climate(&total_tmp, lvl, lvl, Some(&hydro.flow_acc_m2), &clim_cfg)?;
                drop(total_tmp);
                drop(hydro);
                cached_clim_precip = Some((lvl, clim2.precip_m_yr.clone()));
                precip_for_s2 = clim2.precip_m_yr;
            } else if let Some((cw, ref cp)) = cached_clim_precip {
                precip_for_s2 = upsample_bilinear(cp, cw, cw, lvl, lvl);
            } else {
                let clim_cfg = ClimateConfig::for_world_with_target(&lvl_cfg, target_size);
                let total_tmp: Vec<f32> = cur_bedrock
                    .iter()
                    .zip(cur_sediment.iter())
                    .map(|(&b, &s)| b + s)
                    .collect();
                let clim1 = compute_climate(&total_tmp, lvl, lvl, None, &clim_cfg)?;
                drop(total_tmp);
                precip_for_s2 = clim1.precip_m_yr;
            }

            let mut scratch = FluvialLeanScratch::default();
            let mut fluvial_hint_k = 1_usize;
            for _ in 0..n_steps {
                run_fluvial_step_advected(
                    &lvl_cfg,
                    target_size,
                    &mut cur_bedrock,
                    &mut cur_sediment,
                    &uplift_u,
                    &mut lithology_id,
                    &precip_for_s2,
                    &fluvial_params,
                    &mut scratch,
                    &mut t_elapsed_yr,
                    0,
                    &mut fluvial_hint_k,
                    &mut uncapped_fluvial_substeps,
                )?;
            }
            if glacial_steps > 0 {
                let _s2b = run_paleo_glacial_pass(
                    &mut cur_bedrock,
                    &mut cur_sediment,
                    lvl,
                    lvl,
                    &GlacialParams {
                        dx_m: f64::from(lvl_dx),
                        steps: glacial_steps,
                        k_g: glacial_kg,
                        max_step_erode_m: glacial_max_erode,
                        ..GlacialParams::default()
                    },
                );
                update_lithology_advected_in_place(
                    &lvl_cfg,
                    target_size,
                    &cur_bedrock,
                    &uplift_u,
                    t_elapsed_yr,
                    &mut lithology_id,
                )?;
            }
            let mut talus_frac = vec![0.0_f32; lvl_n];
            let (s_lo, s_hi, t_lo, t_hi) = scratch.thermal_halves_mut();
            let _s3_stats = crate::erosion_thermal::relax_thermal_and_hillslope_with_scratch(
                &mut cur_bedrock,
                &mut cur_sediment,
                &mut talus_frac,
                &lithology_id,
                lvl,
                lvl,
                &thermal_params,
                s_lo,
                s_hi,
                t_lo,
                t_hi,
            );
            drop(scratch);
            talus_frac
        };

        // `talus_frac` was returned by whichever branch ran S2; on non-final levels we drop it after S3.
        let talus_frac = if schedule.len() == 1 {
            let mut tf = vec![0.0_f32; lvl_n];
            let _s3_stats = relax_thermal_and_hillslope(
                &mut cur_bedrock,
                &mut cur_sediment,
                &mut tf,
                &lithology_id,
                lvl,
                lvl,
                &thermal_params,
            );
            tf
        } else {
            talus_frac_out
        };

        let is_final = level_idx + 1 == schedule.len();
        if is_final {
            update_lithology_advected_in_place(
                &lvl_cfg,
                target_size,
                &cur_bedrock,
                &uplift_u,
                t_elapsed_yr,
                &mut lithology_id,
            )?;
            if lvl_u32 >= 512 && cfg.sapping_gain > 0.0 {
                apply_caprock_knickpoint_sapping(
                    &mut cur_bedrock,
                    &mut cur_sediment,
                    &lithology_id,
                    &precip_for_s2,
                    lvl,
                    lvl_dx,
                    cfg.sapping_gain,
                );
            }
            apply_late_quaternary_piedmont_subsidence(
                &mut cur_bedrock,
                &mut cur_sediment,
                &precip_for_s2,
                lvl,
                lvl_dx,
                cfg.seed,
                cfg.basin_amp,
            );
            update_lithology_advected_in_place(
                &lvl_cfg,
                target_size,
                &cur_bedrock,
                &uplift_u,
                t_elapsed_yr,
                &mut lithology_id,
            )?;
            inject_fine_littoral_ria(
                &mut cur_bedrock,
                &cur_sediment,
                &lithology_id,
                lvl,
                lvl_dx,
                cfg.seed,
                cfg.coast_frac_amp,
            );
            // Spill uplift_u, lithology_id, talus_frac, bedrock_h, sediment_s; drop unneeded buffers.
            let mut layers: Vec<LayerMeta> = Vec::with_capacity(30);
            drop(cached_clim_precip.take());
            drop(precip_for_s2);
            drop(uplift_u);

            upsert_layer(
                &mut layers,
                write_u8_layer(
                    out_dir,
                    "lithology_id.bin",
                    &lithology_id,
                    target_size,
                    target_size,
                )?,
            );
            drop(lithology_id);

            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "talus_frac.bin",
                    &talus_frac,
                    target_size,
                    target_size,
                )?,
            );
            drop(talus_frac);

            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "bedrock_h_m.bin",
                    &cur_bedrock,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "sediment_s_m.bin",
                    &cur_sediment,
                    target_size,
                    target_size,
                )?,
            );

            // Form total_h in-place inside cur_bedrock (`64 MiB`), drop cur_sediment (`-64 MiB`).
            for (b, &s) in cur_bedrock.iter_mut().zip(cur_sediment.iter()) {
                *b += s;
            }
            let total_h = cur_bedrock;
            drop(cur_sediment);

            // -----------------------------------------------------------------
            // S7 Pass 1 at target N^2 (`total_h` = 64 MiB -> peak ~452 MiB)
            // -----------------------------------------------------------------
            let target_dx = cfg.cell_size_m();
            let clim_cfg = ClimateConfig::for_world_with_target(cfg, target_size);
            let clim1 = compute_climate(&total_h, lvl, lvl, None, &clim_cfg)?;
            let wind_azimuth_rad = clim1.wind_azimuth_rad;
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "temp_c.bin",
                    &clim1.temp_c,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(out_dir, "ela_m.bin", &clim1.ela_m, target_size, target_size)?,
            );
            drop(clim1.temp_c);
            drop(clim1.ela_m);
            drop(clim1.twi);
            drop(clim1.moisture);
            let precip_m_yr = clim1.precip_m_yr;
            let pet_m_yr = clim1.pet_m_yr;
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "precip_m_yr.bin",
                    &precip_m_yr,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(out_dir, "pet_m_yr.bin", &pet_m_yr, target_size, target_size)?,
            );

            // -----------------------------------------------------------------
            // S5 Hydrology Lean (`total_h + precip + pet + pf + hydro` <= 800 MiB)
            // -----------------------------------------------------------------
            let mut pf_filled = total_h.clone();
            let mut d8_dir = vec![NO_DIR; lvl_n];
            let mut pf_ord = Vec::new();
            priority_flood_d8_lean_into(
                &mut pf_filled,
                &mut d8_dir,
                &mut pf_ord,
                lvl,
                lvl,
                Some(0.0),
                false,
            );
            drop(pf_ord);
            let hcfg = worldgen_hydro_config(cfg, target_dx);
            let mut hydro = route_hybrid_mfd_d8_lean_halves(
                &pf_filled,
                &d8_dir,
                lvl,
                lvl,
                &precip_m_yr,
                &pet_m_yr,
                &hcfg,
            );
            drop(pf_filled);
            let q_channel: Vec<bool> = hydro
                .discharge_q_m3_s
                .iter()
                .zip(total_h.iter())
                .map(|(&q, &h_m)| h_m > crate::SEA_LEVEL_M && q >= 0.05)
                .collect();
            let strahler = crate::anmw_v3::compute_strahler_order(&d8_dir, &q_channel, lvl, lvl);
            drop(q_channel);

            // -----------------------------------------------------------------
            // S7 Pass 2 (TWI + Moisture using S5 flow_acc_m2 halves -> peak ~848 MiB)
            // -----------------------------------------------------------------
            let (twi, moisture) = compute_twi_and_moisture_halves(
                &total_h,
                &precip_m_yr,
                &pet_m_yr,
                &hydro.flow_acc_lo,
                &hydro.flow_acc_hi,
                lvl,
                lvl,
                target_dx,
            )?;
            drop(hydro.flow_acc_lo);
            drop(hydro.flow_acc_hi);
            drop(precip_m_yr);
            drop(pet_m_yr);
            drop(total_h);

            upsert_layer(
                &mut layers,
                write_f32_layer(out_dir, "twi.bin", &twi, target_size, target_size)?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(out_dir, "moisture.bin", &moisture, target_size, target_size)?,
            );
            drop(twi);
            drop(moisture);

            // -----------------------------------------------------------------
            // S4 Deposition (reload bedrock_h + sediment_s -> peak ~744 MiB)
            // -----------------------------------------------------------------
            let mut bedrock_h = read_bin_f32(&out_dir.join("bedrock_h_m.bin"), lvl_n)?;
            let mut sediment_s = read_bin_f32(&out_dir.join("sediment_s_m.bin"), lvl_n)?;
            let mut water_kind = vec![WaterKind::None as u8; lvl_n];
            let s4_in = DepositionInput {
                width: lvl,
                height: lvl,
                discharge_q_m3_s: &hydro.discharge_q_m3_s,
                river_w_m: &hydro.river_w_m,
                river_d_m: &hydro.river_d_m,
                flow_dir_x: &hydro.flow_dir_x,
                flow_dir_z: &hydro.flow_dir_z,
                is_channel: &hydro.is_channel,
                d8_receiver: None,
            };
            let s4_cfg = DepositionConfig {
                seed: cfg.seed,
                schema_tag: cfg.schema_tag.clone(),
                dx_m: target_dx,
                meander_max_slope: 0.025,
                meander_min_q_m3_s: 2.0,
                ..DepositionConfig::default()
            };
            let s4 = apply_deposition(
                &mut bedrock_h,
                &mut sediment_s,
                &mut hydro.water_h_m,
                &mut water_kind,
                &s4_in,
                &s4_cfg,
            )?;
            widen_trunk_alluvial_floodplains(
                &mut bedrock_h,
                &mut sediment_s,
                &hydro.river_w_m,
                &strahler,
                &d8_dir,
                lvl,
                target_dx,
            );
            drop(strahler);
            drop(d8_dir);

            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "alluvial_mask.bin",
                    &s4.alluvial_mask,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "meander_scroll.bin",
                    &s4.meander_scroll,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "water_turbidity.bin",
                    &s4.water_turbidity,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "flow_dir_x.bin",
                    &hydro.flow_dir_x,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "flow_dir_z.bin",
                    &hydro.flow_dir_z,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "river_d_m.bin",
                    &hydro.river_d_m,
                    target_size,
                    target_size,
                )?,
            );
            drop(s4.alluvial_mask);
            drop(s4.meander_scroll);
            drop(s4.water_turbidity);
            drop(hydro.flow_dir_x);
            drop(hydro.flow_dir_z);
            drop(hydro.river_d_m);

            // Form total_h in-place inside bedrock_h (`64 MiB`), then clamp opposing needle spikes before S6.
            for (b, &s) in bedrock_h.iter_mut().zip(sediment_s.iter()) {
                *b += s;
            }
            let mut total_h = bedrock_h;
            clamp_opposing_needle_spikes(&mut total_h, lvl, target_dx);

            // -----------------------------------------------------------------
            // S6 Lakes & Waterfalls (reload precip + pet + lithology_id -> peak ~800 MiB)
            // -----------------------------------------------------------------
            let precip_m_yr = read_bin_f32(&out_dir.join("precip_m_yr.bin"), lvl_n)?;
            let pet_m_yr = read_bin_f32(&out_dir.join("pet_m_yr.bin"), lvl_n)?;
            let lithology_id = read_bin_u8(&out_dir.join("lithology_id.bin"), lvl_n)?;
            let s6_in = LakesFallsInput {
                width: lvl,
                height: lvl,
                precip_m_yr: &precip_m_yr,
                pet_m_yr: &pet_m_yr,
                discharge_q_m3_s: &hydro.discharge_q_m3_s,
                river_w_m: &hydro.river_w_m,
                is_channel: &hydro.is_channel,
                lithology_id: &lithology_id,
            };
            let s6_cfg = LakesFallsConfig {
                dx_m: target_dx,
                min_lake_cells: (libm::ceilf(100_000.0 / (target_dx * target_dx)) as usize).max(4),
                waterfall_max_run_m: if target_dx > 20.0 {
                    30.0_f32.max(3.65 * target_dx)
                } else {
                    30.0
                },
                ..LakesFallsConfig::default()
            };
            let s6 = generate_lakes_and_waterfalls(
                &mut total_h,
                &mut sediment_s,
                &mut hydro.water_h_m,
                &mut water_kind,
                &s6_in,
                &s6_cfg,
            )?;

            upsert_layer(
                &mut layers,
                write_u32_layer(
                    out_dir,
                    "lake_id.bin",
                    &s6.lake_id,
                    target_size,
                    target_size,
                )?,
            );
            drop(s6.lake_id);

            drop(hydro.discharge_q_m3_s);
            drop(hydro.river_w_m);
            drop(hydro.is_channel);
            let mut pf_final = total_h.clone();
            let mut d8_final = vec![NO_DIR; lvl_n];
            let mut ord_final = Vec::new();
            priority_flood_d8_lean_into(
                &mut pf_final,
                &mut d8_final,
                &mut ord_final,
                lvl,
                lvl,
                Some(0.0),
                false,
            );
            drop(ord_final);
            let hydro_final = route_hybrid_mfd_d8_lean_halves(
                &pf_final,
                &d8_final,
                lvl,
                lvl,
                &precip_m_yr,
                &pet_m_yr,
                &hcfg,
            );
            drop(pf_final);
            drop(d8_final);
            drop(hydro_final.flow_acc_lo);
            drop(hydro_final.flow_acc_hi);
            for i in 0..lvl_n {
                if water_kind[i] == WaterKind::None as u8 || water_kind[i] == WaterKind::River as u8
                {
                    if total_h[i] > crate::SEA_LEVEL_M
                        && hydro_final.is_channel[i]
                        && hydro_final.river_w_m[i] >= 0.5
                    {
                        water_kind[i] = WaterKind::River as u8;
                        hydro.water_h_m[i] =
                            hydro.water_h_m[i].max(total_h[i] + hydro_final.river_d_m[i]);
                    } else {
                        water_kind[i] = WaterKind::None as u8;
                        hydro.water_h_m[i] = total_h[i];
                    }
                }
            }

            upsert_layer(
                &mut layers,
                write_f32_layer(out_dir, "total_h_m.bin", &total_h, target_size, target_size)?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "sediment_s_m.bin",
                    &sediment_s,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_diff_f32_layer(
                    out_dir,
                    "bedrock_h_m.bin",
                    &total_h,
                    &sediment_s,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "water_h_m.bin",
                    &hydro.water_h_m,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_u8_layer(
                    out_dir,
                    "water_kind.bin",
                    &water_kind,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "flow_dir_x.bin",
                    &hydro_final.flow_dir_x,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "flow_dir_z.bin",
                    &hydro_final.flow_dir_z,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "river_d_m.bin",
                    &hydro_final.river_d_m,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "discharge_q_m3_s.bin",
                    &hydro_final.discharge_q_m3_s,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "river_w_m.bin",
                    &hydro_final.river_w_m,
                    target_size,
                    target_size,
                )?,
            );

            // Compute C4 river_surface_fraction before dropping hydro_final.is_channel.
            // Count only active S5 channel cells (`is_ch && rw >= 0.5`).
            let mut river_amt_sum = 0.0_f64;
            for ((&wk, &is_ch), &rw) in water_kind
                .iter()
                .zip(hydro_final.is_channel.iter())
                .zip(hydro_final.river_w_m.iter())
            {
                if wk == WaterKind::Ocean as u8
                    || wk == WaterKind::FreshwaterLake as u8
                    || wk == WaterKind::EndorheicLake as u8
                {
                    continue;
                }
                if is_ch && rw >= 0.5 {
                    river_amt_sum += f64::from((rw / target_dx).clamp(0.0, 1.0));
                }
            }
            let river_surface_fraction = (river_amt_sum / (lvl_n as f64)) as f32;

            drop(pet_m_yr);
            drop(hydro.water_h_m);
            drop(hydro_final.flow_dir_x);
            drop(hydro_final.flow_dir_z);
            drop(hydro_final.river_d_m);
            drop(hydro_final.discharge_q_m3_s);
            drop(hydro_final.is_channel);
            drop(hydro_final.water_h_m);

            // -----------------------------------------------------------------
            // S8 Biomes (peak ~992 MiB live Vecs @ 4096^2)
            // -----------------------------------------------------------------
            let temp_c = read_bin_f32(&out_dir.join("temp_c.bin"), lvl_n)?;
            let moisture = read_bin_f32(&out_dir.join("moisture.bin"), lvl_n)?;
            let twi = read_bin_f32(&out_dir.join("twi.bin"), lvl_n)?;
            let ela_m = read_bin_f32(&out_dir.join("ela_m.bin"), lvl_n)?;
            let alluvial_mask = read_bin_f32(&out_dir.join("alluvial_mask.bin"), lvl_n)?;
            let talus_frac = read_bin_f32(&out_dir.join("talus_frac.bin"), lvl_n)?;

            let b_in = BiomeInput {
                width: lvl,
                height: lvl,
                total_h_m: &total_h,
                temp_c: &temp_c,
                precip_m_yr: &precip_m_yr,
                moisture: &moisture,
                twi: &twi,
                ela_local_m: &ela_m,
                alluvial_mask: &alluvial_mask,
                talus_frac: &talus_frac,
                sediment_s_m: &sediment_s,
                water_kind: &water_kind,
                river_w_m: &hydro_final.river_w_m,
                lithology_id: &lithology_id,
            };
            let b_cfg = BiomeConfig {
                dx_m: target_dx,
                wind_dir: (libm::cosf(wind_azimuth_rad), libm::sinf(wind_azimuth_rad)),
                seed: cfg.seed,
                schema_tag: cfg.schema_tag.clone(),
                params_grid_size: Some(target_size),
                ..BiomeConfig::default()
            };
            let bf = classify_biomes(&b_in, &b_cfg)?;

            drop(precip_m_yr);
            drop(moisture);
            drop(twi);
            drop(ela_m);
            drop(alluvial_mask);
            drop(talus_frac);
            drop(sediment_s);
            drop(hydro_final.river_w_m);

            upsert_layer(
                &mut layers,
                write_u8x4_layer(
                    out_dir,
                    "biome_ids.bin",
                    &bf.biome_ids,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_u8x4_layer(
                    out_dir,
                    "biome_weights.bin",
                    &bf.biome_weights,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_u8_layer(
                    out_dir,
                    "dominant_biome.bin",
                    &bf.dominant_biome,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "glacier_h_m.bin",
                    &bf.glacier_h_m,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_u8_layer(
                    out_dir,
                    "moraine_debris.bin",
                    &bf.moraine_debris,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "dune_h_m.bin",
                    &bf.dune_h_m,
                    target_size,
                    target_size,
                )?,
            );

            // Compute C1/C2 biome stats before dropping biome_ids/biome_weights.
            let mut biome_counts = vec![0_usize; BIOME_COUNT];
            let mut land_cells = 0_usize;
            let mut ecotone_land = 0_usize;
            let mut ocean_cells = 0_usize;
            for i in 0..lvl_n {
                let dom = bf.dominant_biome[i];
                biome_counts[dom as usize] += 1;
                if total_h[i] <= 0.0 || water_kind[i] == WaterKind::Ocean as u8 {
                    ocean_cells += 1;
                }
                if total_h[i] > 0.0
                    && !BiomeId::from_u8(dom).is_some_and(BiomeId::is_water_or_strip)
                {
                    land_cells += 1;
                    if bf.biome_weights[i][0] < 230 {
                        ecotone_land += 1;
                    }
                }
            }
            let ecotone_fraction = ecotone_land as f32 / land_cells.max(1) as f32;
            let ocean_fraction = ocean_cells as f32 / lvl_n as f32;

            let mut adj_pairs = 0_usize;
            let mut jump_pairs = 0_usize;
            for r in 1..(lvl - 1) {
                for c in 1..(lvl - 1) {
                    let i = r * lvl + c;
                    if total_h[i] <= 0.0 || water_kind[i] != WaterKind::None as u8 {
                        continue;
                    }
                    let slope_i = libm::hypotf(
                        (total_h[i + 1] - total_h[i - 1]) / (2.0 * target_dx),
                        (total_h[(r + 1) * lvl + c] - total_h[(r - 1) * lvl + c])
                            / (2.0 * target_dx),
                    );
                    if slope_i > 0.70 {
                        continue;
                    }
                    for &nb in &[i + 1, (r + 1) * lvl + c] {
                        if total_h[nb] <= 0.0 || water_kind[nb] != WaterKind::None as u8 {
                            continue;
                        }
                        adj_pairs += 1;
                        let mut p_a = [0.0_f32; BIOME_COUNT];
                        let mut p_b = [0.0_f32; BIOME_COUNT];
                        for k in 0..4 {
                            p_a[bf.biome_ids[i][k] as usize] +=
                                f32::from(bf.biome_weights[i][k]) / 255.0;
                            p_b[bf.biome_ids[nb][k] as usize] +=
                                f32::from(bf.biome_weights[nb][k]) / 255.0;
                        }
                        let tv = 0.5
                            * p_a
                                .iter()
                                .zip(p_b.iter())
                                .map(|(&a, &b)| (a - b).abs())
                                .sum::<f32>();
                        if tv > 0.70 {
                            jump_pairs += 1;
                        }
                    }
                }
            }
            let non_cliff_jump_fraction = jump_pairs as f32 / adj_pairs.max(1) as f32;

            drop(bf.biome_ids);
            drop(bf.biome_weights);
            drop(bf.moraine_debris);
            drop(bf.dune_h_m);

            // -----------------------------------------------------------------
            // S9 Caves (`240 MiB` retained -> ~296 MiB peak)
            // -----------------------------------------------------------------
            let s9_in = CaveInput {
                width: lvl,
                height: lvl,
                total_h_m: &total_h,
                lithology_id: &lithology_id,
                water_kind: &water_kind,
                temp_c: &temp_c,
                glacier_h_m: &bf.glacier_h_m,
                dominant_biome: &bf.dominant_biome,
            };
            let s9_cfg = CaveConfig {
                seed: cfg.seed,
                schema_tag: cfg.schema_tag.clone(),
                dx_m: target_dx,
                ..CaveConfig::default()
            };
            let caves = generate_caves(&s9_in, &s9_cfg)?;
            drop(total_h);
            drop(lithology_id);
            drop(water_kind);
            drop(temp_c);
            drop(bf.glacier_h_m);
            drop(bf.dominant_biome);

            upsert_layer(
                &mut layers,
                write_u8_layer(
                    out_dir,
                    "cave_mask.bin",
                    &caves.cave_mask,
                    target_size,
                    target_size,
                )?,
            );
            drop(caves.cave_mask);

            let cave_bytes = caves.network.to_bytes();
            let mut cave_hasher = Sha256::new();
            cave_hasher.update(&cave_bytes);
            let cave_network_sha256 = hex_encode(&cave_hasher.finalize());
            let mut cave_file = BufWriter::new(File::create(out_dir.join("cave_network.bin"))?);
            cave_file.write_all(&cave_bytes)?;
            cave_file.flush()?;
            drop(cave_bytes);

            let mut world_hasher = Sha256::new();
            world_hasher.update(cfg.seed.to_le_bytes());
            world_hasher.update(target_size.to_le_bytes());
            world_hasher.update(cfg.schema_tag.as_bytes());
            for layer in &layers {
                world_hasher.update(layer.filename.as_bytes());
                world_hasher.update(layer.sha256.as_bytes());
            }
            world_hasher.update(cave_network_sha256.as_bytes());
            let world_sha256 = hex_encode(&world_hasher.finalize());

            let meta = RawWorldMeta {
                format_version: 1,
                seed: cfg.seed,
                schema_tag: cfg.schema_tag.clone(),
                width: target_size,
                height: target_size,
                dx_m: target_dx,
                extent_m: cfg.extent_m,
                elev_min_m: cfg.elev_min_m,
                elev_max_m: cfg.elev_max_m,
                sea_level_m: crate::SEA_LEVEL_M,
                wind_azimuth_rad,
                config: cfg.clone(),
                stats: PipelineStats {
                    ocean_fraction,
                    lake_surface_fraction: s6.lake_surface_fraction,
                    river_surface_fraction,
                    severed_humid_spillways: s6.severed_humid_spillways,
                    unmodeled_micro_pits: s6.unmodeled_micro_pits,
                    endorheic_land_fraction: s6.endorheic_land_fraction,
                    endorheic_humid_fraction: s6.endorheic_humid_fraction,
                    waterfall_count: s6.waterfalls.len(),
                    cave_entrance_count: caves.network.entrances.len(),
                    fan_count: s4.fan_count,
                    meander_reach_count: s4.meander_reach_count,
                    oxbow_count: s4.oxbow_count,
                    delta_count: s4.delta_count,
                    micro_pond_count: s4.micro_pond_count,
                    ecotone_fraction,
                    non_cliff_jump_fraction,
                    biome_counts,
                    uncapped_fluvial_substeps,
                },
                basins: s6.basins,
                waterfalls: s6.waterfalls,
                layers,
                cave_network_sha256,
                world_sha256,
            };

            let meta_json = serde_json::to_string_pretty(&meta)?;
            std::fs::write(out_dir.join("raw_meta.json"), meta_json)?;
            return Ok(meta);
        } else {
            drop(talus_frac);
            drop(uplift_u);
            drop(lithology_id);
        }
    }

    Err(PipelineError {
        message: "multigrid schedule was empty".to_owned(),
    })
}
