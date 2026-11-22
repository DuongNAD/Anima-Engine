//! Round 2 hardening & bug-characterization tests (`HD_SEED = 0x5EED`).
//!
//! Covers:
//! - `F-058`: `InterventionCommand::active_at` integer wrap in release mode when `duration_ticks = u64::MAX` (`start_tick > 0`).
//! - `F-059`: `Region::Radius` accepts negative `r` identically to `+r` via `r * r`.
//! - `F-060`: `DynamicFields::step_soil` `f32` leaching subtraction drops `absorbed` for large inputs, breaking nutrient conservation.
//! - Sustained heavy rain + negative `moistures` edge behavior in `DynamicFields`.
//! - Exhaustive boundary & tie-breaking invariants for `terrain_navigation`.

use anima_engine_lib::core::dynamic_fields::{DynamicFields, GROUND_CAPACITY, NITROGEN_CAP};
use anima_engine_lib::core::intervention::{Curve, InterventionCommand, InterventionKind, Region};
use anima_engine_lib::core::resources::MapBounds;
use anima_engine_lib::core::terrain::{BiomeType, MapSettings, TerrainMap};
use anima_engine_lib::core::terrain_navigation::{
    cell_center, is_biome_walkable, is_pos_walkable, is_walkable_cell_next_to_water,
    is_water_biome, sample_walkable_pos, snap_to_nearest_walkable, MAX_WALKABLE_RESAMPLE_TRIES,
};
use glam::Vec3;
use rand::{rngs::StdRng, SeedableRng};

const HD_SEED: u64 = 0x5EED;

fn small_terrain() -> TerrainMap {
    TerrainMap::generate(&MapSettings {
        width: 16,
        height: 16,
        seed: HD_SEED as u32,
        ..MapSettings::default()
    })
}

fn synthetic_terrain(width: usize, height: usize, biomes: Vec<u8>) -> TerrainMap {
    let n = biomes.len();
    TerrainMap {
        width,
        height,
        elevations: vec![0.5; n],
        moistures: vec![0.5; n],
        temperatures: vec![0.5; n],
        biomes,
        flows: vec![0.0; n],
        pois: Vec::new(),
    }
}

#[test]
fn f058_intervention_active_at_avoids_u64_overflow_for_open_ended_commands() {
    let terrain = small_terrain();
    let open_ended = InterventionCommand {
        id: 1,
        cause_id: 1,
        kind: InterventionKind::TemperatureDelta,
        region: Region::Global,
        start_tick: 100,
        duration_ticks: u64::MAX,
        intensity: 0.5,
        signed_negative: false,
        curve: Curve::Step,
        reversible: false,
    };

    // F-058 regression lock (`anima-domain/src/intervention.rs`):
    // `(tick - self.start_tick) < self.effective_duration()` avoids `u64` wrap-around in release
    // mode when `duration_ticks = u64::MAX` and handles `tick == u64::MAX` exactly.
    assert!(open_ended.active_at(100));
    assert!(open_ended.active_at(200));
    assert!(open_ended.active_at(u64::MAX));
    assert_eq!(open_ended.signed_intensity_at(100), 0.5);

    let mut base = DynamicFields::from_terrain(&terrain);
    base.step_climate(100, 0.0, &[]);
    let mut with_overflow = DynamicFields::from_terrain(&terrain);
    with_overflow.step_climate(100, 0.0, &[&open_ended]);
    assert_ne!(
        with_overflow.air_temperature, base.air_temperature,
        "F-058: open-ended intervention must remain active inside DynamicFields"
    );

    // Positive control & exact half-open boundary checks at `u64::MAX`.
    let mut finite_cmd = open_ended.clone();
    finite_cmd.duration_ticks = 1_000;
    assert!(finite_cmd.active_at(100));
    assert_eq!(finite_cmd.signed_intensity_at(100), 0.5);
    let mut with_finite = DynamicFields::from_terrain(&terrain);
    with_finite.step_climate(100, 0.0, &[&finite_cmd]);
    assert_eq!(with_overflow.air_temperature, with_finite.air_temperature);

    let zero_start_max_dur = InterventionCommand {
        start_tick: 0,
        duration_ticks: u64::MAX,
        ..open_ended.clone()
    };
    assert!(!zero_start_max_dur.active_at(u64::MAX));

    let max_start_unit_dur = InterventionCommand {
        start_tick: u64::MAX,
        duration_ticks: 1,
        ..open_ended
    };
    assert!(max_start_unit_dur.active_at(u64::MAX));
}

#[test]
fn f059_region_radius_rejects_non_positive_and_non_finite_radii() {
    let neg = Region::Radius {
        cx: 5.0,
        cy: 5.0,
        r: -4.0,
    };
    let pos = Region::Radius {
        cx: 5.0,
        cy: 5.0,
        r: 4.0,
    };

    // F-059 regression lock (`anima-domain/src/intervention.rs`):
    // `Region::Radius` rejects `r <= 0.0` and non-finite parameters.
    assert!(!neg.contains(5, 5));
    assert!(!neg.contains(9, 5));
    assert!(!neg.contains(10, 5));
    assert!(pos.contains(5, 5));
    assert!(pos.contains(9, 5));
    assert!(!pos.contains(10, 5));

    for invalid in [
        Region::Radius {
            cx: 5.0,
            cy: 5.0,
            r: 0.0,
        },
        Region::Radius {
            cx: 5.0,
            cy: 5.0,
            r: f32::NAN,
        },
        Region::Radius {
            cx: 5.0,
            cy: 5.0,
            r: f32::INFINITY,
        },
        Region::Radius {
            cx: f32::NAN,
            cy: 5.0,
            r: 4.0,
        },
        Region::Radius {
            cx: 5.0,
            cy: f32::INFINITY,
            r: 4.0,
        },
    ] {
        assert!(
            !invalid.contains(5, 5),
            "invalid radius {invalid:?} must not match"
        );
    }

    let terrain = small_terrain();
    let mut base = DynamicFields::from_terrain(&terrain);
    base.step_climate(10, 0.0, &[]);

    let cmd = InterventionCommand {
        id: 2,
        cause_id: 2,
        kind: InterventionKind::TemperatureDelta,
        region: neg,
        start_tick: 0,
        duration_ticks: 100,
        intensity: 0.25,
        signed_negative: false,
        curve: Curve::Step,
        reversible: true,
    };
    let mut perturbed = DynamicFields::from_terrain(&terrain);
    perturbed.step_climate(10, 0.0, &[&cmd]);
    assert_eq!(
        perturbed.air_temperature, base.air_temperature,
        "F-059: negative radius disc must not perturb DynamicFields"
    );
}

#[test]
fn f060_dynamic_fields_step_soil_preserves_nutrient_conservation_with_large_input() {
    let terrain = small_terrain();
    let mut f = DynamicFields::from_terrain(&terrain);
    let cell_count = f.width * f.height;
    assert_eq!(cell_count, 256);

    let initial_stored = f.total_nutrient();
    let huge_cmd = InterventionCommand {
        id: 3,
        cause_id: 3,
        kind: InterventionKind::AddNutrient,
        region: Region::Global,
        start_tick: 0,
        duration_ticks: 10,
        intensity: 1e8,
        signed_negative: false,
        curve: Curve::Step,
        reversible: true,
    };
    f.step_soil(1, &[&huge_cmd]);

    let min_n = f
        .soil_nitrogen
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min);
    assert_eq!(min_n, NITROGEN_CAP, "all cells saturated to NITROGEN_CAP");
    let stored_gain = f.total_nutrient() - initial_stored;
    assert!(
        stored_gain > 1_000.0,
        "pool genuinely gained > 1,000 NM across 256 cells (gained {stored_gain})"
    );

    // F-060 regression lock (`dynamic_fields.rs`):
    // Leached delta computed in `f64` from actual stored increase preserves conservation.
    assert!(f.cum_nutrient_leached > 0.0);
    assert!(
        f.cum_nutrient_leached < f.cum_nutrient_added,
        "leached ({}) must be strictly less than added ({}) when cells absorbed > 1,000 NM",
        f.cum_nutrient_leached,
        f.cum_nutrient_added
    );
    assert!(
        f.nutrient_budget_residual().abs() < 1e-2,
        "F-060: expected single-tick nutrient conservation residual < 1e-2 NM, got {}",
        f.nutrient_budget_residual()
    );

    // Positive control: moderate AddNutrient conserves the budget cleanly.
    let mut ctrl = DynamicFields::from_terrain(&terrain);
    let mod_cmd = InterventionCommand {
        id: 4,
        cause_id: 4,
        kind: InterventionKind::AddNutrient,
        region: Region::Global,
        start_tick: 0,
        duration_ticks: 100,
        intensity: 0.5,
        signed_negative: false,
        curve: Curve::Step,
        reversible: true,
    };
    for tick in 1..=50 {
        ctrl.step_soil(tick, &[&mod_cmd]);
    }
    let scale = (ctrl.initial_nutrient + ctrl.cum_nutrient_added).max(1.0);
    assert!(
        ctrl.nutrient_budget_residual().abs() / scale < 1e-4,
        "moderate AddNutrient must remain conserved"
    );
    assert!(ctrl.cum_nutrient_leached > 0.0);
}

#[test]
fn dynamic_fields_sustained_heavy_rain_conservation_and_negative_moisture_guard() {
    let terrain = small_terrain();
    let mut base = DynamicFields::from_terrain(&terrain);
    let mut storm = DynamicFields::from_terrain(&terrain);
    storm.set_dam(8, 8, true);

    let heavy_rain = InterventionCommand {
        id: 5,
        cause_id: 5,
        kind: InterventionKind::RainfallDelta,
        region: Region::Global,
        start_tick: 0,
        duration_ticks: 1_000,
        intensity: 2.0,
        signed_negative: false,
        curve: Curve::Step,
        reversible: true,
    };

    for tick in 1..=300 {
        let phase = tick as f32 * 0.01;
        base.step_climate(tick, phase, &[]);
        base.step_water();
        base.step_soil(tick, &[]);
        base.step_erosion();

        storm.step_climate(tick, phase, &[&heavy_rain]);
        storm.step_water();
        storm.step_soil(tick, &[]);
        storm.step_erosion();

        let scale = (storm.initial_water + storm.cum_precip).max(1.0);
        let residual_rel = storm.water_budget_residual().abs() / scale;
        assert!(
            residual_rel < 1e-4,
            "heavy rain + dam water budget drifted at tick {tick}: rel={residual_rel}"
        );
    }

    assert!(storm.cum_precip > base.cum_precip * 2.5);
    assert!(storm.total_water() > base.total_water());
    let storm_max_turb = storm.turbidity.iter().copied().fold(0.0f32, f32::max);
    let base_max_turb = base.turbidity.iter().copied().fold(0.0f32, f32::max);
    assert!(storm_max_turb > base_max_turb);

    // Negative `moistures` on custom/corrupted TerrainMap clamps `base_moisture` to 0.0 while
    // `groundwater` initializes negative (`-0.5 * GROUND_CAPACITY`), yielding negative `cum_evap`.
    let mut neg_map = terrain;
    neg_map.moistures.fill(-0.5);
    let mut neg_fields = DynamicFields::from_terrain(&neg_map);
    assert_eq!(neg_fields.base_moisture[0], 0.0);
    assert!(neg_fields.groundwater[0] < -0.5 * GROUND_CAPACITY + 1e-5);
    neg_fields.step_water();
    assert!(
        neg_fields.cum_evap < 0.0,
        "negative groundwater without clamping yields negative evaporation {}",
        neg_fields.cum_evap
    );
}

#[test]
fn terrain_navigation_exhaustive_boundary_and_tiebreak_invariants() {
    // 1. Exhaustive walkability vs water predicates over all BiomeType variants (0..=10).
    for biome_id in 0u8..=10u8 {
        let expect_walkable =
            biome_id != BiomeType::DeepOcean as u8 && biome_id != BiomeType::Ocean as u8;
        let expect_water = biome_id == BiomeType::DeepOcean as u8
            || biome_id == BiomeType::Ocean as u8
            || biome_id == BiomeType::River as u8;
        assert_eq!(
            is_biome_walkable(biome_id),
            expect_walkable,
            "walkability mismatch for biome {biome_id}"
        );
        assert_eq!(
            is_water_biome(biome_id),
            expect_water,
            "water flag mismatch for biome {biome_id}"
        );
    }

    // 2. is_walkable_cell_next_to_water: out-of-bounds, short buffer, ocean, inland, River-adjacent.
    let mut coast_biomes = vec![BiomeType::Grassland as u8; 5 * 5];
    coast_biomes[2 * 5 + 2] = BiomeType::River as u8; // center (2,2) is River (walkable + water)
    coast_biomes[0] = BiomeType::Ocean as u8; // corner (0,0) is Ocean
    let tm = synthetic_terrain(5, 5, coast_biomes);
    assert!(!is_walkable_cell_next_to_water(&tm, 5, 0));
    assert!(!is_walkable_cell_next_to_water(&tm, 0, 5));
    assert!(!is_walkable_cell_next_to_water(&tm, 0, 0));
    assert!(is_walkable_cell_next_to_water(&tm, 1, 1));
    assert!(is_walkable_cell_next_to_water(&tm, 2, 1));
    assert!(!is_walkable_cell_next_to_water(&tm, 4, 4));

    let short_tm = synthetic_terrain(5, 5, vec![BiomeType::Grassland as u8; 24]);
    assert!(!is_walkable_cell_next_to_water(&short_tm, 1, 1));
    assert_eq!(
        snap_to_nearest_walkable(&short_tm, &MapBounds::default(), Vec3::ZERO),
        None
    );

    // 3. Deterministic `(d2, row, col)` tie-break & radius early-break optimality in snap_to_nearest_walkable.
    let bounds = MapBounds {
        min: Vec3::new(0.0, -10.0, 0.0),
        max: Vec3::new(70.0, 10.0, 70.0),
    };
    let mut ring_biomes = vec![BiomeType::DeepOcean as u8; 7 * 7];
    // Four orthogonal land cells around (col=3, row=3) all at distance 1 (d2 = 1):
    // (3, 2) has lowest row (=2), so it must win over (2, 3), (4, 3), and (3, 4).
    ring_biomes[2 * 7 + 3] = BiomeType::Grassland as u8;
    ring_biomes[3 * 7 + 2] = BiomeType::Grassland as u8;
    ring_biomes[3 * 7 + 4] = BiomeType::Grassland as u8;
    ring_biomes[4 * 7 + 3] = BiomeType::Grassland as u8;
    let ring_tm = synthetic_terrain(7, 7, ring_biomes);
    let probe = cell_center(&ring_tm, &bounds, 3, 3) + Vec3::new(0.0, 4.25, 0.0);
    let snapped = snap_to_nearest_walkable(&ring_tm, &bounds, probe).expect("snaps to ring");
    let expected = cell_center(&ring_tm, &bounds, 3, 2);
    assert_eq!(snapped.x, expected.x);
    assert_eq!(snapped.z, expected.z);
    assert_eq!(snapped.y, 4.25, "snap must preserve input y coordinate");

    // Diagonal vs orthogonal early-stop check: (col=1, row=1) is at d2=8 (radius=2), while
    // (col=3, row=0) is at d2=9 (radius=3); AND (col=1, row=1) d2=8 vs (col=5, row=3) d2=4:
    // radius=2 ring discovers BOTH (1,1) d2=8 and (5,3) d2=4 and picks strictly smaller d2=4.
    let mut rad_biomes = vec![BiomeType::DeepOcean as u8; 7 * 7];
    rad_biomes[7 + 1] = BiomeType::Grassland as u8; // (dc=2, dr=2) -> d2 = 8
    rad_biomes[3 * 7 + 5] = BiomeType::Grassland as u8; // (dc=2, dr=0) -> d2 = 4
    let rad_tm = synthetic_terrain(7, 7, rad_biomes);
    let rad_snapped =
        snap_to_nearest_walkable(&rad_tm, &bounds, cell_center(&rad_tm, &bounds, 3, 3))
            .expect("snaps to nearest d2=4");
    let expected_ortho = cell_center(&rad_tm, &bounds, 5, 3);
    assert_eq!(rad_snapped.x, expected_ortho.x);
    assert_eq!(rad_snapped.z, expected_ortho.z);

    // Degenerate bounds check returns None safely.
    let zero_bounds = MapBounds {
        min: Vec3::ZERO,
        max: Vec3::ZERO,
    };
    assert_eq!(
        snap_to_nearest_walkable(&rad_tm, &zero_bounds, Vec3::ZERO),
        None
    );
    assert!(!is_pos_walkable(&rad_tm, &zero_bounds, Vec3::ZERO));

    // 4. Seeded sample_walkable_pos hits walkable land and returns None on all-ocean.
    let mut rng = StdRng::seed_from_u64(HD_SEED);
    let sampled = sample_walkable_pos(&mut rng, &tm, &bounds, MAX_WALKABLE_RESAMPLE_TRIES)
        .expect("mostly grassland terrain samples walkable pos cleanly");
    assert!(
        is_pos_walkable(&tm, &bounds, sampled),
        "sampled position {sampled:?} must be walkable"
    );

    let all_ocean = synthetic_terrain(4, 4, vec![BiomeType::DeepOcean as u8; 16]);
    assert_eq!(
        sample_walkable_pos(&mut rng, &all_ocean, &bounds, MAX_WALKABLE_RESAMPLE_TRIES),
        None
    );
}
