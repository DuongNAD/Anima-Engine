#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

//! Deterministic seeded decorative `flora_tiles` placement & `ANMF` v1 `256²` sector serializer
//! (`§3.2` #12; `ADR-0010` §4 decorative-only separation until underground/flora ADR).

use crate::bake::BakeError;
use crate::rng::stage_rng;
use crate::{BiomeId, WaterKind};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

/// Magic header for each `flora/s_<tx>_<ty>.anmf` sector blob (`ANMF`).
pub const FLORA_MAGIC: [u8; 4] = *b"ANMF";
/// Binary format version (`1`).
pub const FLORA_VERSION: u32 = 1;
/// Sector edge length in fine grid cells (`256 × 256` per sector).
pub const FLORA_SECTOR_SIZE: usize = 256;
/// Fixed header size in bytes (`16`).
pub const FLORA_HEADER_LEN: usize = 16;
/// Fixed packed record size per flora instance (`8` bytes LE).
pub const FLORA_INSTANCE_LEN: usize = 8;
/// Mandatory decorative-only flag bit (`ADR-0010 §4`).
pub const FLORA_FLAG_DECORATIVE: u8 = 0x01;

/// Canonical decorative vegetation species archetype encoded per instance.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloraSpeciesId {
    RainforestCanopy = 0,
    MontaneBroadleaf = 1,
    ElfinConifer = 2,
    DryThornScrub = 3,
    MangroveProp = 4,
    SavannaAcacia = 5,
    ParamoEspeletia = 6,
    RiparianWillow = 7,
}

/// Packed `8`-byte decorative tree/shrub placement (`§3.2` item 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloraInstance {
    /// Local cell offset inside sector normalized `0..=65535` across `[0, FLORA_SECTOR_SIZE)`.
    pub x_local_u16: u16,
    /// Local cell offset inside sector normalized `0..=65535` across `[0, FLORA_SECTOR_SIZE)`.
    pub z_local_u16: u16,
    pub species_id: u8,
    pub scale_u8: u8,
    pub yaw_u8: u8,
    pub flags_u8: u8,
}

/// One `256²` decorative flora sector blob (`s_<tx>_<ty>.anmf`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloraSectorBlob {
    pub sector_x: u16,
    pub sector_z: u16,
    pub decorative_only: bool,
    pub instances: Vec<FloraInstance>,
}

/// Summary metadata emitted for `flora_tiles` placement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloraPlacementSummary {
    pub decorative_only: bool,
    pub sector_size: usize,
    pub sector_count: usize,
    pub total_instances: usize,
    pub by_species: BTreeMap<String, usize>,
}

impl FloraSectorBlob {
    /// Serializes a sector blob to exact little-endian `ANMF` v1 bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, BakeError> {
        if !self.decorative_only {
            return Err(BakeError::new(
                "flora_tiles MUST remain marked decorative_only=true (ADR-0010 §4)",
            ));
        }
        let total = FLORA_HEADER_LEN + self.instances.len() * FLORA_INSTANCE_LEN;
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(&FLORA_MAGIC);
        out.extend_from_slice(&FLORA_VERSION.to_le_bytes());
        out.extend_from_slice(&self.sector_x.to_le_bytes());
        out.extend_from_slice(&self.sector_z.to_le_bytes());
        out.extend_from_slice(&(self.instances.len() as u32).to_le_bytes());
        for inst in &self.instances {
            if inst.flags_u8 & FLORA_FLAG_DECORATIVE == 0 {
                return Err(BakeError::new(
                    "FloraInstance missing FLORA_FLAG_DECORATIVE bit",
                ));
            }
            out.extend_from_slice(&inst.x_local_u16.to_le_bytes());
            out.extend_from_slice(&inst.z_local_u16.to_le_bytes());
            out.push(inst.species_id);
            out.push(inst.scale_u8);
            out.push(inst.yaw_u8);
            out.push(inst.flags_u8);
        }
        Ok(out)
    }

    /// Decodes and validates an `ANMF` v1 sector binary blob.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BakeError> {
        if bytes.len() < FLORA_HEADER_LEN {
            return Err(BakeError::new("ANMF buffer shorter than 16-byte header"));
        }
        if bytes[0..4] != FLORA_MAGIC {
            return Err(BakeError::new("invalid ANMF magic bytes"));
        }
        let ver = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if ver != FLORA_VERSION {
            return Err(BakeError::new(format!("unsupported ANMF version {ver}")));
        }
        let sector_x = u16::from_le_bytes([bytes[8], bytes[9]]);
        let sector_z = u16::from_le_bytes([bytes[10], bytes[11]]);
        let count = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        let expected_len = count
            .checked_mul(FLORA_INSTANCE_LEN)
            .and_then(|n| n.checked_add(FLORA_HEADER_LEN))
            .ok_or_else(|| BakeError::new("ANMF instance byte length overflow"))?;
        if bytes.len() != expected_len {
            return Err(BakeError::new(format!(
                "ANMF byte length mismatch: expected {expected_len}, got {}",
                bytes.len()
            )));
        }
        let mut instances = Vec::with_capacity(count);
        let mut off = FLORA_HEADER_LEN;
        for _ in 0..count {
            let x_local_u16 = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
            let z_local_u16 = u16::from_le_bytes([bytes[off + 2], bytes[off + 3]]);
            let species_id = bytes[off + 4];
            let scale_u8 = bytes[off + 5];
            let yaw_u8 = bytes[off + 6];
            let flags_u8 = bytes[off + 7];
            if flags_u8 & FLORA_FLAG_DECORATIVE == 0 {
                return Err(BakeError::new(
                    "decoded ANMF FloraInstance lacks mandatory decorative flag",
                ));
            }
            if species_id > FloraSpeciesId::RiparianWillow as u8 {
                return Err(BakeError::new(format!(
                    "invalid ANMF species_id {species_id}"
                )));
            }
            instances.push(FloraInstance {
                x_local_u16,
                z_local_u16,
                species_id,
                scale_u8,
                yaw_u8,
                flags_u8,
            });
            off += FLORA_INSTANCE_LEN;
        }
        Ok(Self {
            sector_x,
            sector_z,
            decorative_only: true,
            instances,
        })
    }
}

fn species_name(id: u8) -> &'static str {
    match id {
        0 => "rainforest_canopy",
        1 => "montane_broadleaf",
        2 => "elfin_conifer",
        3 => "dry_thorn_scrub",
        4 => "mangrove_prop",
        5 => "savanna_acacia",
        6 => "paramo_espeletia",
        7 => "riparian_willow",
        _ => "unknown",
    }
}

fn pick_flora_for_cell(
    biome_id: u8,
    moisture: f32,
    twi: f32,
    river_w_m: f32,
    slope: f32,
) -> Option<(u8, f32)> {
    if slope > 0.65 {
        return None;
    }
    let m = moisture.clamp(0.0, 1.0);
    let spec = match BiomeId::from_u8(biome_id) {
        Some(BiomeId::Jungle) => Some((FloraSpeciesId::RainforestCanopy as u8, 0.042)),
        Some(BiomeId::Forest) => Some((FloraSpeciesId::MontaneBroadleaf as u8, 0.036)),
        Some(BiomeId::Taiga) => Some((FloraSpeciesId::ElfinConifer as u8, 0.028)),
        Some(BiomeId::Mangrove) => Some((FloraSpeciesId::MangroveProp as u8, 0.034)),
        Some(BiomeId::Swamp) => Some((FloraSpeciesId::RiparianWillow as u8, 0.026)),
        Some(BiomeId::Savanna) => Some((FloraSpeciesId::SavannaAcacia as u8, 0.010)),
        Some(BiomeId::Shrubland | BiomeId::Desert | BiomeId::Badlands) => {
            if twi > 7.0 || m > 0.28 {
                Some((FloraSpeciesId::DryThornScrub as u8, 0.009))
            } else {
                None
            }
        }
        Some(BiomeId::Grassland) => {
            if river_w_m >= 0.5 || twi > 8.0 {
                Some((FloraSpeciesId::RiparianWillow as u8, 0.016))
            } else {
                Some((FloraSpeciesId::SavannaAcacia as u8, 0.005))
            }
        }
        Some(BiomeId::Alpine) => Some((FloraSpeciesId::ParamoEspeletia as u8, 0.012)),
        Some(BiomeId::Tundra | BiomeId::Bog) => {
            Some((FloraSpeciesId::ParamoEspeletia as u8, 0.006))
        }
        _ => None,
    }?;
    let slope_atten = (1.0 - slope / 0.65).clamp(0.15, 1.0);
    let moist_boost = 0.75 + 0.5 * m;
    Some((
        spec.0,
        (spec.1 * slope_atten * moist_boost).clamp(0.0, 0.12),
    ))
}

/// Deterministically places decorative foliage instances per `256²` sector (row-strip compatible).
#[allow(clippy::too_many_arguments)]
pub fn generate_decorative_flora_sectors(
    seed: u64,
    schema_tag: &str,
    width: usize,
    height: usize,
    dx_m: f32,
    total_h_m: &[f32],
    dominant_biome: &[u8],
    water_kind: &[u8],
    moisture: &[f32],
    twi: &[f32],
    river_w_m: &[f32],
    glacier_h_m: &[f32],
    sector_z_range: Option<(usize, usize)>,
) -> Result<(Vec<FloraSectorBlob>, FloraPlacementSummary), BakeError> {
    if !width.is_multiple_of(FLORA_SECTOR_SIZE) || !height.is_multiple_of(FLORA_SECTOR_SIZE) {
        return Err(BakeError::new(format!(
            "flora placement requires grid {width}x{height} divisible by {FLORA_SECTOR_SIZE}"
        )));
    }
    let sx_count = width / FLORA_SECTOR_SIZE;
    let sz_count = height / FLORA_SECTOR_SIZE;
    let (sz_start, sz_end) = sector_z_range.unwrap_or((0, sz_count));
    if sz_start > sz_end || sz_end > sz_count {
        return Err(BakeError::new("flora sector_z_range out of bounds"));
    }

    let mut blobs = Vec::with_capacity((sz_end - sz_start) * sx_count);
    let mut by_species: BTreeMap<String, usize> = BTreeMap::new();
    let mut total_instances = 0_usize;

    for sz in sz_start..sz_end {
        for sx in 0..sx_count {
            let tag = format!("wv2_5_flora_s_{sx}_{sz}");
            let mut rng = stage_rng(seed, width as u32, height as u32, schema_tag, &tag);
            let mut instances = Vec::new();

            let r0 = sz * FLORA_SECTOR_SIZE;
            let c0 = sx * FLORA_SECTOR_SIZE;
            // Subsample every 2 cells (`4` m..`20` m spacing) for organic canopy clustering.
            for lr in (0..FLORA_SECTOR_SIZE).step_by(2) {
                let gr = r0 + lr;
                let r_prev = gr.saturating_sub(1);
                let r_next = (gr + 1).min(height - 1);
                for lc in (0..FLORA_SECTOR_SIZE).step_by(2) {
                    let gc = c0 + lc;
                    let idx = gr * width + gc;
                    if total_h_m[idx] <= 0.0
                        || water_kind[idx] != WaterKind::None as u8
                        || glacier_h_m[idx] > 0.1
                    {
                        continue;
                    }
                    let c_prev = gc.saturating_sub(1);
                    let c_next = (gc + 1).min(width - 1);
                    let dh_dx = (total_h_m[gr * width + c_next] - total_h_m[gr * width + c_prev])
                        / ((c_next - c_prev).max(1) as f32 * dx_m);
                    let dh_dz = (total_h_m[r_next * width + gc] - total_h_m[r_prev * width + gc])
                        / ((r_next - r_prev).max(1) as f32 * dx_m);
                    let slope = libm::hypotf(dh_dx, dh_dz);

                    let Some((spec_id, prob)) = pick_flora_for_cell(
                        dominant_biome[idx],
                        moisture[idx],
                        twi[idx],
                        river_w_m[idx],
                        slope,
                    ) else {
                        continue;
                    };
                    // Compensate for 2x2 stride step (`4x` probability per sampled cell, capped).
                    let eff_prob = (prob * 4.0).clamp(0.0, 0.35);
                    if rng.gen::<f32>() >= eff_prob {
                        continue;
                    }
                    let jx: f32 = rng.gen_range(-0.45..0.45);
                    let jz: f32 = rng.gen_range(-0.45..0.45);
                    let x_norm =
                        ((lc as f32 + 0.5 + jx) / FLORA_SECTOR_SIZE as f32).clamp(0.0, 1.0);
                    let z_norm =
                        ((lr as f32 + 0.5 + jz) / FLORA_SECTOR_SIZE as f32).clamp(0.0, 1.0);
                    let x_local_u16 = (x_norm * 65_535.0).round() as u16;
                    let z_local_u16 = (z_norm * 65_535.0).round() as u16;
                    let scale_u8 = rng.gen_range(96_u8..=240_u8);
                    let yaw_u8 = rng.gen::<u8>();

                    instances.push(FloraInstance {
                        x_local_u16,
                        z_local_u16,
                        species_id: spec_id,
                        scale_u8,
                        yaw_u8,
                        flags_u8: FLORA_FLAG_DECORATIVE,
                    });
                    total_instances += 1;
                    *by_species
                        .entry(species_name(spec_id).to_owned())
                        .or_insert(0) += 1;
                }
            }

            blobs.push(FloraSectorBlob {
                sector_x: sx as u16,
                sector_z: sz as u16,
                decorative_only: true,
                instances,
            });
        }
    }

    let sector_count = blobs.len();
    Ok((
        blobs,
        FloraPlacementSummary {
            decorative_only: true,
            sector_size: FLORA_SECTOR_SIZE,
            sector_count,
            total_instances,
            by_species,
        },
    ))
}

/// Writes `flora/s_<tx>_<ty>.anmf` files and `flora_summary.json` into `flora_dir`.
pub fn write_flora_sectors(
    flora_dir: &Path,
    blobs: &[FloraSectorBlob],
    summary: &FloraPlacementSummary,
) -> Result<(), BakeError> {
    fs::create_dir_all(flora_dir)?;
    for blob in blobs {
        let bytes = blob.to_bytes()?;
        let path = flora_dir.join(format!("s_{}_{}.anmf", blob.sector_x, blob.sector_z));
        let mut writer = BufWriter::new(File::create(path)?);
        writer.write_all(&bytes)?;
        writer.flush()?;
    }
    let json = serde_json::to_string_pretty(summary)?;
    fs::write(flora_dir.join("flora_summary.json"), format!("{json}\n"))?;
    Ok(())
}
