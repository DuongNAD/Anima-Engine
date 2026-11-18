//! Card S11 (`live.grass_steering`): opt-in grass-seeking steering for prey.
//!
//! Covers all verification criteria:
//! 1. Key absent and key = 0.0 give the exact pre-card trajectory at every sampled tick on worlds
//!    with locomotion on (`S11_TRAJ` lines match the unmodified base commit).
//! 2. Preflight validation refuses `live.grass_steering = 1.0` when `live.locomotion` is off/absent.
//! 3. Unit tests for `GRASS_STAY_FRACTION`, `GRASS_SCAN_RADIUS` (7x7 clipped window), S10
//!    `PlantReservePolicy` floor, lowest `(row, col)` tie-breaking, strictly-closer `Food`
//!    override + fallback, thirst-steering priority, and predators remaining unchanged.
//! 4. Behaviour in the SEL2-E best world (100 founders, 8 lakes, prey_cap 320,
//!    resource_capacity_scale 0.7, dehydration_ticks 960, seed 2, ticks 2,000-6,000 every 500):
//!    with `live.grass_steering = 1.0` the median of per-sample median `r / r_max` under prey is
//!    at least half the field median and at least one sampled prey reaches `energy >= 0.65 *
//!    energy_target`; with it off the same ratio stays below 0.15.
//! 5. Zero heap allocations once warm in `steering_drive_system` with grass steering on
//!    (`ThreadTrackingAllocator`, calling thread).
//! 6. 4-process determinism with grass steering on, plus closed-EU conservation (`< 1e-6`).

mod common;

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::components::{
    Agent, Food, InertiaComponent, Lake, ParentLink, Position, Predator, Prey,
};
use anima_engine_lib::core::ecology::ResourceField;
use anima_engine_lib::core::experiment::{ExperimentError, InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LiveWorldConfig, LIVE_KEYS, LIVE_KEY_GRASS_STEERING,
};
use anima_engine_lib::core::locomotion::{
    steering_drive_system, GRASS_SCAN_RADIUS, GRASS_STAY_FRACTION, PREY_MAX_SPEED,
};
use anima_engine_lib::core::resources::{LocomotionPolicy, PlantReservePolicy};
use bevy_ecs::prelude::*;
use glam::Vec3;
use std::process::Command;

#[global_allocator]
static ALLOCATOR: common::allocator::ThreadTrackingAllocator =
    common::allocator::ThreadTrackingAllocator::new();

const CHILD_ROLE: &str = "ANIMA_S11_CHILD";
const MARKER: &str = "S11_CHECKSUM";

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn initial(extra: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(extra.iter().map(|(k, v)| (k.to_string(), *v)).collect())
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
        width: side,
        height: side,
        min_x: 0.0,
        min_z: 0.0,
        max_x: (side * 10) as f32,
        max_z: (side * 10) as f32,
        r: vec![r; n],
        r_max: vec![r_max; n],
        growth_rate: 0.0,
        regrowth_phase: 0,
    }
}

fn run_steering_once(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(steering_drive_system);
    schedule.run(world);
}

fn median(values: &mut [f64]) -> f64 {
    assert!(!values.is_empty(), "median of empty slice");
    values.sort_by(|a, b| a.total_cmp(b));
    let mid = values.len() / 2;
    if values.len() % 2 == 1 {
        values[mid]
    } else {
        0.5 * (values[mid - 1] + values[mid])
    }
}

// ---- 1. Key registration and preflight validation ----------------------------------------------

#[test]
fn key_is_registered_and_refused_at_preflight_without_locomotion() {
    assert_eq!(LIVE_KEY_GRASS_STEERING, "live.grass_steering");
    assert_eq!(LIVE_KEYS.len(), 25);
    assert!(LIVE_KEYS.contains(&LIVE_KEY_GRASS_STEERING));
    assert!(!LiveWorldConfig::default().grass_steering);

    // Explicit 0.0 is accepted even when locomotion is off/absent.
    let zero_only =
        LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_GRASS_STEERING, 0.0)]))
            .expect("0.0 is valid without locomotion");
    assert!(!zero_only.grass_steering);

    // 1.0 with locomotion = 1.0 succeeds.
    let on = LiveWorldConfig::from_initial_conditions(&initial(&[
        ("live.locomotion", 1.0),
        (LIVE_KEY_GRASS_STEERING, 1.0),
    ]))
    .expect("1.0 with locomotion = 1.0 is valid");
    assert!(on.grass_steering);

    // 1.0 without locomotion (absent or 0.0) must be refused at preflight.
    for extra in [
        vec![(LIVE_KEY_GRASS_STEERING, 1.0)],
        vec![("live.locomotion", 0.0), (LIVE_KEY_GRASS_STEERING, 1.0)],
    ] {
        let init = initial(&extra);
        let err = LiveWorldConfig::from_initial_conditions(&init)
            .expect_err("grass_steering = 1.0 without locomotion must fail");
        assert!(
            matches!(err, ExperimentError::InvalidLaw { .. }),
            "expected InvalidLaw, got {err:?}"
        );
        let adapter_res = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &init,
            &[],
            1,
            (16, 16),
            0,
        );
        assert!(
            matches!(adapter_res, Err(ExperimentError::InvalidLaw { .. })),
            "from_manifest must refuse grass_steering = 1.0 at preflight"
        );
    }

    // Non-boolean numbers must be rejected.
    for bad in [-1.0, 0.5, 2.0, f64::NAN, f64::INFINITY] {
        assert!(
            LiveWorldConfig::from_initial_conditions(&initial(&[
                ("live.locomotion", 1.0),
                (LIVE_KEY_GRASS_STEERING, bad),
            ]))
            .is_err(),
            "{bad:?} must be refused"
        );
    }
}

// ---- 2. Trajectory equivalence when absent or 0.0 ----------------------------------------------

fn sample_trajectory(
    label: Option<&str>,
    init: &InitialConditionSet,
    seed: u64,
    ticks: u64,
) -> Vec<(u64, u32)> {
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        init,
        &[],
        seed,
        (16, 16),
        ticks,
    )
    .expect("the live world builds");
    let mut out = Vec::new();
    for tick in 1..=ticks {
        a.run_schedule_once();
        if tick % 100 == 0 {
            let sum = a.checksum();
            if let Some(tag) = label {
                println!("S11_TRAJ world={tag} tick={tick} checksum={sum:#010x}");
            }
            out.push((tick, sum));
        }
    }
    out
}

#[test]
fn absent_and_zero_match_every_sampled_tick_with_locomotion_on() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let world_a_base = [("live.locomotion", 1.0)];
    let world_b_base = [
        ("live.founders", 30.0),
        ("live.predator_fraction", 0.0),
        ("live.trees", 8.0),
        ("live.lakes", 4.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
        ("live.locomotion", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
        ("live.plant_reserve", 0.2),
    ];

    let a_absent = sample_trajectory(Some("A"), &initial(&world_a_base), 2, 600);
    let mut a_zero_pairs = world_a_base.to_vec();
    a_zero_pairs.push((LIVE_KEY_GRASS_STEERING, 0.0));
    let a_zero = sample_trajectory(None, &initial(&a_zero_pairs), 2, 600);
    assert_eq!(a_absent, a_zero, "World A: absent vs 0.0 diverged");

    let mut a_on_pairs = world_a_base.to_vec();
    a_on_pairs.push((LIVE_KEY_GRASS_STEERING, 1.0));
    let a_on = sample_trajectory(None, &initial(&a_on_pairs), 2, 600);
    assert_ne!(
        a_on, a_absent,
        "World A: grass_steering = 1.0 had no effect"
    );

    let b_absent = sample_trajectory(Some("B"), &initial(&world_b_base), 2, 1000);
    let mut b_zero_pairs = world_b_base.to_vec();
    b_zero_pairs.push((LIVE_KEY_GRASS_STEERING, 0.0));
    let b_zero = sample_trajectory(None, &initial(&b_zero_pairs), 2, 1000);
    assert_eq!(b_absent, b_zero, "World B: absent vs 0.0 diverged");

    let mut b_on_pairs = world_b_base.to_vec();
    b_on_pairs.push((LIVE_KEY_GRASS_STEERING, 1.0));
    let b_on = sample_trajectory(None, &initial(&b_on_pairs), 2, 1000);
    assert_ne!(
        b_on, b_absent,
        "World B: grass_steering = 1.0 had no effect"
    );
}

// ---- 3. Unit semantics: stay threshold, S10 reserve, 7x7 window, tie-breaking, Food, thirst ----

#[test]
fn prey_stays_above_threshold_and_respects_s10_plant_reserve() {
    assert!((GRASS_STAY_FRACTION - 0.25).abs() < f32::EPSILON);
    assert_eq!(GRASS_SCAN_RADIUS, 3);

    // (a) Current cell grazeable == 0.25 * r_max -> stays (target_velocity == ZERO) even with richer
    // neighbour and nearby Food.
    let mut world = World::new();
    world.insert_resource(LocomotionPolicy {
        enabled: true,
        drive_floor: 1.0,
        thirst_steering: true,
        grass_steering: true,
    });
    let mut field = uniform_field(10, 2.5, 10.0); // cell (5,5) at 25% of 10.0
    field.r[5 * 10 + 6] = 10.0; // richer cell right next door
    world.insert_resource(field);
    world.spawn((
        Food {
            energy_value: 20.0,
            hydration_value: 0.0,
        },
        Position(Vec3::new(65.0, 0.0, 55.0)),
    ));
    let prey = world
        .spawn((
            Agent,
            Prey,
            Position(Vec3::new(55.0, 0.0, 55.0)),
            homeo(50.0, 100.0, 100.0, 100.0),
            InertiaComponent {
                target_velocity: Vec3::new(1.0, 0.0, 0.0),
                ..Default::default()
            },
        ))
        .id();
    run_steering_once(&mut world);
    assert_eq!(
        world.get::<InertiaComponent>(prey).unwrap().target_velocity,
        Vec3::ZERO,
        "at exact GRASS_STAY_FRACTION the prey must stay and graze"
    );

    // (b) Just below threshold (0.24 * r_max) -> leaves toward richer neighbour (5, 6) centre (65, 55).
    world.resource_mut::<ResourceField>().r[5 * 10 + 5] = 2.4;
    run_steering_once(&mut world);
    let vel = world.get::<InertiaComponent>(prey).unwrap().target_velocity;
    assert!(
        vel.x > 1.4 && vel.z.abs() < 1e-5,
        "below stay fraction the prey must steer toward richer cell (+x), got {vel:?}"
    );

    // (c) Barren current cell (r_max == 0.0) must NOT freeze on ZERO >= 0.0.
    {
        let mut f = world.resource_mut::<ResourceField>();
        f.r[5 * 10 + 5] = 0.0;
        f.r_max[5 * 10 + 5] = 0.0;
    }
    run_steering_once(&mut world);
    let vel_barren = world.get::<InertiaComponent>(prey).unwrap().target_velocity;
    assert!(
        vel_barren.x > 1.4,
        "barren current cell (r_max=0) must steer out to richer grass, got {vel_barren:?}"
    );

    // (d) S10 PlantReservePolicy (rho = 0.3):
    //     - Current cell (5,5): r = 4.5, r_max = 10.0 -> floor = 3.0 -> grazeable = 1.5 < 2.5 (leaves!)
    //     - Cell (5,4): r = 5.0, r_max = 10.0 -> floor = 3.0 -> grazeable = 2.0
    //     - Cell (5,6): r = 5.5, r_max = 15.0 -> floor = 4.5 -> grazeable = 1.0
    //     By raw r, (5,6) > (5,4), but by grazeable (r - floor), (5,4) wins (2.0 > 1.0)!
    let mut reserve_world = World::new();
    reserve_world.insert_resource(LocomotionPolicy {
        enabled: true,
        drive_floor: 1.0,
        thirst_steering: false,
        grass_steering: true,
    });
    reserve_world.insert_resource(PlantReservePolicy { fraction: 0.3 });
    let mut rf = uniform_field(10, 0.0, 10.0);
    rf.r[5 * 10 + 5] = 4.5;
    rf.r_max[5 * 10 + 5] = 10.0;
    rf.r[5 * 10 + 4] = 5.0;
    rf.r_max[5 * 10 + 4] = 10.0;
    rf.r[5 * 10 + 6] = 5.5;
    rf.r_max[5 * 10 + 6] = 15.0;
    reserve_world.insert_resource(rf);
    let p2 = reserve_world
        .spawn((
            Agent,
            Prey,
            Position(Vec3::new(55.0, 0.0, 55.0)),
            homeo(50.0, 100.0, 100.0, 100.0),
            InertiaComponent::default(),
        ))
        .id();
    run_steering_once(&mut reserve_world);
    let v2 = reserve_world
        .get::<InertiaComponent>(p2)
        .unwrap()
        .target_velocity;
    assert!(
        v2.x < -1.4 && v2.z.abs() < 1e-5,
        "S10 reserve floor must rank (5,4) grazeable=2.0 above (5,6) grazeable=1.0 (-x), got {v2:?}"
    );
}

#[test]
fn window_clipping_tie_breaking_food_strictly_closer_thirst_priority_and_predators_unchanged() {
    // (a) Corner clipping + GRASS_SCAN_RADIUS = 3:
    // Prey at cell (0,0) = (5, 0, 5). Cell (0,4) at distance 4 has r=10.0 (outside 7x7 window!),
    // while cell (0,3) has r=4.0 (inside 7x7 window). Expect steering toward (0,3) (+x), and with
    // (0,3) empty and no Food, ZERO (distance-4 cell is ignored).
    let mut world = World::new();
    world.insert_resource(LocomotionPolicy {
        enabled: true,
        drive_floor: 1.0,
        thirst_steering: true,
        grass_steering: true,
    });
    let mut field = uniform_field(10, 0.0, 10.0);
    field.r[4] = 10.0; // (row=0, col=4) outside radius 3
    world.insert_resource(field);
    let prey = world
        .spawn((
            Agent,
            Prey,
            Position(Vec3::new(5.0, 0.0, 5.0)),
            homeo(50.0, 100.0, 100.0, 100.0),
            InertiaComponent::default(),
        ))
        .id();
    run_steering_once(&mut world);
    assert_eq!(
        world.get::<InertiaComponent>(prey).unwrap().target_velocity,
        Vec3::ZERO,
        "cell at col=4 must be outside GRASS_SCAN_RADIUS=3 from col=0"
    );

    world.resource_mut::<ResourceField>().r[3] = 4.0; // (row=0, col=3) inside radius 3
    run_steering_once(&mut world);
    let v_corner = world.get::<InertiaComponent>(prey).unwrap().target_velocity;
    assert!(
        v_corner.x > 1.4 && v_corner.z.abs() < 1e-5,
        "clipped corner window must find (0,3), got {v_corner:?}"
    );

    // (b) Tie-breaking by lowest (row, col):
    // Prey at (5,5) = (55, 0, 55), empty field except (3,6), (3,4), and (4,3) all at 8.0.
    // Lowest (row, col) is (3, 4) whose centre is (45, 0, 35) -> dx = -10, dz = -20.
    let mut tie_world = World::new();
    tie_world.insert_resource(LocomotionPolicy {
        enabled: true,
        drive_floor: 1.0,
        thirst_steering: true,
        grass_steering: true,
    });
    let mut tf = uniform_field(10, 0.0, 10.0);
    tf.r[3 * 10 + 6] = 8.0;
    tf.r[3 * 10 + 4] = 8.0;
    tf.r[4 * 10 + 3] = 8.0;
    tie_world.insert_resource(tf);
    let tp = tie_world
        .spawn((
            Agent,
            Prey,
            Position(Vec3::new(55.0, 0.0, 55.0)),
            homeo(50.0, 100.0, 100.0, 100.0),
            InertiaComponent::default(),
        ))
        .id();
    run_steering_once(&mut tie_world);
    let expected_dir = Vec3::new(-10.0, 0.0, -20.0).normalize() * PREY_MAX_SPEED;
    let got = tie_world
        .get::<InertiaComponent>(tp)
        .unwrap()
        .target_velocity;
    assert!(
        (got - expected_dir).length() < 1e-4,
        "tie-breaking must pick lowest (row, col) = (3, 4); expected {expected_dir:?}, got {got:?}"
    );

    // (c) Food override: grass target (3,4) centre (45, 0, 35) is at dist sqrt(500).
    // Place Food on +z at exact equal distance sqrt(500) -> grass still wins.
    let eq_dist = 500.0f32.sqrt();
    let food = tie_world
        .spawn((
            Food {
                energy_value: 20.0,
                hydration_value: 0.0,
            },
            Position(Vec3::new(55.0, 0.0, 55.0 + eq_dist)),
        ))
        .id();
    run_steering_once(&mut tie_world);
    let got_eq = tie_world
        .get::<InertiaComponent>(tp)
        .unwrap()
        .target_velocity;
    assert!(
        (got_eq - expected_dir).length() < 1e-4,
        "equal-distance Food must NOT override grass target; got {got_eq:?}"
    );

    // Move Food strictly closer (distance 5.0 along +z) -> Food wins!
    tie_world.get_mut::<Position>(food).unwrap().0 = Vec3::new(55.0, 0.0, 60.0);
    run_steering_once(&mut tie_world);
    let got_closer = tie_world
        .get::<InertiaComponent>(tp)
        .unwrap()
        .target_velocity;
    assert!(
        got_closer.z > 1.4 && got_closer.x.abs() < 1e-5,
        "strictly closer Food (+z) must win over grass target, got {got_closer:?}"
    );

    // (d) No cell in window has more grazeable grass than current cell -> keep existing Food target.
    tie_world.get_mut::<Position>(food).unwrap().0 = Vec3::new(55.0, 0.0, 95.0); // far Food
    {
        let mut f = tie_world.resource_mut::<ResourceField>();
        f.r.fill(0.0); // current and all neighbours have 0.0 (none strictly higher)
    }
    run_steering_once(&mut tie_world);
    let got_fallback = tie_world
        .get::<InertiaComponent>(tp)
        .unwrap()
        .target_velocity;
    assert!(
        got_fallback.z > 1.4 && got_fallback.x.abs() < 1e-5,
        "when no 7x7 cell beats current cell, prey must fall back to Food (+z), got {got_fallback:?}"
    );

    // (e) Thirst steering keeps priority over grass steering when a non-empty lake exists.
    tie_world.resource_mut::<ResourceField>().r[5 * 10 + 4] = 10.0; // rich grass on -x
    let lake = tie_world
        .spawn((
            Lake {
                current_water: 50.0,
                max_water: 100.0,
                replenishment_rate: 1.0,
            },
            Position(Vec3::new(90.0, 0.0, 55.0)), // lake on +x
        ))
        .id();
    tie_world.get_mut::<HomeostaticState>(tp).unwrap().hydration = 10.0; // < 0.5 * 100
    run_steering_once(&mut tie_world);
    let got_thirst = tie_world
        .get::<InertiaComponent>(tp)
        .unwrap()
        .target_velocity;
    assert!(
        got_thirst.x > 1.4 && got_thirst.z.abs() < 1e-5,
        "thirst steering must keep priority over grass steering (+x lake), got {got_thirst:?}"
    );
    // Dry lake -> falls back to grass (-x).
    tie_world.get_mut::<Lake>(lake).unwrap().current_water = 0.0;
    run_steering_once(&mut tie_world);
    let got_dry_lake = tie_world
        .get::<InertiaComponent>(tp)
        .unwrap()
        .target_velocity;
    assert!(
        got_dry_lake.x < -1.4,
        "dry lake must not capture thirst steering; prey should steer to grass (-x), got {got_dry_lake:?}"
    );

    // (f) Predators unchanged: predator at (55, 0, 55) chases prey even with grass around it.
    let mut pred_world = World::new();
    pred_world.insert_resource(LocomotionPolicy {
        enabled: true,
        drive_floor: 1.0,
        thirst_steering: false,
        grass_steering: true,
    });
    let mut pf = uniform_field(10, 10.0, 10.0);
    pf.r[5 * 10 + 4] = 10.0;
    pred_world.insert_resource(pf);
    let pred = pred_world
        .spawn((
            Agent,
            Predator,
            Position(Vec3::new(55.0, 0.0, 55.0)),
            homeo(50.0, 100.0, 100.0, 100.0),
            InertiaComponent::default(),
        ))
        .id();
    pred_world.spawn((
        Agent,
        Prey,
        Position(Vec3::new(55.0, 0.0, 80.0)),
        homeo(50.0, 100.0, 100.0, 100.0),
    ));
    run_steering_once(&mut pred_world);
    let got_pred = pred_world
        .get::<InertiaComponent>(pred)
        .unwrap()
        .target_velocity;
    assert!(
        got_pred.z > 1.9 && got_pred.x.abs() < 1e-5,
        "predators must keep chasing prey (+z) regardless of grass_steering, got {got_pred:?}"
    );
}

// ---- 4. Behaviour in the SEL2-E best world -----------------------------------------------------

fn measure_sel2e_best_world(grass_steering: f64) -> (f64, f64, usize) {
    const TICKS: u64 = 6_000;
    const SAMPLE_START: u64 = 2_000;
    const SAMPLE_EVERY: u64 = 500;
    let init = initial(&[
        ("live.founders", 100.0),
        ("live.predator_fraction", 0.0),
        ("live.trees", 8.0),
        ("live.lakes", 8.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
        ("live.locomotion", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
        ("live.prey_cap", 320.0),
        ("live.resource_capacity_scale", 0.7),
        ("live.dehydration_ticks", 960.0),
        (LIVE_KEY_GRASS_STEERING, grass_steering),
    ]);
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &init,
        &[],
        2,
        (16, 16),
        TICKS,
    )
    .expect("SEL2-E best world builds");

    let mut prey_sample_medians = Vec::new();
    let mut field_sample_medians = Vec::new();
    let mut total_breed_ready = 0usize;

    for tick in 1..=TICKS {
        a.run_schedule_once();
        if tick < SAMPLE_START || tick % SAMPLE_EVERY != 0 {
            continue;
        }
        let mut w = a.world();
        let field = w.resource::<ResourceField>().clone();
        let mut field_ratios: Vec<f64> = field
            .r
            .iter()
            .zip(field.r_max.iter())
            .filter_map(|(r, m)| {
                if *m > 0.0 {
                    Some((*r / *m) as f64)
                } else {
                    None
                }
            })
            .collect();
        let field_med = median(&mut field_ratios);

        let mut q = w.query_filtered::<(&HomeostaticState, &Position), (With<Agent>, With<Prey>, Without<ParentLink>)>();
        let mut under_prey_ratios = Vec::new();
        let mut breed_ready_here = 0usize;
        for (h, p) in q.iter(&w) {
            if h.energy_target > 0.0 && h.energy >= 0.65 * h.energy_target {
                breed_ready_here += 1;
            }
            if let Some(idx) = field.cell_index(p.0.x, p.0.z) {
                if field.r_max[idx] > 0.0 {
                    under_prey_ratios.push((field.r[idx] / field.r_max[idx]) as f64);
                }
            }
        }
        let prey_med = median(&mut under_prey_ratios);
        println!(
            "S11_BEHAVIOUR grass={grass_steering} tick={tick} prey_n={} under_prey_med={prey_med:.4} field_med={field_med:.4} breed_ready={breed_ready_here}",
            under_prey_ratios.len()
        );
        prey_sample_medians.push(prey_med);
        field_sample_medians.push(field_med);
        total_breed_ready += breed_ready_here;
    }

    let overall_prey_med = median(&mut prey_sample_medians);
    let overall_field_med = median(&mut field_sample_medians);
    println!(
        "S11_BEHAVIOUR_SUMMARY grass={grass_steering} median_under_prey={overall_prey_med:.4} median_field={overall_field_med:.4} total_breed_ready={total_breed_ready}"
    );
    (overall_prey_med, overall_field_med, total_breed_ready)
}

#[test]
fn sel2e_best_world_behaviour_restores_under_prey_grass_and_breeding_energy() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (off_prey, off_field, off_breed) = measure_sel2e_best_world(0.0);
    let (on_prey, on_field, on_breed) = measure_sel2e_best_world(1.0);

    assert!(
        off_prey < 0.15,
        "control (grass_steering=0) under-prey median {off_prey:.4} expected < 0.15 (field {off_field:.4}, breed {off_breed})"
    );
    assert!(
        on_prey >= 0.5 * on_field,
        "treatment (grass_steering=1) under-prey median {on_prey:.4} is less than half field median {on_field:.4}"
    );
    assert!(
        on_breed >= 1,
        "treatment (grass_steering=1) had no sampled prey reaching >= 0.65 * energy_target"
    );
}

// ---- 5. Zero heap allocation once warm ---------------------------------------------------------

#[test]
fn steering_drive_system_allocates_nothing_once_warm_with_grass_steering_on() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let init = initial(&[
        ("live.founders", 30.0),
        ("live.predator_fraction", 0.2),
        ("live.locomotion", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
        ("live.plant_reserve", 0.2),
        (LIVE_KEY_GRASS_STEERING, 1.0),
    ]);
    let mut a =
        LiveExperimentAdapter::from_manifest(&WorldLawSet::baseline(), &init, &[], 2, (16, 16), 0)
            .expect("builds");
    for _ in 0..50 {
        a.run_schedule_once();
    }
    let mut world = a.world();
    // Force some prey below GRASS_STAY_FRACTION so both stay-and-graze and 7x7 scan execute.
    {
        let mut q =
            world.query_filtered::<&Position, (With<Agent>, With<Prey>, Without<ParentLink>)>();
        let prey_positions: Vec<Vec3> = q
            .iter(&world)
            .enumerate()
            .filter_map(|(idx, p)| if idx % 2 == 0 { Some(p.0) } else { None })
            .collect();
        let mut field_mut = world.resource_mut::<ResourceField>();
        for pos in prey_positions {
            if let Some(cell) = field_mut.cell_index(pos.x, pos.z) {
                field_mut.r[cell] = 0.0;
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
    println!("S11_ZERO_ALLOC allocations={allocations}");
    assert_eq!(
        allocations, 0,
        "steering_drive_system allocated {allocations} times with grass_steering on"
    );

    // Negative control: verify ThreadTrackingAllocator observes a real heap allocation on this thread.
    ALLOCATOR.start_tracking();
    let probe = std::hint::black_box(Vec::<u8>::with_capacity(64));
    drop(probe);
    let control_allocs = ALLOCATOR.stop_tracking();
    assert!(
        control_allocs >= 1,
        "ThreadTrackingAllocator negative control failed: observed {control_allocs}"
    );
}

// ---- 6. Multi-process determinism and closed EU ------------------------------------------------

fn grass_on_config() -> InitialConditionSet {
    initial(&[
        ("live.founders", 30.0),
        ("live.predator_fraction", 0.2),
        ("live.trees", 8.0),
        ("live.lakes", 4.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
        ("live.locomotion", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
        ("live.plant_reserve", 0.2),
        (LIVE_KEY_GRASS_STEERING, 1.0),
    ])
}

#[test]
fn s11_child_entry_point() {
    if std::env::var(CHILD_ROLE).is_err() {
        return;
    }
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &grass_on_config(),
        &[],
        2,
        (16, 16),
        1_500,
    )
    .expect("builds");
    for _ in 0..1_500 {
        a.run_schedule_once();
    }
    println!("{MARKER} {:#010x}", a.checksum());
}

fn checksum_from_child() -> u32 {
    let output = Command::new(std::env::current_exe().expect("current_exe"))
        .args(["--exact", "s11_child_entry_point", "--nocapture"])
        .env(CHILD_ROLE, "1")
        .env("ANIMA_USE_GPU", "0")
        .output()
        .expect("spawn child");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|l| l.trim_start().starts_with(MARKER))
        .unwrap_or_else(|| {
            panic!(
                "child printed no checksum\n{stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
    let hex = line.trim().rsplit(' ').next().expect("token");
    u32::from_str_radix(hex.trim_start_matches("0x"), 16).expect("hex checksum")
}

#[test]
fn grass_steering_checksum_agrees_across_four_processes_and_keeps_closed_eu() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sums: Vec<u32> = (0..4).map(|_| checksum_from_child()).collect();
    println!(
        "S11_CHECKSUMS {}",
        sums.iter()
            .map(|s| format!("{s:#010x}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    assert!(
        sums.iter().all(|s| *s == sums[0]),
        "processes disagree: {sums:x?}"
    );
    assert_ne!(sums[0], 0);

    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &grass_on_config(),
        &[],
        2,
        (16, 16),
        2_000,
    )
    .expect("builds");
    let closed_eu = |a: &LiveExperimentAdapter| {
        a.observables()
            .iter()
            .find(|(k, _)| k == "live.closed_eu_total")
            .expect("closed_eu_total")
            .1
    };
    a.run_schedule_once();
    let start = closed_eu(&a);
    let mut worst = 0.0_f64;
    for _ in 1..2_000 {
        a.run_schedule_once();
        worst = worst.max((closed_eu(&a) - start).abs() / start.abs().max(1.0));
    }
    println!("S11_EU start={start} worst_relative_drift={worst}");
    assert!(worst < 1e-6, "closed EU drifted by {worst} (relative)");
}
