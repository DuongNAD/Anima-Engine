//! `ANMW v3` simulation bundle codec, `LD-2` dual identities, and Strahler-order coarse block
//! projection (`§3.3`–`§3.4`, Card `WV2-3`).
//!
//! Available under `reader` (pure Rust, `dep:sha2` only — zero `libm`, zero `serde`, zero C
//! dependencies). Directory ingestion from offline `anima-worldgen generate` outputs is gated
//! behind `generate`.

#[cfg(feature = "generate")]
use crate::caves::CaveNetwork;
#[cfg(feature = "generate")]
use crate::hydrology::{priority_flood_d8_lean_into, NO_DIR as HYDRO_NO_DIR};
#[cfg(feature = "generate")]
use crate::pipeline::{read_bin_f32, read_bin_u8, PipelineError, RawWorldMeta};
use crate::{
    denormalize_elevation, normalize_elevation, BiomeId, WaterKind, BIOME_COUNT, SEA_LEVEL_M,
    SEA_LEVEL_NORM, WATER_KIND_COUNT,
};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::fmt;
#[cfg(feature = "generate")]
use std::path::Path;

/// Magic header bytes for `ANMW` world artifacts (`v2` & `v3`).
pub const ANMW_MAGIC: [u8; 4] = *b"ANMW";
/// Container format version for `sim_256.anmw3`.
pub const ANMW_V3_VERSION: u32 = 3;
/// Fixed little-endian header size in bytes (`44`).
pub const ANMW_V3_HEADER_LEN: usize = 44;
/// Byte offset where `sim_seed_u32` sits (matching `WorldArtifact::peek_seed` v2 offset `20..24`).
pub const ANMW_V3_PEEK_SEED_OFFSET: usize = 20;
/// Canonical simulation grid width/height for `sim_256.anmw3`.
pub const ANMW_V3_SIM_SIZE: u32 = 256;
/// Canonical world-space extent (`MapBounds` axis width `200.0`).
pub const ANMW_V3_WORLD_SCALE: f32 = 200.0;
/// Standard uncompressed `sim_256.anmw3` flags value (`0`).
pub const ANMW_V3_FLAGS_STANDARD: u32 = 0;
/// Serialized byte size of each [`AnmwV3Waterfall`] record (`32`).
pub const ANMW_V3_WATERFALL_LEN: usize = 32;
/// Serialized byte size of each [`AnmwV3CaveEntrance`] record (`24`).
pub const ANMW_V3_CAVE_ENTRANCE_LEN: usize = 24;
/// Safety upper bound on serialized record counts (`65_536`).
pub const ANMW_V3_MAX_RECORD_COUNT: usize = 65_536;
/// Sentinel indicating no D8 downstream link (border or marine outlet).
pub const D8_NO_DIR: u8 = 8;

const DX8: [isize; 8] = [1, 1, 0, -1, -1, -1, 0, 1];
const DZ8: [isize; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
const DEEP_OCEAN_THRESHOLD_M: f32 = -220.0;
const TEMP_NORM_MIN_C: f32 = -20.0;
const TEMP_NORM_SPAN_C: f32 = 60.0;

/// Error returned when validating, encoding, decoding, or verifying an `ANMW v3` simulation bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnmwV3Error {
    pub message: String,
}

impl AnmwV3Error {
    #[inline]
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for AnmwV3Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AnmwV3Error {}

/// Mapped waterfall summary carried in `sim_256.anmw3` (`32` bytes LE on disk).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnmwV3Waterfall {
    pub x_m: f32,
    pub z_m: f32,
    pub lip_elev_m: f32,
    pub drop_m: f32,
    pub yaw_rad: f32,
    pub discharge_q_m3_s: f32,
    pub plunge_depth_m: f32,
    pub flags: u32,
}

/// Mapped cave portal summary carried in `sim_256.anmw3` (`24` bytes LE on disk).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnmwV3CaveEntrance {
    pub x_m: f32,
    pub z_m: f32,
    pub elevation_m: f32,
    pub yaw_rad: f32,
    pub clearance_m: f32,
    pub mouth_kind: u8,
    pub realm_status: u8,
}

/// Decoded `ANMW v3` simulation bundle (`sim_256.anmw3`, `§3.3.1`).
///
/// Carries **no** embedded SHA-256 identity inside itself so `sim_world_identity = sha256(bytes)`
/// has no hash circularity (`LD-2`).
#[derive(Debug, Clone, PartialEq)]
pub struct AnmwV3Sim {
    pub width: u32,
    pub height: u32,
    pub sea_level_norm: f32,
    pub sim_seed_u32: u32,
    pub seed_u64: u64,
    pub generator_version: u32,
    pub world_scale: f32,
    pub flags: u32,
    pub elevation: Vec<f32>,
    pub moisture: Vec<f32>,
    pub temperature: Vec<f32>,
    pub flow: Vec<f32>,
    pub slope: Vec<f32>,
    pub biome_22: Vec<u8>,
    pub water_kind: Vec<u8>,
    pub waterfalls: Vec<AnmwV3Waterfall>,
    pub cave_entrances: Vec<AnmwV3CaveEntrance>,
}

/// Derives the canonical `u32` simulation seed from the full 64-bit worldgen seed (`§3.3.1`).
#[must_use]
pub const fn derive_sim_seed_u32(seed_u64: u64) -> u32 {
    (seed_u64 ^ (seed_u64 >> 32)) as u32
}

/// Returns `true` if `bytes` opens with an `ANMW` magic and version `3` header (`>= 8` bytes).
#[must_use]
pub fn is_anmw_v3_header(bytes: &[u8]) -> bool {
    bytes.len() >= 8
        && bytes[0..4] == ANMW_MAGIC
        && u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) == ANMW_V3_VERSION
}

/// Maps physical temperature in Celsius (`[-20 °C, +40 °C]`) into normalized `[0.0, 1.0]`.
#[must_use]
pub fn normalize_temperature_c(temp_c: f32) -> f32 {
    ((temp_c - TEMP_NORM_MIN_C) / TEMP_NORM_SPAN_C).clamp(0.0, 1.0)
}

/// Computes `sim_world_identity = sha256(sim_256.anmw3 raw bytes)` (`LD-2`).
#[must_use]
pub fn compute_sim_world_identity(sim_bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(sim_bytes);
    hasher.finalize().into()
}

/// Computes `bundle_identity = sha256(committed index.json raw bytes)` (`LD-2`).
///
/// Must hash raw committed bytes on disk without JSON re-canonicalisation (`§3.3.2`).
#[must_use]
pub fn compute_bundle_identity(index_json_bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(index_json_bytes);
    hasher.finalize().into()
}

/// Verifies raw `sim_256.anmw3` bytes against an expected `sim_world_identity` SHA-256 digest.
pub fn verify_sim_world_identity(sim_bytes: &[u8], expected: &[u8; 32]) -> Result<(), AnmwV3Error> {
    let actual = compute_sim_world_identity(sim_bytes);
    if &actual != expected {
        return Err(AnmwV3Error::new(format!(
            "sim_world_identity mismatch (expected {}, got {})",
            sha256_to_hex(expected),
            sha256_to_hex(&actual)
        )));
    }
    Ok(())
}

/// Verifies raw committed `index.json` bytes against an expected `bundle_identity` SHA-256 digest.
pub fn verify_bundle_identity(
    index_json_bytes: &[u8],
    expected: &[u8; 32],
) -> Result<(), AnmwV3Error> {
    let actual = compute_bundle_identity(index_json_bytes);
    if &actual != expected {
        return Err(AnmwV3Error::new(format!(
            "bundle_identity mismatch (expected {}, got {})",
            sha256_to_hex(expected),
            sha256_to_hex(&actual)
        )));
    }
    Ok(())
}

/// Encodes a 32-byte SHA-256 digest as lowercase 64-character hexadecimal.
#[must_use]
pub fn sha256_to_hex(digest: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for &b in digest {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0F) as usize] as char);
    }
    out
}

/// Decodes a 64-character hex SHA-256 digest into `[u8; 32]`.
pub fn sha256_from_hex(hex: &str) -> Result<[u8; 32], AnmwV3Error> {
    let bytes = hex.as_bytes();
    if bytes.len() != 64 {
        return Err(AnmwV3Error::new(format!(
            "expected 64-char sha256 hex string, got length {}",
            bytes.len()
        )));
    }
    let nibble = |c: u8| -> Result<u8, AnmwV3Error> {
        match c {
            b'0'..=b'9' => Ok(c - b'0'),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            b'A'..=b'F' => Ok(c - b'A' + 10),
            _ => Err(AnmwV3Error::new(format!(
                "invalid hex byte 0x{c:02x} in sha256"
            ))),
        }
    };
    let mut out = [0_u8; 32];
    for (idx, dst) in out.iter_mut().enumerate() {
        let hi = nibble(bytes[idx * 2])?;
        let lo = nibble(bytes[idx * 2 + 1])?;
        *dst = (hi << 4) | lo;
    }
    Ok(out)
}

/// Projects a canonical 22-biome index (`0..=21`) onto the backend's 11-variant `BiomeType` (`0..=10`).
///
/// Mirrors `src/core/world_artifact.rs::map_biome_frontend_to_backend`.
#[must_use]
pub const fn map_biome_22_to_11(front: u8) -> u8 {
    match front {
        0 => 1,   // Ocean       -> Ocean
        1 => 2,   // Beach       -> Beach
        2 => 8,   // Desert      -> Desert
        3 => 4,   // Savanna     -> Grassland
        4 => 4,   // Grassland   -> Grassland
        5 => 4,   // Shrubland   -> Grassland
        6 => 5,   // Forest      -> TemperateForest
        7 => 7,   // Jungle      -> Rainforest
        8 => 6,   // Taiga       -> BorealForest
        9 => 6,   // Tundra      -> BorealForest
        10 => 7,  // Swamp       -> Rainforest
        11 => 9,  // Rock        -> MountainRock
        12 => 10, // Snow        -> Snow
        13 => 3,  // River       -> River
        14 => 1,  // Lake        -> Ocean
        15 => 7,  // Mangrove    -> Rainforest
        16 => 4,  // Chaparral   -> Grassland
        17 => 4,  // Steppe      -> Grassland
        18 => 4,  // Alpine      -> Grassland
        19 => 8,  // Badlands    -> Desert
        20 => 10, // Glacier     -> Snow
        21 => 6,  // Bog         -> BorealForest
        _ => 4,   // unknown     -> Grassland
    }
}

/// Maps `(biome_22, water_kind, normalized_elevation)` to the backend's 11-variant `BiomeType`
/// (`§3.4` item 1: deep vs shallow ocean at `-220 m`, inland lakes to `Ocean (1)`).
#[must_use]
pub fn map_water_and_biome_to_11(biome_22: u8, water_kind: u8, elev_norm: f32) -> u8 {
    let elev_m = denormalize_elevation(elev_norm);
    if water_kind == WaterKind::Ocean as u8 || biome_22 == BiomeId::Ocean as u8 {
        return if elev_m < DEEP_OCEAN_THRESHOLD_M {
            0
        } else {
            1
        };
    }
    if water_kind == WaterKind::FreshwaterLake as u8
        || water_kind == WaterKind::EndorheicLake as u8
        || biome_22 == BiomeId::Lake as u8
    {
        return 1;
    }
    if water_kind == WaterKind::River as u8
        || water_kind == WaterKind::Waterfall as u8
        || biome_22 == BiomeId::River as u8
    {
        return 3;
    }
    if water_kind == WaterKind::DeltaTurbid as u8 && elev_m <= SEA_LEVEL_M + 0.1 {
        return if elev_m < DEEP_OCEAN_THRESHOLD_M {
            0
        } else {
            1
        };
    }
    map_biome_22_to_11(biome_22)
}

#[derive(Debug, Clone, Copy)]
struct AnmwV3Header {
    width: u32,
    height: u32,
    sea_level_norm: f32,
    sim_seed_u32: u32,
    seed_u64: u64,
    generator_version: u32,
    world_scale: f32,
    flags: u32,
}

#[inline]
fn read_u32_le(bytes: &[u8], off: usize) -> Result<u32, AnmwV3Error> {
    let slice = bytes
        .get(off..off + 4)
        .ok_or_else(|| AnmwV3Error::new("ANMW v3 truncated (u32)"))?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

#[inline]
fn read_u64_le(bytes: &[u8], off: usize) -> Result<u64, AnmwV3Error> {
    let slice = bytes
        .get(off..off + 8)
        .ok_or_else(|| AnmwV3Error::new("ANMW v3 truncated (u64)"))?;
    Ok(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

#[inline]
fn read_f32_le(bytes: &[u8], off: usize) -> Result<f32, AnmwV3Error> {
    let slice = bytes
        .get(off..off + 4)
        .ok_or_else(|| AnmwV3Error::new("ANMW v3 truncated (f32)"))?;
    Ok(f32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_f32_array(bytes: &[u8], off: usize, count: usize) -> Result<Vec<f32>, AnmwV3Error> {
    let byte_len = count
        .checked_mul(4)
        .ok_or_else(|| AnmwV3Error::new("ANMW v3 f32 array byte-size overflow"))?;
    let end = off
        .checked_add(byte_len)
        .ok_or_else(|| AnmwV3Error::new("ANMW v3 offset overflow"))?;
    let slice = bytes
        .get(off..end)
        .ok_or_else(|| AnmwV3Error::new("ANMW v3 truncated (f32 array)"))?;
    let mut out = Vec::with_capacity(count);
    let (chunks, _) = slice.as_chunks::<4>();
    for &chunk in chunks {
        out.push(f32::from_le_bytes(chunk));
    }
    Ok(out)
}

fn parse_header(bytes: &[u8]) -> Result<AnmwV3Header, AnmwV3Error> {
    if bytes.len() < ANMW_V3_HEADER_LEN {
        return Err(AnmwV3Error::new(format!(
            "ANMW v3 buffer shorter than {ANMW_V3_HEADER_LEN}-byte header (got {})",
            bytes.len()
        )));
    }
    if bytes[0..4] != ANMW_MAGIC {
        return Err(AnmwV3Error::new("bad ANMW v3 magic (expected ANMW)"));
    }
    let version = read_u32_le(bytes, 4)?;
    if version != ANMW_V3_VERSION {
        return Err(AnmwV3Error::new(format!(
            "unsupported ANMW version {version} (expected {ANMW_V3_VERSION})"
        )));
    }
    let width = read_u32_le(bytes, 8)?;
    let height = read_u32_le(bytes, 12)?;
    let sea_level_norm = read_f32_le(bytes, 16)?;
    let sim_seed_u32 = read_u32_le(bytes, ANMW_V3_PEEK_SEED_OFFSET)?;
    let seed_u64 = read_u64_le(bytes, 24)?;
    let generator_version = read_u32_le(bytes, 32)?;
    let world_scale = read_f32_le(bytes, 36)?;
    let flags = read_u32_le(bytes, 40)?;

    if width == 0 || height == 0 {
        return Err(AnmwV3Error::new("ANMW v3 has zero grid dimensions"));
    }
    if !sea_level_norm.is_finite() || !world_scale.is_finite() || world_scale <= 0.0 {
        return Err(AnmwV3Error::new(
            "ANMW v3 header has invalid non-finite or non-positive float metadata",
        ));
    }
    if flags != ANMW_V3_FLAGS_STANDARD {
        return Err(AnmwV3Error::new(format!(
            "unsupported ANMW v3 flags {flags} (expected {ANMW_V3_FLAGS_STANDARD})"
        )));
    }
    let expected_sim_seed = derive_sim_seed_u32(seed_u64);
    if sim_seed_u32 != expected_sim_seed {
        return Err(AnmwV3Error::new(format!(
            "ANMW v3 sim_seed_u32 ({sim_seed_u32}) does not match seed_u64 fold ({expected_sim_seed})"
        )));
    }

    Ok(AnmwV3Header {
        width,
        height,
        sea_level_norm,
        sim_seed_u32,
        seed_u64,
        generator_version,
        world_scale,
        flags,
    })
}

fn validate_float_slice(slice: &[f32], name: &str, non_negative: bool) -> Result<(), AnmwV3Error> {
    for (idx, &val) in slice.iter().enumerate() {
        if !val.is_finite() || (non_negative && val < 0.0) {
            return Err(AnmwV3Error::new(format!(
                "ANMW v3 invalid {name}[{idx}] = {val}"
            )));
        }
    }
    Ok(())
}

impl AnmwV3Sim {
    /// Reads only the header and resolves `sim_seed_u32` at byte offset `20..24` in $O(1)$.
    pub fn peek_seed(bytes: &[u8]) -> Result<u32, AnmwV3Error> {
        Ok(parse_header(bytes)?.sim_seed_u32)
    }

    /// Validates all header invariants, field lengths, finite values, and enum tags.
    pub fn validate(&self) -> Result<(), AnmwV3Error> {
        if self.width == 0 || self.height == 0 {
            return Err(AnmwV3Error::new("ANMW v3 has zero grid dimensions"));
        }
        let n = (self.width as usize)
            .checked_mul(self.height as usize)
            .ok_or_else(|| AnmwV3Error::new("ANMW v3 dimension multiplication overflow"))?;
        if !self.sea_level_norm.is_finite()
            || !self.world_scale.is_finite()
            || self.world_scale <= 0.0
        {
            return Err(AnmwV3Error::new(
                "ANMW v3 header has invalid float metadata",
            ));
        }
        if self.flags != ANMW_V3_FLAGS_STANDARD {
            return Err(AnmwV3Error::new(format!(
                "unsupported ANMW v3 flags {} (expected {ANMW_V3_FLAGS_STANDARD})",
                self.flags
            )));
        }
        let expected_seed = derive_sim_seed_u32(self.seed_u64);
        if self.sim_seed_u32 != expected_seed {
            return Err(AnmwV3Error::new(format!(
                "ANMW v3 sim_seed_u32 ({}) != derived ({expected_seed})",
                self.sim_seed_u32
            )));
        }
        for (name, len) in [
            ("elevation", self.elevation.len()),
            ("moisture", self.moisture.len()),
            ("temperature", self.temperature.len()),
            ("flow", self.flow.len()),
            ("slope", self.slope.len()),
            ("biome_22", self.biome_22.len()),
            ("water_kind", self.water_kind.len()),
        ] {
            if len != n {
                return Err(AnmwV3Error::new(format!(
                    "ANMW v3 {name} length mismatch: expected {n}, got {len}"
                )));
            }
        }
        validate_float_slice(&self.elevation, "elevation", false)?;
        validate_float_slice(&self.moisture, "moisture", false)?;
        validate_float_slice(&self.temperature, "temperature", false)?;
        validate_float_slice(&self.flow, "flow", true)?;
        validate_float_slice(&self.slope, "slope", true)?;

        for (idx, &b) in self.biome_22.iter().enumerate() {
            if (b as usize) >= BIOME_COUNT {
                return Err(AnmwV3Error::new(format!(
                    "ANMW v3 invalid biome_22[{idx}] = {b} (>= {BIOME_COUNT})"
                )));
            }
        }
        for (idx, &wk) in self.water_kind.iter().enumerate() {
            if (wk as usize) >= WATER_KIND_COUNT {
                return Err(AnmwV3Error::new(format!(
                    "ANMW v3 invalid water_kind[{idx}] = {wk} (>= {WATER_KIND_COUNT})"
                )));
            }
        }
        if self.waterfalls.len() > ANMW_V3_MAX_RECORD_COUNT {
            return Err(AnmwV3Error::new(format!(
                "ANMW v3 waterfalls count {} exceeds limit {ANMW_V3_MAX_RECORD_COUNT}",
                self.waterfalls.len()
            )));
        }
        for (idx, wf) in self.waterfalls.iter().enumerate() {
            if !wf.x_m.is_finite()
                || !wf.z_m.is_finite()
                || !wf.lip_elev_m.is_finite()
                || !wf.drop_m.is_finite()
                || !wf.yaw_rad.is_finite()
                || !wf.discharge_q_m3_s.is_finite()
                || !wf.plunge_depth_m.is_finite()
                || wf.drop_m < 0.0
                || wf.discharge_q_m3_s < 0.0
                || wf.plunge_depth_m < 0.0
            {
                return Err(AnmwV3Error::new(format!(
                    "ANMW v3 invalid waterfall[{idx}]"
                )));
            }
        }
        if self.cave_entrances.len() > ANMW_V3_MAX_RECORD_COUNT {
            return Err(AnmwV3Error::new(format!(
                "ANMW v3 cave_entrances count {} exceeds limit {ANMW_V3_MAX_RECORD_COUNT}",
                self.cave_entrances.len()
            )));
        }
        for (idx, ent) in self.cave_entrances.iter().enumerate() {
            if !ent.x_m.is_finite()
                || !ent.z_m.is_finite()
                || !ent.elevation_m.is_finite()
                || !ent.yaw_rad.is_finite()
                || !ent.clearance_m.is_finite()
                || ent.clearance_m <= 0.0
                || ent.mouth_kind >= 3
            {
                return Err(AnmwV3Error::new(format!(
                    "ANMW v3 invalid cave_entrance[{idx}]"
                )));
            }
        }
        Ok(())
    }

    /// Serializes the validated container to canonical uncompressed little-endian `ANMW v3` bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, AnmwV3Error> {
        self.validate()?;
        let n = (self.width as usize) * (self.height as usize);
        let total_len = ANMW_V3_HEADER_LEN
            + n * (5 * 4 + 2)
            + 4
            + self.waterfalls.len() * ANMW_V3_WATERFALL_LEN
            + 4
            + self.cave_entrances.len() * ANMW_V3_CAVE_ENTRANCE_LEN;
        let mut out = Vec::with_capacity(total_len);

        out.extend_from_slice(&ANMW_MAGIC);
        out.extend_from_slice(&ANMW_V3_VERSION.to_le_bytes());
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.extend_from_slice(&self.sea_level_norm.to_le_bytes());
        out.extend_from_slice(&self.sim_seed_u32.to_le_bytes());
        out.extend_from_slice(&self.seed_u64.to_le_bytes());
        out.extend_from_slice(&self.generator_version.to_le_bytes());
        out.extend_from_slice(&self.world_scale.to_le_bytes());
        out.extend_from_slice(&self.flags.to_le_bytes());

        for field in [
            &self.elevation,
            &self.moisture,
            &self.temperature,
            &self.flow,
            &self.slope,
        ] {
            for &val in field {
                out.extend_from_slice(&val.to_le_bytes());
            }
        }
        out.extend_from_slice(&self.biome_22);
        out.extend_from_slice(&self.water_kind);

        out.extend_from_slice(&(self.waterfalls.len() as u32).to_le_bytes());
        for wf in &self.waterfalls {
            out.extend_from_slice(&wf.x_m.to_le_bytes());
            out.extend_from_slice(&wf.z_m.to_le_bytes());
            out.extend_from_slice(&wf.lip_elev_m.to_le_bytes());
            out.extend_from_slice(&wf.drop_m.to_le_bytes());
            out.extend_from_slice(&wf.yaw_rad.to_le_bytes());
            out.extend_from_slice(&wf.discharge_q_m3_s.to_le_bytes());
            out.extend_from_slice(&wf.plunge_depth_m.to_le_bytes());
            out.extend_from_slice(&wf.flags.to_le_bytes());
        }

        out.extend_from_slice(&(self.cave_entrances.len() as u32).to_le_bytes());
        for ent in &self.cave_entrances {
            out.extend_from_slice(&ent.x_m.to_le_bytes());
            out.extend_from_slice(&ent.z_m.to_le_bytes());
            out.extend_from_slice(&ent.elevation_m.to_le_bytes());
            out.extend_from_slice(&ent.yaw_rad.to_le_bytes());
            out.extend_from_slice(&ent.clearance_m.to_le_bytes());
            out.push(ent.mouth_kind);
            out.push(ent.realm_status);
            out.extend_from_slice(&[0_u8; 2]);
        }

        debug_assert_eq!(out.len(), total_len);
        Ok(out)
    }

    /// Decodes and strictly validates an `ANMW v3` byte buffer (no trailing bytes permitted).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AnmwV3Error> {
        let header = parse_header(bytes)?;
        let n = (header.width as usize)
            .checked_mul(header.height as usize)
            .ok_or_else(|| AnmwV3Error::new("ANMW v3 dimension multiplication overflow"))?;
        let grid_f32_bytes = n
            .checked_mul(4)
            .and_then(|v| v.checked_mul(5))
            .ok_or_else(|| AnmwV3Error::new("ANMW v3 float payload byte overflow"))?;
        let grid_u8_bytes = n
            .checked_mul(2)
            .ok_or_else(|| AnmwV3Error::new("ANMW v3 byte payload overflow"))?;
        let grid_end = ANMW_V3_HEADER_LEN
            .checked_add(grid_f32_bytes)
            .and_then(|v| v.checked_add(grid_u8_bytes))
            .ok_or_else(|| AnmwV3Error::new("ANMW v3 offset overflow"))?;
        if bytes.len() < grid_end + 8 {
            return Err(AnmwV3Error::new(format!(
                "ANMW v3 payload truncated: expected at least {} bytes, got {}",
                grid_end + 8,
                bytes.len()
            )));
        }

        let mut cursor = ANMW_V3_HEADER_LEN;
        let step_f32 = n * 4;
        let elevation = read_f32_array(bytes, cursor, n)?;
        cursor += step_f32;
        let moisture = read_f32_array(bytes, cursor, n)?;
        cursor += step_f32;
        let temperature = read_f32_array(bytes, cursor, n)?;
        cursor += step_f32;
        let flow = read_f32_array(bytes, cursor, n)?;
        cursor += step_f32;
        let slope = read_f32_array(bytes, cursor, n)?;
        cursor += step_f32;
        let biome_22 = bytes[cursor..cursor + n].to_vec();
        cursor += n;
        let water_kind = bytes[cursor..cursor + n].to_vec();
        cursor += n;

        let wf_count = read_u32_le(bytes, cursor)? as usize;
        cursor += 4;
        if wf_count > ANMW_V3_MAX_RECORD_COUNT {
            return Err(AnmwV3Error::new(format!(
                "ANMW v3 waterfall_count {wf_count} exceeds limit {ANMW_V3_MAX_RECORD_COUNT}"
            )));
        }
        let wf_bytes = wf_count
            .checked_mul(ANMW_V3_WATERFALL_LEN)
            .ok_or_else(|| AnmwV3Error::new("ANMW v3 waterfall byte-length overflow"))?;
        let wf_end = cursor
            .checked_add(wf_bytes)
            .and_then(|v| v.checked_add(4))
            .ok_or_else(|| AnmwV3Error::new("ANMW v3 waterfall offset overflow"))?;
        if bytes.len() < wf_end {
            return Err(AnmwV3Error::new(format!(
                "ANMW v3 truncated before cave count: expected >= {wf_end}, got {}",
                bytes.len()
            )));
        }

        let mut waterfalls = Vec::with_capacity(wf_count);
        for _ in 0..wf_count {
            waterfalls.push(AnmwV3Waterfall {
                x_m: read_f32_le(bytes, cursor)?,
                z_m: read_f32_le(bytes, cursor + 4)?,
                lip_elev_m: read_f32_le(bytes, cursor + 8)?,
                drop_m: read_f32_le(bytes, cursor + 12)?,
                yaw_rad: read_f32_le(bytes, cursor + 16)?,
                discharge_q_m3_s: read_f32_le(bytes, cursor + 20)?,
                plunge_depth_m: read_f32_le(bytes, cursor + 24)?,
                flags: read_u32_le(bytes, cursor + 28)?,
            });
            cursor += ANMW_V3_WATERFALL_LEN;
        }

        let cave_count = read_u32_le(bytes, cursor)? as usize;
        cursor += 4;
        if cave_count > ANMW_V3_MAX_RECORD_COUNT {
            return Err(AnmwV3Error::new(format!(
                "ANMW v3 cave_entrance_count {cave_count} exceeds limit {ANMW_V3_MAX_RECORD_COUNT}"
            )));
        }
        let cave_bytes = cave_count
            .checked_mul(ANMW_V3_CAVE_ENTRANCE_LEN)
            .ok_or_else(|| AnmwV3Error::new("ANMW v3 cave byte-length overflow"))?;
        let expected_len = cursor
            .checked_add(cave_bytes)
            .ok_or_else(|| AnmwV3Error::new("ANMW v3 total byte-length overflow"))?;
        if bytes.len() != expected_len {
            return Err(AnmwV3Error::new(format!(
                "ANMW v3 byte length mismatch: expected {expected_len}, got {}",
                bytes.len()
            )));
        }

        let mut cave_entrances = Vec::with_capacity(cave_count);
        for _ in 0..cave_count {
            if bytes[cursor + 22] != 0 || bytes[cursor + 23] != 0 {
                return Err(AnmwV3Error::new(
                    "ANMW v3 non-zero reserved cave entrance bytes",
                ));
            }
            cave_entrances.push(AnmwV3CaveEntrance {
                x_m: read_f32_le(bytes, cursor)?,
                z_m: read_f32_le(bytes, cursor + 4)?,
                elevation_m: read_f32_le(bytes, cursor + 8)?,
                yaw_rad: read_f32_le(bytes, cursor + 12)?,
                clearance_m: read_f32_le(bytes, cursor + 16)?,
                mouth_kind: bytes[cursor + 20],
                realm_status: bytes[cursor + 21],
            });
            cursor += ANMW_V3_CAVE_ENTRANCE_LEN;
        }

        let out = Self {
            width: header.width,
            height: header.height,
            sea_level_norm: header.sea_level_norm,
            sim_seed_u32: header.sim_seed_u32,
            seed_u64: header.seed_u64,
            generator_version: header.generator_version,
            world_scale: header.world_scale,
            flags: header.flags,
            elevation,
            moisture,
            temperature,
            flow,
            slope,
            biome_22,
            water_kind,
            waterfalls,
            cave_entrances,
        };
        out.validate()?;
        Ok(out)
    }

    /// Physical elevation in metres (`[-1500, +5700]`) at flattened cell index `idx`.
    #[must_use]
    pub fn elevation_m_at(&self, idx: usize) -> f32 {
        denormalize_elevation(self.elevation[idx])
    }

    /// Projects cell `idx` onto the backend 11-biome index (`0..=10`) preserving DeepOcean (`<-220m`)
    /// and inland lakes (`Ocean = 1`, `§3.4`).
    #[must_use]
    pub fn backend_biome_at(&self, idx: usize) -> u8 {
        map_water_and_biome_to_11(
            self.biome_22[idx],
            self.water_kind[idx],
            self.elevation[idx],
        )
    }

    /// Projects the full `width * height` grid into backend 11-variant `BiomeType` indices (`0..=10`).
    #[must_use]
    pub fn backend_biomes(&self) -> Vec<u8> {
        self.biome_22
            .iter()
            .zip(self.water_kind.iter())
            .zip(self.elevation.iter())
            .map(|((&b, &wk), &e)| map_water_and_biome_to_11(b, wk, e))
            .collect()
    }

    /// Picks up to `max_pois` deterministic land POIs (`elev >= sea_level_norm`), matching
    /// `WorldArtifact::to_terrain_map`'s coprime stride sampling.
    #[must_use]
    pub fn land_pois(&self, max_pois: usize) -> Vec<(usize, usize)> {
        let w = self.width as usize;
        let h = self.height as usize;
        let tn = w * h;
        let mut pois = Vec::with_capacity(max_pois);
        if tn == 0 || max_pois == 0 {
            return pois;
        }
        let stride = (tn / 97).max(1);
        let mut idx = 0_usize;
        let mut steps = 0_usize;
        while pois.len() < max_pois && steps < tn {
            let wk = self.water_kind[idx];
            if self.elevation[idx] >= self.sea_level_norm
                && wk != WaterKind::Ocean as u8
                && wk != WaterKind::FreshwaterLake as u8
                && wk != WaterKind::EndorheicLake as u8
            {
                pois.push((idx % w, idx / w));
            }
            idx = (idx + stride) % tn;
            steps += 1;
        }
        pois
    }
}

/// Computes iterative bottom-up Strahler stream order on channel cells (`0` for non-channel cells).
///
/// Uses Kahn topological ordering on the `d8_dir` DAG so deep dendritic networks (e.g. `4096^2`)
/// never recurse on the call stack.
#[must_use]
pub fn compute_strahler_order(
    d8_dir: &[u8],
    is_channel: &[bool],
    width: usize,
    height: usize,
) -> Vec<u8> {
    let n = width * height;
    assert_eq!(d8_dir.len(), n);
    assert_eq!(is_channel.len(), n);

    let mut downstream = vec![u32::MAX; n];
    let mut indegree = vec![0_u16; n];
    for idx in 0..n {
        if !is_channel[idx] {
            continue;
        }
        let dir = d8_dir[idx];
        if dir < 8 {
            let r = (idx / width) as isize + DZ8[dir as usize];
            let c = (idx % width) as isize + DX8[dir as usize];
            if r >= 0 && c >= 0 && r < height as isize && c < width as isize {
                let nb = (r as usize) * width + (c as usize);
                if nb != idx && is_channel[nb] {
                    downstream[idx] = nb as u32;
                    indegree[nb] = indegree[nb].saturating_add(1);
                }
            }
        }
    }

    let mut strahler = vec![0_u8; n];
    let mut max_child = vec![0_u8; n];
    let mut count_max = vec![0_u8; n];
    let mut queue = VecDeque::new();

    for idx in 0..n {
        if is_channel[idx] && indegree[idx] == 0 {
            strahler[idx] = 1;
            queue.push_back(idx as u32);
        }
    }

    while let Some(u_u32) = queue.pop_front() {
        let u = u_u32 as usize;
        let ord = strahler[u];
        let v_u32 = downstream[u];
        if v_u32 == u32::MAX {
            continue;
        }
        let v = v_u32 as usize;
        if ord > max_child[v] {
            max_child[v] = ord;
            count_max[v] = 1;
        } else if ord == max_child[v] {
            count_max[v] = count_max[v].saturating_add(1);
        }
        if indegree[v] > 0 {
            indegree[v] -= 1;
            if indegree[v] == 0 {
                strahler[v] = if count_max[v] >= 2 {
                    max_child[v].saturating_add(1)
                } else {
                    max_child[v].max(1)
                };
                queue.push_back(v as u32);
            }
        }
    }

    strahler
}

/// Fine-resolution Stage S1–S9 arrays projected into `sim_256.anmw3` (`§3.4`).
pub struct RawSimProjectionInput<'a> {
    pub src_size: u32,
    pub dx_m: f32,
    pub seed_u64: u64,
    pub generator_version: u32,
    pub world_scale: f32,
    pub total_h_m: &'a [f32],
    pub moisture: &'a [f32],
    pub temp_c: &'a [f32],
    pub discharge_q_m3_s: &'a [f32],
    /// Optional precomputed local terrain slope (`|\nabla h|`). Computed from `total_h_m` when `None`.
    pub slope: Option<&'a [f32]>,
    pub river_w_m: &'a [f32],
    pub water_kind: &'a [u8],
    pub dominant_biome: &'a [u8],
    pub d8_dir: &'a [u8],
    pub is_channel: &'a [bool],
    pub waterfalls: Vec<AnmwV3Waterfall>,
    pub cave_entrances: Vec<AnmwV3CaveEntrance>,
}

#[inline]
fn is_oceanic_fine(wk: u8, h_m: f32) -> bool {
    wk == WaterKind::Ocean as u8 || (wk == WaterKind::DeltaTurbid as u8 && h_m <= SEA_LEVEL_M + 0.1)
}

#[inline]
fn is_lake_fine(wk: u8) -> bool {
    wk == WaterKind::FreshwaterLake as u8 || wk == WaterKind::EndorheicLake as u8
}

#[inline]
fn d8_neighbor_idx(idx: usize, dir: u8, width: usize, height: usize) -> Option<usize> {
    if dir >= 8 {
        return None;
    }
    let nr = (idx / width) as isize + DZ8[dir as usize];
    let nc = (idx % width) as isize + DX8[dir as usize];
    if nr >= 0 && nc >= 0 && nr < height as isize && nc < width as isize {
        Some((nr as usize) * width + (nc as usize))
    } else {
        None
    }
}

fn compute_slope_from_elev(total_h_m: &[f32], size: usize, dx_m: f32) -> Vec<f32> {
    let mut slope = vec![0.0_f32; size * size];
    if size < 2 || !dx_m.is_finite() || dx_m <= 0.0 {
        return slope;
    }
    for r in 0..size {
        let ru = r.saturating_sub(1);
        let rd = (r + 1).min(size - 1);
        let span_z = dx_m * (rd - ru) as f32;
        for c in 0..size {
            let cl = c.saturating_sub(1);
            let cr = (c + 1).min(size - 1);
            let span_x = dx_m * (cr - cl) as f32;
            let dh_dx = (total_h_m[r * size + cr] - total_h_m[r * size + cl]) / span_x;
            let dh_dz = (total_h_m[rd * size + c] - total_h_m[ru * size + c]) / span_z;
            slope[r * size + c] = (dh_dx * dh_dx + dh_dz * dh_dz).sqrt();
        }
    }
    slope
}

/// Projects fine raw fields (`512²`..`8192²`, or `256²` `1:1`) onto `target_size × target_size`
/// `ANMW v3` using exact raw `f32` block averages and Strahler-order `>= 4` connected river
/// projection (`§3.4`, `GS-1`).
pub fn project_raw_to_sim(
    input: &RawSimProjectionInput<'_>,
    target_size: u32,
) -> Result<AnmwV3Sim, AnmwV3Error> {
    if target_size == 0
        || input.src_size < target_size
        || !input.src_size.is_multiple_of(target_size)
    {
        return Err(AnmwV3Error::new(format!(
            "src_size ({}) must be a positive multiple of target_size ({target_size})",
            input.src_size
        )));
    }
    if !input.dx_m.is_finite() || input.dx_m <= 0.0 {
        return Err(AnmwV3Error::new("dx_m must be finite and > 0"));
    }
    let src = input.src_size as usize;
    let dst = target_size as usize;
    let block = src / dst;
    let block_cells = block * block;
    let src_n = src
        .checked_mul(src)
        .ok_or_else(|| AnmwV3Error::new("src grid size overflow"))?;
    let dst_n = dst * dst;

    for (name, len) in [
        ("total_h_m", input.total_h_m.len()),
        ("moisture", input.moisture.len()),
        ("temp_c", input.temp_c.len()),
        ("discharge_q_m3_s", input.discharge_q_m3_s.len()),
        ("river_w_m", input.river_w_m.len()),
        ("water_kind", input.water_kind.len()),
        ("dominant_biome", input.dominant_biome.len()),
        ("d8_dir", input.d8_dir.len()),
        ("is_channel", input.is_channel.len()),
    ] {
        if len != src_n {
            return Err(AnmwV3Error::new(format!(
                "input slice {name} length mismatch: expected {src_n}, got {len}"
            )));
        }
    }
    if let Some(s) = input.slope {
        if s.len() != src_n {
            return Err(AnmwV3Error::new(format!(
                "input slice slope length mismatch: expected {src_n}, got {}",
                s.len()
            )));
        }
        validate_float_slice(s, "slope", true)?;
    }

    validate_float_slice(input.total_h_m, "total_h_m", false)?;
    validate_float_slice(input.moisture, "moisture", false)?;
    validate_float_slice(input.temp_c, "temp_c", false)?;
    validate_float_slice(input.discharge_q_m3_s, "discharge_q_m3_s", true)?;
    validate_float_slice(input.river_w_m, "river_w_m", true)?;

    let computed_slope;
    let slope_slice = match input.slope {
        Some(s) => s,
        None => {
            computed_slope = compute_slope_from_elev(input.total_h_m, src, input.dx_m);
            &computed_slope
        }
    };

    let strahler = compute_strahler_order(input.d8_dir, input.is_channel, src, src);

    let mut elevation = vec![0.0_f32; dst_n];
    let mut moisture = vec![0.0_f32; dst_n];
    let mut temperature = vec![0.0_f32; dst_n];
    let mut flow = vec![0.0_f32; dst_n];
    let mut slope = vec![0.0_f32; dst_n];
    let mut biome_22 = vec![BiomeId::Grassland as u8; dst_n];
    let mut water_kind = vec![WaterKind::None as u8; dst_n];

    let mut block_is_ocean = vec![false; dst_n];
    let mut block_is_lake = vec![false; dst_n];
    let mut block_has_major_river = vec![false; dst_n];
    let mut block_has_waterfall = vec![false; dst_n];
    let mut block_has_delta = vec![false; dst_n];
    let mut block_endorheic_wins = vec![false; dst_n];
    let mut block_modal_biome = vec![BiomeId::Grassland as u8; dst_n];

    let inv_block_cells = 1.0_f64 / (block_cells as f64);

    for tr in 0..dst {
        for tc in 0..dst {
            let b_idx = tr * dst + tc;
            let mut sum_e = 0.0_f64;
            let mut sum_m = 0.0_f64;
            let mut sum_t = 0.0_f64;
            let mut sum_f = 0.0_f64;
            let mut sum_s = 0.0_f64;

            let mut ocean_cnt = 0_usize;
            let mut lake_fresh_cnt = 0_usize;
            let mut lake_endo_cnt = 0_usize;
            let mut wf_cnt = 0_usize;
            let mut delta_cnt = 0_usize;
            let mut major_river_hit = false;
            let mut biome_hist = [0_usize; BIOME_COUNT];

            let r0 = tr * block;
            let c0 = tc * block;
            for dr in 0..block {
                let row_off = (r0 + dr) * src + c0;
                for dc in 0..block {
                    let s_idx = row_off + dc;
                    let h_m = input.total_h_m[s_idx];
                    sum_e += f64::from(normalize_elevation(h_m));
                    sum_m += f64::from(input.moisture[s_idx].clamp(0.0, 1.0));
                    sum_t += f64::from(normalize_temperature_c(input.temp_c[s_idx]));
                    sum_f += f64::from(input.discharge_q_m3_s[s_idx]);
                    sum_s += f64::from(slope_slice[s_idx]);

                    let wk = input.water_kind[s_idx];
                    let dom = input.dominant_biome[s_idx];
                    if (dom as usize) < BIOME_COUNT {
                        biome_hist[dom as usize] += 1;
                    }
                    if is_oceanic_fine(wk, h_m) {
                        ocean_cnt += 1;
                    } else if wk == WaterKind::FreshwaterLake as u8 {
                        lake_fresh_cnt += 1;
                    } else if wk == WaterKind::EndorheicLake as u8 {
                        lake_endo_cnt += 1;
                    } else {
                        if wk == WaterKind::Waterfall as u8 {
                            wf_cnt += 1;
                        } else if wk == WaterKind::DeltaTurbid as u8 {
                            delta_cnt += 1;
                        }
                        if (input.is_channel[s_idx]
                            && (strahler[s_idx] >= 4 || input.river_w_m[s_idx] >= 8.0))
                            || (block == 1
                                && (dom == BiomeId::River as u8 || wk == WaterKind::River as u8))
                        {
                            major_river_hit = true;
                        }
                    }
                }
            }

            elevation[b_idx] = (sum_e * inv_block_cells) as f32;
            moisture[b_idx] = (sum_m * inv_block_cells) as f32;
            temperature[b_idx] = (sum_t * inv_block_cells) as f32;
            flow[b_idx] = (sum_f * inv_block_cells) as f32;
            slope[b_idx] = (sum_s * inv_block_cells) as f32;

            let lake_cnt = lake_fresh_cnt + lake_endo_cnt;
            block_is_ocean[b_idx] = ocean_cnt * 2 >= block_cells;
            block_is_lake[b_idx] = !block_is_ocean[b_idx] && lake_cnt * 2 >= block_cells;
            block_has_waterfall[b_idx] = wf_cnt > 0;
            block_has_delta[b_idx] = delta_cnt > 0;
            block_endorheic_wins[b_idx] = lake_endo_cnt > lake_fresh_cnt;
            if !block_is_ocean[b_idx] && !block_is_lake[b_idx] && major_river_hit {
                block_has_major_river[b_idx] = true;
            }

            // Pick modal canonical non-ocean, non-lake biome cleanly (fallback to overall modal
            // biome if every fine cell was ocean/lake in a mixed boundary block).
            let mut best_b = BiomeId::Grassland as u8;
            let mut best_count = 0_usize;
            for (b_id, &cnt) in biome_hist.iter().enumerate() {
                if b_id == BiomeId::Ocean as usize || b_id == BiomeId::Lake as usize {
                    continue;
                }
                if cnt > best_count {
                    best_count = cnt;
                    best_b = b_id as u8;
                }
            }
            if best_count == 0 {
                for (b_id, &cnt) in biome_hist.iter().enumerate() {
                    if cnt > best_count {
                        best_count = cnt;
                        best_b = b_id as u8;
                    }
                }
            }
            block_modal_biome[b_idx] = best_b;
        }
    }

    // 8-connected centerline bridging along upstream/downstream Strahler >= 4 / w >= 8 m paths
    // so major rivers never break into disconnected blocks on `256^2` (`§3.4`, `GS-1`).
    let mut visited_fine = vec![false; src_n];
    for start in 0..src_n {
        if visited_fine[start] {
            continue;
        }
        let wk = input.water_kind[start];
        let h_m = input.total_h_m[start];
        if is_oceanic_fine(wk, h_m) || is_lake_fine(wk) {
            continue;
        }
        if !input.is_channel[start] || (strahler[start] < 4 && input.river_w_m[start] < 8.0) {
            continue;
        }

        let mut cur = start;
        let mut steps = 0_usize;
        while steps < src_n {
            steps += 1;
            let cur_wk = input.water_kind[cur];
            let cur_h = input.total_h_m[cur];
            let b_cur = ((cur / src) / block) * dst + ((cur % src) / block);

            if is_oceanic_fine(cur_wk, cur_h) {
                if !block_is_ocean[b_cur] && !block_is_lake[b_cur] {
                    block_has_major_river[b_cur] = true;
                }
                break;
            }
            if !block_is_ocean[b_cur] && !block_is_lake[b_cur] {
                block_has_major_river[b_cur] = true;
            } else if block_is_lake[b_cur] {
                break;
            }

            if visited_fine[cur] {
                break;
            }
            visited_fine[cur] = true;

            match d8_neighbor_idx(cur, input.d8_dir[cur], src, src) {
                Some(nxt) if nxt != cur => cur = nxt,
                _ => break,
            }
        }
    }

    for b_idx in 0..dst_n {
        if block_is_ocean[b_idx] {
            biome_22[b_idx] = BiomeId::Ocean as u8;
            water_kind[b_idx] = WaterKind::Ocean as u8;
        } else if block_has_major_river[b_idx] {
            biome_22[b_idx] = BiomeId::River as u8;
            water_kind[b_idx] = if block_has_waterfall[b_idx] {
                WaterKind::Waterfall as u8
            } else if block_has_delta[b_idx] {
                WaterKind::DeltaTurbid as u8
            } else {
                WaterKind::River as u8
            };
        } else if block_is_lake[b_idx] {
            biome_22[b_idx] = BiomeId::Lake as u8;
            water_kind[b_idx] = if block_endorheic_wins[b_idx] {
                WaterKind::EndorheicLake as u8
            } else {
                WaterKind::FreshwaterLake as u8
            };
        } else {
            biome_22[b_idx] = block_modal_biome[b_idx];
            water_kind[b_idx] = WaterKind::None as u8;
        }
    }

    let sim = AnmwV3Sim {
        width: target_size,
        height: target_size,
        sea_level_norm: SEA_LEVEL_NORM,
        sim_seed_u32: derive_sim_seed_u32(input.seed_u64),
        seed_u64: input.seed_u64,
        generator_version: input.generator_version,
        world_scale: input.world_scale,
        flags: ANMW_V3_FLAGS_STANDARD,
        elevation,
        moisture,
        temperature,
        flow,
        slope,
        biome_22,
        water_kind,
        waterfalls: input.waterfalls.clone(),
        cave_entrances: input.cave_entrances.clone(),
    };
    sim.validate()?;
    Ok(sim)
}

/// Projects fine raw fields (`>= 256²`) into the canonical `256 × 256` `sim_256.anmw3` container.
pub fn project_raw_to_sim_256(input: &RawSimProjectionInput<'_>) -> Result<AnmwV3Sim, AnmwV3Error> {
    project_raw_to_sim(input, ANMW_V3_SIM_SIZE)
}

/// Loads `raw_meta.json` and spilled binary layers from an `anima-worldgen generate` output
/// directory, reconstructs D8 flow directions, and projects into canonical `sim_256.anmw3` (`256²`).
#[cfg(feature = "generate")]
pub fn project_raw_dir_to_sim_256(
    raw_dir: &Path,
    generator_version: u32,
) -> Result<AnmwV3Sim, PipelineError> {
    let meta_text = std::fs::read_to_string(raw_dir.join("raw_meta.json"))?;
    let meta: RawWorldMeta = serde_json::from_str(&meta_text)?;
    if meta.width == 0 || meta.width != meta.height {
        return Err(PipelineError {
            message: format!(
                "raw_meta.json dimensions {}x{} must be equal and non-zero",
                meta.width, meta.height
            ),
        });
    }
    let size = meta.width as usize;
    let n = size * size;

    let total_h_m = read_bin_f32(&raw_dir.join("total_h_m.bin"), n)?;
    let moisture = read_bin_f32(&raw_dir.join("moisture.bin"), n)?;
    let temp_c = read_bin_f32(&raw_dir.join("temp_c.bin"), n)?;
    let discharge_q_m3_s = read_bin_f32(&raw_dir.join("discharge_q_m3_s.bin"), n)?;
    let river_w_m = read_bin_f32(&raw_dir.join("river_w_m.bin"), n)?;
    let water_kind = read_bin_u8(&raw_dir.join("water_kind.bin"), n)?;
    let dominant_biome = read_bin_u8(&raw_dir.join("dominant_biome.bin"), n)?;

    let cave_bytes = std::fs::read(raw_dir.join("cave_network.bin"))?;
    let cave_net = CaveNetwork::from_bytes(&cave_bytes).map_err(|e| PipelineError {
        message: format!("failed to decode cave_network.bin: {e}"),
    })?;

    let mut filled_h = total_h_m.clone();
    let mut d8_dir = vec![HYDRO_NO_DIR; n];
    let mut pf_order = Vec::new();
    priority_flood_d8_lean_into(
        &mut filled_h,
        &mut d8_dir,
        &mut pf_order,
        size,
        size,
        Some(meta.sea_level_m),
        false,
    );
    drop(filled_h);
    drop(pf_order);

    let is_channel: Vec<bool> = river_w_m
        .iter()
        .zip(water_kind.iter())
        .zip(total_h_m.iter())
        .map(|((&rw, &wk), &h)| {
            !is_oceanic_fine(wk, h)
                && !is_lake_fine(wk)
                && (rw >= 0.5 || wk == WaterKind::River as u8 || wk == WaterKind::Waterfall as u8)
        })
        .collect();

    let waterfalls: Vec<AnmwV3Waterfall> = meta
        .waterfalls
        .iter()
        .map(|wf| AnmwV3Waterfall {
            x_m: wf.x_m,
            z_m: wf.z_m,
            lip_elev_m: wf.lip_elev_m,
            drop_m: wf.drop_m,
            yaw_rad: wf.yaw_rad,
            discharge_q_m3_s: wf.discharge_q_m3_s,
            plunge_depth_m: wf.plunge_depth_m,
            flags: 0,
        })
        .collect();

    let mut cave_entrances = Vec::with_capacity(cave_net.entrances.len());
    for ent in &cave_net.entrances {
        let node = cave_net
            .nodes
            .get(ent.node_idx as usize)
            .ok_or_else(|| PipelineError {
                message: format!("cave entrance node_idx {} out of bounds", ent.node_idx),
            })?;
        cave_entrances.push(AnmwV3CaveEntrance {
            x_m: node.x_m,
            z_m: node.z_m,
            elevation_m: ent.elevation_m,
            yaw_rad: ent.yaw_rad,
            clearance_m: ent.clearance_m,
            mouth_kind: ent.mouth_kind,
            realm_status: ent.realm_status,
        });
    }

    let proj_in = RawSimProjectionInput {
        src_size: meta.width,
        dx_m: meta.dx_m,
        seed_u64: meta.seed,
        generator_version,
        world_scale: ANMW_V3_WORLD_SCALE,
        total_h_m: &total_h_m,
        moisture: &moisture,
        temp_c: &temp_c,
        discharge_q_m3_s: &discharge_q_m3_s,
        slope: None,
        river_w_m: &river_w_m,
        water_kind: &water_kind,
        dominant_biome: &dominant_biome,
        d8_dir: &d8_dir,
        is_channel: &is_channel,
        waterfalls,
        cave_entrances,
    };
    project_raw_to_sim_256(&proj_in).map_err(|e| PipelineError {
        message: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sim(width: u32, height: u32, seed_u64: u64) -> AnmwV3Sim {
        let n = (width * height) as usize;
        let mut elevation = vec![0.0_f32; n];
        let mut moisture = vec![0.0_f32; n];
        let mut temperature = vec![0.0_f32; n];
        let mut flow = vec![0.0_f32; n];
        let mut slope = vec![0.0_f32; n];
        let mut biome_22 = vec![BiomeId::Grassland as u8; n];
        let mut water_kind = vec![WaterKind::None as u8; n];

        for i in 0..n {
            let t = i as f32 / n.max(1) as f32;
            if i < n / 4 {
                elevation[i] = normalize_elevation(-350.0);
                biome_22[i] = BiomeId::Ocean as u8;
                water_kind[i] = WaterKind::Ocean as u8;
            } else if i < n / 3 {
                elevation[i] = normalize_elevation(-40.0);
                biome_22[i] = BiomeId::Ocean as u8;
                water_kind[i] = WaterKind::Ocean as u8;
            } else if i == n / 2 {
                elevation[i] = normalize_elevation(120.0);
                biome_22[i] = BiomeId::Lake as u8;
                water_kind[i] = WaterKind::FreshwaterLake as u8;
            } else if i == n / 2 + 1 {
                elevation[i] = normalize_elevation(150.0);
                biome_22[i] = BiomeId::River as u8;
                water_kind[i] = WaterKind::River as u8;
            } else {
                elevation[i] = (SEA_LEVEL_NORM + 0.05 + 0.5 * t).clamp(0.0, 1.0);
                biome_22[i] = BiomeId::Forest as u8;
            }
            moisture[i] = 0.25 + 0.5 * t;
            temperature[i] = 0.7 - 0.3 * t;
            flow[i] = (i as f32) * 0.1;
            slope[i] = 0.02 + 0.1 * t;
        }

        AnmwV3Sim {
            width,
            height,
            sea_level_norm: SEA_LEVEL_NORM,
            sim_seed_u32: derive_sim_seed_u32(seed_u64),
            seed_u64,
            generator_version: 3,
            world_scale: ANMW_V3_WORLD_SCALE,
            flags: ANMW_V3_FLAGS_STANDARD,
            elevation,
            moisture,
            temperature,
            flow,
            slope,
            biome_22,
            water_kind,
            waterfalls: vec![AnmwV3Waterfall {
                x_m: 12_340.0,
                z_m: 22_110.0,
                lip_elev_m: 1_240.0,
                drop_m: 28.5,
                yaw_rad: 1.25,
                discharge_q_m3_s: 4.5,
                plunge_depth_m: 3.1,
                flags: 0,
            }],
            cave_entrances: vec![AnmwV3CaveEntrance {
                x_m: 9_800.0,
                z_m: 15_200.0,
                elevation_m: 780.0,
                yaw_rad: -0.75,
                clearance_m: 4.2,
                mouth_kind: 0,
                realm_status: 1,
            }],
        }
    }

    #[test]
    fn header_peek_seed_and_roundtrip_preserve_exact_bytes_and_dual_identity() {
        let seed_u64 = 0x1234_5678_9ABC_DEF0;
        let sim = sample_sim(16, 16, seed_u64);
        let bytes = sim.to_bytes().expect("serialize v3 sim");

        assert!(is_anmw_v3_header(&bytes));
        assert_eq!(&bytes[0..4], b"ANMW");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 3);

        // `peek_seed` works cleanly on the 44-byte header prefix alone and agrees with offset 20..24.
        let expected_seed = derive_sim_seed_u32(seed_u64);
        assert_eq!(
            AnmwV3Sim::peek_seed(&bytes[..ANMW_V3_HEADER_LEN]).unwrap(),
            expected_seed
        );
        assert_eq!(
            u32::from_le_bytes(bytes[20..24].try_into().unwrap()),
            expected_seed
        );

        let decoded = AnmwV3Sim::from_bytes(&bytes).expect("decode v3 sim");
        assert_eq!(decoded, sim);
        assert_eq!(decoded.to_bytes().unwrap(), bytes);

        // Backend biome projection distinguishes DeepOcean (< -220 m), shallow Ocean, Lake, River.
        assert_eq!(decoded.backend_biome_at(0), 0);
        assert_eq!(decoded.backend_biome_at((16 * 16) / 4 + 1), 1);
        assert_eq!(decoded.backend_biome_at((16 * 16) / 2), 1);
        assert_eq!(decoded.backend_biome_at((16 * 16) / 2 + 1), 3);
        assert!(decoded.backend_biomes().into_iter().all(|b| b <= 10));

        // LD-2 dual identities: index.json viewer edits move bundle_identity without altering sim_world_identity.
        let sim_id = compute_sim_world_identity(&bytes);
        verify_sim_world_identity(&bytes, &sim_id).expect("sim_world_identity matches");

        let index_v1 = br#"{"sim_world_identity":"ok","tiles":["lighting_v1.ktx2"]}"#;
        let index_v2 = br#"{"sim_world_identity":"ok","tiles":["lighting_v2.ktx2"]}"#;
        let bundle_1 = compute_bundle_identity(index_v1);
        let bundle_2 = compute_bundle_identity(index_v2);
        assert_ne!(bundle_1, bundle_2);
        verify_bundle_identity(index_v1, &bundle_1).unwrap();
        assert!(verify_bundle_identity(index_v2, &bundle_1).is_err());

        // Hex round-trip + tamper negative control on sim_256.
        let hex = sha256_to_hex(&sim_id);
        assert_eq!(sha256_from_hex(&hex).unwrap(), sim_id);
        let mut tampered = bytes.clone();
        tampered[ANMW_V3_HEADER_LEN + 17] ^= 0x01;
        assert!(verify_sim_world_identity(&tampered, &sim_id).is_err());
    }

    #[test]
    fn corrupt_headers_trailing_bytes_and_bad_enums_are_rejected() {
        let valid = sample_sim(8, 8, 42).to_bytes().unwrap();

        assert!(AnmwV3Sim::from_bytes(&valid[..10]).is_err());
        assert!(AnmwV3Sim::peek_seed(&valid[..43]).is_err());

        let mut bad_magic = valid.clone();
        bad_magic[0] = b'X';
        assert!(!is_anmw_v3_header(&bad_magic));
        assert!(AnmwV3Sim::from_bytes(&bad_magic).is_err());

        let mut bad_ver = valid.clone();
        bad_ver[4..8].copy_from_slice(&2_u32.to_le_bytes());
        assert!(!is_anmw_v3_header(&bad_ver));
        assert!(AnmwV3Sim::from_bytes(&bad_ver).is_err());

        let mut bad_seed = valid.clone();
        bad_seed[20..24].copy_from_slice(&0xDEAD_BEEF_u32.to_le_bytes());
        assert!(AnmwV3Sim::peek_seed(&bad_seed).is_err());
        assert!(AnmwV3Sim::from_bytes(&bad_seed).is_err());

        let mut bad_flags = valid.clone();
        bad_flags[40..44].copy_from_slice(&1_u32.to_le_bytes());
        assert!(AnmwV3Sim::from_bytes(&bad_flags).is_err());

        let mut trailing = valid.clone();
        trailing.push(0);
        assert!(AnmwV3Sim::from_bytes(&trailing).is_err());

        let mut sim_nan = sample_sim(8, 8, 42);
        sim_nan.elevation[5] = f32::NAN;
        assert!(sim_nan.to_bytes().is_err());

        let mut sim_bad_biome = sample_sim(8, 8, 42);
        sim_bad_biome.biome_22[3] = 22;
        assert!(sim_bad_biome.to_bytes().is_err());
    }

    #[test]
    fn strahler_4_projection_beats_25pct_rule_and_preserves_connectivity_and_f32_parity() {
        // 512x512 fine grid downsampled into 256x256 (2x2 blocks -> a 1-cell wide river covers 2/4 or 1/4,
        // and we also test 512x512 -> 128x128 (4x4 blocks = 16 cells, where a 1-cell trunk covers 4/16 = 25%
        // OR diagonal 3/16 = 18.75% < 25%!) and 512x512 -> 64x64 (8x8 = 64 cells, < 15% per block!)).
        let src = 512_usize;
        let dst = 64_usize;
        let block = src / dst;
        let src_n = src * src;

        let mut total_h_m = vec![400.0_f32; src_n];
        let mut moisture = vec![0.6_f32; src_n];
        let temp_c = vec![22.0_f32; src_n];
        let mut discharge_q = vec![0.0_f32; src_n];
        let mut river_w = vec![0.0_f32; src_n];
        let mut water_kind = vec![WaterKind::None as u8; src_n];
        let mut dominant_biome = vec![BiomeId::Forest as u8; src_n];
        let mut d8_dir = vec![D8_NO_DIR; src_n];
        let mut is_channel = vec![false; src_n];

        // Marine belt along east border c >= 496 (all ocean -> 100% ocean in east block columns 62, 63).
        for r in 0..src {
            for c in 496..src {
                let idx = r * src + c;
                total_h_m[idx] = if c >= 504 { -300.0 } else { -20.0 };
                water_kind[idx] = WaterKind::Ocean as u8;
                dominant_biome[idx] = BiomeId::Ocean as u8;
            }
        }

        // Build a synthetic binary tributary tree feeding a trunk river along row 256 from c = 40 -> 496,
        // yielding Strahler order >= 4 cleanly with only 1-cell wide channel across 8x8 blocks (8/64 = 12.5% < 25%!).
        // Trunk: row 256, c in 40..=495 draining East (dir = 0) into ocean at c = 496.
        for c in 40..496 {
            let idx = 256 * src + c;
            is_channel[idx] = true;
            d8_dir[idx] = 0; // East (+1, 0)
            water_kind[idx] = WaterKind::River as u8;
            dominant_biome[idx] = BiomeId::River as u8;
            river_w[idx] = 6.5; // Intentionally < 8.0 m so only Strahler >= 4 triggers river classification!
            discharge_q[idx] = 12.0;
            moisture[idx] = 0.85;
            total_h_m[idx] = 200.0 - (c as f32) * 0.25;
        }
        // Build 2 disjoint Strahler-3 subtrees joining the trunk at (256, 44) and (256, 56) -> Strahler 4!
        let add_trib = |d8: &mut [u8],
                        ch: &mut [bool],
                        wk: &mut [u8],
                        db: &mut [u8],
                        cells: &[(usize, usize, u8)]| {
            for &(r, c, dir) in cells {
                let i = r * src + c;
                ch[i] = true;
                d8[i] = dir;
                wk[i] = WaterKind::River as u8;
                db[i] = BiomeId::River as u8;
            }
        };
        add_trib(
            &mut d8_dir,
            &mut is_channel,
            &mut water_kind,
            &mut dominant_biome,
            &[
                // Subtree 1 -> ord 3 entering trunk at (256, 44):
                (249, 43, 1),
                (249, 45, 3), // merge at (250, 44) -> ord 2
                (250, 44, 2),
                (251, 44, 2),
                (252, 44, 2),
                (252, 42, 1),
                (254, 42, 7), // merge at (253, 43) -> ord 2
                (253, 43, 0), // -> (253, 44) promotes to ord 3
                (253, 44, 2),
                (254, 44, 2),
                (255, 44, 2), // -> (256, 44) promotes trunk c >= 44 to ord 3
                // Subtree 2 -> ord 3 entering trunk at (256, 56) -> promotes trunk c >= 56 to ord 4:
                (248, 55, 1),
                (248, 57, 3), // merge at (249, 56) -> ord 2
                (249, 56, 2), // -> (250, 56)
                (249, 54, 1),
                (251, 54, 7), // merge at (250, 55) -> ord 2
                (250, 55, 0), // -> (250, 56) promotes to ord 3
                (250, 56, 2),
                (251, 56, 2),
                (252, 56, 2),
                (253, 56, 2),
                (254, 56, 2),
                (255, 56, 2),
            ],
        );
        // Trunk before c=56: make c=40..55 already ord 3 by two merges so (256, 56) gets ord 3 + ord 3 = ord 4!
        let strahler_check = compute_strahler_order(&d8_dir, &is_channel, src, src);
        assert!(
            strahler_check[256 * src + 60] >= 4,
            "synthetic trunk should reach Strahler >= 4, got {}",
            strahler_check[256 * src + 60]
        );

        let proj_in = RawSimProjectionInput {
            src_size: src as u32,
            dx_m: 80.0,
            seed_u64: 999,
            generator_version: 3,
            world_scale: ANMW_V3_WORLD_SCALE,
            total_h_m: &total_h_m,
            moisture: &moisture,
            temp_c: &temp_c,
            discharge_q_m3_s: &discharge_q,
            slope: None,
            river_w_m: &river_w,
            water_kind: &water_kind,
            dominant_biome: &dominant_biome,
            d8_dir: &d8_dir,
            is_channel: &is_channel,
            waterfalls: Vec::new(),
            cave_entrances: Vec::new(),
        };

        let sim = project_raw_to_sim(&proj_in, dst as u32).expect("project 512->64");
        // Verify GS-1 arithmetic mean elevation parity <= 1e-5 against raw f32 block averages.
        for tr in 0..dst {
            for tc in 0..dst {
                let mut raw_sum = 0.0_f64;
                for dr in 0..block {
                    for dc in 0..block {
                        let s_idx = (tr * block + dr) * src + (tc * block + dc);
                        raw_sum += f64::from(normalize_elevation(total_h_m[s_idx]));
                    }
                }
                let raw_mean = (raw_sum / (block * block) as f64) as f32;
                let diff = (sim.elevation[tr * dst + tc] - raw_mean).abs();
                assert!(
                    diff <= 1e-5,
                    "elev parity diff {diff} > 1e-5 at ({tr},{tc})"
                );
            }
        }

        // Row 32 (= 256 / 8) across columns 7..62 is River despite filling only 8/64 = 12.5% (< 25% naive cutoff!).
        let trunk_row = 256 / block;
        let river_cols: Vec<usize> = (7..62)
            .filter(|&tc| sim.biome_22[trunk_row * dst + tc] == BiomeId::River as u8)
            .collect();
        assert_eq!(
            river_cols.len(),
            55,
            "expected continuous 8-connected River reach across all 55 coarse blocks into ocean"
        );
        // Naive >= 25% block count check on block (trunk_row, 10): only 8 / 64 = 12.5% cells were River.
        let mut fine_river_in_block = 0_usize;
        for dr in 0..block {
            for dc in 0..block {
                let s_idx = (trunk_row * block + dr) * src + (10 * block + dc);
                if dominant_biome[s_idx] == BiomeId::River as u8 {
                    fine_river_in_block += 1;
                }
            }
        }
        assert!(
            fine_river_in_block * 4 < block * block,
            "negative-control premise: fine river fraction {}/{}, naive >=25% would drop it",
            fine_river_in_block,
            block * block
        );
        // Ocean mask at col 62 (shallow) -> Ocean (1), col 63 (deep -300m) -> DeepOcean (0).
        assert_eq!(sim.backend_biome_at(trunk_row * dst + 62), 1);
        assert_eq!(sim.backend_biome_at(trunk_row * dst + 63), 0);
    }

    #[cfg(feature = "generate")]
    #[test]
    fn raw_dir_worldgen_projects_deterministically_into_anmw_v3() {
        use crate::config::WorldV2Config;
        use crate::pipeline::run_worldgen_to_dir;

        let dir = std::env::temp_dir().join(format!(
            "anima_anmw_v3_dir_test_{}_{}",
            std::process::id(),
            0x5633_u32
        ));
        let _ = std::fs::remove_dir_all(&dir);

        let cfg = WorldV2Config::for_size(0x5756_3301, 256);
        let _meta = run_worldgen_to_dir(&cfg, &dir).expect("generate 256^2 raw world");

        let sim_a = project_raw_dir_to_sim_256(&dir, 3).expect("project A");
        let sim_b = project_raw_dir_to_sim_256(&dir, 3).expect("project B");
        let bytes_a = sim_a.to_bytes().expect("encode A");
        let bytes_b = sim_b.to_bytes().expect("encode B");
        assert_eq!(bytes_a, bytes_b, "projection must be byte-identical");
        assert_eq!(
            AnmwV3Sim::peek_seed(&bytes_a).unwrap(),
            derive_sim_seed_u32(cfg.seed)
        );
        assert_eq!(sim_a.width, 256);
        assert_eq!(sim_a.height, 256);
        assert_eq!(sim_a.elevation.len(), 256 * 256);
        assert!(sim_a.backend_biomes().into_iter().all(|b| b <= 10));

        let decoded = AnmwV3Sim::from_bytes(&bytes_a).expect("round-trip decode");
        assert_eq!(decoded, sim_a);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
