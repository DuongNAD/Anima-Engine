//! ADR-0006 option A (F-042): `live.locomotion = 1` makes creatures steer toward their sensory
//! target through `steering_drive_system`. Every test drives `LiveExperimentAdapter` through the
//! real tick schedule (`simulation_schedule::build_tick_schedule`).
//!
//! * (a) off is bit-identical: key absent == `live.locomotion = 0` == a world with no policy
//!   (compared with each other; no absolute checksum is pinned)
//! * (b) on: founders translate far beyond the F-042 bound (0.5) and the path grows
//! * (c) on + mortality + reproduction: a predator strikes a prey and gains energy from the kill
//! * (d) on: the checksum agrees across 3 separate OS processes
//! * (e) on: closed EU stays constant
//! * speed is heritable: the drive follows the CPG amplitude, and zero amplitude is zero drive
//!
//! The zero-allocation guarantee is in `locomotion_steering_zero_alloc_tests.rs` (it needs its own
//! global allocator).

use anima_engine_lib::ai::cpg::CpgOscillator;
use anima_engine_lib::core::agent_systems::AgentEvaluation;
use anima_engine_lib::core::components::{
    Agent, ChildrenLinks, InertiaComponent, ParentLink, Position, Predator, Velocity,
};
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LiveWorldConfig, LIVE_KEYS, LIVE_KEY_LOCOMOTION,
};
use anima_engine_lib::core::resources::LocomotionPolicy;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::*;
use std::collections::HashMap;
use std::process::Command;

const SEED: u64 = 1;
const CHILD_ROLE: &str = "ANIMA_LOCO_CHILD";
const MARKER: &str = "LOCO_CHECKSUM";

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn initial(extra: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(extra.iter().map(|(k, v)| (k.to_string(), *v)).collect())
}

fn build(init: &InitialConditionSet) -> LiveExperimentAdapter {
    LiveExperimentAdapter::from_manifest(&WorldLawSet::baseline(), init, &[], SEED, (16, 16), 0)
        .expect("the live world builds")
}

fn checksum_after(init: &InitialConditionSet, ticks: u64) -> u32 {
    let mut a = build(init);
    for _ in 0..ticks {
        a.run_schedule_once();
    }
    a.checksum()
}

fn roots(world: &mut World) -> HashMap<Entity, [f32; 3]> {
    let mut q = world.query_filtered::<(Entity, &Position), (With<Agent>, Without<ParentLink>)>();
    q.iter(world)
        .map(|(e, p)| (e, [p.0.x, p.0.y, p.0.z]))
        .collect()
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

// ---- (a) ---------------------------------------------------------------------------------------

#[test]
fn a_locomotion_off_is_bit_identical_to_the_world_without_the_key() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    const N: u64 = 600;
    let absent = checksum_after(&initial(&[]), N);
    let zero = checksum_after(&initial(&[("live.locomotion", 0.0)]), N);
    assert_eq!(
        zero, absent,
        "live.locomotion = 0 must equal the key absent"
    );

    // A world with no policy at all (the shape every pre-ADR-0006 world had).
    let mut bare = build(&initial(&[]));
    bare.world().remove_resource::<LocomotionPolicy>();
    for _ in 0..N {
        bare.run_schedule_once();
    }
    assert_eq!(absent, bare.checksum(), "absent policy != disabled policy");

    // And the key really does change the trajectory when on, so the comparison is not vacuous.
    let on = checksum_after(&initial(&[("live.locomotion", 1.0)]), N);
    assert_ne!(on, absent, "locomotion on left the trajectory unchanged");
}

// ---- (b) ---------------------------------------------------------------------------------------

#[test]
fn b_founders_translate_with_locomotion_on() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    const TICKS: u32 = 1_000;
    let mut a = build(&initial(&[("live.locomotion", 1.0)]));
    let start = roots(&mut a.world());
    assert!(!start.is_empty());
    let mut last = start.clone();
    let mut path: HashMap<Entity, f32> = start.keys().map(|e| (*e, 0.0)).collect();
    // Realised vs target speed, only on ticks where the drive is actually asking for movement.
    let (mut ratio_sum, mut ratio_n) = (0.0f64, 0u64);
    let (mut vel_sum, mut tgt_sum) = (0.0f64, 0.0f64);
    for _ in 0..TICKS {
        a.run_schedule_once();
        let mut w = a.world();
        let now = roots(&mut w);
        for (e, p) in &now {
            if let Some(l) = last.get(e) {
                *path.entry(*e).or_insert(0.0) += dist(*l, *p);
            }
        }
        last = now;
        let mut q = w
            .query_filtered::<(&Velocity, &InertiaComponent), (With<Agent>, Without<ParentLink>)>();
        for (v, i) in q.iter(&w) {
            let t = i.target_velocity.length();
            if t > 0.05 {
                ratio_sum += (v.0.length() / t) as f64;
                ratio_n += 1;
                vel_sum += v.0.length() as f64;
                tgt_sum += t as f64;
            }
        }
    }
    let (mut max_net, mut path_sum, mut n) = (0.0f32, 0.0f32, 0usize);
    for (e, s) in &start {
        let Some(end) = last.get(e) else { continue };
        max_net = max_net.max(dist(*s, *end));
        path_sum += path[e];
        n += 1;
    }
    let mean_path = path_sum / n.max(1) as f32;
    println!(
        "LOCO_B founders={n} max_net={max_net} mean_path={mean_path} realised_over_target_mean={} \
         mean_speed={} mean_target={} samples={ratio_n}",
        ratio_sum / ratio_n.max(1) as f64,
        vel_sum / ratio_n.max(1) as f64,
        tgt_sum / ratio_n.max(1) as f64
    );
    assert!(n > 0, "founders must survive the run (no mortality)");
    assert!(ratio_n > 0, "the drive never asked for movement");
    // F-042 bound was max_net < 0.5 and mean path ~0.5.
    assert!(
        max_net > 2.0,
        "max net displacement {max_net} not above 2.0"
    );
    assert!(mean_path > 2.0, "mean path {mean_path} did not grow");
}

// ---- speed is heritable ------------------------------------------------------------------------

/// Pins every body oscillator amplitude of every agent.
fn set_amplitudes(a: &mut LiveExperimentAdapter, amplitude: f32) {
    let mut w = a.world();
    let kids: Vec<Entity> = {
        let mut q = w.query_filtered::<&ChildrenLinks, (With<Agent>, Without<ParentLink>)>();
        q.iter(&w).flat_map(|c| c.0.iter().copied()).collect()
    };
    for k in kids {
        if let Some(mut o) = w.get_mut::<CpgOscillator>(k) {
            o.amplitude = amplitude;
        }
    }
}

fn mean_target_speed(a: &mut LiveExperimentAdapter) -> f32 {
    let mut w = a.world();
    let mut q = w.query_filtered::<&InertiaComponent, (With<Agent>, Without<ParentLink>)>();
    let v: Vec<f32> = q.iter(&w).map(|i| i.target_velocity.length()).collect();
    v.iter().sum::<f32>() / v.len().max(1) as f32
}

#[test]
fn speed_follows_the_cpg_amplitude() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let init = initial(&[("live.locomotion", 1.0)]);
    let mut a = build(&init);
    for _ in 0..30 {
        a.run_schedule_once();
    }
    // Pin the amplitude, then run ONLY the steering system: the full schedule's action
    // resolution would rewrite the oscillators from the brain output first.
    let mut only = Schedule::default();
    only.add_systems(anima_engine_lib::core::locomotion::steering_drive_system);
    let mut speeds = Vec::new();
    for amp in [0.0f32, 0.75, 1.5] {
        set_amplitudes(&mut a, amp);
        only.run(&mut a.world());
        speeds.push(mean_target_speed(&mut a));
    }
    println!("LOCO_SPEED amp0/0.75/1.5 -> mean target speed {speeds:?}");
    assert_eq!(speeds[0], 0.0, "zero CPG amplitude must mean zero drive");
    assert!(speeds[1] > 0.1 && speeds[2] > speeds[1] * 1.5, "{speeds:?}");
}

// ---- (c) ---------------------------------------------------------------------------------------

#[test]
fn c_a_predator_reaches_a_prey_and_gains_energy_from_the_kill() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    const TICKS: u32 = 4_000;
    let mut a = build(&initial(&[
        ("live.locomotion", 1.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
    ]));
    let (mut best_gain, mut first_tick) = (0.0f32, None);
    // Per-predator running maximum, so a predator that dies keeps the meal it earned.
    let mut gains: HashMap<Entity, f32> = HashMap::new();
    for t in 0..TICKS {
        a.run_schedule_once();
        let mut w = a.world();
        let mut q = w.query_filtered::<(Entity, &AgentEvaluation), With<Predator>>();
        for (e, ev) in q.iter(&w) {
            let g = gains.entry(e).or_insert(0.0);
            *g = g.max(ev.prey_energy_gained);
        }
        let total: f32 = gains.values().sum();
        if total > 0.0 && first_tick.is_none() {
            first_tick = Some(t);
        }
        best_gain = best_gain.max(total);
    }
    println!("LOCO_C first_kill_tick={first_tick:?} summed_prey_energy_gained={best_gain}");
    assert!(
        best_gain > 0.0,
        "no predator ever gained energy from a prey in {TICKS} ticks"
    );
}

// ---- (d) ---------------------------------------------------------------------------------------

fn on_checksum() -> u32 {
    checksum_after(&initial(&[("live.locomotion", 1.0)]), 1_500)
}

/// Child entry point. A plain test that does nothing unless the parent set the role variable.
#[test]
fn locomotion_child_entry_point() {
    if std::env::var(CHILD_ROLE).is_err() {
        return;
    }
    println!("{MARKER} {:#010x}", on_checksum());
}

fn checksum_from_child() -> u32 {
    let output = Command::new(std::env::current_exe().expect("current_exe"))
        .args(["--exact", "locomotion_child_entry_point", "--nocapture"])
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
fn d_the_locomotion_checksum_agrees_across_three_separate_processes() {
    let sums: Vec<u32> = (0..3).map(|_| checksum_from_child()).collect();
    println!("LOCO_CHECKSUMS {sums:x?}");
    assert!(
        sums.iter().all(|s| *s == sums[0]),
        "processes disagree: {sums:x?}"
    );
    assert_ne!(sums[0], 0);
}

// ---- (e) ---------------------------------------------------------------------------------------

#[test]
fn e_closed_eu_stays_closed_with_locomotion_on() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut a = build(&initial(&[
        ("live.locomotion", 1.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
    ]));
    let obs = |a: &LiveExperimentAdapter| {
        a.observables()
            .iter()
            .find(|(k, _)| k == "live.closed_eu_total")
            .expect("closed_eu_total")
            .1
    };
    // Tick 1 settles genesis (a constant seeded regrowth lands on the first tick), so the closed
    // total is baselined after it, as in the other closed-EU tests.
    a.run_schedule_once();
    let start = obs(&a);
    let mut worst = 0.0_f64;
    for _ in 1..2_000 {
        a.run_schedule_once();
        worst = worst.max((obs(&a) - start).abs() / start.abs().max(1.0));
    }
    println!("LOCO_EU start={start} worst_relative_drift={worst}");
    assert!(worst < 1e-6, "closed EU drifted by {worst} (relative)");
}

// ---- the declared key --------------------------------------------------------------------------

#[test]
fn the_key_is_registered_and_must_be_an_exact_boolean() {
    assert!(LIVE_KEYS.contains(&LIVE_KEY_LOCOMOTION));
    assert_eq!(LIVE_KEY_LOCOMOTION, "live.locomotion");
    assert!(!LiveWorldConfig::default().locomotion);
    let on = LiveWorldConfig::from_initial_conditions(&initial(&[("live.locomotion", 1.0)]))
        .expect("1.0 parses");
    assert!(on.locomotion);
    assert!(
        LiveWorldConfig::from_initial_conditions(&initial(&[("live.locomotion", 0.5)])).is_err()
    );
}
