//! Adversarial challenge test suite for Milestone M1 (Unified Natural Map & Binary Sync).
//!
//! Stress-tests:
//! 1. Binary `WorldArtifact` parsing against malformed headers, invalid magic, version divergence,
//!    integer overflows in dimension arithmetic, payload truncations, 1-bit checksum corruptions,
//!    and trailing garbage bytes.
//! 2. Coordinate space conversions (`cell <-> uv <-> world <-> terrain`) at extreme boundary limits,
//!    sub-epsilon offsets, inverted bounds, degenerate single-cell grids, non-square dimensions,
//!    and non-finite float inputs (NaN, Infinity).
//! 3. Totality and robustness of the Whittaker 22 <-> 11 biome mapping under full u8 input space (0..=255).
//! 4. Downsampling and upsampling stress under extreme dimensional ratios (2048x2048 to 1x1, 3x7, 1024x1024)
//!    and extreme terrain distributions (all ocean, all mountain).
//! 5. `WorldIdentity` fingerprint stability, legacy save tolerance, and mismatch sensitivity.
//!
//! Follows `AGENTS.md` rules: deterministic seed `HD_SEED = 0x5EED`, behavioral assertions.

use anima_engine_lib::core::resources::MapBounds;
use anima_engine_lib::core::sim_rules::{
    cell_center_to_world_xz, cell_center_uv, cell_index, uv_to_cell, world_xz_to_cell,
    WORLD_MAX_XZ, WORLD_MAX_Y, WORLD_MIN_XZ, WORLD_MIN_Y,
};
use anima_engine_lib::core::terrain::TerrainMap;
use anima_engine_lib::core::world_artifact::{
    fnv1a_32, map_biome_backend_to_frontend, map_biome_frontend_to_backend, WorldArtifact,
    WorldIdentity, CANONICAL_WORLD_SCALE, WORLD_ARTIFACT_HEADER_LEN, WORLD_ARTIFACT_MAGIC,
    WORLD_ARTIFACT_VERSION,
};
use glam::Vec3;

const HD_SEED: u32 = 0x5EED;

/// Deterministic Xorshift32 PRNG seeded with `HD_SEED` for synthetic data generation.
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

fn create_test_artifact(w: usize, h: usize, seed: u32) -> WorldArtifact {
    let n = w * h;
    let mut rng = DeterministicRng::new(seed);
    let mut elevation = Vec::with_capacity(n);
    let mut moisture = Vec::with_capacity(n);
    let mut temperature = Vec::with_capacity(n);
    let mut flow = Vec::with_capacity(n);
    let mut biome = Vec::with_capacity(n);

    for _ in 0..n {
        elevation.push(rng.next_f32());
        moisture.push(rng.next_f32());
        temperature.push(rng.next_f32());
        flow.push(if rng.next_f32() < 0.1 { rng.next_f32() * 50.0 } else { 0.0 });
        biome.push((rng.next_u32() % 22) as u8);
    }

    WorldArtifact {
        width: w,
        height: h,
        sea_level: 0.4,
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
// 1. Binary WorldArtifact Format & Stress Tests
// =================================================================================================

#[test]
fn test_adversarial_artifact_header_malformations() {
    let valid = create_test_artifact(4, 4, HD_SEED).to_bytes();

    // 1. Magic bytes permutation attacks
    let bad_magics = [
        *b"ANM\0",
        *b"\0NMW",
        *b"anmw",
        *b"WMAN",
        *b"\xFF\xFF\xFF\xFF",
        *b"    ",
    ];
    for bad_m in bad_magics {
        let mut buf = valid.clone();
        buf[0..4].copy_from_slice(&bad_m);
        let err = WorldArtifact::from_bytes(&buf).unwrap_err();
        assert!(err.contains("bad artifact magic"), "must reject magic {bad_m:?}");
        assert!(WorldArtifact::peek_seed(&buf).is_err());
    }

    // 2. Version stress (only version 2 allowed)
    let bad_versions = [0u32, 1, 3, 4, 100, 0x7FFF_FFFF, u32::MAX];
    for v in bad_versions {
        let mut buf = valid.clone();
        buf[4..8].copy_from_slice(&v.to_le_bytes());
        let err = WorldArtifact::from_bytes(&buf).unwrap_err();
        assert!(
            err.contains("unsupported artifact version") && err.contains("regenerate"),
            "must reject version {v} with regenerate instruction"
        );
        assert!(WorldArtifact::peek_seed(&buf).is_err());
    }

    // 3. Zero dimensions
    let zero_dims = [(0u32, 0u32), (0, 4), (4, 0)];
    for (zw, zh) in zero_dims {
        let mut buf = valid.clone();
        buf[8..12].copy_from_slice(&zw.to_le_bytes());
        buf[12..16].copy_from_slice(&zh.to_le_bytes());
        let err = WorldArtifact::from_bytes(&buf).unwrap_err();
        assert!(
            err.contains("zero cells") || err.contains("truncated") || err.contains("overflow"),
            "must reject zero dimension ({zw}x{zh})"
        );
    }

    // 4. Integer multiplication overflow in dimensions
    let overflow_dims = [
        (u32::MAX, 2u32),
        (1 << 30, 4),
        (0x8000_0000, 2),
        (u32::MAX, u32::MAX),
    ];
    for (ow, oh) in overflow_dims {
        let mut buf = valid.clone();
        buf[8..12].copy_from_slice(&ow.to_le_bytes());
        buf[12..16].copy_from_slice(&oh.to_le_bytes());
        let err = WorldArtifact::from_bytes(&buf).unwrap_err();
        assert!(
            err.contains("overflow") || err.contains("truncated"),
            "must safely reject overflow dimensions ({ow}x{oh}) without panic"
        );
    }
}

#[test]
fn test_adversarial_artifact_truncation_and_trailing_sweep() {
    let artifact = create_test_artifact(8, 8, HD_SEED);
    let valid = artifact.to_bytes();
    let total_len = valid.len();
    assert_eq!(total_len, WORLD_ARTIFACT_HEADER_LEN + 64 * 17);

    // 1. Truncation at every single byte from 0 to total_len - 1
    for cut in 0..total_len {
        let truncated = &valid[..cut];
        let res = WorldArtifact::from_bytes(truncated);
        assert!(
            res.is_err(),
            "buffer truncated at byte {cut}/{total_len} must return Err"
        );
    }

    // 2. Trailing garbage from 1 to 128 extra bytes
    for extra in [1, 2, 3, 4, 7, 16, 64, 128] {
        let mut padded = valid.clone();
        padded.extend(std::iter::repeat(0xEE).take(extra));
        let err = WorldArtifact::from_bytes(&padded).unwrap_err();
        assert!(
            err.contains("trailing byte"),
            "extra {extra} bytes must trigger trailing byte rejection"
        );
    }
}

#[test]
fn test_adversarial_checksum_single_bit_and_swapping_corruption() {
    let artifact = create_test_artifact(6, 6, HD_SEED);
    let valid = artifact.to_bytes();

    // 1. Corrupt each byte in the header's checksum field (bytes 32..36)
    for off in 32..36 {
        let mut bad_chk = valid.clone();
        bad_chk[off] ^= 0xFF;
        let err = WorldArtifact::from_bytes(&bad_chk).unwrap_err();
        assert!(err.contains("artifact checksum mismatch"));
    }

    // 2. Corrupt individual bits across all 5 fields
    let n = 36;
    let field_offsets = [
        36,                       // first elevation byte
        36 + n * 2,               // middle elevation byte
        36 + n * 4,               // first moisture byte
        36 + n * 8,               // first temperature byte
        36 + n * 12,              // first flow byte
        36 + n * 16,              // first biome byte
        valid.len() - 1,          // last biome byte
    ];

    for off in field_offsets {
        for bit in 0..8 {
            let mut corrupted = valid.clone();
            corrupted[off] ^= 1 << bit;
            let err = WorldArtifact::from_bytes(&corrupted).unwrap_err();
            assert!(
                err.contains("artifact checksum mismatch"),
                "bit flip at offset {off}, bit {bit} must be caught by checksum"
            );
        }
    }

    // 3. Value swapping (e.g. swap first and second elevation floats)
    let mut swapped = valid.clone();
    let (slice_a, slice_b) = swapped[36..44].split_at_mut(4);
    let tmp = [slice_a[0], slice_a[1], slice_a[2], slice_a[3]];
    slice_a.copy_from_slice(slice_b);
    slice_b.copy_from_slice(&tmp);
    let err = WorldArtifact::from_bytes(&swapped).unwrap_err();
    assert!(err.contains("artifact checksum mismatch"), "swapped values must invalidate checksum");
}

#[test]
fn test_adversarial_artifact_extreme_floats_and_seeds() {
    let mut artifact = create_test_artifact(4, 4, u32::MAX);
    artifact.sea_level = -100.0;
    artifact.world_scale = 1000.0;
    artifact.elevation[0] = f32::MIN_POSITIVE;
    artifact.elevation[1] = 1.0;
    artifact.elevation[2] = 0.0;
    artifact.flow[0] = 10000.0;

    let bytes = artifact.to_bytes();
    let decoded = WorldArtifact::from_bytes(&bytes).expect("extreme valid floats must decode");
    assert_eq!(decoded.seed, u32::MAX);
    assert_eq!(decoded.sea_level, -100.0);
    assert_eq!(decoded.world_scale, 1000.0);
    assert_eq!(decoded.elevation[0], f32::MIN_POSITIVE);
    assert_eq!(decoded.flow[0], 10000.0);

    // Seed 0
    let mut artifact_zero_seed = create_test_artifact(4, 4, 0);
    let bytes_zero = artifact_zero_seed.to_bytes();
    let decoded_zero = WorldArtifact::from_bytes(&bytes_zero).expect("seed 0 must decode");
    assert_eq!(decoded_zero.seed, 0);
    assert_eq!(WorldArtifact::peek_seed(&bytes_zero).unwrap(), 0);
}

// =================================================================================================
// 2. Coordinate Boundaries & Degenerate Geometries
// =================================================================================================

#[test]
fn test_adversarial_coordinate_extreme_boundaries_and_epsilons() {
    let bounds = MapBounds::default(); // min: (-100, -100), max: (100, 100)
    let w = 256;
    let h = 256;

    // 1. Exact boundary values
    assert_eq!(world_xz_to_cell(WORLD_MIN_XZ, WORLD_MIN_XZ, w, h, &bounds), Some((0, 0)));
    assert_eq!(world_xz_to_cell(WORLD_MAX_XZ, WORLD_MIN_XZ, w, h, &bounds), Some((w - 1, 0)));
    assert_eq!(world_xz_to_cell(WORLD_MIN_XZ, WORLD_MAX_XZ, w, h, &bounds), Some((0, h - 1)));
    assert_eq!(world_xz_to_cell(WORLD_MAX_XZ, WORLD_MAX_XZ, w, h, &bounds), Some((w - 1, h - 1)));
    assert_eq!(world_xz_to_cell(0.0, 0.0, w, h, &bounds), Some((w / 2, h / 2)));

    // 2. Positions strictly outside bounds (exceeding f32 ULP for magnitude 100.0)
    let epsilons = [1e-4, 1e-3, 0.01, 0.1, 1.0, 10.0, 100.0];
    for eps in epsilons {
        assert!(world_xz_to_cell(WORLD_MIN_XZ - eps, 0.0, w, h, &bounds).is_none());
        assert!(world_xz_to_cell(WORLD_MAX_XZ + eps, 0.0, w, h, &bounds).is_none());
        assert!(world_xz_to_cell(0.0, WORLD_MIN_XZ - eps, w, h, &bounds).is_none());
        assert!(world_xz_to_cell(0.0, WORLD_MAX_XZ + eps, w, h, &bounds).is_none());
    }

    // 3. Non-finite values (NaN, Infinity) must return None without panic
    let non_finites = [
        (f32::NAN, 0.0),
        (0.0, f32::NAN),
        (f32::NAN, f32::NAN),
        (f32::INFINITY, 0.0),
        (0.0, f32::NEG_INFINITY),
        (f32::INFINITY, f32::INFINITY),
    ];
    for (nx, nz) in non_finites {
        assert!(
            world_xz_to_cell(nx, nz, w, h, &bounds).is_none(),
            "world_xz_to_cell({nx}, {nz}) must be None"
        );
    }
}

#[test]
fn test_adversarial_coordinate_degenerate_bounds_and_single_cell() {
    let w = 256;
    let h = 256;

    // 1. Degenerate bounds: min == max (zero area)
    let zero_bounds = MapBounds {
        min: Vec3::new(10.0, 0.0, 10.0),
        max: Vec3::new(10.0, 10.0, 10.0),
    };
    assert!(
        world_xz_to_cell(10.0, 10.0, w, h, &zero_bounds).is_none(),
        "zero area bounds must return None without divide by zero"
    );

    // 2. Inverted bounds: min > max
    let inverted_bounds = MapBounds {
        min: Vec3::new(100.0, 0.0, 100.0),
        max: Vec3::new(-100.0, 10.0, -100.0),
    };
    assert!(
        world_xz_to_cell(0.0, 0.0, w, h, &inverted_bounds).is_none(),
        "inverted bounds must return None"
    );

    // 3. Single cell grid (1x1)
    let single_bounds = MapBounds::default();
    let (cx, cz) = cell_center_to_world_xz(0, 0, 1, 1, &single_bounds);
    assert_eq!((cx, cz), (0.0, 0.0));
    assert_eq!(world_xz_to_cell(0.0, 0.0, 1, 1, &single_bounds), Some((0, 0)));
    assert_eq!(world_xz_to_cell(WORLD_MIN_XZ, WORLD_MIN_XZ, 1, 1, &single_bounds), Some((0, 0)));
    assert_eq!(world_xz_to_cell(WORLD_MAX_XZ, WORLD_MAX_XZ, 1, 1, &single_bounds), Some((0, 0)));

    // 4. Non-square grids (1x100, 100x1, 3x7, 137x200)
    for &(gw, gh) in &[(1, 100), (100, 1), (3, 7), (137, 200), (2048, 256)] {
        for iy in [0, gh / 2, gh - 1] {
            for ix in [0, gw / 2, gw - 1] {
                let (wx, wz) = cell_center_to_world_xz(ix, iy, gw, gh, &single_bounds);
                let mapped = world_xz_to_cell(wx, wz, gw, gh, &single_bounds);
                assert_eq!(
                    mapped,
                    Some((ix, iy)),
                    "non-square grid {gw}x{gh} cell ({ix},{iy}) must round trip"
                );
            }
        }
    }
}

#[test]
fn test_adversarial_terrain_elevation_interpolation_robustness() {
    let artifact = create_test_artifact(16, 16, HD_SEED);
    let tm = artifact.to_terrain_map(16, 16);
    let bounds = MapBounds::default();

    // Query across extreme out-of-bounds positions
    let test_points = [
        Vec3::new(-1000.0, 0.0, -1000.0),
        Vec3::new(1000.0, 0.0, 1000.0),
        Vec3::new(-100.0, 0.0, 100.0),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(WORLD_MIN_XZ, 0.0, WORLD_MAX_XZ),
    ];

    for pos in test_points {
        let elev = tm.get_elevation_at_pos(pos, &bounds);
        assert!(
            elev.is_finite(),
            "elevation lookup at {pos:?} must yield finite float, got {elev}"
        );
        assert!(
            elev >= 0.0 && elev <= 1.0,
            "elevation lookup at {pos:?} must be clamped in [0,1], got {elev}"
        );
    }
}

// =================================================================================================
// 3. Whittaker 22 <-> 11 Biome Mapping Totality & Range Stress
// =================================================================================================

#[test]
fn test_adversarial_biome_mapping_full_u8_space_totality() {
    // 1. map_biome_frontend_to_backend totality over all 256 u8 values
    for val in 0u8..=255 {
        let backend = map_biome_frontend_to_backend(val);
        assert!(
            backend <= 10,
            "frontend biome {val} produced out-of-bounds backend biome {backend}"
        );
        if val >= 22 {
            assert_eq!(
                backend, 4,
                "out-of-range frontend biome {val} must safely fallback to Grassland(4)"
            );
        }
    }

    // 2. map_biome_backend_to_frontend totality over all 256 u8 values
    for val in 0u8..=255 {
        let frontend = map_biome_backend_to_frontend(val);
        assert!(
            frontend <= 21,
            "backend biome {val} lifted to out-of-bounds frontend biome {frontend}"
        );
        if val >= 11 {
            assert_eq!(
                frontend, 4,
                "out-of-range backend biome {val} must safely fallback to Grassland(4)"
            );
        }
    }
}

// =================================================================================================
// 4. Downsampling & Upsampling Stress & Extreme Terrains
// =================================================================================================

#[test]
fn test_adversarial_downsampling_and_upsampling_extreme_ratios() {
    let artifact = create_test_artifact(2048, 2048, HD_SEED);

    // 1. Extreme downsampling to 1x1
    let tm_1x1 = artifact.to_terrain_map(1, 1);
    assert_eq!(tm_1x1.width, 1);
    assert_eq!(tm_1x1.height, 1);
    assert_eq!(tm_1x1.elevations.len(), 1);
    assert_eq!(tm_1x1.biomes.len(), 1);
    assert!(tm_1x1.biomes[0] <= 10);

    // 2. Non-square downsampling (e.g. 17x31)
    let tm_prime = artifact.to_terrain_map(17, 31);
    assert_eq!(tm_prime.width, 17);
    assert_eq!(tm_prime.height, 31);
    assert_eq!(tm_prime.elevations.len(), 17 * 31);
    assert!(tm_prime.biomes.iter().all(|&b| b <= 10));

    // 3. Upsampling from small to large (4x4 to 128x128)
    let small = create_test_artifact(4, 4, HD_SEED);
    let tm_up = small.to_terrain_map(128, 128);
    assert_eq!(tm_up.width, 128);
    assert_eq!(tm_up.height, 128);
    assert_eq!(tm_up.elevations.len(), 128 * 128);
    assert!(tm_up.biomes.iter().all(|&b| b <= 10));
}

#[test]
fn test_adversarial_all_ocean_and_all_mountain_poi_generation() {
    // 1. All Ocean World (sea_level = 0.9, all elevation = 0.1)
    let mut ocean_artifact = create_test_artifact(32, 32, HD_SEED);
    ocean_artifact.sea_level = 0.9;
    ocean_artifact.elevation.fill(0.1);
    let ocean_tm = ocean_artifact.to_terrain_map(16, 16);
    assert_eq!(
        ocean_tm.pois.len(),
        0,
        "an all-ocean world with no land must have 0 POIs without panicking"
    );

    // 2. All Mountain World (sea_level = 0.2, all elevation = 0.8)
    let mut mountain_artifact = create_test_artifact(32, 32, HD_SEED);
    mountain_artifact.sea_level = 0.2;
    mountain_artifact.elevation.fill(0.8);
    let mountain_tm = mountain_artifact.to_terrain_map(16, 16);
    assert_eq!(
        mountain_tm.pois.len(),
        10,
        "an all-land world must collect the max 10 POIs"
    );
    for &(px, py) in &mountain_tm.pois {
        assert!(px < 16 && py < 16);
    }
}

// =================================================================================================
// 5. WorldIdentity Mismatch Sensitivity & Legacy Tolerances
// =================================================================================================

#[test]
fn test_adversarial_world_identity_mismatch_detection() {
    let artifact = create_test_artifact(16, 16, HD_SEED);
    let base_id = artifact.identity();

    // 1. Exact match -> None
    assert!(base_id.mismatch_against(&base_id).is_none());

    // 2. Legacy save (all zero) -> None
    assert!(WorldIdentity::default().mismatch_against(&base_id).is_none());

    // 3. Seed difference -> Some
    let mut diff_seed = base_id;
    diff_seed.seed ^= 1;
    let err = diff_seed.mismatch_against(&base_id).unwrap();
    assert!(err.contains("saved world") && err.contains("seed"));

    // 4. Generator version difference -> Some
    let mut diff_gen = base_id;
    diff_gen.generator_version += 1;
    let err = diff_gen.mismatch_against(&base_id).unwrap();
    assert!(err.contains("gen"));

    // 5. Dimension difference -> Some
    let mut diff_dim = base_id;
    diff_dim.width = 32;
    let err = diff_dim.mismatch_against(&base_id).unwrap();
    assert!(err.contains("32x"));

    // 6. Checksum difference -> Some
    let mut diff_chk = base_id;
    diff_chk.checksum ^= 0x1234_5678;
    let err = diff_chk.mismatch_against(&base_id).unwrap();
    assert!(err.contains("checksum"));
}
