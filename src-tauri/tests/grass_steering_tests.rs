//! Card SEL2P2-PREP (`live.grass_steering`): opt-in prey pasture-seeking steering ported from
//! `origin/dell/s11-grass-steering` (`226d4b2`, `95044c4b`) and adapted to trunk (`LAND1`
//! walkability, explicit thirst > local Food > grass > global Food priority).
//!
//! Covers every acceptance requirement without pinning trajectory floats or absolute intake:
//! 1. Key registration (`LIVE_KEY_GRASS_STEERING = "live.grass_steering"`, `LIVE_KEYS.len() = 30`),
//!    exact `{0.0, 1.0}` validation, `GrassSteeringPolicy` / `LiveWorldConfig` defaults, serde
//!    backward compatibility, and preflight refusal without `live.locomotion = 1.0`.
//! 2. `absent` and `0.0` produce identical checksums and observables across every sampled tick
//!    (tested both with and without `live.land_mask = 1.0`).
//! 3. Priority order unit tests: thirst (> 50% deficit) > local `Food` > pasture `ResourceField`
//!    (`GRASS_STAY_FRACTION`, fixed clipped `7x7` scan `GRASS_SCAN_RADIUS = 3`, S10
//!    `PlantReservePolicy`, lowest `(row, col)` tie-break, LAND1 `TerrainNavigation` walkability)
//!    > global `Food` fallback, while predators stay unchanged.
//! 4. Behavioural stripped-cell movement: with `grass_steering = 1.0`, a prey standing on a
//!    stripped cell adjacent to rich grass moves toward it within N ticks; with `0.0` it does not.
//! 5. Crowded-founder grazing intake rises with `grass_steering = 1.0` vs `0.0` (never pinned).
//! 6. Zero heap allocations in `steering_drive_system` once warm with grass steering + LAND1 on.
//! 7. Deterministic checksums and closed-EU conservation (`|delta| / start < 1e-6`).

mod common;

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::AgentEvaluation;
use anima_engine_lib::core::components::{
    Agent, Food, InertiaComponent, Lake, Position, Predator, Prey,
};
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::experiment::{ExperimentError, InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    live_observables, LiveExperimentAdapter, LiveWorldConfig, LIVE_KEYS, LIVE_KEY_DRIVE_FLOOR,
    LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS, LIVE_KEY_GRASS_STEERING, LIVE_KEY_LAKES,
    LIVE_KEY_LAKE_LAYOUT, LIVE_KEY_LAND_MASK, LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY,
    LIVE_KEY_PREDATOR_FRACTION, LIVE_KEY_REPRODUCTION, LIVE_KEY_RESOURCE_CAPACITY_SCALE,
    LIVE_KEY_THIRST_STEERING, LIVE_KEY_TREES,
};
use anima_engine_lib::core::locomotion::{
    steering_drive_system, GRASS_SCAN_RADIUS, GRASS_STAY_FRACTION, PREY_MAX_SPEED,
};
use anima_engine_lib::core::resources::{
    GrassSteeringPolicy, LocomotionPolicy, MapBounds, PlantReservePolicy, TerrainNavigation,
};
use anima_engine_lib::core::terrain::{BiomeType, TerrainMap};
use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;
use glam::Vec3;
use std::sync::Mutex;

#[global_allocator]
static ALLOCATOR: common::allocator::ThreadTrackingAllocator =
    common::allocator::ThreadTrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn initial(extra: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(extra.iter().map(|(k, v)| ((*k).to_string(), *v)).collect())
}

fn homeo(
    energy: f32,
    energy_target: f32,
    hydration: f32,
    hydration_target: f32,
) -> HomeostaticState {
    HomeostaticState {
        energy,
        energy_target,
        hydration,
        hydration_target,
        ..Default::default()
    }
}

fn uniform_field(side: usize, r: f32, r_max: f32) -> ResourceField {
    let n = side * side;
    ResourceField {
        r: vec![r; n],
        r_max: vec![r_max; n],
        width: side,
        height: side,
        min_x: 0.0,
        min_z: 0.0,
        max_x: (side as f32) * 10.0,
        max_z: (side as f32) * 10.0,
        growth_rate: 0.0,
        regrowth_phase: 0,
    }
}

fn run_steering_once(world: &mut World) {
    world.run_system_once(steering_drive_system);
}

// ---- 1. Registration, validation, and serde backward compatibility ------------------------------

#[test]
fn key_is_registered_validated_and_refused_without_locomotion() {
    assert_eq!(LIVE_KEY_GRASS_STEERING, "live.grass_steering");
    assert_eq!(LIVE_KEYS.len(), 30);
    assert!(LIVE_KEYS.contains(&LIVE_KEY_GRASS_STEERING));

    assert!(!LiveWorldConfig::default().grass_steering);
    assert_eq!(
        GrassSteeringPolicy::default(),
        GrassSteeringPolicy { enabled: false }
    );
    assert_eq!(
        GrassSteeringPolicy::disabled(),
        GrassSteeringPolicy { enabled: false }
    );

    let cfg_absent =
        LiveWorldConfig::from_initial_conditions(&initial(&[])).expect("empty manifest builds");
    assert!(!cfg_absent.grass_steering);

    let cfg_zero = LiveWorldConfig::from_initial_conditions(&initial(&[
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_GRASS_STEERING, 0.0),
    ]))
    .expect("0.0 builds");
    assert!(!cfg_zero.grass_steering);

    let cfg_one = LiveWorldConfig::from_initial_conditions(&initial(&[
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_GRASS_STEERING, 1.0),
    ]))
    .expect("1.0 with locomotion builds");
    assert!(cfg_one.grass_steering);

    // 0.0 without locomotion is explicitly allowed as legacy.
    let cfg_zero_no_loco =
        LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_GRASS_STEERING, 0.0)]))
            .expect("0.0 without locomotion allowed");
    assert!(!cfg_zero_no_loco.grass_steering);

    // 1.0 without locomotion must be refused at preflight.
    for pairs in [
        vec![(LIVE_KEY_GRASS_STEERING, 1.0)],
        vec![(LIVE_KEY_LOCOMOTION, 0.0), (LIVE_KEY_GRASS_STEERING, 1.0)],
    ] {
        let err = LiveWorldConfig::from_initial_conditions(&initial(&pairs))
            .expect_err("grass_steering=1.0 without locomotion=1.0 must fail preflight");
        assert!(
            matches!(err, ExperimentError::InvalidLaw { .. }),
            "expected InvalidLaw, got {err:?}"
        );
    }

    // Non-boolean and non-finite values rejected.
    for bad in [-1.0, 0.5, 2.0] {
        let err = LiveWorldConfig::from_initial_conditions(&initial(&[
            (LIVE_KEY_LOCOMOTION, 1.0),
            (LIVE_KEY_GRASS_STEERING, bad),
        ]))
        .expect_err("non-boolean grass_steering must fail validation");
        assert!(
            matches!(err, ExperimentError::OutOfRange { .. }),
            "expected OutOfRange for {bad:?}, got {err:?}"
        );
    }
    for non_finite in [f64::NAN, f64::INFINITY] {
        let err = LiveWorldConfig::from_initial_conditions(&initial(&[
            (LIVE_KEY_LOCOMOTION, 1.0),
            (LIVE_KEY_GRASS_STEERING, non_finite),
        ]))
        .expect_err("non-finite grass_steering must fail validation");
        assert!(
            matches!(err, ExperimentError::NotFinite { .. }),
            "expected NotFinite for {non_finite:?}, got {err:?}"
        );
    }

    // Serde backward compatibility: legacy config JSON missing `grass_steering` defaults to false.
    let mut legacy_val = serde_json::to_value(LiveWorldConfig::default()).expect("serialize");
    legacy_val
        .as_object_mut()
        .expect("object")
        .remove("grass_steering");
    let decoded: LiveWorldConfig = serde_json::from_value(legacy_val).expect("deserialize");
    assert_eq!(decoded, LiveWorldConfig::default());

    // Adapter resource insertion checks.
    let a_off = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial(&[(LIVE_KEY_LOCOMOTION, 1.0)]),
        &[],
        1,
        (16, 16),
        10,
    )
    .expect("off builds");
    assert_eq!(
        *a_off.world().resource::<GrassSteeringPolicy>(),
        GrassSteeringPolicy { enabled: false }
    );

    let a_on = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial(&[(LIVE_KEY_LOCOMOTION, 1.0), (LIVE_KEY_GRASS_STEERING, 1.0)]),
        &[],
        1,
        (16, 16),
        10,
    )
    .expect("on builds");
    assert_eq!(
        *a_on.world().resource::<GrassSteeringPolicy>(),
        GrassSteeringPolicy { enabled: true }
    );
}

// ---- 2. Legacy parity: absent == 0.0 run-to-run ------------------------------------------------

#[test]
fn absent_and_zero_produce_identical_runs() {
    let run_pair = |with_land_mask: bool, ticks: u64| {
        let mut base = vec![
            (LIVE_KEY_FOUNDERS, 30.0),
            (LIVE_KEY_PREDATOR_FRACTION, 0.2),
            (LIVE_KEY_TREES, 8.0),
            (LIVE_KEY_LAKES, 8.0),
            (LIVE_KEY_LAKE_LAYOUT, 1.0),
            (LIVE_KEY_FOOD_CAP, 50.0),
            (LIVE_KEY_MORTALITY, 1.0),
            (LIVE_KEY_REPRODUCTION, 1.0),
            (LIVE_KEY_LOCOMOTION, 1.0),
            (LIVE_KEY_DRIVE_FLOOR, 0.5),
            (LIVE_KEY_THIRST_STEERING, 1.0),
        ];
        if with_land_mask {
            base.push((LIVE_KEY_LAND_MASK, 1.0));
        }

        let sample = |extra: Option<f64>| {
            let mut pairs = base.clone();
            if let Some(v) = extra {
                pairs.push((LIVE_KEY_GRASS_STEERING, v));
            }
            let mut a = LiveExperimentAdapter::from_manifest(
                &WorldLawSet::baseline(),
                &initial(&pairs),
                &[],
                7,
                (16, 16),
                ticks,
            )
            .expect("adapter builds");
            let mut out = Vec::new();
            for t in 1..=ticks {
                a.run_schedule_once();
                if t == 1 || t % 100 == 0 {
                    out.push((t, a.checksum(), a.observables()));
                }
            }
            out
        };

        assert_eq!(
            sample(None),
            sample(Some(0.0)),
            "absent vs 0.0 differed (land_mask={with_land_mask})"
        );
    };

    run_pair(false, 1_000);
    run_pair(true, 500);
}

// ---- 3. Explicit priority order, window bounds, S10 reserve, LAND1 walkability, predators ------

#[test]
fn priority_order_thirst_then_local_food_then_grass_with_land1_and_s10() {
    assert!((GRASS_STAY_FRACTION - 0.25).abs() < f32::EPSILON);
    assert_eq!(GRASS_SCAN_RADIUS, 3);

    // (a) Priority 1 (thirst < 50%) beats BOTH local Food and rich grass.
    let mut world = World::new();
    world.insert_resource(LocomotionPolicy {
        enabled: true,
        drive_floor: 1.0,
        thirst_steering: true,
    });
    world.insert_resource(GrassSteeringPolicy { enabled: true });
    let mut field = uniform_field(10, 5.0, 10.0); // cell (5,5) is rich (50% >= 25%)
    field.r[5 * 10 + 6] = 10.0;
    world.insert_resource(field);
    let lake = world
        .spawn((
            Lake {
                current_water: 50.0,
                max_water: 100.0,
                replenishment_rate: 1.0,
            },
            Position(Vec3::new(55.0, 0.0, 20.0)), // -z
        ))
        .id();
    world.spawn((
        Food {
            energy_value: 20.0,
            hydration_value: 0.0,
        },
        Position(Vec3::new(65.0, 0.0, 55.0)), // local Food on +x (dist 10 <= 35)
    ));
    let prey = world
        .spawn((
            Agent,
            Prey,
            Position(Vec3::new(55.0, 0.0, 55.0)),
            homeo(50.0, 100.0, 49.0, 100.0), // below 50% hydration
            InertiaComponent::default(),
        ))
        .id();
    run_steering_once(&mut world);
    let vel_thirst = world.get::<InertiaComponent>(prey).unwrap().target_velocity;
    assert!(
        vel_thirst.z < -1.4 && vel_thirst.x.abs() < 1e-5,
        "Priority 1 (thirst) must steer toward lake (-z) over Food and grass, got {vel_thirst:?}"
    );

    // At hydration == 50% (not below threshold), Priority 2 (local Food within scan radius) wins
    // over staying on rich grass.
    world.get_mut::<HomeostaticState>(prey).unwrap().hydration = 50.0;
    run_steering_once(&mut world);
    let vel_local_food = world.get::<InertiaComponent>(prey).unwrap().target_velocity;
    assert!(
        vel_local_food.x > 1.4 && vel_local_food.z.abs() < 1e-5,
        "Priority 2 (local Food) must win over rich current grass (+x), got {vel_local_food:?}"
    );

    // Empty lake cannot capture Priority 1 even when thirsty.
    world.get_mut::<HomeostaticState>(prey).unwrap().hydration = 10.0;
    world.get_mut::<Lake>(lake).unwrap().current_water = 0.0;
    run_steering_once(&mut world);
    let vel_dry_lake = world.get::<InertiaComponent>(prey).unwrap().target_velocity;
    assert!(
        vel_dry_lake.x > 1.4 && vel_dry_lake.z.abs() < 1e-5,
        "empty lake must yield to Priority 2 local Food, got {vel_dry_lake:?}"
    );
    world.get_mut::<HomeostaticState>(prey).unwrap().hydration = 100.0;

    // (b) Move Food outside local scan radius (dist = 40 > (3 + 0.5) * 10 = 35):
    // Priority 3 (grass) takes over: stays at >= 0.25 * r_max even with richer neighbour.
    let mut food_q = world.query_filtered::<&mut Position, With<Food>>();
    for mut fp in food_q.iter_mut(&mut world) {
        fp.0 = Vec3::new(95.0, 0.0, 55.0); // dist 40 > local radius 35
    }
    world.resource_mut::<ResourceField>().r[5 * 10 + 5] = 2.5; // exact 0.25 * 10.0
    run_steering_once(&mut world);
    assert_eq!(
        world.get::<InertiaComponent>(prey).unwrap().target_velocity,
        Vec3::ZERO,
        "with only distant Food, exact GRASS_STAY_FRACTION must hold prey in place"
    );

    // Below 0.25 -> steers toward richer neighbour (5, 6), ignoring distant Food.
    world.resource_mut::<ResourceField>().r[5 * 10 + 5] = 2.4;
    run_steering_once(&mut world);
    let vel_grass = world.get::<InertiaComponent>(prey).unwrap().target_velocity;
    assert!(
        vel_grass.x > 1.4 && vel_grass.z.abs() < 1e-5,
        "below stay fraction with distant Food, prey must steer toward richer grass (5,6), got {vel_grass:?}"
    );

    // Barren current cell (r_max == 0.0) must NOT freeze on ZERO >= 0.0.
    {
        let mut f = world.resource_mut::<ResourceField>();
        f.r[5 * 10 + 5] = 0.0;
        f.r_max[5 * 10 + 5] = 0.0;
    }
    run_steering_once(&mut world);
    let vel_barren = world.get::<InertiaComponent>(prey).unwrap().target_velocity;
    assert!(
        vel_barren.x > 1.4,
        "barren current cell must steer to richer grass, got {vel_barren:?}"
    );

    // No better cell in 7x7 window -> global Food fallback.
    world.resource_mut::<ResourceField>().r.fill(0.0);
    run_steering_once(&mut world);
    let vel_fallback = world.get::<InertiaComponent>(prey).unwrap().target_velocity;
    assert!(
        vel_fallback.x > 1.4 && vel_fallback.z.abs() < 1e-5,
        "when no 7x7 grass improves on current cell, prey must fall back to distant Food, got {vel_fallback:?}"
    );

    // (c) Corner clipping (radius 3) & lowest (row, col) tie-breaking & S10 reserve floor.
    let mut corner = World::new();
    corner.insert_resource(LocomotionPolicy {
        enabled: true,
        drive_floor: 1.0,
        thirst_steering: false,
    });
    corner.insert_resource(GrassSteeringPolicy { enabled: true });
    let mut cf = uniform_field(10, 0.0, 10.0);
    cf.r[4] = 10.0; // (0, 4): dist 4 > GRASS_SCAN_RADIUS = 3
    corner.insert_resource(cf);
    let cp = corner
        .spawn((
            Agent,
            Prey,
            Position(Vec3::new(5.0, 0.0, 5.0)), // (0, 0)
            homeo(50.0, 100.0, 100.0, 100.0),
            InertiaComponent::default(),
        ))
        .id();
    run_steering_once(&mut corner);
    assert_eq!(
        corner.get::<InertiaComponent>(cp).unwrap().target_velocity,
        Vec3::ZERO,
        "cell at col=4 must lie outside GRASS_SCAN_RADIUS=3"
    );
    corner.resource_mut::<ResourceField>().r[3] = 4.0; // (0, 3) inside window
    run_steering_once(&mut corner);
    let vc = corner.get::<InertiaComponent>(cp).unwrap().target_velocity;
    assert!(
        vc.x > 1.4 && vc.z.abs() < 1e-5,
        "must find (0,3), got {vc:?}"
    );

    // Tie-break + S10 PlantReservePolicy (`(5,4)` grazeable 2.0 > `(5,6)` grazeable 1.0 despite lower raw r).
    let mut s10 = World::new();
    s10.insert_resource(LocomotionPolicy {
        enabled: true,
        drive_floor: 1.0,
        thirst_steering: false,
    });
    s10.insert_resource(GrassSteeringPolicy { enabled: true });
    s10.insert_resource(PlantReservePolicy { fraction: 0.3 });
    let mut rf = uniform_field(10, 0.0, 10.0);
    rf.r[5 * 10 + 5] = 4.5; // floor 3.0 -> grazeable 1.5 < 2.5
    rf.r[5 * 10 + 4] = 5.0; // floor 3.0 -> grazeable 2.0
    rf.r[5 * 10 + 6] = 5.5;
    rf.r_max[5 * 10 + 6] = 15.0; // floor 4.5 -> grazeable 1.0
    s10.insert_resource(rf);
    let p_s10 = s10
        .spawn((
            Agent,
            Prey,
            Position(Vec3::new(55.0, 0.0, 55.0)),
            homeo(50.0, 100.0, 100.0, 100.0),
            InertiaComponent::default(),
        ))
        .id();
    run_steering_once(&mut s10);
    let vs10 = s10.get::<InertiaComponent>(p_s10).unwrap().target_velocity;
    assert!(
        vs10.x < -1.4 && vs10.z.abs() < 1e-5,
        "S10 reserve must pick (5,4) over richer-raw Ocean-free (5,6), got {vs10:?}"
    );

    // Lowest (row, col) tie-break: (3,6), (3,4), and (4,3) equal -> (3,4) center (45, 0, 35) wins.
    let mut tie = World::new();
    tie.insert_resource(LocomotionPolicy {
        enabled: true,
        drive_floor: 1.0,
        thirst_steering: false,
    });
    tie.insert_resource(GrassSteeringPolicy { enabled: true });
    let mut tf = uniform_field(10, 0.0, 10.0);
    tf.r[3 * 10 + 6] = 8.0;
    tf.r[3 * 10 + 4] = 8.0;
    tf.r[4 * 10 + 3] = 8.0;
    tie.insert_resource(tf);
    let pt = tie
        .spawn((
            Agent,
            Prey,
            Position(Vec3::new(55.0, 0.0, 55.0)),
            homeo(50.0, 100.0, 100.0, 100.0),
            InertiaComponent::default(),
        ))
        .id();
    run_steering_once(&mut tie);
    let expected = Vec3::new(-10.0, 0.0, -20.0).normalize() * PREY_MAX_SPEED;
    let vt = tie.get::<InertiaComponent>(pt).unwrap().target_velocity;
    assert!(
        (vt - expected).length() < 1e-4,
        "lowest (row, col) tie-break failed: expected {expected:?}, got {vt:?}"
    );

    // (d) LAND1 TerrainNavigation walkability: Ocean cell (5,6) has r=10.0 while walkable Grassland
    // (5,4) has r=6.0. With TerrainNavigation enabled prey steers -x toward (5,4); positive control
    // with enabled=false steers +x toward Ocean cell (5,6). Also predators ignore ResourceField.
    let mut land_w = World::new();
    land_w.insert_resource(LocomotionPolicy {
        enabled: true,
        drive_floor: 1.0,
        thirst_steering: false,
    });
    land_w.insert_resource(GrassSteeringPolicy { enabled: true });
    land_w.insert_resource(TerrainNavigation { enabled: true });
    land_w.insert_resource(MapBounds {
        min: Vec3::ZERO,
        max: Vec3::new(100.0, 0.0, 100.0),
    });
    let mut biomes = vec![BiomeType::Grassland as u8; 100];
    biomes[5 * 10 + 6] = BiomeType::Ocean as u8;
    let tm = TerrainMap {
        width: 10,
        height: 10,
        elevations: vec![10.0; 100],
        moistures: vec![0.5; 100],
        temperatures: vec![0.5; 100],
        biomes,
        flows: vec![0.0; 100],
        pois: Vec::new(),
    };
    land_w.insert_resource(tm);
    let mut lf = uniform_field(10, 0.0, 10.0);
    lf.r[5 * 10 + 6] = 10.0; // Ocean
    lf.r[5 * 10 + 4] = 6.0; // Grassland
    land_w.insert_resource(lf);
    let lp = land_w
        .spawn((
            Agent,
            Prey,
            Position(Vec3::new(55.0, 0.0, 55.0)),
            homeo(50.0, 100.0, 100.0, 100.0),
            InertiaComponent::default(),
        ))
        .id();
    let pred = land_w
        .spawn((
            Agent,
            Predator,
            Position(Vec3::new(15.0, 0.0, 55.0)),
            homeo(50.0, 100.0, 100.0, 100.0),
            InertiaComponent::default(),
        ))
        .id();
    run_steering_once(&mut land_w);
    let v_land = land_w.get::<InertiaComponent>(lp).unwrap().target_velocity;
    assert!(
        v_land.x < -1.4 && v_land.z.abs() < 1e-5,
        "with LAND1 enabled, prey must skip Ocean (5,6) and steer to walkable (5,4) (-x), got {v_land:?}"
    );
    let v_pred = land_w
        .get::<InertiaComponent>(pred)
        .unwrap()
        .target_velocity;
    assert!(
        v_pred.x > 1.9,
        "predator must chase Prey (+x) regardless of grass_steering, got {v_pred:?}"
    );

    // Positive control: TerrainNavigation { enabled: false } picks Ocean (5,6) (+x).
    land_w.insert_resource(TerrainNavigation { enabled: false });
    run_steering_once(&mut land_w);
    let v_no_land = land_w.get::<InertiaComponent>(lp).unwrap().target_velocity;
    assert!(
        v_no_land.x > 1.4 && v_no_land.z.abs() < 1e-5,
        "positive control without LAND1 should pick richer Ocean cell (5,6), got {v_no_land:?}"
    );
}

// ---- 4. Behavioural stripped-cell movement -----------------------------------------------------

#[test]
fn behavioural_agent_moves_from_stripped_cell_to_rich_neighbour_within_n_ticks() {
    const N_TICKS: u64 = 240;
    let run_stripped_probe = |grass_on: f64| -> (bool, f32, usize, usize) {
        let init = initial(&[
            (LIVE_KEY_FOUNDERS, 1.0),
            (LIVE_KEY_PREDATOR_FRACTION, 0.0),
            (LIVE_KEY_TREES, 0.0),
            (LIVE_KEY_FOOD_CAP, 0.0),
            (LIVE_KEY_LAKES, 2.0),
            (LIVE_KEY_MORTALITY, 0.0),
            (LIVE_KEY_REPRODUCTION, 0.0),
            (LIVE_KEY_LOCOMOTION, 1.0),
            (LIVE_KEY_DRIVE_FLOOR, 1.0),
            (LIVE_KEY_THIRST_STEERING, 0.0),
            (LIVE_KEY_GRASS_STEERING, grass_on),
        ]);
        let mut a = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &init,
            &[],
            11,
            (16, 16),
            N_TICKS,
        )
        .expect("probe builds");

        let (start_cell, rich_cell) = {
            let mut w = a.world();
            let pos = {
                let mut q = w.query_filtered::<
                    (&Position, &mut HomeostaticState),
                    (With<Agent>, With<Prey>),
                >();
                let (p, mut h) = q.iter_mut(&mut w).next().expect("founder");
                h.energy = h.energy_target * 0.5;
                p.0
            };
            let mut field = w.resource_mut::<ResourceField>();
            let cur_idx = field.cell_index(pos.x, pos.z).expect("inside field");
            let cur_row = cur_idx / field.width;
            let cur_col = cur_idx % field.width;
            let target_col = if cur_col + 2 < field.width {
                cur_col + 2
            } else {
                cur_col - 2
            };
            let target_idx = cur_row * field.width + target_col;
            field.r.fill(0.0);
            let target_cap = field.r_max[target_idx].max(1.0);
            field.r_max[target_idx] = target_cap;
            field.r[target_idx] = target_cap;
            (cur_idx, target_idx)
        };

        let mut reached_rich = false;
        for _ in 1..=N_TICKS {
            a.run_schedule_once();
            let mut w = a.world();
            let pos = {
                let mut q = w.query_filtered::<&Position, (With<Agent>, With<Prey>)>();
                q.iter(&w).next().expect("living prey").0
            };
            if w.resource::<ResourceField>().cell_index(pos.x, pos.z) == Some(rich_cell) {
                reached_rich = true;
                break;
            }
        }
        let mut w = a.world();
        let (pos, gained) = {
            let mut q =
                w.query_filtered::<(&Position, &AgentEvaluation), (With<Agent>, With<Prey>)>();
            let (p, eval) = q.iter(&w).next().expect("living prey");
            (p.0, eval.total_energy_gained)
        };
        let end_cell = w
            .resource::<ResourceField>()
            .cell_index(pos.x, pos.z)
            .unwrap_or(start_cell);
        (reached_rich, gained, start_cell, end_cell)
    };

    let (on_reached, on_gain, on_start, on_end) = run_stripped_probe(1.0);
    let (off_reached, off_gain, off_start, off_end) = run_stripped_probe(0.0);
    assert_eq!(on_start, off_start);
    assert!(
        on_reached && on_end != on_start && on_gain > 0.0,
        "grass_steering=1.0 prey must leave stripped cell {on_start} and reach rich neighbour (end={on_end}, reached={on_reached}, gain={on_gain})"
    );
    assert!(
        !off_reached && off_end == off_start && off_gain == 0.0,
        "positive control (grass_steering=0.0) must not move to rich neighbour (end={off_end}, reached={off_reached}, gain={off_gain})"
    );
}

// ---- 5. Crowded founders: grazing intake rises with grass_steering on vs off --------------------

fn measure_crowded_intake(
    trees: f64,
    food_cap: f64,
    grass_steering: f64,
    hungry_start: bool,
) -> (f64, f64, f64) {
    const TICKS: u64 = 500;
    let init = initial(&[
        (LIVE_KEY_FOUNDERS, 100.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.0),
        (LIVE_KEY_TREES, trees),
        (LIVE_KEY_FOOD_CAP, food_cap),
        (LIVE_KEY_LAKES, 16.0),
        (LIVE_KEY_LAKE_LAYOUT, 1.0),
        (LIVE_KEY_RESOURCE_CAPACITY_SCALE, 0.63),
        (LIVE_KEY_MORTALITY, 0.0),
        (LIVE_KEY_REPRODUCTION, 0.0),
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_DRIVE_FLOOR, 0.5),
        (LIVE_KEY_THIRST_STEERING, 1.0),
        (LIVE_KEY_GRASS_STEERING, grass_steering),
    ]);
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &init,
        &[],
        17,
        (16, 16),
        TICKS,
    )
    .expect("crowded world builds");
    if hungry_start {
        let mut w = a.world();
        let mut drained = 0.0f64;
        {
            let mut q = w.query_filtered::<&mut HomeostaticState, (With<Agent>, With<Prey>)>();
            for mut h in q.iter_mut(&mut w) {
                let target = 0.4 * h.energy_target;
                if h.energy > target {
                    drained += (h.energy - target) as f64;
                    h.energy = target;
                }
            }
        }
        w.resource_mut::<EcosystemBiomass>().detritus += drained;
    }
    for _ in 1..=TICKS {
        a.run_schedule_once();
    }
    let mut w = a.world();
    let field = w.resource::<ResourceField>().clone();
    let mut q = w.query_filtered::<
        (&Position, &HomeostaticState, &AgentEvaluation),
        (With<Agent>, With<Prey>),
    >();
    let mut count = 0usize;
    let mut total_gain = 0.0f64;
    let mut total_energy = 0.0f64;
    let mut depleted = 0usize;
    for (p, h, eval) in q.iter(&w) {
        count += 1;
        total_gain += eval.total_energy_gained as f64;
        total_energy += h.energy as f64;
        if let Some(idx) = field.cell_index(p.0.x, p.0.z) {
            let r_max = field.r_max[idx];
            if r_max > 0.0 && field.r[idx] < GRASS_STAY_FRACTION * r_max {
                depleted += 1;
            }
        }
    }
    assert_eq!(count, 100, "all 100 founders must be present");
    (
        total_gain / count as f64,
        total_energy / count as f64,
        depleted as f64 / count as f64,
    )
}

#[test]
fn crowded_founder_grazing_intake_rises_with_grass_steering_on_vs_off() {
    // 1. Pure grazing setup (trees = 0, food_cap = 0): total_energy_gained is 100% pasture
    // grazing intake across 100 crowded founders over 500 ticks.
    let (graze_off, energy_off_0, dep_off) = measure_crowded_intake(0.0, 0.0, 0.0, false);
    let (graze_on, energy_on_0, dep_on) = measure_crowded_intake(0.0, 0.0, 1.0, false);
    println!(
        "CROWDED_PURE_GRAZING off_intake={graze_off:.6} on_intake={graze_on:.6} \
         off_energy={energy_off_0:.4} on_energy={energy_on_0:.4} off_dep={dep_off:.4} on_dep={dep_on:.4}"
    );
    assert!(
        graze_on > graze_off,
        "pure grazing intake per agent must rise with grass_steering=1.0 ({graze_on:.6} vs {graze_off:.6})"
    );
    assert!(
        energy_on_0 > energy_off_0,
        "pure grazing mean energy must rise with grass_steering=1.0 ({energy_on_0:.4} vs {energy_off_0:.4})"
    );
    assert!(
        dep_on < dep_off,
        "depleted-cell share must fall with grass_steering=1.0 ({dep_on:.4} vs {dep_off:.4})"
    );

    // 2. Crowded setup with SEL2-P trees/food drops and hungry founders (0.4 * energy_target,
    // closed EU via detritus): verifies depleted-cell trapping drops without asserting that
    // total intake (grazing + 20 EU distant tree-fruit drops) exceeds whole-map Food chasing at
    // t = 500; short-horizon fruit-confounded intake is printed for SEL2-P2 diagnostics.
    let (sel2_off, sel2_energy_off, dep_sel2_off) = measure_crowded_intake(8.0, 50.0, 0.0, true);
    let (sel2_on, sel2_energy_on, dep_sel2_on) = measure_crowded_intake(8.0, 50.0, 1.0, true);
    println!(
        "CROWDED_SEL2_TREES off_intake={sel2_off:.6} on_intake={sel2_on:.6} \
         off_energy={sel2_energy_off:.4} on_energy={sel2_energy_on:.4} off_dep={dep_sel2_off:.4} on_dep={dep_sel2_on:.4}"
    );
    assert!(
        sel2_on > 0.0,
        "non-vacuity: hungry crowded founders must record positive intake ({sel2_on:.6})"
    );
    assert!(
        dep_sel2_on < dep_sel2_off,
        "depleted-cell share must drop under grass_steering=1.0 ({dep_sel2_on:.4} vs {dep_sel2_off:.4})"
    );
}

// ---- 6. Zero heap allocation once warm ---------------------------------------------------------

#[test]
fn steering_drive_system_allocates_nothing_once_warm_with_grass_steering_and_land_mask() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let init = initial(&[
        (LIVE_KEY_FOUNDERS, 40.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.2),
        (LIVE_KEY_TREES, 8.0),
        (LIVE_KEY_LAKES, 8.0),
        (LIVE_KEY_LAKE_LAYOUT, 1.0),
        (LIVE_KEY_FOOD_CAP, 50.0),
        (LIVE_KEY_MORTALITY, 1.0),
        (LIVE_KEY_REPRODUCTION, 1.0),
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_DRIVE_FLOOR, 0.5),
        (LIVE_KEY_THIRST_STEERING, 1.0),
        (LIVE_KEY_GRASS_STEERING, 1.0),
        (LIVE_KEY_LAND_MASK, 1.0),
    ]);
    let mut a =
        LiveExperimentAdapter::from_manifest(&WorldLawSet::baseline(), &init, &[], 5, (16, 16), 0)
            .expect("zero-alloc world builds");
    for _ in 0..50 {
        a.run_schedule_once();
    }

    let mut world = a.world();
    // Deplete current cells and make immediate neighbours rich so every prey runs the full 7x7
    // + LAND1 walkability scan inside the measured window.
    {
        let mut prey_positions = Vec::new();
        {
            let mut pq = world.query_filtered::<&Position, (With<Agent>, With<Prey>)>();
            for p in pq.iter(&world) {
                prey_positions.push(p.0);
            }
        }
        let mut field = world.resource_mut::<ResourceField>();
        let w = field.width;
        let h = field.height;
        for pos in prey_positions {
            if let Some(idx) = field.cell_index(pos.x, pos.z) {
                field.r[idx] = 0.0;
                let row = idx / w;
                let col = idx % w;
                let nb_col = if col + 1 < w {
                    col + 1
                } else {
                    col.saturating_sub(1)
                };
                let nb = row * w + nb_col;
                if nb < w * h {
                    field.r[nb] = field.r_max[nb].max(1.0);
                }
            }
        }
    }

    let mut only = Schedule::default();
    only.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    only.add_systems(steering_drive_system);
    for _ in 0..10 {
        only.run(&mut world);
    }

    ALLOCATOR.start_tracking();
    for _ in 0..200 {
        only.run(&mut world);
    }
    let allocations = ALLOCATOR.stop_tracking();
    assert_eq!(
        allocations, 0,
        "steering_drive_system allocated {allocations} times with grass_steering + land_mask on"
    );

    // Negative control: ensure allocator tracker catches real heap allocation on this thread.
    ALLOCATOR.start_tracking();
    let probe = std::hint::black_box(Vec::<u8>::with_capacity(64));
    drop(probe);
    assert!(ALLOCATOR.stop_tracking() >= 1);
}

// ---- 7. Deterministic checksum & closed-EU conservation ----------------------------------------

#[test]
fn grass_steering_preserves_closed_eu_and_determinism() {
    let init = initial(&[
        (LIVE_KEY_FOUNDERS, 30.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.2),
        (LIVE_KEY_TREES, 8.0),
        (LIVE_KEY_LAKES, 8.0),
        (LIVE_KEY_LAKE_LAYOUT, 1.0),
        (LIVE_KEY_FOOD_CAP, 50.0),
        (LIVE_KEY_MORTALITY, 1.0),
        (LIVE_KEY_REPRODUCTION, 1.0),
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_DRIVE_FLOOR, 0.5),
        (LIVE_KEY_THIRST_STEERING, 1.0),
        (LIVE_KEY_GRASS_STEERING, 1.0),
        (LIVE_KEY_LAND_MASK, 1.0),
    ]);
    let mut a1 = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &init,
        &[],
        23,
        (16, 16),
        1_000,
    )
    .expect("run 1 builds");
    let mut a2 = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &init,
        &[],
        23,
        (16, 16),
        1_000,
    )
    .expect("run 2 builds");

    let closed_eu = |a: &LiveExperimentAdapter| {
        let mut w = a.world();
        let obs = live_observables(&mut w);
        obs.iter()
            .find(|(k, _)| k == "live.closed_eu_total")
            .expect("closed_eu_total")
            .1
    };

    a1.run_schedule_once();
    a2.run_schedule_once();
    let start_eu = closed_eu(&a1);
    let mut worst_rel_drift = 0.0f64;
    for _ in 1..1_000 {
        a1.run_schedule_once();
        a2.run_schedule_once();
        worst_rel_drift =
            worst_rel_drift.max((closed_eu(&a1) - start_eu).abs() / start_eu.abs().max(1.0));
    }
    assert_eq!(
        a1.checksum(),
        a2.checksum(),
        "same seed with grass_steering=1.0 must stay deterministic"
    );
    assert!(
        worst_rel_drift < 1e-6,
        "closed EU drifted by {worst_rel_drift} relative to {start_eu}"
    );
}
