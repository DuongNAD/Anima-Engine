//! ADR-0007 option B ("thẻ B"): `live.kill_hydration`, `live.kill_efficiency` and
//! `live.target_min_energy`, all opt-in; a world without them is bit-identical to today.
//!
//! * (a) default unchanged: keys absent == the defaults declared explicitly (compared with each
//!   other, never against a pinned checksum)
//! * (b) each key has a behavioural effect, asserted on a staged encounter, in BOTH strike branches
//!   of `combat_system` (with and without the `CombatEvents` telemetry resource)
//! * (c) closed EU with all three keys on
//! * (d) three-process determinism with the keys on
//! * (e) restore: run N == run K -> snapshot -> from_snapshot -> run N-K, with the policy restored
//! * (f) key registration and validation

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::components::{Agent, InertiaComponent, Position, Predator, Prey};
use anima_engine_lib::core::ecology::{EcosystemBiomass, LINDEMAN_EFFICIENCY};
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LiveWorldConfig, LIVE_KEYS, LIVE_KEY_KILL_EFFICIENCY,
    LIVE_KEY_KILL_HYDRATION, LIVE_KEY_TARGET_MIN_ENERGY,
};
use anima_engine_lib::core::locomotion::steering_drive_system;
use anima_engine_lib::core::resources::{CombatEvents, LocomotionPolicy, PredatorIncomePolicy};
use anima_engine_lib::core::world_systems::combat_system;
use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;
use glam::Vec3;
use std::process::Command;

const CHILD_ROLE: &str = "ANIMA_ECOB_CHILD";
const MARKER: &str = "ECOB_CHECKSUM";

fn initial(extra: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(extra.iter().map(|(k, v)| (k.to_string(), *v)).collect())
}

fn build(init: &InitialConditionSet, seed: u64) -> LiveExperimentAdapter {
    LiveExperimentAdapter::from_manifest(&WorldLawSet::baseline(), init, &[], seed, (16, 16), 0)
        .expect("the live world builds")
}

fn checksum_after(init: &InitialConditionSet, seed: u64, ticks: u64) -> u32 {
    let mut a = build(init, seed);
    for _ in 0..ticks {
        a.run_schedule_once();
    }
    a.checksum()
}

/// A world on which predators strike and steer: locomotion with a drive floor, both mortality and
/// reproduction so the population turns over.
fn base_keys() -> Vec<(&'static str, f64)> {
    vec![
        ("live.founders", 10.0),
        ("live.predator_fraction", 0.3),
        ("live.trees", 8.0),
        ("live.lakes", 2.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
        ("live.locomotion", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
    ]
}

fn with(extra: &[(&'static str, f64)]) -> InitialConditionSet {
    let mut v = base_keys();
    v.extend_from_slice(extra);
    initial(&v)
}

fn all_b_keys() -> InitialConditionSet {
    with(&[
        ("live.kill_hydration", 1.0),
        ("live.kill_efficiency", 0.6),
        ("live.target_min_energy", 10.0),
    ])
}

// ---- (a) ---------------------------------------------------------------------------------------

#[test]
fn a_absent_keys_equal_the_defaults_declared_explicitly() {
    const N: u64 = 1_500;
    for seed in [1u64, 2] {
        let absent = checksum_after(&with(&[]), seed, N);
        let defaults = checksum_after(
            &with(&[
                ("live.kill_hydration", 0.0),
                ("live.kill_efficiency", LINDEMAN_EFFICIENCY as f64),
                ("live.target_min_energy", 0.0),
            ]),
            seed,
            N,
        );
        assert_eq!(
            defaults, absent,
            "seed {seed}: declaring k=0, e=0.40, E=0 changed the world"
        );
    }
}

#[test]
fn a_the_world_resource_defaults_are_the_legacy_values() {
    let d = PredatorIncomePolicy::default();
    assert_eq!(
        (d.kill_hydration, d.kill_efficiency, d.target_min_energy),
        (0.0, LINDEMAN_EFFICIENCY, 0.0)
    );
    let cfg = LiveWorldConfig::default();
    assert_eq!(
        (
            cfg.kill_hydration,
            cfg.kill_efficiency,
            cfg.target_min_energy
        ),
        (0.0, LINDEMAN_EFFICIENCY, 0.0)
    );
}

// ---- (b) staged strike -------------------------------------------------------------------------

fn homeo(energy: f32, hydration: f32) -> HomeostaticState {
    HomeostaticState {
        energy,
        energy_target: 100.0,
        hydration,
        hydration_target: 100.0,
        ..Default::default()
    }
}

struct Strike {
    pred_energy_gain: f64,
    pred_hydration_gain: f32,
    prey_energy_lost: f64,
    detritus: f64,
}

/// One `combat_system` pass: a hungry, thirsty predator beside one prey. `telemetry` selects the
/// strike branch (the `CombatEvents` resource present or absent).
fn strike(policy: Option<PredatorIncomePolicy>, telemetry: bool, pred_hydration: f32) -> Strike {
    let mut world = World::new();
    if telemetry {
        world.insert_resource(CombatEvents::default());
    }
    if let Some(p) = policy {
        world.insert_resource(p);
    }
    world.insert_resource(EcosystemBiomass::default());
    let pred = world
        .spawn((
            Agent,
            Predator,
            Position(Vec3::ZERO),
            homeo(20.0, pred_hydration),
        ))
        .id();
    let prey = world
        .spawn((
            Agent,
            Prey,
            Position(Vec3::new(1.0, 0.0, 0.0)),
            homeo(80.0, 50.0),
        ))
        .id();
    world.run_system_once(combat_system);
    let p = world.get::<HomeostaticState>(pred).unwrap().clone();
    let q = world.get::<HomeostaticState>(prey).unwrap().clone();
    Strike {
        pred_energy_gain: p.energy as f64 - 20.0,
        pred_hydration_gain: p.hydration - pred_hydration,
        prey_energy_lost: 80.0 - q.energy as f64,
        detritus: world.resource::<EcosystemBiomass>().detritus,
    }
}

fn pol(k: f32, e: f32, min_e: f32) -> Option<PredatorIncomePolicy> {
    Some(PredatorIncomePolicy {
        kill_hydration: k,
        kill_efficiency: e,
        target_min_energy: min_e,
    })
}

#[test]
fn b_kill_hydration_adds_k_times_the_energy_gained_in_both_strike_branches() {
    for telemetry in [false, true] {
        let off = strike(pol(0.0, LINDEMAN_EFFICIENCY, 0.0), telemetry, 10.0);
        assert!(off.pred_energy_gain > 0.5, "the staged strike must land");
        assert_eq!(off.pred_hydration_gain, 0.0, "k = 0 must not water");
        let absent = strike(None, telemetry, 10.0);
        assert_eq!(absent.pred_hydration_gain, 0.0);
        assert_eq!(absent.pred_energy_gain, off.pred_energy_gain);

        let k = 0.5f32;
        let on = strike(pol(k, LINDEMAN_EFFICIENCY, 0.0), telemetry, 10.0);
        let expected = k * on.pred_energy_gain as f32;
        println!(
            "ECOB_KILL telemetry={telemetry} dE={} dHydration={} expected={expected}",
            on.pred_energy_gain, on.pred_hydration_gain
        );
        assert!(
            (on.pred_hydration_gain - expected).abs() < 1e-4,
            "telemetry={telemetry}: hydration rose by {} not k*dE = {expected}",
            on.pred_hydration_gain
        );
        // The energy side of the strike is untouched by the hydration knob.
        assert_eq!(on.pred_energy_gain, off.pred_energy_gain);
        assert_eq!(on.detritus, off.detritus);

        // Capped at the target: a nearly full predator cannot exceed 100.
        let capped = strike(pol(10.0, LINDEMAN_EFFICIENCY, 0.0), telemetry, 99.0);
        assert!(
            (capped.pred_hydration_gain - 1.0).abs() < 1e-4,
            "telemetry={telemetry}: hydration exceeded its target: +{}",
            capped.pred_hydration_gain
        );
    }
}

#[test]
fn b_kill_efficiency_replaces_lindeman_in_both_branches_and_the_rest_goes_to_detritus() {
    for telemetry in [false, true] {
        let base = strike(pol(0.0, LINDEMAN_EFFICIENCY, 0.0), telemetry, 10.0);
        let better = strike(pol(0.0, 0.6, 0.0), telemetry, 10.0);
        println!(
            "ECOB_EFF telemetry={telemetry} gain 0.40={} 0.60={} detritus 0.40={} 0.60={}",
            base.pred_energy_gain, better.pred_energy_gain, base.detritus, better.detritus
        );
        assert!(
            better.pred_energy_gain > base.pred_energy_gain * 1.3,
            "telemetry={telemetry}: e = 0.6 did not raise the predator's gain: {} vs {}",
            better.pred_energy_gain,
            base.pred_energy_gain
        );
        assert!(
            better.detritus < base.detritus,
            "telemetry={telemetry}: a more efficient strike must return less to detritus"
        );
        // Same capture on the prey side, so the whole difference is the efficiency.
        assert_eq!(better.prey_energy_lost, base.prey_energy_lost);
        // Closure at the staging level: what left the prey is what the predator gained plus what
        // detritus received, to the bit.
        for s in [&base, &better] {
            assert!(
                (s.prey_energy_lost - (s.pred_energy_gain + s.detritus)).abs() < 1e-9,
                "telemetry={telemetry}: strike leaked EU: lost {} vs gained {} + detritus {}",
                s.prey_energy_lost,
                s.pred_energy_gain,
                s.detritus
            );
        }
    }
}

/// A predator at the origin, a near low-energy prey (+x, E=5) and a far rich one (+z, E=50).
fn steering_world(min_energy: f32) -> (World, Entity) {
    let mut world = World::new();
    world.insert_resource(LocomotionPolicy {
        enabled: true,
        drive_floor: 1.0,
        thirst_steering: false,
    });
    if min_energy >= 0.0 {
        world.insert_resource(PredatorIncomePolicy {
            target_min_energy: min_energy,
            ..Default::default()
        });
    }
    let pred = world
        .spawn((
            Agent,
            Predator,
            Position(Vec3::ZERO),
            homeo(60.0, 100.0),
            InertiaComponent::default(),
        ))
        .id();
    world.spawn((
        Agent,
        Prey,
        Position(Vec3::new(5.0, 0.0, 0.0)),
        homeo(5.0, 50.0),
    ));
    world.spawn((
        Agent,
        Prey,
        Position(Vec3::new(0.0, 0.0, 20.0)),
        homeo(50.0, 50.0),
    ));
    (world, pred)
}

fn steer(min_energy: f32) -> Vec3 {
    let (mut world, pred) = steering_world(min_energy);
    world.run_system_once(steering_drive_system);
    world.get::<InertiaComponent>(pred).unwrap().target_velocity
}

#[test]
fn b_target_min_energy_makes_steering_skip_weak_prey() {
    let nearest = steer(0.0);
    assert!(nearest.x > 0.5 && nearest.z.abs() < 1e-6, "{nearest:?}");
    // No resource at all behaves like E = 0.
    let (mut w, pred) = steering_world(-1.0);
    w.run_system_once(steering_drive_system);
    assert_eq!(
        w.get::<InertiaComponent>(pred).unwrap().target_velocity,
        nearest
    );
    // E = 10: the near prey (energy 5) is skipped, the rich one is chased.
    let rich = steer(10.0);
    assert!(rich.z > 0.5 && rich.x.abs() < 1e-6, "{rich:?}");
    // E above every prey: no target, so the predator stands still, as with no prey today.
    assert_eq!(steer(1_000.0), Vec3::ZERO);
}

#[test]
fn b_the_strike_is_not_filtered_by_target_min_energy() {
    // The chosen minimal semantics: E only steers. A weak prey inside the strike radius is still
    // struck, because the exploration's definition ("only chase prey with E >= 10") is about
    // targeting, and combat keeps its "first prey in query order" rule.
    let weak = strike(pol(0.0, LINDEMAN_EFFICIENCY, 10_000.0), false, 10.0);
    let normal = strike(None, false, 10.0);
    assert_eq!(weak.pred_energy_gain, normal.pred_energy_gain);
    assert!(weak.pred_energy_gain > 0.0);
}

#[test]
fn b_the_keys_change_a_live_world() {
    // World-level: with predators striking, each of the three keys moves the trajectory.
    const N: u64 = 3_000;
    let absent = checksum_after(&with(&[]), 2, N);
    for extra in [
        ("live.kill_hydration", 1.0),
        ("live.kill_efficiency", 0.6),
        ("live.target_min_energy", 10.0),
    ] {
        let c = checksum_after(&with(&[extra]), 2, N);
        println!("ECOB_LIVE {} changed={}", extra.0, c != absent);
        assert_ne!(c, absent, "{} left the live world unchanged", extra.0);
    }
}

// ---- (c) ---------------------------------------------------------------------------------------

#[test]
fn c_closed_eu_stays_closed_with_all_three_keys_on() {
    let mut a = build(&all_b_keys(), 2);
    let obs = |a: &LiveExperimentAdapter| {
        a.observables()
            .iter()
            .find(|(k, _)| k == "live.closed_eu_total")
            .expect("closed_eu_total")
            .1
    };
    a.run_schedule_once();
    let start = obs(&a);
    let mut worst = 0.0_f64;
    for _ in 1..3_000 {
        a.run_schedule_once();
        worst = worst.max((obs(&a) - start).abs() / start.abs().max(1.0));
    }
    println!("ECOB_EU start={start} worst_relative_drift={worst}");
    assert!(worst < 1e-9, "closed EU drifted by {worst}");
}

// ---- (d) ---------------------------------------------------------------------------------------

#[test]
fn ecob_child_entry_point() {
    if std::env::var(CHILD_ROLE).is_err() {
        return;
    }
    println!("{MARKER} {:#010x}", checksum_after(&all_b_keys(), 2, 2_000));
}

fn checksum_from_child() -> u32 {
    let output = Command::new(std::env::current_exe().expect("current_exe"))
        .args(["--exact", "ecob_child_entry_point", "--nocapture"])
        .env(CHILD_ROLE, "1")
        .env("ANIMA_USE_GPU", "0")
        .output()
        .expect("spawn child");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|l| l.trim_start().starts_with(MARKER))
        .unwrap_or_else(|| panic!("child printed no checksum\n{stdout}"));
    let hex = line.trim().rsplit(' ').next().expect("token");
    u32::from_str_radix(hex.trim_start_matches("0x"), 16).expect("hex checksum")
}

#[test]
fn d_checksum_agrees_across_three_processes_with_all_three_keys_on() {
    let sums: Vec<u32> = (0..3).map(|_| checksum_from_child()).collect();
    println!("ECOB_CHECKSUMS {sums:x?}");
    assert!(sums.iter().all(|s| *s == sums[0]), "{sums:x?}");
    assert_ne!(sums[0], 0);
}

// ---- (e) ---------------------------------------------------------------------------------------

#[test]
fn e_a_resumed_run_keeps_the_declared_policy_and_the_trajectory() {
    const N: u64 = 1_200;
    const K: u64 = 400;
    let init = all_b_keys();
    let reference = checksum_after(&init, 2, N);

    let mut first = build(&init, 2);
    for _ in 0..K {
        first.run_schedule_once();
    }
    let mut resumed =
        LiveExperimentAdapter::from_snapshot(&first.snapshot()).expect("the snapshot resumes");
    let policy = *resumed.world().resource::<PredatorIncomePolicy>();
    assert_eq!(
        (
            policy.kill_hydration,
            policy.kill_efficiency,
            policy.target_min_energy
        ),
        (1.0, 0.6, 10.0),
        "a restore must declare the manifest's policy again, not fall back to the defaults"
    );
    for _ in 0..(N - K) {
        resumed.run_schedule_once();
    }
    assert_eq!(
        resumed.checksum(),
        reference,
        "a run resumed at tick {K} diverged from the uninterrupted one"
    );
}

// ---- (f) ---------------------------------------------------------------------------------------

#[test]
fn f_the_keys_are_registered_and_validated() {
    // 19 at card B; card S6 (QD-SEL1b = A) added live.prey_cap and live.resource_capacity_scale.
    assert_eq!(LIVE_KEYS.len(), 21);
    for (c, name) in [
        (LIVE_KEY_KILL_HYDRATION, "live.kill_hydration"),
        (LIVE_KEY_KILL_EFFICIENCY, "live.kill_efficiency"),
        (LIVE_KEY_TARGET_MIN_ENERGY, "live.target_min_energy"),
    ] {
        assert_eq!(c, name);
        assert!(LIVE_KEYS.contains(&c));
    }
    let on = LiveWorldConfig::from_initial_conditions(&initial(&[
        ("live.kill_hydration", 1.0),
        ("live.kill_efficiency", 0.6),
        ("live.target_min_energy", 10.0),
    ]))
    .unwrap();
    assert_eq!(
        (on.kill_hydration, on.kill_efficiency, on.target_min_energy),
        (1.0, 0.6, 10.0)
    );
    for bad in [
        ("live.kill_hydration", -0.1),
        ("live.kill_efficiency", 0.0),
        ("live.kill_efficiency", 1.01),
        ("live.kill_efficiency", -0.4),
        ("live.target_min_energy", -1.0),
        ("live.kill_hydration", f64::NAN),
    ] {
        assert!(
            LiveWorldConfig::from_initial_conditions(&initial(&[bad])).is_err(),
            "{bad:?} must be refused"
        );
    }
    // The edge of the range is accepted.
    assert!(LiveWorldConfig::from_initial_conditions(&initial(&[
        ("live.kill_efficiency", 1.0),
        ("live.kill_hydration", 0.0),
        ("live.target_min_energy", 0.0),
    ]))
    .is_ok());
}
