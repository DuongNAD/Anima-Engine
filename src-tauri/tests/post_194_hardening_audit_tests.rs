//! Post-PR-#194 hardening & real-bug audit tests (`HD_SEED = 0x5EED`).
//!
//! Focuses on interactions across recent trunk changes (`F-053` windowed observables,
//! `live.grass_steering` + `live.land_mask`, combined post-#194 snapshot restore + closed-EU,
//! `terrain_navigation` non-finite root/segment recovery, and `W2-lite` preflight vs genesis).

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::commands::parse_and_validate_manifest_for_viewer;
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
use anima_engine_lib::core::snapshot::{self, SnapshotEnvelope};
use anima_engine_lib::core::terrain::{BiomeType, TerrainMap};
use anima_engine_lib::core::terrain_navigation::{
    is_pos_walkable, snap_to_nearest_walkable, terrain_navigation_system, wrap_pos,
};
use anima_engine_lib::core::world_artifact::WorldIdentity;
use anima_engine_lib::core::world_systems::wrap_coordinates_system;
use anima_engine_lib::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};
use anima_engine_lib::physics::dynamics::{integrate_physics_system, RigidBody};
use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;
use glam::Vec3;
use rand::{rngs::StdRng, SeedableRng};
use std::sync::atomic::{AtomicU64, Ordering};

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

    // Bug F-053 characterization: on aligned runs (`duration_ticks % sample_period == 0`), the
    // final time-series sample calls `VitalStatistics::sample_window()` and empties all five
    // windowed vital observables right before `assemble_result` reads `final_observables`.
    // Invert these assertions to > 0.0 (or cumulative totals) when F-053 is fixed.
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
            "F-053: aligned run drains final_observables[{metric}] to 0.0"
        );
    }

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
    // Bug characterization (`select_prey_grass_target` Priority 2 in locomotion.rs:116-120):
    // despite GrassContext carrying `nav` (`TerrainNavigation`), Priority 2 picks local Food on
    // DeepOcean without checking `grass.is_walkable(food_pos)`, so it drives the prey toward ocean.
    // Invert to `vel_priority2 == Vec3::ZERO` (stay on rich land grass) once fixed.
    assert!(
        vel_priority2.x > 1.4 && vel_priority2.z.abs() < 1e-5,
        "expected Priority 2 ocean-Food lure along +x, got {vel_priority2:?}"
    );

    // Stepping physics + terrain navigation shows the practical consequence: the prey repeatedly
    // tries to walk into DeepOcean and gets snapped back every tick instead of eating nearby land.
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
    assert!(
        revert_count > 0,
        "steering toward ocean Food under LAND1 triggers shoreline revert trap (reverts={revert_count})"
    );
    assert!(
        is_pos_walkable(
            world.resource::<TerrainMap>(),
            &bounds,
            world.get::<Position>(prey).expect("prey pos").0
        ),
        "LAND1 keeps root out of water while trapped at shore"
    );

    // Priority 3 fallback check (`locomotion.rs:165`): even when grass is exhausted and Food is
    // outside local radius on DeepOcean, global Food fallback still steers into ocean.
    world.resource_mut::<ResourceField>().r.fill(0.0);
    world.get_mut::<Position>(prey).unwrap().0 = land_start;
    world.get_mut::<LastWalkablePosition>(prey).unwrap().pos = land_start;
    world.get_mut::<Position>(food_entity).unwrap().0 = Vec3::new(45.0, 0.0, 5.0);
    world.run_system_once(steering_drive_system);
    let vel_fallback = world
        .get::<InertiaComponent>(prey)
        .expect("prey has inertia")
        .target_velocity;
    // Invert to `Vec3::ZERO` once Food candidates respect `TerrainNavigation`.
    assert!(
        vel_fallback.x > 1.4 && vel_fallback.z.abs() < 1e-5,
        "expected global Food fallback to steer toward distant DeepOcean Food, got {vel_fallback:?}"
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

    // Corrupt root position to NaN while LastWalkablePosition still holds valid finite land.
    world.get_mut::<Position>(root).unwrap().0 = Vec3::new(f32::NAN, 0.0, 5.0);

    world.run_system_once(terrain_navigation_system);

    let root_after = world.get::<Position>(root).unwrap().0;
    assert_eq!(
        root_after, valid_start,
        "root position is repaired to LastWalkablePosition"
    );
    let revert_delta = world
        .get::<LastWalkablePosition>(root)
        .unwrap()
        .revert_delta;
    let mut seg_q = world.query_filtered::<(&ParentAgent, &Position), Without<Agent>>();
    let segs: Vec<Vec3> = seg_q
        .iter(&world)
        .filter(|(p, _)| p.0 == root)
        .map(|(_, pos)| pos.0)
        .collect();
    assert_eq!(segs.len(), 1);

    // Bug characterization (`terrain_navigation.rs:231-249`): `delta = target - pos.0` subtracts
    // corrupted `NaN` `pos.0` -> `revert_delta.x = NaN`, and `seg_pos.0 += last.revert_delta`
    // poisons all child segments to NaN even after root recovery!
    // Invert these assertions to `revert_delta.is_finite()` / `segs[0].is_finite()` once fixed.
    assert!(
        revert_delta.x.is_nan() && segs[0].x.is_nan(),
        "expected NaN delta to poison child segment, got delta={revert_delta:?} seg={:?}",
        segs[0]
    );
}

#[test]
fn w2_lite_preflight_vs_try_apply_live_genesis_and_strict_cap_above_ceiling() {
    // Part A: W2-lite viewer preflight accepts `founder_layout = 1` + `land_mask = 1` + `founders = 20`,
    // whereas `try_apply_live_genesis_and_policies` rejects a World whose TerrainMap has fewer than
    // 20 walkable cells (and `simulation_loop.rs:1539` invokes the panicking `apply_live_genesis_and_policies`).
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
