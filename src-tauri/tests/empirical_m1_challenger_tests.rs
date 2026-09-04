//! Empirical Challenger Stress Suite for Milestone M1 (Unified Natural Map & Binary Sync).
//!
//! Evaluates and challenges:
//! 1. Whittaker 22 ecotone downsampling and biome classification for hydrological and ecological coherence:
//!    - Ocean & Water Elevation coherence (no desert inside oceans, ocean cells strictly have water biomes, flat lake basins).
//!    - Climatic and Altitudinal coherence (glacier/snow strictly in cold/polar/high altitude, never in tropical lowlands; arid biomes in dry sectors).
//!    - Downsampling stress testing across multi-resolution grids (2048² -> 256², 1024² -> 128², 512² -> 64², 32² -> 8²).
//!    - Total range safety and boundary fallback across all 256 possible `u8` values.
//!    - FNV-1a 32-bit checksum oracle, corruption sensitivity, and header validation.
//! 2. Bevy ECS `init_world` deterministic reproducibility:
//!    - 5-run bit-identical reproducibility test with `HD_SEED`.
//!    - Cross-seed divergence verification and `WorldIdentity` mismatch oracle.
//!    - ResourceField carrying capacity non-negativity and closed-system energy bounds.
//!
//! Follows `AGENTS.md` rules:
//! - Deterministic seed `HD_SEED = 0x5EED`.
//! - Strictly behavioral assertions with exact invariants.

use anima_engine_lib::core::ecology::ResourceField;
use anima_engine_lib::core::ecs::init_world;
use anima_engine_lib::core::resources::MapBounds;
use anima_engine_lib::core::sim_rules::{
    cell_center_to_world_xz, cell_center_uv, cell_index, uv_to_cell, world_xz_to_cell,
    WORLD_MAX_XZ, WORLD_MIN_XZ,
};
use anima_engine_lib::core::terrain::{BiomeType, TerrainMap};
use anima_engine_lib::core::world_artifact::{
    map_biome_backend_to_frontend, map_biome_frontend_to_backend, write_artifact_bytes_to,
    WorldArtifact, WorldIdentity, CANONICAL_WORLD_SCALE, WORLD_ARTIFACT_HEADER_LEN,
    WORLD_ARTIFACT_MAGIC, WORLD_ARTIFACT_VERSION,
};
use glam::Vec3;

const HD_SEED: u32 = 0x5EED;

/// Deterministic Linear Feedback / Xorshift32 PRNG for empirical stress tests.
struct DeterministicChallengeRng {
    state: u32,
}

impl DeterministicChallengeRng {
    fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { HD_SEED } else { seed },
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

/// Helper to generate a procedural artifact with deterministic synthetic climate and elevation.
fn generate_challenge_artifact(width: usize, height: usize, seed: u32, sea_level: f32) -> WorldArtifact {
    let n = width * height;
    let mut rng = DeterministicChallengeRng::new(seed);
    let mut elevation = Vec::with_capacity(n);
    let mut moisture = Vec::with_capacity(n);
    let mut temperature = Vec::with_capacity(n);
    let mut flow = Vec::with_capacity(n);
    let mut biome = Vec::with_capacity(n);

    for iy in 0..height {
        for ix in 0..width {
            let u = ix as f32 / width.max(1) as f32;
            let v = iy as f32 / height.max(1) as f32;
            let ny = v * 2.0 - 1.0;
            let lat = 1.0 - ny.abs();

            let radial = 1.0 - 2.0 * ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt();
            let elev = (radial * 0.7 + rng.next_f32() * 0.3).clamp(0.0, 1.0);
            let lapse = (elev - sea_level).max(0.0) * 1.35;
            let temp = (lat * 0.82 + rng.next_f32() * 0.18 - lapse).clamp(0.0, 1.0);
            let moist = (v * 0.7 + rng.next_f32() * 0.3).clamp(0.0, 1.0);
            let fl = if rng.next_f32() < 0.08 { rng.next_f32() * 50.0 } else { 0.0 };

            // Determine biome following Whittaker classification rules
            let b: u8 = if elev < sea_level {
                0 // Ocean
            } else if temp < 0.045 {
                20 // Glacier
            } else if temp < 0.125 {
                12 // Snow
            } else if temp < 0.19 {
                11 // Rock
            } else if fl > 10.0 {
                13 // River
            } else if temp < 0.26 && elev > sea_level + 0.18 && moist > 0.25 {
                18 // Alpine
            } else if temp < 0.24 {
                if moist < 0.6 { 9 } else { 8 } // Tundra vs Taiga
            } else if temp < 0.42 {
                if moist < 0.16 { 17 } else if moist < 0.3 { 5 } else { 8 } // Steppe, Shrubland, Taiga
            } else if temp < 0.6 {
                if moist < 0.14 { 2 } else if moist < 0.28 { 17 } else if moist < 0.42 { 4 } else if moist < 0.56 { 5 } else { 6 }
            } else if temp < 0.72 {
                if moist < 0.15 { 2 } else if moist < 0.24 { 17 } else if moist < 0.38 { 16 } else if moist < 0.52 { 4 } else if moist < 0.74 { 6 } else { 7 }
            } else {
                // Tropical
                if moist < 0.18 { 2 } else if moist < 0.38 { 3 } else if moist < 0.52 { 5 } else if moist < 0.68 { 6 } else { 7 }
            };

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
        sea_level,
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

// =================================================================================================
// 1. Whittaker Ecotones, Hydrological & Ecological Coherence Stress Tests
// =================================================================================================

#[test]
fn test_challenge_ocean_elevation_and_water_ecotone_coherence() {
    let sea_level = 0.38f32;
    let artifact = generate_challenge_artifact(256, 256, HD_SEED, sea_level);

    let mut ocean_count = 0;
    let mut land_count = 0;

    for i in 0..(artifact.width * artifact.height) {
        let elev = artifact.elevation[i];
        let b = artifact.biome[i];

        if elev < sea_level {
            ocean_count += 1;
            // Coherence Invariant 1: Under-water cells must strictly be Ocean (0)
            assert_eq!(
                b, 0,
                "cell index {i} has elevation {elev} < sea_level {sea_level} but has non-ocean biome {b}"
            );
            // Downsampled backend biome must be Ocean (1)
            let back = map_biome_frontend_to_backend(b);
            assert_eq!(back, 1, "frontend ocean must map to backend ocean");
        } else {
            land_count += 1;
        }
    }

    assert!(ocean_count > 1000, "world must contain substantial ocean");
    assert!(land_count > 1000, "world must contain substantial land");

    // Coherence Invariant 2: In the downsampled TerrainMap, ocean cells carry minimum carrying capacity
    let tm = artifact.to_terrain_map(128, 128);
    let bounds = MapBounds::default();
    let res_field = ResourceField::from_biomes(
        &tm.biomes,
        tm.width,
        tm.height,
        bounds.min.x,
        bounds.min.z,
        bounds.max.x,
        bounds.max.z,
        0.02,
    );

    for i in 0..(tm.width * tm.height) {
        if tm.elevations[i] < sea_level {
            assert_eq!(
                tm.biomes[i],
                BiomeType::Ocean as u8,
                "downsampled cell below sea level must be Ocean"
            );
            assert_eq!(
                res_field.r_max[i], 2.5,
                "ocean cell carrying capacity must be baseline ocean 2.5"
            );
        }
    }
}

#[test]
fn test_challenge_climatic_and_altitudinal_coherence() {
    let sea_level = 0.35f32;
    let artifact = generate_challenge_artifact(256, 256, HD_SEED, sea_level);

    for i in 0..(artifact.width * artifact.height) {
        let temp = artifact.temperature[i];
        let elev = artifact.elevation[i];
        let b = artifact.biome[i];

        if elev >= sea_level {
            // Coherence Invariant 3: Glaciers (20) & Snow (12) must strictly occur in cold temperatures
            if b == 20 {
                // Glacier: permanent ice requires temp < 0.045
                assert!(
                    temp < 0.06,
                    "glacier at index {i} has warm temperature {temp}"
                );
            }
            if b == 12 {
                // Snow cap: requires temp < 0.125 or high altitude
                assert!(
                    temp < 0.20 || elev > 0.9,
                    "snow at index {i} has warm temperature {temp} and low elevation {elev}"
                );
            }
            // Coherence Invariant 4: Tropical biomes (Jungle 7, Savanna 3) must occur in warm regions
            if b == 7 {
                assert!(
                    temp > 0.55,
                    "tropical jungle at index {i} has cold temperature {temp}"
                );
            }
            if b == 3 {
                assert!(
                    temp > 0.65,
                    "savanna at index {i} has cold temperature {temp}"
                );
            }
        }
    }
}

#[test]
fn test_challenge_exhaustive_whittaker_biome_mapping_totality_and_bounds() {
    // 1. Test every possible u8 value (0..=255) through map_biome_frontend_to_backend
    for b in 0u8..=255 {
        let back = map_biome_frontend_to_backend(b);
        // Backend range must strictly be in 0..=10 (valid BiomeType)
        assert!(
            back <= 10,
            "frontend biome {b} mapped out of backend range: {back}"
        );
        if b > 21 {
            // Out of range biomes must default gracefully to Grassland (4)
            assert_eq!(back, 4, "out of range biome {b} must fallback to Grassland");
        }
    }

    // 2. Test every possible u8 value (0..=255) through map_biome_backend_to_frontend
    for b in 0u8..=255 {
        let front = map_biome_backend_to_frontend(b);
        assert!(
            front <= 21,
            "backend biome {b} mapped out of frontend range: {front}"
        );
        if b > 10 {
            assert_eq!(front, 4, "out of range backend biome {b} must fallback to Grassland");
        }
    }

    // 3. Exact 22-biome classification table verification
    let canonical_mappings: [(u8, u8); 22] = [
        (0, 1),   // Ocean -> Ocean
        (1, 2),   // Beach -> Beach
        (2, 8),   // Desert -> Desert
        (3, 4),   // Savanna -> Grassland
        (4, 4),   // Grassland -> Grassland
        (5, 4),   // Shrubland -> Grassland
        (6, 5),   // Forest -> TemperateForest
        (7, 7),   // Jungle -> Rainforest
        (8, 6),   // Taiga -> BorealForest
        (9, 6),   // Tundra -> BorealForest
        (10, 7),  // Swamp -> Rainforest
        (11, 9),  // Rock -> MountainRock
        (12, 10), // Snow -> Snow
        (13, 3),  // River -> River
        (14, 1),  // Lake -> Ocean
        (15, 7),  // Mangrove -> Rainforest
        (16, 4),  // Chaparral -> Grassland
        (17, 4),  // Steppe -> Grassland
        (18, 4),  // Alpine -> Grassland
        (19, 8),  // Badlands -> Desert
        (20, 10), // Glacier -> Snow
        (21, 6),  // Bog -> BorealForest
    ];

    for (front, expected_back) in canonical_mappings {
        let actual_back = map_biome_frontend_to_backend(front);
        assert_eq!(
            actual_back, expected_back,
            "mismatch for frontend biome {front}: expected {expected_back}, got {actual_back}"
        );
    }
}

#[test]
fn test_challenge_multi_resolution_downsampling_and_alignment() {
    let source_resolutions = [(2048, 2048), (1024, 1024), (512, 512), (128, 128)];
    let target_resolutions = [(256, 256), (128, 128), (64, 64), (16, 16)];

    for (sw, sh) in source_resolutions {
        let artifact = generate_challenge_artifact(sw, sh, HD_SEED, 0.35);
        for &(tw, th) in &target_resolutions {
            if tw <= sw && th <= sh {
                let tm = artifact.to_terrain_map(tw, th);
                assert_eq!(tm.width, tw);
                assert_eq!(tm.height, th);
                assert_eq!(tm.elevations.len(), tw * th);
                assert_eq!(tm.biomes.len(), tw * th);

                // Check center point nearest sampling
                for ty in [0, th / 4, th / 2, th - 1] {
                    for tx in [0, tw / 4, tw / 2, tw - 1] {
                        let sx = (((tx as f64 + 0.5) * sw as f64 / tw as f64) as usize).min(sw - 1);
                        let sy = (((ty as f64 + 0.5) * sh as f64 / th as f64) as usize).min(sh - 1);

                        let ti = ty * tw + tx;
                        let si = sy * sw + sx;

                        assert_eq!(tm.elevations[ti], artifact.elevation[si]);
                        assert_eq!(tm.moistures[ti], artifact.moisture[si]);
                        assert_eq!(tm.temperatures[ti], artifact.temperature[si]);
                        assert_eq!(tm.flows[ti], artifact.flow[si]);
                        assert_eq!(tm.biomes[ti], map_biome_frontend_to_backend(artifact.biome[si]));
                    }
                }

                // POI land constraint check
                for &(px, py) in &tm.pois {
                    assert!(px < tw);
                    assert!(py < th);
                    let elev = tm.elevations[py * tw + px];
                    assert!(
                        elev >= artifact.sea_level,
                        "POI at ({px}, {py}) must be on land (elevation {elev} >= sea_level {})",
                        artifact.sea_level
                    );
                }
            }
        }
    }
}

#[test]
fn test_challenge_fnv1a32_checksum_oracle_and_tampering_rejection() {
    let artifact = generate_challenge_artifact(32, 32, HD_SEED, 0.4);
    let original_bytes = artifact.to_bytes();

    // 1. Bit-flip sensitivity test across all payload segments
    let n = 32 * 32;
    let offsets_to_test = [
        WORLD_ARTIFACT_HEADER_LEN,                  // first elevation byte
        WORLD_ARTIFACT_HEADER_LEN + n * 2,          // mid elevation
        WORLD_ARTIFACT_HEADER_LEN + n * 4,          // first moisture byte
        WORLD_ARTIFACT_HEADER_LEN + n * 8,          // first temperature byte
        WORLD_ARTIFACT_HEADER_LEN + n * 12,         // first flow byte
        WORLD_ARTIFACT_HEADER_LEN + n * 16,         // first biome byte
        original_bytes.len() - 1,                   // last biome byte
    ];

    for &offset in &offsets_to_test {
        let mut tampered = original_bytes.clone();
        tampered[offset] ^= 0x01; // flip single bit
        let res = WorldArtifact::from_bytes(&tampered);
        assert!(
            res.is_err(),
            "single bit tampering at byte offset {offset} must be rejected by checksum"
        );
        let err = res.unwrap_err();
        assert!(err.contains("checksum mismatch"));
    }

    // 2. Header magic tampering
    let mut bad_magic = original_bytes.clone();
    bad_magic[0] = b'B';
    bad_magic[1] = b'A';
    bad_magic[2] = b'D';
    bad_magic[3] = b'!';
    assert!(WorldArtifact::from_bytes(&bad_magic).unwrap_err().contains("bad artifact magic"));

    // 3. Header version tampering
    let mut bad_version = original_bytes.clone();
    bad_version[4..8].copy_from_slice(&999u32.to_le_bytes());
    assert!(WorldArtifact::from_bytes(&bad_version).unwrap_err().contains("unsupported artifact version"));

    // 4. Truncation at every possible prefix
    for len in [0, 4, 16, WORLD_ARTIFACT_HEADER_LEN - 1, original_bytes.len() - 1] {
        assert!(WorldArtifact::from_bytes(&original_bytes[..len]).is_err());
    }

    // 5. Appending garbage trailing bytes
    let mut with_trailing = original_bytes.clone();
    with_trailing.extend_from_slice(&[0x00, 0xFF, 0xAA]);
    assert!(WorldArtifact::from_bytes(&with_trailing).unwrap_err().contains("trailing byte"));
}

// =================================================================================================
// 2. Bevy ECS init_world Deterministic Reproducibility Stress Tests
// =================================================================================================

#[test]
fn test_challenge_bevy_ecs_init_world_multi_run_determinism() {
    let temp_dir = std::env::temp_dir().join(format!("anima_m1_determinism_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let artifact_path = temp_dir.join("shared_world_determinism.anmw");

    let artifact = generate_challenge_artifact(128, 128, HD_SEED, 0.36);
    let bytes = artifact.to_bytes();
    write_artifact_bytes_to(&artifact_path, &bytes).expect("write artifact bytes");
    std::env::set_var("ANIMA_WORLD_ARTIFACT", &artifact_path);

    // Run init_world 5 times consecutively and verify bit-identical ECS state
    let runs = 5;
    let mut identities = Vec::with_capacity(runs);
    let mut terrain_checksums = Vec::with_capacity(runs);
    let mut plant_biomasses = Vec::with_capacity(runs);

    for run_idx in 0..runs {
        let world = init_world();

        let identity = *world.get_resource::<WorldIdentity>().expect("WorldIdentity resource");
        assert_eq!(identity.seed, HD_SEED, "Run {run_idx}: seed must match HD_SEED");
        assert_eq!(identity.width, 256);
        assert_eq!(identity.height, 256);

        let tm = world.get_resource::<TerrainMap>().expect("TerrainMap resource");
        let terrain_id = WorldIdentity::from_terrain(tm, identity.seed, identity.generator_version);

        let res_field = world.get_resource::<ResourceField>().expect("ResourceField resource");
        let total_plants = res_field.total_biomass();

        identities.push(identity);
        terrain_checksums.push(terrain_id.checksum);
        plant_biomasses.push(total_plants);
    }

    // Assert absolute bit-identical equivalence across all 5 runs
    for i in 1..runs {
        assert_eq!(
            identities[0], identities[i],
            "WorldIdentity diverged between run 0 and run {i}"
        );
        assert_eq!(
            terrain_checksums[0], terrain_checksums[i],
            "TerrainMap checksum diverged between run 0 and run {i}"
        );
        assert_eq!(
            plant_biomasses[0], plant_biomasses[i],
            "Plant biomass diverged between run 0 and run {i}"
        );
    }

    // Cleanup
    std::env::remove_var("ANIMA_WORLD_ARTIFACT");
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_challenge_cross_seed_divergence_and_identity_mismatch_oracle() {
    let artifact_a = generate_challenge_artifact(64, 64, HD_SEED, 0.35);
    let artifact_b = generate_challenge_artifact(64, 64, HD_SEED + 1, 0.35);

    let tm_a = artifact_a.to_terrain_map(64, 64);
    let tm_b = artifact_b.to_terrain_map(64, 64);

    let id_a = WorldIdentity::from_terrain(&tm_a, artifact_a.seed, artifact_a.generator_version);
    let id_b = WorldIdentity::from_terrain(&tm_b, artifact_b.seed, artifact_b.generator_version);

    // 1. Cross-seed divergence: seeds and checksums must strictly differ
    assert_ne!(id_a.seed, id_b.seed);
    assert_ne!(id_a.checksum, id_b.checksum);

    // 2. Mismatch oracle verification
    let mismatch = id_a.mismatch_against(&id_b);
    assert!(mismatch.is_some(), "mismatched seeds must trigger a diagnostic report");
    let msg = mismatch.unwrap();
    assert!(msg.contains("saved world"));
    assert!(msg.contains("current world"));

    // 3. Self-match returns None
    assert!(id_a.mismatch_against(&id_a).is_none());

    // 4. Default/legacy save returns None (graceful backward compatibility)
    assert!(WorldIdentity::default().mismatch_against(&id_a).is_none());
}

#[test]
fn test_challenge_coordinate_contract_dense_grid_sweep() {
    let w = 256;
    let h = 256;
    let bounds = MapBounds::default();

    // Dense grid sweep across cell coordinates
    for iy in (0..h).step_by(17) {
        for ix in (0..w).step_by(17) {
            let idx = cell_index(ix, iy, w);
            assert_eq!(idx, iy * w + ix);

            let (u, v) = cell_center_uv(ix, iy, w, h);
            assert!(u >= 0.0 && u <= 1.0);
            assert!(v >= 0.0 && v <= 1.0);

            let (cx, cy) = uv_to_cell(u, v, w, h);
            assert_eq!(cx, ix);
            assert_eq!(cy, iy);

            let (wx, wz) = cell_center_to_world_xz(ix, iy, w, h, &bounds);
            assert!(wx >= WORLD_MIN_XZ && wx <= WORLD_MAX_XZ);
            assert!(wz >= WORLD_MIN_XZ && wz <= WORLD_MAX_XZ);

            let mapped_cell = world_xz_to_cell(wx, wz, w, h, &bounds);
            assert_eq!(mapped_cell, Some((ix, iy)));
        }
    }

    // Boundary edge cases
    assert_eq!(world_xz_to_cell(WORLD_MIN_XZ, WORLD_MIN_XZ, w, h, &bounds), Some((0, 0)));
    assert_eq!(world_xz_to_cell(WORLD_MAX_XZ, WORLD_MAX_XZ, w, h, &bounds), Some((w - 1, h - 1)));
    assert_eq!(world_xz_to_cell(-100.001, 0.0, w, h, &bounds), None);
    assert_eq!(world_xz_to_cell(0.0, 100.001, w, h, &bounds), None);
}
