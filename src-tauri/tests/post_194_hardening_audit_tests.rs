//! Post-PR-#194 hardening & real-bug audit tests (`HD_SEED = 0x5EED`).
//!
//! Focuses on interactions across recent trunk changes (`F-053` windowed observables,
//! `live.grass_steering` + `live.land_mask`, combined post-#194 snapshot restore + closed-EU,
//! `terrain_navigation` non-finite root/segment recovery, and `W2-lite` preflight vs genesis).

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::commands::{
    parse_and_validate_manifest_for_viewer, start_from_manifest_with_engine, EvolutionSettings,
    MapElitesGridState,
};
use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::components::{
    Agent, Food, InertiaComponent, LastWalkablePosition, ParentAgent, Position, Prey, Velocity,
};
use anima_engine_lib::core::ecology::ResourceField;
use anima_engine_lib::core::ecs::init_world;
use anima_engine_lib::core::experiment::{
    ExperimentError, ExperimentManifest, InitialConditionSet, ObservableRegistry, ObserverPolicy,
    WorldLawSet, MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{run_manifest_seed, ExperimentModel, RunStatus};
use anima_engine_lib::core::live_experiment::{
    all_live_observable_ids, try_apply_live_genesis_and_policies, LiveExperimentAdapter,
    LiveSnapshot, LIVE_KEY_DRIFT_CONTROL, LIVE_KEY_DRIFT_RESET_TICKS, LIVE_KEY_DRIVE_FLOOR,
    LIVE_KEY_EVOLVED_BRAINS, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS, LIVE_KEY_FOUNDER_LAYOUT,
    LIVE_KEY_GRASS_STEERING, LIVE_KEY_LAKES, LIVE_KEY_LAKE_LAYOUT, LIVE_KEY_LAND_MASK,
    LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY, LIVE_KEY_PLANT_RESERVE, LIVE_KEY_PREDATOR_FRACTION,
    LIVE_KEY_PREY_CAP, LIVE_KEY_REPRODUCTION, LIVE_KEY_STRICT_PREY_CAP, LIVE_KEY_THIRST_STEERING,
    LIVE_KEY_TREES, LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::locomotion::steering_drive_system;
use anima_engine_lib::core::resources::{
    GrassSteeringPolicy, LocomotionPolicy, MapBounds, PlantReservePolicy, TerrainNavigation,
    VitalStatistics,
};
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::core::simulation_loop::SimulationEngine;
use anima_engine_lib::core::snapshot::{self, SnapshotEnvelope};
use anima_engine_lib::core::terrain::{BiomeType, TerrainMap};
use anima_engine_lib::core::terrain_navigation::{
    is_pos_walkable, snap_to_nearest_walkable, terrain_navigation_system, wrap_pos,
};
use anima_engine_lib::core::world_artifact::{WorldArtifact, WorldIdentity, CANONICAL_WORLD_SCALE};
use anima_engine_lib::core::world_systems::wrap_coordinates_system;
use anima_engine_lib::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};
use anima_engine_lib::physics::dynamics::{integrate_physics_system, RigidBody};
use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;
use glam::Vec3;
use rand::{rngs::StdRng, SeedableRng};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const HD_SEED: u64 = 0x5EED;
static SNAP_SEQ: AtomicU64 = AtomicU64::new(0);

fn initial(pairs: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(pairs.iter().map(|(k, v)| ((*k).to_string(), *v)).collect())
}

fn obs(o: &[(String, f64)], id: &str) -> f64 {
    o.iter()
        .find(|(k, _)| k == id)
        .unwrap_or_else(|| panic!("missing observable {id}"))
        .1
}

fn through_disk_snapshot(snap: &LiveSnapshot, label: &str) -> LiveSnapshot {
    let seq = SNAP_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "anima_post194_{}_{}_{}",
        std::process::id(),
        seq,
        label
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir for checkpoint");
    let path = dir.join("checkpoint.json");
    let envelope = SnapshotEnvelope::seal(snap.saved.clone()).expect("seal snapshot");
    snapshot::write_atomic(&path, &envelope).expect("write atomic snapshot");
    let saved = snapshot::read(&path).expect("read snapshot back");
    let _ = std::fs::remove_dir_all(&dir);
    LiveSnapshot {
        saved,
        config: snap.config,
        seed: snap.seed,
    }
}

fn split_coast_terrain_and_field(side: usize, bounds: &MapBounds) -> (TerrainMap, ResourceField) {
    let n = side * side;
    let mut biomes = vec![BiomeType::Grassland as u8; n];
    let mut r = vec![10.0f32; n];
    let mut r_max = vec![10.0f32; n];
    for row in 0..side {
        for col in (side / 2)..side {
            let idx = row * side + col;
            biomes[idx] = BiomeType::DeepOcean as u8;
            r[idx] = 0.0;
            r_max[idx] = 0.0;
        }
    }
    let tm = TerrainMap {
        width: side,
        height: side,
        elevations: vec![0.5; n],
        moistures: vec![0.5; n],
        temperatures: vec![0.5; n],
        biomes,
        flows: vec![0.0; n],
        pois: Vec::new(),
    };
    let field = ResourceField {
        r,
        r_max,
        width: side,
        height: side,
        min_x: bounds.min.x,
        min_z: bounds.min.z,
        max_x: bounds.max.x,
        max_z: bounds.max.z,
        growth_rate: 0.0,
        regrowth_phase: 0,
    };
    (tm, field)
}

#[test]
fn f053_final_observables_drains_all_five_windowed_vital_metrics_on_aligned_duration() {
    let observable_ids: Vec<String> = all_live_observable_ids()
        .into_iter()
        .map(str::to_string)
        .collect();
    let base_init = initial(&[
        (LIVE_KEY_FOUNDERS, 30.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.0),
        (LIVE_KEY_TREES, 4.0),
        (LIVE_KEY_LAKES, 2.0),
        (LIVE_KEY_MORTALITY, 1.0),
        (LIVE_KEY_REPRODUCTION, 1.0),
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_DRIVE_FLOOR, 0.5),
        (LIVE_KEY_THIRST_STEERING, 1.0),
    ]);
    let registry = ObservableRegistry::live_default();

    let aligned_manifest = ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "f053-aligned-audit".to_string(),
        name: "f053-aligned-audit".to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: base_init.clone(),
        interventions: vec![],
        seeds: vec![HD_SEED],
        duration_ticks: 3_000,
        sample_period: 60,
        observable_ids: observable_ids.clone(),
        exotic_interventions: vec![],
        observer: ObserverPolicy::default(),
    };
    let aligned = run_manifest_seed::<LiveExperimentAdapter>(
        &aligned_manifest,
        &registry,
        HD_SEED,
        None,
        None,
    );
    assert_eq!(
        aligned.status,
        RunStatus::Completed,
        "{:?}",
        aligned.warnings
    );

    let sum_births: f64 = aligned
        .series
        .iter()
        .map(|s| obs(&s.observables, "live.births"))
        .sum();
    let sum_deaths: f64 = aligned
        .series
        .iter()
        .map(|s| obs(&s.observables, "live.deaths"))
        .sum();
    let sum_starve: f64 = aligned
        .series
        .iter()
        .map(|s| obs(&s.observables, "live.deaths_starvation"))
        .sum();
    let sum_dehyd: f64 = aligned
        .series
        .iter()
        .map(|s| obs(&s.observables, "live.deaths_dehydration"))
        .sum();
    let max_lifespan: f64 = aligned
        .series
        .iter()
        .map(|s| obs(&s.observables, "live.mean_lifespan"))
        .fold(0.0, f64::max);

    assert!(
        sum_births > 0.0,
        "non-vacuity: series must record births ({sum_births})"
    );
    assert!(
        sum_deaths > 0.0,
        "non-vacuity: series must record deaths ({sum_deaths})"
    );
    assert!(
        sum_starve + sum_dehyd > 0.0,
        "non-vacuity: series must record starvation/dehydration cause deaths"
    );
    assert!(
        max_lifespan > 0.0,
        "non-vacuity: series must record positive mean_lifespan ({max_lifespan})"
    );

    // Find a trailing interval from the same run where both births and deaths occurred so an
    // unaligned control proves the metrics are genuinely non-zero over identical simulation ticks.
    assert!(
        aligned.series.iter().any(|s| s.tick > 2_000
            && obs(&s.observables, "live.births") > 0.0
            && obs(&s.observables, "live.deaths") > 0.0
            && obs(&s.observables, "live.mean_lifespan") > 0.0),
        "series must contain births and deaths in (2000, 3000]"
    );

    // F-053 legacy non-regression: on aligned runs (`duration_ticks % sample_period == 0`), the
    // five legacy sample-windowed vital observables keep their exact bit-for-bit behavior (0.0 in
    // `final_observables` after the final sample tick drains `VitalStatistics::sample_window()`),
    // preserving registered E2/E3/E-SEL1/SEL2-P artifacts.
    for metric in [
        "live.births",
        "live.deaths",
        "live.mean_lifespan",
        "live.deaths_starvation",
        "live.deaths_dehydration",
    ] {
        assert_eq!(
            obs(&aligned.final_observables, metric),
            0.0,
            "F-053 legacy semantics preserved: aligned run drains final_observables[{metric}] to 0.0"
        );
    }

    // F053-FIX: the explicitly [0, tick] cumulative lifespan observable is NEVER drained by
    // sample_window() and reports the exact death-weighted cumulative mean lifespan.
    let aligned_cum_lifespan = obs(&aligned.final_observables, "live.cumulative_mean_lifespan");
    assert!(
        aligned_cum_lifespan > 0.0,
        "F053-FIX: aligned final_observables[live.cumulative_mean_lifespan] must be > 0.0"
    );
    let weighted_lifespan_sum: f64 = aligned
        .series
        .iter()
        .map(|s| obs(&s.observables, "live.deaths") * obs(&s.observables, "live.mean_lifespan"))
        .sum();
    let expected_cum_lifespan = weighted_lifespan_sum / sum_deaths;
    assert!(
        (aligned_cum_lifespan - expected_cum_lifespan).abs() <= 1e-3,
        "F053-FIX: aligned cumulative_mean_lifespan ({aligned_cum_lifespan}) must match death-weighted series mean ({expected_cum_lifespan})"
    );

    // Positive control: identical seed and identical duration_ticks = 3000 with unaligned
    // sample_period = 2000 leaves the trailing (2000, 3000] window open until final_observables.
    let mut unaligned_manifest = aligned_manifest;
    unaligned_manifest.sample_period = 2_000;
    let unaligned = run_manifest_seed::<LiveExperimentAdapter>(
        &unaligned_manifest,
        &registry,
        HD_SEED,
        None,
        None,
    );
    assert_eq!(
        unaligned.status,
        RunStatus::Completed,
        "{:?}",
        unaligned.warnings
    );
    assert_eq!(
        unaligned.final_checksum, aligned.final_checksum,
        "sampling cadence must not alter the simulation trajectory"
    );
    assert!(
        obs(&unaligned.final_observables, "live.births") > 0.0
            && obs(&unaligned.final_observables, "live.deaths") > 0.0
            && obs(&unaligned.final_observables, "live.mean_lifespan") > 0.0
            && (obs(&unaligned.final_observables, "live.deaths_starvation")
                + obs(&unaligned.final_observables, "live.deaths_dehydration"))
                > 0.0,
        "unaligned run preserves non-zero vital metrics in final_observables ({:?})",
        unaligned.final_observables
    );
    assert_eq!(
        aligned_cum_lifespan.to_bits(),
        obs(
            &unaligned.final_observables,
            "live.cumulative_mean_lifespan"
        )
        .to_bits(),
        "F053-FIX: live.cumulative_mean_lifespan must be invariant to sampling cadence"
    );
}

#[test]
fn grass_steering_with_land_mask_steers_prey_into_ocean_food_instead_of_grazing_land_grass() {
    let bounds = MapBounds {
        min: Vec3::new(-50.0, -50.0, -50.0),
        max: Vec3::new(50.0, 50.0, 50.0),
    };
    let (tm, field) = split_coast_terrain_and_field(10, &bounds);
    let cell_size = 10.0;

    // Priority 2 check: prey sits on walkable rich grassland (-5, 0, 5) well above
    // GRASS_STAY_FRACTION * r_max (10.0 >= 2.5), while a Food item lies across the coast on
    // non-walkable DeepOcean (+5, 0, 5) within local_food_radius (dist 10 <= 35).
    let mut world = World::new();
    world.insert_resource(bounds);
    world.insert_resource(TimeStep(1.0));
    world.insert_resource(tm);
    world.insert_resource(field);
    world.insert_resource(TerrainNavigation { enabled: true });
    world.insert_resource(GrassSteeringPolicy { enabled: true });
    world.insert_resource(PlantReservePolicy { fraction: 0.0 });
    world.insert_resource(LocomotionPolicy {
        enabled: true,
        drive_floor: 1.0,
        thirst_steering: false,
    });

    let land_start = Vec3::new(-0.5 * cell_size, 0.0, 0.5 * cell_size); // col 4, row 5 (Grassland)
    assert!(is_pos_walkable(
        world.resource::<TerrainMap>(),
        &bounds,
        land_start
    ));

    let prey = world
        .spawn((
            Agent,
            Prey,
            Position(land_start),
            Velocity(Vec3::ZERO),
            RigidBody {
                mass: 1.0,
                velocity: Vec3::ZERO,
                force: Vec3::ZERO,
            },
            HomeostaticState {
                energy: 50.0,
                energy_target: 100.0,
                hydration: 100.0,
                hydration_target: 100.0,
                ..Default::default()
            },
            InertiaComponent::default(),
            LastWalkablePosition::new(land_start),
        ))
        .id();

    let ocean_food_local = Vec3::new(0.5 * cell_size, 0.0, 0.5 * cell_size); // col 5, row 5 (DeepOcean)
    assert!(!is_pos_walkable(
        world.resource::<TerrainMap>(),
        &bounds,
        ocean_food_local
    ));
    let food_entity = world
        .spawn((
            Food {
                energy_value: 20.0,
                hydration_value: 0.0,
            },
            Position(ocean_food_local),
        ))
        .id();

    world.run_system_once(steering_drive_system);
    let vel_priority2 = world
        .get::<InertiaComponent>(prey)
        .expect("prey has inertia")
        .target_velocity;
    // F-055 fixed: Priority 2 ignores non-walkable DeepOcean Food and lets the prey stay on rich
    // walkable grassland.
    assert_eq!(
        vel_priority2,
        Vec3::ZERO,
        "F-055: Priority 2 must ignore non-walkable ocean Food and stay on rich land grass, got {vel_priority2:?}"
    );

    // Stepping physics + terrain navigation confirms zero shoreline revert traps.
    let mut revert_count = 0usize;
    for _ in 0..10 {
        world.run_system_once(steering_drive_system);
        world.run_system_once(integrate_physics_system);
        world.run_system_once(terrain_navigation_system);
        world.run_system_once(wrap_coordinates_system);
        if world
            .get::<LastWalkablePosition>(prey)
            .expect("LastWalkablePosition present")
            .reverted_this_tick
        {
            revert_count += 1;
        }
    }
    assert_eq!(
        revert_count, 0,
        "F-055: prey grazing on land must trigger 0 shoreline reverts, got {revert_count}"
    );
    assert!(
        is_pos_walkable(
            world.resource::<TerrainMap>(),
            &bounds,
            world.get::<Position>(prey).expect("prey pos").0
        ),
        "LAND1 keeps root out of water"
    );

    // Priority 3 fallback check: when grass is exhausted and the only Food is on DeepOcean, global
    // Food fallback ignores non-walkable Food and returns Vec3::ZERO.
    world.resource_mut::<ResourceField>().r.fill(0.0);
    world.get_mut::<Position>(prey).unwrap().0 = land_start;
    world.get_mut::<LastWalkablePosition>(prey).unwrap().pos = land_start;
    world.get_mut::<Position>(food_entity).unwrap().0 = Vec3::new(45.0, 0.0, 5.0);
    world.run_system_once(steering_drive_system);
    let vel_fallback = world
        .get::<InertiaComponent>(prey)
        .expect("prey has inertia")
        .target_velocity;
    assert_eq!(
        vel_fallback,
        Vec3::ZERO,
        "F-055: global Food fallback must ignore non-walkable ocean Food, got {vel_fallback:?}"
    );

    // Non-finite Food coordinates must also be ignored cleanly.
    let nan_food = world
        .spawn((
            Food {
                energy_value: 20.0,
                hydration_value: 0.0,
            },
            Position(Vec3::new(f32::NAN, 0.0, 5.0)),
        ))
        .id();
    world.run_system_once(steering_drive_system);
    assert_eq!(
        world
            .get::<InertiaComponent>(prey)
            .expect("prey has inertia")
            .target_velocity,
        Vec3::ZERO
    );
    world.despawn(nan_food);

    // Positive controls: walkable Food on Grassland is still chosen in both Priority 2 (even when
    // an ocean Food is closer) and Priority 3 global fallback.
    world.get_mut::<Position>(food_entity).unwrap().0 = ocean_food_local; // dist 10 on ocean
    let land_food_local = Vec3::new(-25.0, 0.0, 5.0); // dist 20 <= 35 on Grassland
    assert!(is_pos_walkable(
        world.resource::<TerrainMap>(),
        &bounds,
        land_food_local
    ));
    let land_food = world
        .spawn((
            Food {
                energy_value: 20.0,
                hydration_value: 0.0,
            },
            Position(land_food_local),
        ))
        .id();
    world.run_system_once(steering_drive_system);
    let vel_walkable_local = world
        .get::<InertiaComponent>(prey)
        .expect("prey has inertia")
        .target_velocity;
    assert!(
        vel_walkable_local.x < -1.4 && vel_walkable_local.z.abs() < 1e-5,
        "positive control (Priority 2): farther walkable Food must be selected over closer ocean Food, got {vel_walkable_local:?}"
    );

    // Distant walkable Food fallback (> local_food_radius = 35): still selected over closer ocean Food.
    world.get_mut::<Position>(land_food).unwrap().0 = Vec3::new(-45.0, 0.0, 5.0);
    world.run_system_once(steering_drive_system);
    let vel_walkable_fallback = world
        .get::<InertiaComponent>(prey)
        .expect("prey has inertia")
        .target_velocity;
    assert!(
        vel_walkable_fallback.x < -1.4 && vel_walkable_fallback.z.abs() < 1e-5,
        "positive control (Priority 3 fallback): distant walkable Food must be selected over closer ocean Food, got {vel_walkable_fallback:?}"
    );
}

#[test]
fn combined_post194_stack_restore_equivalence_closed_eu_and_invariants() {
    let init = initial(&[
        (LIVE_KEY_FOUNDERS, 16.0),
        (LIVE_KEY_FOUNDER_LAYOUT, 2.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.25),
        (LIVE_KEY_PREY_CAP, 12.0),
        (LIVE_KEY_STRICT_PREY_CAP, 1.0),
        (LIVE_KEY_LAND_MASK, 1.0),
        (LIVE_KEY_GRASS_STEERING, 1.0),
        (LIVE_KEY_PLANT_RESERVE, 0.25),
        (LIVE_KEY_TREES, 6.0),
        (LIVE_KEY_LAKES, 4.0),
        (LIVE_KEY_LAKE_LAYOUT, 1.0),
        (LIVE_KEY_FOOD_CAP, 30.0),
        (LIVE_KEY_MORTALITY, 1.0),
        (LIVE_KEY_REPRODUCTION, 1.0),
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_DRIVE_FLOOR, 0.5),
        (LIVE_KEY_THIRST_STEERING, 1.0),
        (LIVE_KEY_EVOLVED_BRAINS, 1.0),
        (LIVE_KEY_DRIFT_CONTROL, 0.0),
        (LIVE_KEY_DRIFT_RESET_TICKS, 0.0),
    ]);

    let mut uninterrupted = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &init,
        &[],
        HD_SEED,
        (16, 16),
        400,
    )
    .expect("combined stack builds");

    let mut clock = SimClock { tick: 0 };
    let mut ledger_u = CausalLedger::new();
    let mut rng_u = StdRng::seed_from_u64(HD_SEED);
    // Step tick 1 first so `ecosystem_census_system` populates `EcosystemBiomass::animals` and
    // locks `EnergyLedger` baseline before measuring closed-EU drift (matching existing closed-EU gates).
    let _ = clock.advance();
    uninterrupted.step(&clock, &[], &mut ledger_u, &mut rng_u);
    let initial_eu = obs(&uninterrupted.observables(), "live.closed_eu_total");
    assert!(
        initial_eu > 1_000.0,
        "non-vacuity: positive closed EU ({initial_eu})"
    );

    for _ in 1..200 {
        let _ = clock.advance();
        uninterrupted.step(&clock, &[], &mut ledger_u, &mut rng_u);
    }

    let snap = through_disk_snapshot(&uninterrupted.snapshot(), "combined_200");
    let mut resumed =
        LiveExperimentAdapter::from_snapshot(&snap).expect("combined stack restores cleanly");
    assert_eq!(
        resumed.checksum(),
        uninterrupted.checksum(),
        "combined stack checksum must match immediately after restore at tick 200"
    );

    let mut ledger_r = ledger_u.clone();
    let mut rng_r = rng_u.clone();
    for t in 201..=400 {
        let _ = clock.advance();
        uninterrupted.step(&clock, &[], &mut ledger_u, &mut rng_u);
        resumed.step(&clock, &[], &mut ledger_r, &mut rng_r);
        assert_eq!(
            resumed.checksum(),
            uninterrupted.checksum(),
            "combined post-#194 stack diverged after restore at tick {t}"
        );
    }

    let end_obs = uninterrupted.observables();
    let end_eu = obs(&end_obs, "live.closed_eu_total");
    let eu_drift = (end_eu - initial_eu).abs();
    assert!(
        eu_drift <= 1e-3,
        "closed EU conservation failed across combined post-#194 stack: start={initial_eu}, end={end_eu}, drift={eu_drift}"
    );
    assert!(
        obs(&end_obs, "live.herbivore_count") <= 12.0,
        "strict_prey_cap must stay <= 12, got {}",
        obs(&end_obs, "live.herbivore_count")
    );

    // Every living agent root must sit on walkable terrain under LAND1.
    let mut w = uninterrupted.world();
    let bounds = *w.resource::<MapBounds>();
    let tm = w.resource::<TerrainMap>().clone();
    let mut q = w.query_filtered::<(Entity, &Position), With<Agent>>();
    let mut checked_agents = 0usize;
    for (entity, pos) in q.iter(&w) {
        checked_agents += 1;
        assert!(
            is_pos_walkable(&tm, &bounds, pos.0),
            "surviving agent {entity:?} at {:?} ended on non-walkable ocean",
            pos.0
        );
    }
    assert!(checked_agents > 0, "expected surviving agents at tick 400");
}

#[test]
fn terrain_navigation_nan_root_poisons_child_segments_even_after_root_recovery() {
    let bounds = MapBounds {
        min: Vec3::new(-50.0, -50.0, -50.0),
        max: Vec3::new(50.0, 50.0, 50.0),
    };
    let (tm, _field) = split_coast_terrain_and_field(10, &bounds);
    let mut all_ocean = tm.clone();
    all_ocean.biomes.fill(BiomeType::DeepOcean as u8);

    // Verify boundary behavior of helpers first.
    assert!(!is_pos_walkable(
        &tm,
        &bounds,
        Vec3::new(f32::NAN, 0.0, 0.0)
    ));
    assert!(!is_pos_walkable(
        &tm,
        &bounds,
        Vec3::new(f32::INFINITY, 0.0, 0.0)
    ));
    assert!(
        is_pos_walkable(&tm, &bounds, Vec3::new(-50.0, 0.0, -50.0)),
        "exact min corner on Grassland must be walkable"
    );
    assert_eq!(
        snap_to_nearest_walkable(&all_ocean, &bounds, Vec3::ZERO),
        None,
        "all-ocean terrain has no walkable cell to snap to"
    );
    let wrapped_nan = wrap_pos(Vec3::new(f32::NAN, 0.0, 0.0), &bounds);
    assert!(!wrapped_nan.x.is_finite());

    // Build a multi-segment creature whose root is on walkable Grassland (-15, 0, 5).
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.2,
        mass: 1.5,
    });
    genotype.add_node(MorphologyNode {
        id: 1,
        length: 1.0,
        radius: 0.2,
        mass: 1.0,
    });
    genotype.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 1,
        joint_anchor: Vec3::new(1.0, 0.0, 0.0),
        joint_axis: Vec3::new(0.0, 0.0, 1.0),
    });

    let mut world = World::new();
    world.insert_resource(bounds);
    world.insert_resource(tm);
    world.insert_resource(TerrainNavigation { enabled: true });

    let valid_start = Vec3::new(-15.0, 0.0, 5.0);
    let root = decode_genotype(&mut world, &genotype, valid_start, glam::Quat::IDENTITY);
    let child_entity = {
        let mut seg_q = world.query_filtered::<(Entity, &ParentAgent), Without<Agent>>();
        seg_q
            .iter(&world)
            .find(|(_, p)| p.0 == root)
            .expect("child segment exists")
            .0
    };
    let initial_child_pos = world.get::<Position>(child_entity).unwrap().0;
    assert!(initial_child_pos.is_finite());

    // Helper verifying that any non-finite root position recovers cleanly without poisoning
    // `revert_delta` or child segments.
    let bad_positions = [
        Vec3::new(f32::NAN, 0.0, 5.0),
        Vec3::new(valid_start.x, f32::NAN, valid_start.z),
        Vec3::new(f32::INFINITY, 0.0, 5.0),
        Vec3::new(f32::NEG_INFINITY, 0.0, 5.0),
        Vec3::new(valid_start.x, f32::INFINITY, valid_start.z),
    ];
    for bad_pos in bad_positions {
        world.get_mut::<Position>(root).unwrap().0 = bad_pos;
        world.get_mut::<Position>(child_entity).unwrap().0 = initial_child_pos;
        world.run_system_once(terrain_navigation_system);

        let root_after = world.get::<Position>(root).unwrap().0;
        assert_eq!(
            root_after, valid_start,
            "root position must recover to LastWalkablePosition for {bad_pos:?}"
        );
        let last = world.get::<LastWalkablePosition>(root).unwrap();
        assert!(last.pos.is_finite() && last.reverted_this_tick);
        assert_eq!(
            last.revert_delta,
            Vec3::ZERO,
            "F-056: non-finite root must clamp revert_delta to Vec3::ZERO (got {:?})",
            last.revert_delta
        );
        let child_after = world.get::<Position>(child_entity).unwrap().0;
        assert!(
            child_after.is_finite(),
            "F-056: child segment must stay finite after root recovery from {bad_pos:?}, got {child_after:?}"
        );
        assert_eq!(
            child_after, initial_child_pos,
            "child segment must retain its finite position when revert_delta is clamped to zero"
        );
    }

    // Even if both root and child segment are non-finite (and LastWalkablePosition is valid), both
    // recover to finite coordinates.
    world.get_mut::<Position>(root).unwrap().0 = Vec3::new(f32::NAN, f32::INFINITY, 5.0);
    world.get_mut::<Position>(child_entity).unwrap().0 = Vec3::new(f32::NAN, 0.0, 5.0);
    world.run_system_once(terrain_navigation_system);
    assert_eq!(world.get::<Position>(root).unwrap().0, valid_start);
    assert_eq!(world.get::<Position>(child_entity).unwrap().0, valid_start);
}

#[test]
fn w2_lite_preflight_vs_try_apply_live_genesis_and_strict_cap_above_ceiling() {
    // Part A: On the default generated 256x256 terrain (plenty of walkable land), W2-lite viewer
    // preflight accepts `founder_layout = 1` + `land_mask = 1` + `founders = 20`, whereas
    // `try_apply_live_genesis_and_policies` still refuses an explicit scarce-land World whose
    // TerrainMap has only 4 walkable cells (F-057 viewer scarce-land preflight and non-panicking
    // engine fallback are verified in `f057_scarce_land_artifact_refused_at_preflight_and_non_panicking_in_engine`).
    let observable_ids: Vec<String> = LIVE_OBSERVABLE_IDS
        .iter()
        .copied()
        .map(str::to_string)
        .collect();
    let manifest = ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "w2-lite-scarce-land".to_string(),
        name: "w2-lite-scarce-land".to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: initial(&[
            (LIVE_KEY_FOUNDERS, 20.0),
            (LIVE_KEY_FOUNDER_LAYOUT, 1.0),
            (LIVE_KEY_LAND_MASK, 1.0),
            (LIVE_KEY_TREES, 2.0),
            (LIVE_KEY_LAKES, 2.0),
        ]),
        interventions: vec![],
        seeds: vec![HD_SEED],
        duration_ticks: 100,
        sample_period: 50,
        observable_ids,
        exotic_interventions: vec![],
        observer: ObserverPolicy::default(),
    };
    let manifest_json = serde_json::to_string(&manifest).expect("serialize manifest");
    let pending =
        parse_and_validate_manifest_for_viewer(&manifest_json).expect("viewer preflight passes");
    assert_eq!(pending.config.founders, 20);
    assert_eq!(pending.config.founder_layout, 1);

    let mut scarce_world = init_world();
    let bounds = MapBounds::default();
    let mut tiny_island_biomes = vec![BiomeType::DeepOcean as u8; 16 * 16];
    tiny_island_biomes[..4].fill(BiomeType::Grassland as u8);
    scarce_world.insert_resource(TerrainMap {
        width: 16,
        height: 16,
        elevations: vec![0.2; 16 * 16],
        moistures: vec![0.5; 16 * 16],
        temperatures: vec![0.5; 16 * 16],
        biomes: tiny_island_biomes,
        flows: vec![0.0; 16 * 16],
        pois: Vec::new(),
    });
    let genesis_err = try_apply_live_genesis_and_policies(
        &mut scarce_world,
        pending.seed,
        &pending.config,
        bounds,
    )
    .expect_err("try_apply must return Err when walkable cells < founders");
    assert!(
        matches!(genesis_err, ExperimentError::InvalidPopulation { .. }),
        "expected InvalidPopulation, got {genesis_err:?}"
    );
    assert!(
        genesis_err.to_string().contains("distinct walkable cell"),
        "unexpected message: {genesis_err}"
    );

    // Part B: `live.strict_prey_cap = 1.0` when initial founders > prey_cap (20 founders, cap 12,
    // mortality 0) must allow 0 prey births while preserving closed EU.
    let init_over_cap = initial(&[
        (LIVE_KEY_FOUNDERS, 20.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.0),
        (LIVE_KEY_PREY_CAP, 12.0),
        (LIVE_KEY_STRICT_PREY_CAP, 1.0),
        (LIVE_KEY_REPRODUCTION, 1.0),
        (LIVE_KEY_MORTALITY, 0.0),
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_DRIVE_FLOOR, 0.5),
        (LIVE_KEY_FOOD_CAP, 40.0),
        (LIVE_KEY_TREES, 4.0),
        (LIVE_KEY_LAKES, 2.0),
    ]);
    let mut over_cap = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &init_over_cap,
        &[],
        HD_SEED,
        (16, 16),
        300,
    )
    .expect("over-cap strict adapter builds");
    let mut clock = SimClock { tick: 0 };
    let mut ledger = CausalLedger::new();
    let mut rng = StdRng::seed_from_u64(HD_SEED);
    let _ = clock.advance();
    over_cap.step(&clock, &[], &mut ledger, &mut rng);
    let start_eu = obs(&over_cap.observables(), "live.closed_eu_total");
    for _ in 1..300 {
        let _ = clock.advance();
        over_cap.step(&clock, &[], &mut ledger, &mut rng);
    }
    let end_o = over_cap.observables();
    let total_births = over_cap.world().resource::<VitalStatistics>().total_births;
    assert_eq!(
        total_births, 0,
        "no prey births allowed while population exceeds strict cap"
    );
    assert_eq!(
        obs(&end_o, "live.herbivore_count"),
        20.0,
        "all 20 founders survive with zero extra births"
    );
    let end_eu = obs(&end_o, "live.closed_eu_total");
    assert!(
        (end_eu - start_eu).abs() <= 1e-3,
        "closed EU must hold over-cap: start={start_eu}, end={end_eu}"
    );
}

const F057_CHILD_ENV: &str = "ANIMA_F057_SCARCE_LAND_CHILD";

fn build_scarce_land_manifest(founders: usize, layout: usize) -> ExperimentManifest {
    let observable_ids: Vec<String> = LIVE_OBSERVABLE_IDS
        .iter()
        .copied()
        .map(str::to_string)
        .collect();
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: format!("f057-scarce-{founders}-{layout}"),
        name: format!("f057-scarce-{founders}-{layout}"),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: initial(&[
            (LIVE_KEY_FOUNDERS, founders as f64),
            (LIVE_KEY_FOUNDER_LAYOUT, layout as f64),
            (LIVE_KEY_LAND_MASK, 1.0),
            (LIVE_KEY_TREES, 1.0),
            (LIVE_KEY_LAKES, 1.0),
        ]),
        interventions: vec![],
        seeds: vec![HD_SEED],
        duration_ticks: 100,
        sample_period: 50,
        observable_ids,
        exotic_interventions: vec![],
        observer: ObserverPolicy::default(),
    }
}

#[test]
fn f057_scarce_land_child_probe() {
    if std::env::var_os(F057_CHILD_ENV).is_none() {
        return;
    }

    let evo_settings = Arc::new(Mutex::new(EvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    }));
    let evo_running = Arc::new(AtomicBool::new(false));
    let map_grid = Arc::new(Mutex::new(MapElitesGridState {
        grid: std::collections::HashMap::new(),
        grid_resolution: 50,
    }));

    // 1. Both grid (1) and scatter (2) layouts with founders=20 (> 4 walkable cells) must fail at
    //    viewer preflight with the exact capacity message.
    for layout in [1usize, 2usize] {
        let scarce_json =
            serde_json::to_string(&build_scarce_land_manifest(20, layout)).expect("serialize");
        let err = parse_and_validate_manifest_for_viewer(&scarce_json)
            .expect_err("scarce-land artifact must fail viewer preflight");
        assert!(
            err.contains("distinct walkable cell"),
            "unexpected preflight error for layout={layout}: {err}"
        );
    }

    // 2. Start a valid manifest (founders=2 <= 4 walkable cells), confirm it runs, then verify
    //    `start_from_manifest_with_engine` refuses the scarce manifest without stopping the engine.
    let engine = SimulationEngine::new();
    let ok_json = serde_json::to_string(&build_scarce_land_manifest(2, 1)).expect("serialize ok");
    start_from_manifest_with_engine::<tauri::test::MockRuntime>(
        &engine,
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_grid),
        &ok_json,
    )
    .expect("2 founders fit on 4 walkable cells");

    let deadline = Instant::now() + Duration::from_secs(10);
    while engine.get_status().tick_count == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        engine.running.load(Ordering::SeqCst),
        "valid engine should be running"
    );

    let scarce_json =
        serde_json::to_string(&build_scarce_land_manifest(20, 1)).expect("serialize scarce");
    let refused = start_from_manifest_with_engine::<tauri::test::MockRuntime>(
        &engine,
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_grid),
        &scarce_json,
    )
    .expect_err("start_from_manifest_with_engine must refuse scarce-land manifest");
    assert!(refused.contains("distinct walkable cell"), "{refused}");
    assert!(
        engine.running.load(Ordering::SeqCst),
        "refused scarce-land manifest must not stop an already-running simulation"
    );
    engine.stop();

    // 3. Direct `SimulationEngine::start` bypassing preflight onto a scarce-land world must abort
    //    cleanly without panicking the `sim` thread (`running == false`, `status.running == false`).
    let bypassed_engine = SimulationEngine::new();
    let bypassed_manifest = build_scarce_land_manifest(20, 2);
    let bypassed_config =
        anima_engine_lib::core::live_experiment::LiveWorldConfig::from_initial_conditions(
            &bypassed_manifest.initial_conditions,
        )
        .expect("valid config shape");
    *bypassed_engine
        .pending_manifest
        .lock()
        .unwrap_or_else(|e| e.into_inner()) =
        Some(anima_engine_lib::core::simulation_loop::PendingManifest {
            name: bypassed_manifest.name,
            seed: bypassed_manifest.seeds[0],
            config: bypassed_config,
        });
    bypassed_engine.start::<tauri::test::MockRuntime>(None, evo_settings, evo_running, map_grid);

    let abort_deadline = Instant::now() + Duration::from_secs(10);
    while (bypassed_engine.running.load(Ordering::SeqCst) || bypassed_engine.get_status().running)
        && Instant::now() < abort_deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !bypassed_engine.running.load(Ordering::SeqCst),
        "aborted scarce-land start must reset engine.running to false"
    );
    assert!(
        !bypassed_engine.get_status().running,
        "aborted scarce-land start must reset status.running to false"
    );
    assert_eq!(bypassed_engine.get_status().tick_count, 0);
}

#[test]
fn f057_scarce_land_artifact_refused_at_preflight_and_non_panicking_in_engine() {
    let dir = std::env::temp_dir().join(format!("anima_f057_scarce_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir for f057 artifact");
    let artifact_path = dir.join("scarce_island.anmw");

    // Build a valid 256x256 WorldArtifact with only 4 frontend Grassland (index 4) cells and all
    // remaining cells Ocean (index 0).
    let side = 256usize;
    let n = side * side;
    let mut biome = vec![0u8; n];
    biome[0] = 4;
    biome[1] = 4;
    biome[side] = 4;
    biome[side + 1] = 4;
    let artifact = WorldArtifact {
        width: side,
        height: side,
        sea_level: 0.5,
        seed: 42,
        generator_version: 1,
        world_scale: CANONICAL_WORLD_SCALE,
        elevation: vec![0.2; n],
        moisture: vec![0.5; n],
        temperature: vec![0.5; n],
        flow: vec![0.0; n],
        biome,
    };
    std::fs::write(&artifact_path, artifact.to_bytes()).expect("write scarce-land artifact");

    let exe = std::env::current_exe().expect("current test executable");
    let output = std::process::Command::new(exe)
        .env("ANIMA_WORLD_ARTIFACT", &artifact_path)
        .env(F057_CHILD_ENV, "1")
        .args(["--exact", "f057_scarce_land_child_probe", "--nocapture"])
        .output()
        .expect("spawn isolated f057 child probe");

    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success(),
        "isolated f057 probe failed (status={}):\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
