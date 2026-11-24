//! Neutral, language-agnostic **World Artifact** format — the shared source of truth that unifies
//! the rich frontend generator (`src/components/Landscape/utils/worldGen.ts`, 22 biomes) with the
//! backend simulation world (`terrain.rs`, 11 biomes). Under the chosen A→B direction the frontend
//! generates the authoritative world ONCE and serializes it to this format; the backend loads it at
//! `init_world` and downsamples it into its working `TerrainMap` so the agents live in the SAME world
//! that is rendered — instead of the two sides generating separate, disconnected worlds.
//!
//! The format is a flat little-endian binary blob (NOT bincode, which is Rust-only) so both Rust and
//! TypeScript can read/write it with a few `DataView` / `from_le_bytes` calls. The identical layout is
//! implemented on the frontend in `worldArtifact.ts`; keep the two in sync (a committed fixture +
//! cross-language test guards this).
//!
//! ```text
//! offset  type       field
//! 0       [u8; 4]    magic  = b"ANMW"
//! 4       u32 LE     version (= WORLD_ARTIFACT_VERSION)
//! 8       u32 LE     width
//! 12      u32 LE     height
//! 16      f32 LE     sea_level
//! 20      u32 LE     seed              (world identity)
//! 24      u32 LE     generator_version (which generator produced this world)
//! 28      f32 LE     world_scale       (world-units per axis; canonical 200 = MapBounds extent)
//! 32      u32 LE     checksum          (FNV-1a 32 of the field payload, offset 36..end)
//! 36      f32[n] LE  elevation         (n = width * height)
//! ...     f32[n] LE  moisture
//! ...     f32[n] LE  temperature
//! ...     f32[n] LE  flow
//! ...     u8[n]      biome             (frontend 22-biome enum indices)
//! ```
//!
//! **v2** (this version) added the `seed` / `generator_version` / `world_scale` metadata and the
//! `checksum` integrity field on top of the v1 layout. A v1 artifact is rejected by the v2 decoder
//! with a clear "regenerate the world" message (scenario S09); there is no in-place migration
//! because worlds are cheap to regenerate from their seed.

use crate::core::terrain::TerrainMap;
use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const WORLD_ARTIFACT_MAGIC: [u8; 4] = *b"ANMW";
pub const WORLD_ARTIFACT_VERSION: u32 = 2;
pub const WORLD_ARTIFACT_HEADER_LEN: usize = 36;

/// Canonical world-space extent per axis, in world-units — must equal `sim_rules::WORLD_MAX_XZ -
/// WORLD_MIN_XZ` (200). Stored in the artifact so a consumer knows how to place the normalized grid
/// into world space without a side channel.
pub const CANONICAL_WORLD_SCALE: f32 = 200.0;

/// FNV-1a 32-bit hash over a byte slice. Cheap, dependency-free and byte-identical to the
/// TypeScript implementation in `worldArtifact.ts`, so a checksum computed on one side verifies on
/// the other. Used over the artifact's field payload (everything after the header) to detect
/// corruption before the sim boots on the data.
pub fn fnv1a_32(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for &b in bytes {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

/// Decoded world artifact. Biome indices are in the **frontend 22-biome space**
/// (`Biome` enum in `worldGen.ts`); use [`map_biome_frontend_to_backend`] to project to the
/// backend's 11-variant `BiomeType`.
#[derive(Clone, Debug)]
pub struct WorldArtifact {
    pub width: usize,
    pub height: usize,
    pub sea_level: f32,
    /// World seed that produced this artifact — part of its identity (see [`WorldArtifact::checksum`]).
    pub seed: u32,
    /// Version of the generator (frontend `WORLD_GEN_VERSION`, or the Rust generator under direction B).
    pub generator_version: u32,
    /// World-units per axis; see [`CANONICAL_WORLD_SCALE`].
    pub world_scale: f32,
    pub elevation: Vec<f32>,
    pub moisture: Vec<f32>,
    pub temperature: Vec<f32>,
    pub flow: Vec<f32>,
    pub biome: Vec<u8>,
}

pub mod opt_sha256_hex {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(value: &Option<[u8; 32]>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            let hex = value.as_ref().map(anima_worldgen::anmw_v3::sha256_to_hex);
            hex.serialize(serializer)
        } else {
            value.serialize(serializer)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<[u8; 32]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let opt = Option::<String>::deserialize(deserializer)?;
            opt.map(|hex| {
                anima_worldgen::anmw_v3::sha256_from_hex(&hex).map_err(|e| {
                    serde::de::Error::custom(format!("invalid sim_world_sha256 hex: {e}"))
                })
            })
            .transpose()
        } else {
            Option::<[u8; 32]>::deserialize(deserializer)
        }
    }
}

/// A world's identity fingerprint, persisted in save state so a load re-binds to the SAME world
/// instead of silently regenerating a different one (M1.5 / S08). Stored as a Bevy `Resource` at
/// `init_world` and copied into `SavedSimulationState`; a mismatch on load means the save belongs to
/// a different world. `Default` (all-zero) marks an unknown/legacy save that predates this field.
#[derive(Resource, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct WorldIdentity {
    pub seed: u32,
    pub generator_version: u32,
    pub width: u32,
    pub height: u32,
    pub checksum: u32,
    /// Optional SHA-256 of the raw `sim_256.anmw3` bytes (World v2 `LD-2`); `None` for legacy v1/v2
    /// and headless `TerrainSource::Generated` worlds so JSON fixtures and manifest fingerprints stay
    /// byte-identical.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::core::world_artifact::opt_sha256_hex"
    )]
    pub sim_world_sha256: Option<[u8; 32]>,
}

impl WorldIdentity {
    /// Fingerprint the working `TerrainMap` the simulation actually runs on — one identity domain
    /// regardless of whether the world came from a shared artifact or local generation. The
    /// checksum is FNV-1a over the terrain's elevation + biome bytes (the fields that define where
    /// agents can live), keeping it cheap and deterministic.
    pub fn from_terrain(map: &TerrainMap, seed: u32, generator_version: u32) -> Self {
        let mut payload = Vec::with_capacity(map.elevations.len() * 4 + map.biomes.len());
        for v in &map.elevations {
            payload.extend_from_slice(&v.to_le_bytes());
        }
        payload.extend_from_slice(&map.biomes);
        WorldIdentity {
            seed,
            generator_version,
            width: map.width as u32,
            height: map.height as u32,
            checksum: fnv1a_32(&payload),
            sim_world_sha256: None,
        }
    }

    /// Compare a SAVED identity (`self`) against the CURRENT world's identity. Returns `Some(reason)`
    /// only when `self` is non-default and differs — a default (all-zero) saved identity is a legacy
    /// save that predates world pinning and is never treated as a mismatch (S08). The load path uses
    /// this to warn before dropping saved agents into a different world.
    pub fn mismatch_against(&self, current: &WorldIdentity) -> Option<String> {
        if *self == WorldIdentity::default() || self == current {
            return None;
        }
        let legacy = format!(
            "saved world (seed {}, gen {}, {}x{}, checksum {:#010x}) != current world (seed {}, gen {}, {}x{}, checksum {:#010x})",
            self.seed,
            self.generator_version,
            self.width,
            self.height,
            self.checksum,
            current.seed,
            current.generator_version,
            current.width,
            current.height,
            current.checksum,
        );
        if self.sim_world_sha256.is_none() && current.sim_world_sha256.is_none() {
            Some(legacy)
        } else {
            let fmt_sha = |opt: Option<[u8; 32]>| match opt {
                Some(d) => anima_worldgen::anmw_v3::sha256_to_hex(&d),
                None => "none".to_string(),
            };
            Some(format!(
                "{legacy} [sim_world_sha256 {} != {}]",
                fmt_sha(self.sim_world_sha256),
                fmt_sha(current.sim_world_sha256)
            ))
        }
    }
}

fn read_u32_le(bytes: &[u8], off: usize) -> Result<u32, String> {
    bytes
        .get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| "artifact truncated (u32)".to_string())
}

fn read_f32_le(bytes: &[u8], off: usize) -> Result<f32, String> {
    bytes
        .get(off..off + 4)
        .map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| "artifact truncated (f32)".to_string())
}

fn read_f32_array(bytes: &[u8], off: usize, n: usize) -> Result<Vec<f32>, String> {
    let byte_len = n
        .checked_mul(4)
        .ok_or_else(|| "artifact size overflow".to_string())?;
    let end = off
        .checked_add(byte_len)
        .ok_or_else(|| "artifact size overflow".to_string())?;
    let slice = bytes
        .get(off..end)
        .ok_or_else(|| "artifact truncated (f32 array)".to_string())?;
    let mut out = Vec::with_capacity(n);
    // `byte_len` is `n * 4`, so the remainder `as_chunks` returns is empty.
    for chunk in slice.as_chunks::<4>().0 {
        out.push(f32::from_le_bytes(*chunk));
    }
    Ok(out)
}

impl WorldArtifact {
    /// Read just the seed out of the header, without decoding the field payload.
    ///
    /// Exists because the run's RNG seed must be resolved *before* the ECS world is built (the
    /// evolution thread starts first), and decoding a 2048² artifact to learn one `u32` would be
    /// absurd. Still validates magic, version and header length, so a corrupt file is rejected here
    /// exactly as [`WorldArtifact::from_bytes`] would reject it — it skips the payload checksum
    /// only because it never looks at the payload.
    pub fn peek_seed(bytes: &[u8]) -> Result<u32, String> {
        if bytes.len() < WORLD_ARTIFACT_HEADER_LEN {
            return Err("artifact too short for header".to_string());
        }
        if bytes[0..4] != WORLD_ARTIFACT_MAGIC {
            return Err("bad artifact magic (expected ANMW)".to_string());
        }
        let version = read_u32_le(bytes, 4)?;
        if version == anima_worldgen::anmw_v3::ANMW_V3_VERSION {
            return Err(format!(
                "unsupported ANMW v3 artifact version {} (expected {}): pre-switchover desktop reader refuses ANMW v3 before WV2-7; regenerate or wait for WV2-7",
                version, WORLD_ARTIFACT_VERSION
            ));
        }
        if version != WORLD_ARTIFACT_VERSION {
            return Err(format!(
                "unsupported artifact version {} (expected {}); regenerate the world",
                version, WORLD_ARTIFACT_VERSION
            ));
        }
        read_u32_le(bytes, 20)
    }

    /// Parse a world artifact from its binary bytes. Validates magic, version, length **and the
    /// payload checksum** — a corrupt world is rejected rather than silently booted (S06/S09).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < WORLD_ARTIFACT_HEADER_LEN {
            return Err("artifact too short for header".to_string());
        }
        if bytes[0..4] != WORLD_ARTIFACT_MAGIC {
            return Err("bad artifact magic (expected ANMW)".to_string());
        }
        let version = read_u32_le(bytes, 4)?;
        if version == anima_worldgen::anmw_v3::ANMW_V3_VERSION {
            return Err(format!(
                "unsupported ANMW v3 artifact version {} (expected {}): pre-switchover desktop reader refuses ANMW v3 before WV2-7; regenerate or wait for WV2-7",
                version, WORLD_ARTIFACT_VERSION
            ));
        }
        if version != WORLD_ARTIFACT_VERSION {
            return Err(format!(
                "unsupported artifact version {} (expected {}); regenerate the world",
                version, WORLD_ARTIFACT_VERSION
            ));
        }
        let width = read_u32_le(bytes, 8)? as usize;
        let height = read_u32_le(bytes, 12)? as usize;
        let sea_level = read_f32_le(bytes, 16)?;
        let seed = read_u32_le(bytes, 20)?;
        let generator_version = read_u32_le(bytes, 24)?;
        let world_scale = read_f32_le(bytes, 28)?;
        let checksum = read_u32_le(bytes, 32)?;
        let n = width
            .checked_mul(height)
            .ok_or_else(|| "artifact dimensions overflow".to_string())?;
        if n == 0 {
            return Err("artifact has zero cells".to_string());
        }

        let mut off = WORLD_ARTIFACT_HEADER_LEN;
        let elevation = read_f32_array(bytes, off, n)?;
        off += n * 4;
        let moisture = read_f32_array(bytes, off, n)?;
        off += n * 4;
        let temperature = read_f32_array(bytes, off, n)?;
        off += n * 4;
        let flow = read_f32_array(bytes, off, n)?;
        off += n * 4;
        let biome = bytes
            .get(off..off + n)
            .ok_or_else(|| "artifact truncated (biome array)".to_string())?
            .to_vec();
        let biome_end = off + n;

        // Integrity: recompute the checksum over the exact payload and compare against the header.
        let payload = bytes
            .get(WORLD_ARTIFACT_HEADER_LEN..biome_end)
            .ok_or_else(|| "artifact truncated (payload)".to_string())?;
        let actual = fnv1a_32(payload);
        if actual != checksum {
            return Err(format!(
                "artifact checksum mismatch (header {checksum:#010x}, computed {actual:#010x}); world data is corrupt"
            ));
        }

        // Reject trailing bytes so a padded/corrupt buffer is not silently accepted (the checksum
        // only covers the declared payload). Kept identical to the TS decoder's exact-length check.
        if bytes.len() != biome_end {
            return Err(format!(
                "artifact has {} trailing byte(s) after the payload",
                bytes.len() - biome_end
            ));
        }

        Ok(WorldArtifact {
            width,
            height,
            sea_level,
            seed,
            generator_version,
            world_scale,
            elevation,
            moisture,
            temperature,
            flow,
            biome,
        })
    }

    /// The field payload (elevation, moisture, temperature, flow, biome) as the exact bytes the
    /// checksum is computed over — i.e. everything after the header.
    fn payload_bytes(&self) -> Vec<u8> {
        let n = self.width * self.height;
        let mut payload = Vec::with_capacity(n * 17);
        for v in &self.elevation {
            payload.extend_from_slice(&v.to_le_bytes());
        }
        for v in &self.moisture {
            payload.extend_from_slice(&v.to_le_bytes());
        }
        for v in &self.temperature {
            payload.extend_from_slice(&v.to_le_bytes());
        }
        for v in &self.flow {
            payload.extend_from_slice(&v.to_le_bytes());
        }
        payload.extend_from_slice(&self.biome);
        payload
    }

    /// FNV-1a 32 checksum of this artifact's field payload — its content fingerprint, used as the
    /// world's identity in save state (S08) and for integrity on decode (S06).
    pub fn checksum(&self) -> u32 {
        fnv1a_32(&self.payload_bytes())
    }

    /// This artifact's identity (seed + generator version + dims + payload checksum), for pinning a
    /// save to the world it belongs to (S08).
    pub fn identity(&self) -> WorldIdentity {
        WorldIdentity {
            seed: self.seed,
            generator_version: self.generator_version,
            width: self.width as u32,
            height: self.height as u32,
            checksum: self.checksum(),
            sim_world_sha256: None,
        }
    }

    /// Serialize back to the binary format (used for round-trip tests and, later, backend-produced
    /// artifacts under direction B). Recomputes the payload checksum so serialized bytes are always
    /// self-consistent.
    pub fn to_bytes(&self) -> Vec<u8> {
        let payload = self.payload_bytes();
        let checksum = fnv1a_32(&payload);
        let mut out = Vec::with_capacity(WORLD_ARTIFACT_HEADER_LEN + payload.len());
        out.extend_from_slice(&WORLD_ARTIFACT_MAGIC);
        out.extend_from_slice(&WORLD_ARTIFACT_VERSION.to_le_bytes());
        out.extend_from_slice(&(self.width as u32).to_le_bytes());
        out.extend_from_slice(&(self.height as u32).to_le_bytes());
        out.extend_from_slice(&self.sea_level.to_le_bytes());
        out.extend_from_slice(&self.seed.to_le_bytes());
        out.extend_from_slice(&self.generator_version.to_le_bytes());
        out.extend_from_slice(&self.world_scale.to_le_bytes());
        out.extend_from_slice(&checksum.to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    /// Downsample the (possibly huge, e.g. 2048²) authoritative world into a backend
    /// [`TerrainMap`] of `target_w × target_h`, projecting the 22-biome palette to the backend's 11.
    /// Nearest-neighbour sampling keeps the sim world cheap (its resolution/perf is unchanged) while
    /// its DATA now comes from the shared world. POIs are recomputed as land samples.
    pub fn to_terrain_map(&self, target_w: usize, target_h: usize) -> TerrainMap {
        let tw = target_w.max(1);
        let th = target_h.max(1);
        let tn = tw * th;
        let mut elevations = vec![0.0f32; tn];
        let mut moistures = vec![0.0f32; tn];
        let mut temperatures = vec![0.0f32; tn];
        let mut flows = vec![0.0f32; tn];
        let mut biomes = vec![0u8; tn];

        for ty in 0..th {
            // Sample the source cell nearest the centre of this target cell.
            let sy = (((ty as f64 + 0.5) * self.height as f64 / th as f64) as usize)
                .min(self.height - 1);
            for tx in 0..tw {
                let sx = (((tx as f64 + 0.5) * self.width as f64 / tw as f64) as usize)
                    .min(self.width - 1);
                let si = sy * self.width + sx;
                let ti = ty * tw + tx;
                elevations[ti] = self.elevation[si];
                moistures[ti] = self.moisture[si];
                temperatures[ti] = self.temperature[si];
                flows[ti] = self.flow[si];
                biomes[ti] = map_biome_frontend_to_backend(self.biome[si]);
            }
        }

        // Recompute up to 10 land POIs (elevation above the shared sea level), strided for spread.
        let mut pois: Vec<(usize, usize)> = Vec::with_capacity(10);
        if tn > 0 {
            let stride = (tn / 97).max(1); // coprime-ish stride to scatter samples
            let mut idx = 0usize;
            let mut steps = 0usize;
            while pois.len() < 10 && steps < tn {
                if elevations[idx] >= self.sea_level {
                    pois.push((idx % tw, idx / tw));
                }
                idx = (idx + stride) % tn;
                steps += 1;
            }
        }

        TerrainMap {
            width: tw,
            height: th,
            elevations,
            moistures,
            temperatures,
            biomes,
            flows,
            pois,
        }
    }
}

/// Project a frontend 22-biome index (`Biome` in `worldGen.ts`) onto the backend's 11-variant
/// `BiomeType` (as a `u8`). This is a lossy but ecologically-reasonable collapse; the backend only
/// uses biomes to seed the NPP `ResourceField`, so nearby-productivity biomes fold together.
/// Frontend order: 0 Ocean,1 Beach,2 Desert,3 Savanna,4 Grassland,5 Shrubland,6 Forest,7 Jungle,
/// 8 Taiga,9 Tundra,10 Swamp,11 Rock,12 Snow,13 River,14 Lake,15 Mangrove,16 Chaparral,17 Steppe,
/// 18 Alpine,19 Badlands,20 Glacier,21 Bog.
/// Backend order: 0 DeepOcean,1 Ocean,2 Beach,3 River,4 Grassland,5 TemperateForest,6 BorealForest,
/// 7 Rainforest,8 Desert,9 MountainRock,10 Snow.
pub fn map_biome_frontend_to_backend(front: u8) -> u8 {
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
        9 => 6,   // Tundra      -> BorealForest (nearest cold vegetated; backend lacks tundra)
        10 => 7,  // Swamp       -> Rainforest (wet, high NPP)
        11 => 9,  // Rock        -> MountainRock
        12 => 10, // Snow        -> Snow
        13 => 3,  // River       -> River
        14 => 1,  // Lake        -> Ocean (standing water, ~0 NPP)
        15 => 7,  // Mangrove    -> Rainforest (coastal wet forest)
        16 => 4,  // Chaparral   -> Grassland (dry shrub)
        17 => 4,  // Steppe      -> Grassland
        18 => 4,  // Alpine      -> Grassland (alpine meadow)
        19 => 8,  // Badlands    -> Desert
        20 => 10, // Glacier     -> Snow
        21 => 6,  // Bog         -> BorealForest (cold wetland)
        _ => 4,   // unknown     -> Grassland
    }
}

/// Project a backend 11-variant `BiomeType` index onto a representative frontend 22-biome index
/// (`Biome` in `worldGen.ts`). This is the **forward** legacy→canonical lift (M0.2): it expresses
/// backend-only world data in the shared 22-biome taxonomy, e.g. when the sim world is upsampled
/// back into the authoritative artifact space. It is the near-inverse of
/// [`map_biome_frontend_to_backend`]; the pair round-trips to identity for every legacy biome
/// EXCEPT `DeepOcean`, which the 22-palette has no distinct member for and so intentionally folds
/// into `Ocean`.
/// Backend order: 0 DeepOcean,1 Ocean,2 Beach,3 River,4 Grassland,5 TemperateForest,6 BorealForest,
/// 7 Rainforest,8 Desert,9 MountainRock,10 Snow.
pub fn map_biome_backend_to_frontend(back: u8) -> u8 {
    match back {
        0 => 0,   // DeepOcean       -> Ocean (22-palette has no deep-ocean member)
        1 => 0,   // Ocean           -> Ocean
        2 => 1,   // Beach           -> Beach
        3 => 13,  // River           -> River
        4 => 4,   // Grassland       -> Grassland
        5 => 6,   // TemperateForest -> Forest
        6 => 8,   // BorealForest    -> Taiga
        7 => 7,   // Rainforest      -> Jungle
        8 => 2,   // Desert          -> Desert
        9 => 11,  // MountainRock    -> Rock
        10 => 12, // Snow            -> Snow
        _ => 4,   // unknown         -> Grassland
    }
}

/// Maps a decoded `ANMW v3` simulation bundle (`sim_256.anmw3`) and its exact serialized bytes
/// into the runtime [`TerrainMap`] and dual-pinned [`WorldIdentity`] (`sim_world_sha256 =
/// Some(sha256(sim_bytes))`, `LD-2`).
pub fn terrain_map_from_v3_sim(
    sim: &anima_worldgen::anmw_v3::AnmwV3Sim,
    sim_bytes: &[u8],
) -> Result<(TerrainMap, WorldIdentity), String> {
    sim.validate()
        .map_err(|e| format!("invalid ANMW v3 sim bundle: {e}"))?;
    let terrain = TerrainMap {
        width: sim.width as usize,
        height: sim.height as usize,
        elevations: sim.elevation.clone(),
        moistures: sim.moisture.clone(),
        temperatures: sim.temperature.clone(),
        biomes: sim.backend_biomes(),
        flows: sim.flow.clone(),
        pois: sim.land_pois(10),
    };
    let mut identity =
        WorldIdentity::from_terrain(&terrain, sim.sim_seed_u32, sim.generator_version);
    identity.sim_world_sha256 = Some(anima_worldgen::anmw_v3::compute_sim_world_identity(
        sim_bytes,
    ));
    Ok((terrain, identity))
}

/// Canonical on-disk location of the shared World Artifact: the frontend writes it here during world
/// generation and the simulation reads it at `init_world`. Overridable via `ANIMA_WORLD_ARTIFACT`;
/// defaults to a stable temp path that both the writer (main thread) and the sim thread derive
/// identically without needing a Tauri app handle.
pub fn default_artifact_path() -> PathBuf {
    std::env::var_os("ANIMA_WORLD_ARTIFACT")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("anima_world_artifact.anmw"))
}

/// Fallible counterpart to [`world_seed_from_disk`] that explicitly rejects `ANMW v3` headers
/// before desktop switchover (`WV2-7`) instead of silently falling back to the default seed.
pub fn try_world_seed_from_disk() -> Result<u32, String> {
    let path = default_artifact_path();
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(crate::core::terrain::MapSettings::default().seed);
        }
        Err(e) => {
            return Err(format!(
                "failed to read world artifact at {}: {e}",
                path.display()
            ));
        }
    };
    if anima_worldgen::anmw_v3::is_anmw_v3_header(&bytes) {
        return Err(format!(
            "refusing to read seed from ANMW v3 world artifact at {} before WV2-7 desktop switchover",
            path.display()
        ));
    }
    WorldArtifact::peek_seed(&bytes)
}

/// The world's seed as it can be determined **before** the ECS world is built.
///
/// Mirrors the branch `core::ecs::init_world` takes: a shared artifact on disk supplies the seed,
/// otherwise the local generator's `MapSettings::default().seed` does. Callers that already have a
/// [`WorldIdentity`] must read `identity.seed` instead — this is only for code that runs earlier,
/// such as the evolution thread, which is spawned before the world exists.
///
/// Pre-switchover guard (`WV2-3`): an `ANMW v3` header on disk **panics loudly** instead of
/// silently falling back to the default local seed.
pub fn world_seed_from_disk() -> u32 {
    let path = default_artifact_path();
    match std::fs::read(&path) {
        Ok(bytes) => {
            if anima_worldgen::anmw_v3::is_anmw_v3_header(&bytes) {
                panic!(
                    "refusing to read seed from ANMW v3 world artifact at {} before WV2-7 desktop switchover (pre-switchover loud rejection)",
                    path.display()
                );
            }
            WorldArtifact::peek_seed(&bytes)
                .unwrap_or_else(|_| crate::core::terrain::MapSettings::default().seed)
        }
        Err(_) => crate::core::terrain::MapSettings::default().seed,
    }
}

/// Persist artifact bytes to `path`, creating the parent directory. The payload is validated first so
/// a corrupt upload can never become the shared ground truth the sim boots from.
pub fn write_artifact_bytes_to(path: &Path, bytes: &[u8]) -> Result<(), String> {
    WorldArtifact::from_bytes(bytes)
        .map_err(|e| format!("refusing to write invalid artifact: {e}"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir failed: {e}"))?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("write failed: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // 4x4 world with known values. All values are EXACTLY representable in f32 (multiples of powers
    // of two) so the frontend (JS f64→f32) and backend (f32) produce byte-identical encodings — that
    // is what makes the cross-language fixture test below a strict equality check.
    fn tiny_artifact() -> WorldArtifact {
        let n = 16;
        let elevation: Vec<f32> = (0..n).map(|i| i as f32 * 0.5).collect();
        let moisture: Vec<f32> = (0..n).map(|i| i as f32 * 0.25).collect();
        let temperature: Vec<f32> = (0..n).map(|i| i as f32 * 0.125).collect();
        let flow: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let biome: Vec<u8> = (0..n as u8).collect(); // 0..16 covers a spread of frontend biomes
        WorldArtifact {
            width: 4,
            height: 4,
            sea_level: 0.5,
            seed: 1337,
            generator_version: 20,
            world_scale: CANONICAL_WORLD_SCALE,
            elevation,
            moisture,
            temperature,
            flow,
            biome,
        }
    }

    #[test]
    fn bytes_roundtrip_is_exact() {
        let a = tiny_artifact();
        let bytes = a.to_bytes();
        assert_eq!(bytes.len(), WORLD_ARTIFACT_HEADER_LEN + 16 * 17);
        assert_eq!(&bytes[0..4], b"ANMW");
        let b = WorldArtifact::from_bytes(&bytes).expect("decode");
        assert_eq!(b.width, a.width);
        assert_eq!(b.height, a.height);
        assert_eq!(b.sea_level, a.sea_level);
        assert_eq!(b.seed, a.seed);
        assert_eq!(b.generator_version, a.generator_version);
        assert_eq!(b.world_scale, a.world_scale);
        assert_eq!(b.elevation, a.elevation);
        assert_eq!(b.moisture, a.moisture);
        assert_eq!(b.temperature, a.temperature);
        assert_eq!(b.flow, a.flow);
        assert_eq!(b.biome, a.biome);
        assert_eq!(b.checksum(), a.checksum());
    }

    #[test]
    fn rejects_bad_magic_and_truncation() {
        let mut bytes = tiny_artifact().to_bytes();
        bytes[0] = b'X';
        assert!(WorldArtifact::from_bytes(&bytes).is_err());
        let good = tiny_artifact().to_bytes();
        assert!(WorldArtifact::from_bytes(&good[..good.len() - 5]).is_err());
    }

    #[test]
    fn s09_rejects_corrupt_payload_and_old_version() {
        // A single flipped payload byte must be caught by the checksum (S06 integrity / S09 safety).
        let mut bytes = tiny_artifact().to_bytes();
        let last = bytes.len() - 1; // a biome byte
        bytes[last] ^= 0xFF;
        let err = WorldArtifact::from_bytes(&bytes).unwrap_err();
        assert!(
            err.contains("checksum"),
            "expected checksum error, got: {err}"
        );

        // A v1 artifact (old version tag) is rejected with a regenerate hint, never silently read.
        let mut v1 = tiny_artifact().to_bytes();
        v1[4..8].copy_from_slice(&1u32.to_le_bytes());
        let err = WorldArtifact::from_bytes(&v1).unwrap_err();
        assert!(
            err.contains("version") && err.contains("regenerate"),
            "expected version/regenerate error, got: {err}"
        );

        // Trailing bytes after the declared payload are rejected (padding/corruption).
        let mut padded = tiny_artifact().to_bytes();
        padded.push(0);
        let err = WorldArtifact::from_bytes(&padded).unwrap_err();
        assert!(
            err.contains("trailing"),
            "expected trailing-byte error, got: {err}"
        );
    }

    #[test]
    fn biome_mapping_stays_in_backend_range() {
        for front in 0u8..22 {
            assert!(map_biome_frontend_to_backend(front) <= 10);
        }
        assert_eq!(map_biome_frontend_to_backend(0), 1); // Ocean -> Ocean
        assert_eq!(map_biome_frontend_to_backend(13), 3); // River -> River
        assert_eq!(map_biome_frontend_to_backend(12), 10); // Snow -> Snow
    }

    #[test]
    fn s02_legacy_biome_lift_is_total_and_round_trips() {
        // Forward lift: every legacy backend biome (0..=10) maps into the canonical 22-biome range.
        for back in 0u8..=10 {
            let front = map_biome_backend_to_frontend(back);
            assert!(front < 22, "legacy {back} lifted out of 22-range: {front}");
        }
        // Round-trip legacy -> canonical -> legacy is identity for every legacy biome EXCEPT
        // DeepOcean, which the 22-palette folds into Ocean (documented, intentional collapse).
        for back in 0u8..=10 {
            let rt = map_biome_frontend_to_backend(map_biome_backend_to_frontend(back));
            if back == 0 {
                assert_eq!(rt, 1, "DeepOcean is expected to collapse to Ocean");
            } else {
                assert_eq!(rt, back, "legacy biome {back} must survive the round-trip");
            }
        }
        // The reverse map stays total over the whole 22-biome domain (guards that direction too).
        for front in 0u8..22 {
            assert!(map_biome_frontend_to_backend(front) <= 10);
        }
    }

    #[test]
    fn to_terrain_map_downsamples_and_projects_biomes() {
        let a = tiny_artifact();
        let tm = a.to_terrain_map(2, 2);
        assert_eq!(tm.width, 2);
        assert_eq!(tm.height, 2);
        assert_eq!(tm.elevations.len(), 4);
        assert_eq!(tm.biomes.len(), 4);
        // every projected biome is a valid backend variant
        assert!(tm.biomes.iter().all(|&b| b <= 10));
        // upsampling target larger than source still yields correct dims (nearest sampling)
        let up = a.to_terrain_map(8, 8);
        assert_eq!(up.elevations.len(), 64);
    }

    #[test]
    fn s07_elevation_biome_water_parity_at_standard_points() {
        // M1 gate: at identity resolution the backend TerrainMap reproduces the artifact's
        // elevation, its biome (after the 22→11 projection) and its flow ("water") at a set of
        // standard sample points — the agents read the SAME world that was authored & rendered.
        let a = tiny_artifact(); // 4x4
        let tm = a.to_terrain_map(a.width, a.height);
        let standard: [(usize, usize); 4] = [(0, 0), (3, 0), (0, 3), (3, 3)];
        for (ix, iy) in standard {
            let i = iy * a.width + ix;
            assert_eq!(
                tm.elevations[i], a.elevation[i],
                "elevation parity at ({ix},{iy})"
            );
            assert_eq!(tm.flows[i], a.flow[i], "flow/water parity at ({ix},{iy})");
            assert_eq!(
                tm.biomes[i],
                map_biome_frontend_to_backend(a.biome[i]),
                "biome parity at ({ix},{iy})"
            );
        }
    }

    #[test]
    fn s08_world_identity_is_stable_and_distinguishes_worlds() {
        // Same world → same identity (a save reloads onto the SAME world); a changed world → a
        // different checksum (a mismatched save is detectable).
        let a = tiny_artifact();
        let id1 = a.identity();
        let id2 = a.identity();
        assert_eq!(id1, id2);
        assert_eq!(id1.seed, 1337);
        assert_eq!(id1.checksum, a.checksum());

        let mut b = tiny_artifact();
        b.biome[0] ^= 1; // a different world
        assert_ne!(a.identity().checksum, b.identity().checksum);

        // The TerrainMap-domain fingerprint is likewise deterministic and world-sensitive.
        let tm = a.to_terrain_map(a.width, a.height);
        let t1 = WorldIdentity::from_terrain(&tm, a.seed, a.generator_version);
        let t2 = WorldIdentity::from_terrain(&tm, a.seed, a.generator_version);
        assert_eq!(t1, t2);
        assert_eq!(t1.seed, 1337);

        // Load-side mismatch detection: identical → None; a different world → Some; and a default
        // (legacy) saved identity is never a mismatch, so old saves still load without warnings.
        assert!(id1.mismatch_against(&id1).is_none());
        assert!(b.identity().mismatch_against(&id1).is_some());
        assert!(WorldIdentity::default().mismatch_against(&id1).is_none());
    }

    #[test]
    fn s08_world_identity_serde_roundtrips_and_defaults_for_legacy_saves() {
        let id = tiny_artifact().identity();
        let json = serde_json::to_string(&id).expect("serialize identity");
        let back: WorldIdentity = serde_json::from_str(&json).expect("deserialize identity");
        assert_eq!(id, back);

        // Migration safety: a legacy save JSON that predates the `world_identity` field still loads,
        // yielding the all-zero default (this is exactly how `#[serde(default)]` behaves on the
        // `SavedSimulationState` field).
        #[derive(Deserialize)]
        struct LegacySave {
            #[serde(default)]
            world_identity: WorldIdentity,
        }
        let legacy: LegacySave = serde_json::from_str("{}").expect("legacy save without the field");
        assert_eq!(legacy.world_identity, WorldIdentity::default());
    }

    #[test]
    fn decodes_frontend_generated_fixture() {
        // Cross-language proof (no app run needed): the fixture is written by the TypeScript encoder
        // in `worldArtifact.ts` via `scripts/gen_artifact_fixture.mjs` for the SAME tiny world. If the
        // frontend and backend agree on the byte format, these bytes are byte-identical to the Rust
        // encoder's output and decode back to `tiny_artifact()`.
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/world_4x4.anmw");
        let bytes = std::fs::read(&path).unwrap_or_else(|e| {
            panic!(
                "missing cross-language fixture {}: {e} (regenerate from scripts/gen_artifact_fixture.ts)",
                path.display()
            )
        });
        assert_eq!(
            bytes,
            tiny_artifact().to_bytes(),
            "frontend/backend World Artifact byte layout diverged"
        );
        let decoded = WorldArtifact::from_bytes(&bytes).expect("decode frontend fixture");
        assert_eq!(decoded.width, 4);
        assert_eq!(decoded.height, 4);
        assert_eq!(decoded.sea_level, 0.5);
        assert_eq!(decoded.elevation, tiny_artifact().elevation);
        assert_eq!(decoded.biome, tiny_artifact().biome);
    }

    #[test]
    fn write_to_path_validates_then_reloads() {
        // Per-process: this test wipes the directory on both ends, so a fixed name lets one
        // `cargo test` process delete another's artifact between the write and the read-back.
        let dir =
            std::env::temp_dir().join(format!("anima_test_artifact_wr_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("w.anmw");
        let a = tiny_artifact();
        write_artifact_bytes_to(&path, &a.to_bytes()).expect("write valid artifact");
        let bytes = std::fs::read(&path).expect("read back");
        assert_eq!(
            WorldArtifact::from_bytes(&bytes).unwrap().elevation,
            a.elevation
        );
        // A corrupt payload is refused rather than becoming the shared ground truth.
        assert!(write_artifact_bytes_to(&path, b"not an artifact").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn real_frontend_world_becomes_valid_terrain_map() {
        // End-to-end proof WITHOUT running the app: a REAL 128² world from the frontend generator
        // (worldGen.ts), encoded by the frontend's own worldToArtifact via
        // scripts/gen_artifact_fixture.ts, decodes into a valid backend TerrainMap that has both
        // ocean and land — i.e. the render-side world genuinely feeds the simulation.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/world_real_128.anmw");
        let bytes = std::fs::read(&path).unwrap_or_else(|e| {
            panic!(
                "missing real-world fixture {}: {e} (regenerate from scripts/gen_artifact_fixture.ts)",
                path.display()
            )
        });
        let artifact = WorldArtifact::from_bytes(&bytes).expect("decode real world");
        assert_eq!(artifact.width, 128);
        assert_eq!(artifact.height, 128);
        let tm = artifact.to_terrain_map(128, 128);
        assert_eq!(tm.elevations.len(), 128 * 128);
        assert_eq!(tm.biomes.len(), 128 * 128);
        assert!(
            tm.biomes.iter().all(|&b| b <= 10),
            "all biomes in backend range"
        );
        assert!(tm.biomes.contains(&1), "a real world has ocean");
        assert!(
            tm.biomes.iter().any(|&b| b >= 4),
            "a real world has land biomes"
        );
    }

    #[test]
    fn wv2_3_world_identity_is_copy_omits_none_and_roundtrips_sha256_hex() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<WorldIdentity>();

        let mut legacy = tiny_artifact().identity();
        assert_eq!(legacy.sim_world_sha256, None);
        let legacy_json = serde_json::to_string(&legacy).expect("serialize legacy identity");
        assert!(
            !legacy_json.contains("sim_world_sha256"),
            "sim_world_sha256: None must be omitted in JSON: {legacy_json}"
        );
        let back_legacy: WorldIdentity =
            serde_json::from_str(&legacy_json).expect("deserialize legacy");
        assert_eq!(back_legacy, legacy);

        // Deserializing JSON without `sim_world_sha256` keeps `None`.
        let raw_no_sha =
            r#"{"seed":42,"generator_version":3,"width":256,"height":256,"checksum":7}"#;
        let parsed: WorldIdentity = serde_json::from_str(raw_no_sha).expect("parse legacy json");
        assert_eq!(parsed.sim_world_sha256, None);

        // With Some(sha): human-readable JSON emits 64-char lowercase hex and round-trips.
        let digest = [0xABu8; 32];
        let mut v3 = legacy;
        v3.sim_world_sha256 = Some(digest);
        let v3_json = serde_json::to_string(&v3).expect("serialize v3 identity");
        assert!(v3_json.contains("sim_world_sha256"));
        assert!(v3_json.contains(&"ab".repeat(32)));
        let back_v3: WorldIdentity = serde_json::from_str(&v3_json).expect("deserialize v3");
        assert_eq!(back_v3, v3);

        // Non-human-readable (bincode) also round-trips cleanly.
        let encoded =
            bincode::serde::encode_to_vec(v3, bincode::config::standard()).expect("bincode encode");
        let (decoded, _): (WorldIdentity, usize) =
            bincode::serde::decode_from_slice(&encoded, bincode::config::standard())
                .expect("bincode decode");
        assert_eq!(decoded, v3);

        // Invalid hex length / char is rejected loudly.
        let bad_hex = r#"{"seed":1,"generator_version":1,"width":1,"height":1,"checksum":1,"sim_world_sha256":"zz"}"#;
        assert!(serde_json::from_str::<WorldIdentity>(bad_hex).is_err());

        // `mismatch_against` distinguishes `Some` vs `None` and two distinct digests.
        legacy.sim_world_sha256 = Some([0x11u8; 32]);
        let msg = legacy
            .mismatch_against(&v3)
            .expect("distinct sha256 must trigger mismatch");
        assert!(msg.contains("sim_world_sha256"));
    }

    #[test]
    fn wv2_3_reader_refuses_anmw_v3_header_and_maps_v3_sim_bundle() {
        use anima_worldgen::anmw_v3::{
            compute_sim_world_identity, derive_sim_seed_u32, AnmwV3Sim, ANMW_V3_FLAGS_STANDARD,
            ANMW_V3_SIM_SIZE, ANMW_V3_WORLD_SCALE,
        };

        let n = (ANMW_V3_SIM_SIZE * ANMW_V3_SIM_SIZE) as usize;
        let sim = AnmwV3Sim {
            width: ANMW_V3_SIM_SIZE,
            height: ANMW_V3_SIM_SIZE,
            sea_level_norm: anima_worldgen::SEA_LEVEL_NORM,
            sim_seed_u32: derive_sim_seed_u32(0x1234_5678_9ABC_DEF0),
            seed_u64: 0x1234_5678_9ABC_DEF0,
            generator_version: 3,
            world_scale: ANMW_V3_WORLD_SCALE,
            flags: ANMW_V3_FLAGS_STANDARD,
            elevation: vec![0.4; n],
            moisture: vec![0.5; n],
            temperature: vec![0.6; n],
            flow: vec![0.1; n],
            slope: vec![0.02; n],
            biome_22: vec![4; n], // Grassland -> backend 4
            water_kind: vec![0; n],
            waterfalls: Vec::new(),
            cave_entrances: Vec::new(),
        };
        let bytes = sim.to_bytes().expect("encode v3");
        let peek_err = WorldArtifact::peek_seed(&bytes).expect_err("v2 peek_seed must refuse v3");
        assert!(peek_err.contains("ANMW v3") && peek_err.contains("version"));
        let dec_err = WorldArtifact::from_bytes(&bytes).expect_err("v2 from_bytes must refuse v3");
        assert!(dec_err.contains("ANMW v3") && dec_err.contains("version"));

        let decoded = AnmwV3Sim::from_bytes(&bytes).expect("v3 decode");
        let (tm, wid) = terrain_map_from_v3_sim(&decoded, &bytes).expect("map v3 to TerrainMap");
        assert_eq!(tm.width, 256);
        assert_eq!(tm.height, 256);
        assert_eq!(wid.seed, sim.sim_seed_u32);
        assert_eq!(
            wid.sim_world_sha256,
            Some(compute_sim_world_identity(&bytes))
        );
    }
}
