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
use crate::erosion_thermal::{relax_thermal_and_hillslope, ThermalHillslopeParams};
use crate::hydrology::{
    priority_flood_d8_lean_into, route_hybrid_mfd_d8_lean, route_hybrid_mfd_d8_lean_halves,
    HydrologyConfig, NO_DIR,
};
use crate::lakes_falls::{
    generate_lakes_and_waterfalls, LakeBasinRecord, LakesFallsConfig, LakesFallsError,
    LakesFallsInput, WaterfallRecord,
};
use crate::tectonic::{generate_fine_s1_aux, generate_uplift_for_target};
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
        return vec![(target_size, 60)];
    }
    let mut levels = Vec::new();
    for &(sz, steps) in &[(512_u32, 60_usize), (1024, 40), (2048, 25), (4096, 15)] {
        if sz < target_size {
            levels.push((sz, steps));
        }
    }
    let target_steps = match target_size {
        1024 => 40,
        2048 => 25,
        4096 => 15,
        8192 => 8,
        _ => 25,
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

    for (level_idx, &(lvl_u32, n_steps)) in schedule.iter().enumerate() {
        let lvl = lvl_u32 as usize;
        let lvl_n = lvl * lvl;
        let lvl_dx = cfg.extent_m / (lvl_u32 as f32);
        let lvl_cfg = WorldV2Config {
            grid_size: lvl_u32,
            ..cfg.clone()
        };

        let (mut uplift_u, lithology_id) = if level_idx == 0 {
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

            generate_fine_s1_aux(&lvl_cfg, &cur_bedrock, target_size)?
        };
        cur_size = lvl;
        for u in &mut uplift_u {
            *u *= UPLIFT_M_YR_SCALE;
        }

        // 2-pass S7 climate-erosion coupling:
        // - If target <= 512: single level runs Pass 1 (30 steps) -> S5+S7 Pass 2 -> (30 steps).
        // - If target > 512: 512^2 uses Pass 1; 1024^2 runs S5 + S7 Pass 2 and caches precip;
        //   2048^2 and 4096^2 resample cached 1024^2 precip cleanly for S2.
        let fluvial_params = FluvialParams {
            dx_m: f64::from(lvl_dx),
            dt_yr: if lvl_dx <= 12.0 {
                4.0
            } else if lvl_dx <= 25.0 {
                35.0
            } else {
                FLUVIAL_DT_YR
            },
            g_deposition: 0.85,
            max_gs_iters: 64,
            gs_tol_m: 5.0e-4,
            ..FluvialParams::default()
        };
        let thermal_params = ThermalHillslopeParams {
            dx_m: lvl_dx,
            dt_yr: 120.0 * (lvl_dx / 40.0).min(1.0),
            diffusivity_d: if lvl_dx < 30.0 { 0.002 } else { 0.025 },
            talus_rate: if lvl_dx < 30.0 {
                0.45 * (lvl_dx / 40.0) * 0.12
            } else {
                0.45
            },
            substeps: if lvl_dx < 30.0 { 4 } else { 12 },
            ..ThermalHillslopeParams::default()
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
            for _ in 0..half1 {
                let mut grid = FluvialGridLean {
                    bedrock_h: &mut cur_bedrock,
                    sediment_s: &mut cur_sediment,
                    uplift_m_yr: &uplift_u,
                    k_erodibility: FluvialErodibility::LithologyLut {
                        lithology_id: &lithology_id,
                        k_scale: FLUVIAL_K_SCALE,
                    },
                    precip_m_yr: &precip_for_s2,
                    pet_m_yr: None,
                    width: lvl,
                    height: lvl,
                    sea_level_m: Some(0.0),
                };
                erode_fluvial_step_f32_lean(&mut grid, &fluvial_params, Some(&mut scratch))?;
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
            let hcfg = HydrologyConfig {
                dx_m: lvl_dx,
                ..HydrologyConfig::default()
            };
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
                let mut grid = FluvialGridLean {
                    bedrock_h: &mut cur_bedrock,
                    sediment_s: &mut cur_sediment,
                    uplift_m_yr: &uplift_u,
                    k_erodibility: FluvialErodibility::LithologyLut {
                        lithology_id: &lithology_id,
                        k_scale: FLUVIAL_K_SCALE,
                    },
                    precip_m_yr: &precip_for_s2,
                    pet_m_yr: None,
                    width: lvl,
                    height: lvl,
                    sea_level_m: Some(0.0),
                };
                erode_fluvial_step_f32_lean(&mut grid, &fluvial_params, Some(&mut scratch2))?;
            }
            drop(scratch2);
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
                let hcfg = HydrologyConfig {
                    dx_m: lvl_dx,
                    ..HydrologyConfig::default()
                };
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
            for _ in 0..n_steps {
                let mut grid = FluvialGridLean {
                    bedrock_h: &mut cur_bedrock,
                    sediment_s: &mut cur_sediment,
                    uplift_m_yr: &uplift_u,
                    k_erodibility: FluvialErodibility::LithologyLut {
                        lithology_id: &lithology_id,
                        k_scale: FLUVIAL_K_SCALE,
                    },
                    precip_m_yr: &precip_for_s2,
                    pet_m_yr: None,
                    width: lvl,
                    height: lvl,
                    sea_level_m: Some(0.0),
                };
                erode_fluvial_step_f32_lean(&mut grid, &fluvial_params, Some(&mut scratch))?;
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
            let hcfg = HydrologyConfig {
                dx_m: target_dx,
                channel_init_threshold: if target_dx > 45.0 {
                    250.0_f64.max(2.5 * f64::from(target_dx) * f64::from(target_dx))
                } else if target_dx <= 10.5 {
                    30.0
                } else {
                    30.0 + 3.5 * (f64::from(target_dx) - 10.0)
                },
                ..HydrologyConfig::default()
            };
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
            drop(d8_dir);

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

            // Form total_h in-place inside bedrock_h (`64 MiB`).
            for (b, &s) in bedrock_h.iter_mut().zip(sediment_s.iter()) {
                *b += s;
            }
            let mut total_h = bedrock_h;

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
                    "discharge_q_m3_s.bin",
                    &hydro.discharge_q_m3_s,
                    target_size,
                    target_size,
                )?,
            );
            upsert_layer(
                &mut layers,
                write_f32_layer(
                    out_dir,
                    "river_w_m.bin",
                    &hydro.river_w_m,
                    target_size,
                    target_size,
                )?,
            );

            // Compute C4 river_surface_fraction before dropping hydro.is_channel.
            // Count only active S5 channel cells (`is_ch && rw >= 0.5`).
            let mut river_amt_sum = 0.0_f64;
            for ((&wk, &is_ch), &rw) in water_kind
                .iter()
                .zip(hydro.is_channel.iter())
                .zip(hydro.river_w_m.iter())
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
            drop(hydro.discharge_q_m3_s);
            drop(hydro.is_channel);
            drop(hydro.water_h_m);

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
                river_w_m: &hydro.river_w_m,
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
            drop(hydro.river_w_m);

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
