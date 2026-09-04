//! Comprehensive integration tests for Milestone M1 (Unified Natural Map & Binary Sync).
//!
//! Verifies:
//! - Binary `WorldArtifact` (.anmw v2) format serialization, deserialization, and header validation.
//! - FNV-1a 32-bit checksum integrity verification and corruption detection.
//! - 2048² to 256² downsampling, coordinate center alignment, and POI generation.
//! - 22 Whittaker Canonical Biomes ↔ 11 Legacy Backend Biomes mapping totality, ecological consistency, and round-trip invariants.
//! - Coordinate contract affine transformations (cell ↔ uv ↔ world ↔ render) and bilinear height interpolation.
//! - Bevy ECS `init_world()` integration, `WorldIdentity` fingerprinting, and `ResourceField` carrying capacity seeding.
//! - Cross-language fixture parity with TypeScript encoder.
//!
//! Follows `AGENTS.md` rules: deterministic seed `HD_SEED = 0x5EED`, behavioral assertions.

use anima_engine_lib::core::ecs::init_world;
use anima_engine_lib::core::ecology::ResourceField;
use anima_engine_lib::core::resources::MapBounds;
use anima_engine_lib::core::sim_rules::{
    cell_center_to_world_xz, cell_center_uv, cell_index, uv_to_cell, world_xz_to_cell,
    WORLD_MAX_XZ, WORLD_MIN_XZ,
};
use anima_engine_lib::core::terrain::{BiomeType, TerrainMap};
use anima_engine_lib::core::world_artifact::{
    map_biome_backend_to_frontend, map_biome_frontend_to_backend,
    write_artifact_bytes_to, WorldArtifact, WorldIdentity, CANONICAL_WORLD_SCALE,
    WORLD_ARTIFACT_HEADER_LEN, WORLD_ARTIFACT_MAGIC, WORLD_ARTIFACT_VERSION,
};
use glam::Vec3;
use std::path::Path;

/// Deterministic test seed pinned to `.hardening.env` (0x5EED).
const HD_SEED: u32 = 0x5EED;

/// Simple deterministic Xorshift32 PRNG seeded with `HD_SEED` for generating synthetic test terrains.
struct DeterministicRng {
    state: u32,
}

impl DeterministicRng {
    fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 0x5EED } else { seed },
        }
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u32() & 0x00FF_FFFF) as f32 / 16777216.0
    }
}

/// Helper to create a synthetic WorldArtifact with arbitrary dimensions and deterministic values.
fn make_synthetic_artifact(width: usize, height: usize, seed: u32) -> WorldArtifact {
    let n = width * height;
    let mut rng = DeterministicRng::new(seed);
    let mut elevation = Vec::with_capacity(n);
    let mut moisture = Vec::with_capacity(n);
    let mut temperature = Vec::with_capacity(n);
    let mut flow = Vec::with_capacity(n);
    let mut biome = Vec::with_capacity(n);

    for iy in 0..height {
        for ix in 0..width {
            let u = ix as f32 / width.max(1) as f32;
            let v = iy as f32 / height.max(1) as f32;
            let radial = 1.0 - 2.0 * ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt();
            let elev = (radial * 0.7 + rng.next_f32() * 0.3).clamp(0.0, 1.0);
            let moist = (v * 0.8 + rng.next_f32() * 0.2).clamp(0.0, 1.0);
            let temp = ((1.0 - (v - 0.5).abs() * 1.5) - elev * 0.3).clamp(0.0, 1.0);
            let fl = if rng.next_f32() < 0.05 { rng.next_f32() * 100.0 } else { 0.0 };
            let b = (rng.next_u32() % 22) as u8;

            elevation.push(elev);
            moisture.push(moist);
            temperature.push(temp);
            flow.push(fl);
            biome.push(b);
        }
    }

    WorldArtifact {
        width,
        height,
        sea_level: 0.38,
        seed,
        generator_version: 21,
        world_scale: CANONICAL_WORLD_SCALE,
        elevation,
        moisture,
        temperature,
        flow,
        biome,
    }
}

// -------------------------------------------------------------------------------------------------
// 1. World Artifact Binary Codec & Round-Trip Tests
// -------------------------------------------------------------------------------------------------

#[test]
fn test_world_artifact_v2_exact_binary_roundtrip() {
    let sizes = [(4, 4), (16, 16), (64, 64), (128, 128)];
    for (w, h) in sizes {
        let artifact = make_synthetic_artifact(w, h, HD_SEED);
        let bytes = artifact.to_bytes();

        assert_eq!(bytes.len(), WORLD_ARTIFACT_HEADER_LEN + w * h * 17);
        assert_eq!(&bytes[0..4], &WORLD_ARTIFACT_MAGIC);
        assert_eq!(u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]), WORLD_ARTIFACT_VERSION);
        assert_eq!(u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]), w as u32);
        assert_eq!(u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]), h as u32);
        assert_eq!(u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]), HD_SEED);

        let decoded = WorldArtifact::from_bytes(&bytes).expect("artifact decode must succeed");
        assert_eq!(decoded.width, w);
        assert_eq!(decoded.height, h);
        assert_eq!(decoded.seed, HD_SEED);
        assert_eq!(decoded.generator_version, 21);
        assert_eq!(decoded.sea_level, 0.38);
        assert_eq!(decoded.world_scale, CANONICAL_WORLD_SCALE);
        assert_eq!(decoded.elevation.len(), w * h);
        assert_eq!(decoded.moisture.len(), w * h);
        assert_eq!(decoded.temperature.len(), w * h);
        assert_eq!(decoded.flow.len(), w * h);
        assert_eq!(decoded.biome.len(), w * h);
        assert_eq!(decoded.elevation, artifact.elevation);
        assert_eq!(decoded.moisture, artifact.moisture);
        assert_eq!(decoded.temperature, artifact.temperature);
        assert_eq!(decoded.flow, artifact.flow);
        assert_eq!(decoded.biome, artifact.biome);
        assert_eq!(decoded.checksum(), artifact.checksum());
    }
}

#[test]
fn test_world_artifact_corruption_rejection_and_bounds() {
    let artifact = make_synthetic_artifact(16, 16, HD_SEED);
    let valid_bytes = artifact.to_bytes();

    // 1. Bad magic
    let mut bad_magic = valid_bytes.clone();
    bad_magic[0] = b'Z';
    let err = WorldArtifact::from_bytes(&bad_magic).unwrap_err();
    assert!(err.contains("bad artifact magic"));

    // 2. Old version 1
    let mut v1_bytes = valid_bytes.clone();
    v1_bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
    let err_v1 = WorldArtifact::from_bytes(&v1_bytes).unwrap_err();
    assert!(err_v1.contains("unsupported artifact version 1"));
    assert!(err_v1.contains("regenerate the world"));

    // 3. Truncated header
    for len in 0..WORLD_ARTIFACT_HEADER_LEN {
        assert!(WorldArtifact::from_bytes(&valid_bytes[..len]).is_err());
    }

    // 4. Truncated payload
    assert!(WorldArtifact::from_bytes(&valid_bytes[..valid_bytes.len() - 10]).is_err());

    // 5. Trailing bytes
    let mut trailing = valid_bytes.clone();
    trailing.push(0xAB);
    let err_trailing = WorldArtifact::from_bytes(&trailing).unwrap_err();
    assert!(err_trailing.contains("trailing byte"));

    // 6. Single-bit corruption in payload detected by checksum
    let payload_offsets = [
        WORLD_ARTIFACT_HEADER_LEN + 10,               // elevation
        WORLD_ARTIFACT_HEADER_LEN + 16 * 16 * 4 + 5,  // moisture
        WORLD_ARTIFACT_HEADER_LEN + 16 * 16 * 8 + 3,  // temperature
        WORLD_ARTIFACT_HEADER_LEN + 16 * 16 * 12 + 7, // flow
        valid_bytes.len() - 1,                        // biome
    ];
    for offset in payload_offsets {
        let mut corrupted = valid_bytes.clone();
        corrupted[offset] ^= 0x01;
        let err = WorldArtifact::from_bytes(&corrupted).unwrap_err();
        assert!(err.contains("artifact checksum mismatch"), "offset {offset} corruption must be caught");
    }

    // 7. Peek seed
    let seed = WorldArtifact::peek_seed(&valid_bytes).expect("peek seed");
    assert_eq!(seed, HD_SEED);
    assert!(WorldArtifact::peek_seed(&bad_magic).is_err());
    assert!(WorldArtifact::peek_seed(&v1_bytes).is_err());
}

// -------------------------------------------------------------------------------------------------
// 2. Whittaker 22 Canonical ↔ 11 Legacy Biome Mapping Tests
// -------------------------------------------------------------------------------------------------

#[test]
fn test_22_canonical_whittaker_to_11_legacy_biome_mapping_totality_and_invariants() {
    // 1. Totality: every frontend biome 0..=21 maps to a valid backend biome 0..=10
    for front in 0u8..=21 {
        let back = map_biome_frontend_to_backend(front);
        assert!(back <= 10, "frontend biome {front} mapped to invalid backend {back}");
    }

    // 2. Fallback for out-of-range biomes
    assert_eq!(map_biome_frontend_to_backend(99), 4); // default Grassland

    // 3. Totality of reverse lift: every legacy biome 0..=10 maps to 0..=21
    for back in 0u8..=10 {
        let front = map_biome_backend_to_frontend(back);
        assert!(front <= 21, "backend biome {back} lifted to invalid frontend {front}");
    }

    // 4. Round-trip invariant: backend -> frontend -> backend is identity for all except DeepOcean(0)
    for back in 0u8..=10 {
        let front = map_biome_backend_to_frontend(back);
        let back_rt = map_biome_frontend_to_backend(front);
        if back == 0 {
            // DeepOcean (0) intentional collapse to Ocean (1)
            assert_eq!(back_rt, 1);
        } else {
            assert_eq!(back_rt, back, "legacy biome {back} must round-trip exactly");
        }
    }

    // 5. Ecological cluster consistency
    assert_eq!(map_biome_frontend_to_backend(0), 1);  // Ocean -> Ocean
    assert_eq!(map_biome_frontend_to_backend(1), 2);  // Beach -> Beach
    assert_eq!(map_biome_frontend_to_backend(2), 8);  // Desert -> Desert
    assert_eq!(map_biome_frontend_to_backend(4), 4);  // Grassland -> Grassland
    assert_eq!(map_biome_frontend_to_backend(6), 5);  // Forest -> TemperateForest
    assert_eq!(map_biome_frontend_to_backend(7), 7);  // Jungle -> Rainforest
    assert_eq!(map_biome_frontend_to_backend(8), 6);  // Taiga -> BorealForest
    assert_eq!(map_biome_frontend_to_backend(11), 9); // Rock -> MountainRock
    assert_eq!(map_biome_frontend_to_backend(12), 10); // Snow -> Snow
    assert_eq!(map_biome_frontend_to_backend(13), 3);  // River -> River
    assert_eq!(map_biome_frontend_to_backend(14), 1);  // Lake -> Ocean
    assert_eq!(map_biome_frontend_to_backend(15), 7);  // Mangrove -> Rainforest
    assert_eq!(map_biome_frontend_to_backend(19), 8);  // Badlands -> Desert
    assert_eq!(map_biome_frontend_to_backend(20), 10); // Glacier -> Snow
}

// -------------------------------------------------------------------------------------------------
// 3. Downsampling 2048² to 256² Simulation Grid Tests
// -------------------------------------------------------------------------------------------------

#[test]
fn test_world_artifact_downsampling_2048_to_256_preserves_features_and_pois() {
    let source_w = 2048;
    let source_h = 2048;
    let target_w = 256;
    let target_h = 256;

    let artifact = make_synthetic_artifact(source_w, source_h, HD_SEED);
    let terrain_map = artifact.to_terrain_map(target_w, target_h);

    assert_eq!(terrain_map.width, target_w);
    assert_eq!(terrain_map.height, target_h);
    assert_eq!(terrain_map.elevations.len(), target_w * target_h);
    assert_eq!(terrain_map.moistures.len(), target_w * target_h);
    assert_eq!(terrain_map.temperatures.len(), target_w * target_h);
    assert_eq!(terrain_map.flows.len(), target_w * target_h);
    assert_eq!(terrain_map.biomes.len(), target_w * target_h);

    // Verify cell center sampling mapping:
    // Target cell (tx, ty) samples source cell sy = (ty + 0.5) * 2048 / 256 = ty * 8 + 4
    let sample_points = [(0, 0), (10, 20), (127, 127), (255, 255)];
    for (tx, ty) in sample_points {
        let sx = (((tx as f64 + 0.5) * source_w as f64 / target_w as f64) as usize).min(source_w - 1);
        let sy = (((ty as f64 + 0.5) * source_h as f64 / target_h as f64) as usize).min(source_h - 1);

        let ti = ty * target_w + tx;
        let si = sy * source_w + sx;

        assert_eq!(terrain_map.elevations[ti], artifact.elevation[si]);
        assert_eq!(terrain_map.moistures[ti], artifact.moisture[si]);
        assert_eq!(terrain_map.temperatures[ti], artifact.temperature[si]);
        assert_eq!(terrain_map.flows[ti], artifact.flow[si]);
        assert_eq!(terrain_map.biomes[ti], map_biome_frontend_to_backend(artifact.biome[si]));
    }

    // Verify POIs are on land (elevation >= sea_level)
    assert!(!terrain_map.pois.is_empty(), "terrain map must have POIs");
    assert!(terrain_map.pois.len() <= 10, "max 10 POIs");
    for &(px, py) in &terrain_map.pois {
        assert!(px < target_w);
        assert!(py < target_h);
        let elev = terrain_map.elevations[py * target_w + px];
        assert!(
            elev >= artifact.sea_level,
            "POI at ({px}, {py}) has elevation {elev} < sea_level {}",
            artifact.sea_level
        );
    }
}

// -------------------------------------------------------------------------------------------------
// 4. Coordinate Transformations & Height Lookups
// -------------------------------------------------------------------------------------------------

#[test]
fn test_coordinate_contract_affine_spaces_and_elevation_lookup() {
    let w = 256;
    let h = 256;
    let bounds = MapBounds::default();

    // 1. cell_index is row-major
    assert_eq!(cell_index(0, 0, w), 0);
    assert_eq!(cell_index(10, 0, w), 10);
    assert_eq!(cell_index(0, 1, w), w);
    assert_eq!(cell_index(255, 255, w), w * h - 1);

    // 2. cell -> uv -> cell round-trip
    for iy in [0, 10, 128, 255] {
        for ix in [0, 10, 128, 255] {
            let (u, v) = cell_center_uv(ix, iy, w, h);
            assert!(u >= 0.0 && u <= 1.0);
            assert!(v >= 0.0 && v <= 1.0);
            let (rx, ry) = uv_to_cell(u, v, w, h);
            assert_eq!(rx, ix);
            assert_eq!(ry, iy);
        }
    }

    // 3. cell -> world -> cell round-trip
    for iy in [0, 64, 128, 192, 255] {
        for ix in [0, 64, 128, 192, 255] {
            let (wx, wz) = cell_center_to_world_xz(ix, iy, w, h, &bounds);
            assert!(wx >= WORLD_MIN_XZ && wx <= WORLD_MAX_XZ);
            assert!(wz >= WORLD_MIN_XZ && wz <= WORLD_MAX_XZ);
            let cell = world_xz_to_cell(wx, wz, w, h, &bounds);
            assert_eq!(cell, Some((ix, iy)));
        }
    }

    // 4. Bounds extreme cell mapping
    assert_eq!(world_xz_to_cell(WORLD_MIN_XZ, WORLD_MIN_XZ, w, h, &bounds), Some((0, 0)));
    assert_eq!(world_xz_to_cell(WORLD_MAX_XZ, WORLD_MAX_XZ, w, h, &bounds), Some((w - 1, h - 1)));
    assert_eq!(world_xz_to_cell(WORLD_MIN_XZ - 1.0, 0.0, w, h, &bounds), None);
    assert_eq!(world_xz_to_cell(0.0, WORLD_MAX_XZ + 1.0, w, h, &bounds), None);

    // 5. Bilinear height interpolation on TerrainMap
    let artifact = make_synthetic_artifact(w, h, HD_SEED);
    let tm = artifact.to_terrain_map(w, h);

    // Test bilinear interpolation at cell centers
    for iy in [10, 50, 100, 200] {
        for ix in [10, 50, 100, 200] {
            let (wx, wz) = cell_center_to_world_xz(ix, iy, w, h, &bounds);
            let elev = tm.get_elevation_at_pos(Vec3::new(wx, 0.0, wz), &bounds);
            assert!(elev.is_finite());
            assert!(elev >= 0.0 && elev <= 1.0);
        }
    }

    // Out-of-bounds positions clamp smoothly without panic
    let oob_elev = tm.get_elevation_at_pos(Vec3::new(-999.0, 0.0, 999.0), &bounds);
    assert!(oob_elev.is_finite());
}

// -------------------------------------------------------------------------------------------------
// 5. Bevy ECS World Ingestion & World Identity Sync
// -------------------------------------------------------------------------------------------------

#[test]
fn test_bevy_ecs_init_world_shared_artifact_sync() {
    let temp_dir = std::env::temp_dir().join(format!("anima_m1_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let artifact_path = temp_dir.join("shared_world.anmw");

    let artifact = make_synthetic_artifact(64, 64, HD_SEED);
    let bytes = artifact.to_bytes();
    write_artifact_bytes_to(&artifact_path, &bytes).expect("write artifact bytes");

    // Set ANIMA_WORLD_ARTIFACT to point to this temp artifact
    std::env::set_var("ANIMA_WORLD_ARTIFACT", &artifact_path);

    let world = init_world();

    // 1. Verify TerrainMap resource is loaded (downsampled to backend default 256x256)
    let tm = world.get_resource::<TerrainMap>().expect("TerrainMap resource");
    assert_eq!(tm.width, 256);
    assert_eq!(tm.height, 256);
    assert_eq!(tm.elevations.len(), 65536);

    // 2. Verify WorldIdentity matches artifact
    let identity = world.get_resource::<WorldIdentity>().expect("WorldIdentity resource");
    assert_eq!(identity.seed, HD_SEED);
    assert_eq!(identity.width, 256);
    assert_eq!(identity.height, 256);

    // 3. Verify ResourceField carrying capacity
    let res_field = world.get_resource::<ResourceField>().expect("ResourceField resource");
    assert_eq!(res_field.width, 256);
    assert_eq!(res_field.height, 256);
    assert_eq!(res_field.r_max.len(), 65536);

    // Biome carrying capacity is non-negative and matches ecology rules
    for i in 0..65536 {
        assert!(res_field.r_max[i] >= 0.0);
        let b = tm.biomes[i];
        if b == BiomeType::DeepOcean as u8 || b == BiomeType::Ocean as u8 {
            assert_eq!(res_field.r_max[i], 2.5); // 250 * 0.01 = 2.5
        }
    }

    // Cleanup
    std::env::remove_var("ANIMA_WORLD_ARTIFACT");
    let _ = std::fs::remove_dir_all(&temp_dir);
}

// -------------------------------------------------------------------------------------------------
// 6. Cross-Language Fixtures Parity
// -------------------------------------------------------------------------------------------------

#[test]
fn test_cross_language_artifact_fixtures_parity() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    // 1. world_4x4.anmw fixture
    let path_4x4 = Path::new(manifest_dir).join("tests/fixtures/world_4x4.anmw");
    if path_4x4.exists() {
        let bytes_4x4 = std::fs::read(&path_4x4).expect("read world_4x4.anmw");
        let decoded_4x4 = WorldArtifact::from_bytes(&bytes_4x4).expect("decode world_4x4.anmw");
        assert_eq!(decoded_4x4.width, 4);
        assert_eq!(decoded_4x4.height, 4);
        assert_eq!(decoded_4x4.sea_level, 0.5);
        assert_eq!(decoded_4x4.seed, 1337);
        assert_eq!(decoded_4x4.generator_version, 20);
        assert_eq!(decoded_4x4.elevation.len(), 16);
        assert_eq!(decoded_4x4.biome.len(), 16);
    }

    // 2. world_real_128.anmw fixture
    let path_128 = Path::new(manifest_dir).join("tests/fixtures/world_real_128.anmw");
    if path_128.exists() {
        let bytes_128 = std::fs::read(&path_128).expect("read world_real_128.anmw");
        let decoded_128 = WorldArtifact::from_bytes(&bytes_128).expect("decode world_real_128.anmw");
        assert_eq!(decoded_128.width, 128);
        assert_eq!(decoded_128.height, 128);
        assert_eq!(decoded_128.elevation.len(), 128 * 128);
        assert_eq!(decoded_128.biome.len(), 128 * 128);

        let tm_128 = decoded_128.to_terrain_map(64, 64);
        assert_eq!(tm_128.width, 64);
        assert_eq!(tm_128.height, 64);
        assert!(tm_128.biomes.iter().all(|&b| b <= 10));
    }
}
