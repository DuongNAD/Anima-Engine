//! ADR-0007 option A (QD-ECO = A): `live.drive_floor` and `live.thirst_steering`, both opt-in and
//! both inert unless `live.locomotion = 1`.
//!
//! * (a) keys absent == `drive_floor 0` == `thirst_steering 0` with locomotion on (compared with
//!   each other, no absolute pin); with locomotion off the keys change nothing
//! * (b) a floor of 0.5 moves founders that the CPG-only drive leaves nearly still
//! * (c) a thirsty agent walks to the nearest lake and drinks
//! * (d) prey-only world, 6,000 ticks: survival per seed with and without the keys (printed)
//! * (e) 3-process determinism with both keys on, closed EU constant, key validation

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::components::{Agent, Lake, ParentLink, Position, Predator};
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LiveWorldConfig, LIVE_KEYS, LIVE_KEY_DRIVE_FLOOR,
    LIVE_KEY_THIRST_STEERING,
};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::*;
use std::collections::HashMap;
use std::process::Command;

const CHILD_ROLE: &str = "ANIMA_ECO_CHILD";
const MARKER: &str = "ECO_CHECKSUM";

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

fn roots(world: &mut World) -> HashMap<Entity, [f32; 3]> {
    let mut q = world.query_filtered::<(Entity, &Position), (With<Agent>, Without<ParentLink>)>();
    q.iter(world)
        .map(|(e, p)| (e, [p.0.x, p.0.y, p.0.z]))
        .collect()
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn agent_count(a: &mut LiveExperimentAdapter) -> usize {
    let mut w = a.world();
    let mut q = w.query_filtered::<Entity, (With<Agent>, Without<ParentLink>)>();
    q.iter(&w).count()
}

// ---- (a) ---------------------------------------------------------------------------------------

#[test]
fn a_zero_keys_equal_absent_keys_and_are_inert_without_locomotion() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    const N: u64 = 600;
    let loco = ("live.locomotion", 1.0);
    // Seed 2: its founders have a tiny CPG amplitude, so the floor visibly changes the run (in
    // seed 1 the CPG drive already exceeds 0.5 and the floor is a no-op).
    let absent = checksum_after(&initial(&[loco]), 2, N);
    let zeros = checksum_after(
        &initial(&[
            loco,
            ("live.drive_floor", 0.0),
            ("live.thirst_steering", 0.0),
        ]),
        2,
        N,
    );
    assert_eq!(zeros, absent, "explicit zeros must equal the keys absent");
    let floor = checksum_after(&initial(&[loco, ("live.drive_floor", 0.5)]), 2, N);
    assert_ne!(
        floor, absent,
        "drive_floor had no effect with locomotion on"
    );

    // Locomotion off: the keys must not change anything.
    let off = checksum_after(&initial(&[]), 1, N);
    let off_keys = checksum_after(
        &initial(&[("live.drive_floor", 0.5), ("live.thirst_steering", 1.0)]),
        1,
        N,
    );
    assert_eq!(off, off_keys, "the keys acted without live.locomotion");
}

// ---- (b) ---------------------------------------------------------------------------------------

fn mean_founder_path(init: &InitialConditionSet, seed: u64, ticks: u32) -> f32 {
    let mut a = build(init, seed);
    let start = roots(&mut a.world());
    let mut last = start.clone();
    let mut path: HashMap<Entity, f32> = start.keys().map(|e| (*e, 0.0)).collect();
    for _ in 0..ticks {
        a.run_schedule_once();
        let now = roots(&mut a.world());
        for (e, p) in &now {
            if let Some(l) = last.get(e) {
                *path.entry(*e).or_insert(0.0) += dist(*l, *p);
            }
        }
        last = now;
    }
    path.values().sum::<f32>() / path.len().max(1) as f32
}

#[test]
fn b_a_drive_floor_moves_founders_the_cpg_drive_leaves_still() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut rows = Vec::new();
    for seed in [2u64, 4, 5] {
        let base = mean_founder_path(&initial(&[("live.locomotion", 1.0)]), seed, 1_000);
        let floored = mean_founder_path(
            &initial(&[("live.locomotion", 1.0), ("live.drive_floor", 0.5)]),
            seed,
            1_000,
        );
        println!("ECO_B seed={seed} mean_path_no_floor={base} mean_path_floor0.5={floored}");
        rows.push((seed, base, floored));
    }
    let gains = rows.iter().filter(|(_, b, f)| *f > *b * 1.5).count();
    assert!(
        gains >= 1,
        "a 0.5 floor did not clearly raise the mean path on any slow seed: {rows:?}"
    );
}

// ---- (c) ---------------------------------------------------------------------------------------

/// Runs with one prey founder forced thirsty (30% of target, below the 50% threshold). Returns
/// (hydration rose at some tick, closest approach to a lake centre, peak hydration fraction).
fn thirsty_run(thirst: bool, ticks: u32) -> (bool, f32, f32) {
    let mut init = vec![("live.locomotion", 1.0), ("live.drive_floor", 0.5)];
    if thirst {
        init.push(("live.thirst_steering", 1.0));
    }
    let mut a = build(&initial(&init), 1);
    let (agent, lake_dist0) = {
        let mut w = a.world();
        let lakes: Vec<[f32; 3]> = {
            let mut q = w.query::<(&Position, &Lake)>();
            q.iter(&w).map(|(p, _)| [p.0.x, p.0.y, p.0.z]).collect()
        };
        let mut q = w.query_filtered::<(Entity, &Position, &mut HomeostaticState), (
            With<Agent>,
            Without<ParentLink>,
            Without<Predator>,
        )>();
        let lake_d = |p: &Position| {
            lakes
                .iter()
                .map(|l| dist(*l, [p.0.x, p.0.y, p.0.z]))
                .fold(f32::MAX, f32::min)
        };
        // The prey founder closest to a lake, so the walk fits in the tick budget.
        let (e, _, _) = q
            .iter_mut(&mut w)
            .map(|(e, p, h)| (e, lake_d(p), h))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .expect("a prey founder");
        let (_, p, mut h) = q.get_mut(&mut w, e).unwrap();
        h.hydration = h.hydration_target * 0.3;
        let d = lake_d(p);
        (e, d)
    };
    let (mut rose, mut closest) = (false, lake_dist0);
    let mut prev: Option<f32> = None;
    let mut peak = 0.0f32;
    for _ in 0..ticks {
        a.run_schedule_once();
        let mut w = a.world();
        // Isolate thirst: keep the subject fed (an agent with no energy stops steering at all).
        if let Some(mut h) = w.get_mut::<HomeostaticState>(agent) {
            h.energy = h.energy_target;
        }
        let Some(h) = w.get::<HomeostaticState>(agent) else {
            break;
        };
        let (hyd, target) = (h.hydration, h.hydration_target.max(1e-6));
        if prev.is_some_and(|p| hyd > p + 1e-3) {
            rose = true;
        }
        peak = peak.max(hyd / target);
        prev = Some(hyd);
        let p = w.get::<Position>(agent).unwrap().0;
        let mut q = w.query::<(&Position, &Lake)>();
        let d = q
            .iter(&w)
            .map(|(lp, _)| dist([lp.0.x, lp.0.y, lp.0.z], [p.x, p.y, p.z]))
            .fold(f32::MAX, f32::min);
        closest = closest.min(d);
    }
    println!(
        "ECO_C thirst_steering={thirst} start_lake_dist={lake_dist0} closest={closest} \
         drank={rose} peak_hydration_fraction={peak}"
    );
    (rose, closest, peak)
}

#[test]
fn c_a_thirsty_agent_walks_to_the_nearest_lake_and_drinks() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (drank, closest, peak) = thirsty_run(true, 5_000);
    let (_, control_closest, control_peak) = thirsty_run(false, 5_000);
    println!("ECO_C control closest={control_closest} peak={control_peak}");
    assert!(drank, "the thirsty agent never drank (closest {closest})");
    assert!(
        peak > 0.5,
        "hydration never recovered above the threshold (peak {peak})"
    );
}

// ---- (d) ---------------------------------------------------------------------------------------

fn prey_only_survivors(keys: bool, seed: u64) -> (usize, usize) {
    let mut init = vec![
        ("live.locomotion", 1.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
        ("live.predator_fraction", 0.0),
    ];
    if keys {
        init.push(("live.drive_floor", 0.5));
        init.push(("live.thirst_steering", 1.0));
    }
    let mut a = build(&initial(&init), seed);
    let mut peak = 0usize;
    for _ in 0..6_000 {
        a.run_schedule_once();
        peak = peak.max(agent_count(&mut a));
    }
    (agent_count(&mut a), peak)
}

#[test]
fn d_prey_only_survival_per_seed_with_and_without_the_keys() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut base_alive, mut key_alive) = (0, 0);
    for seed in 1..=5u64 {
        let (b, bp) = prey_only_survivors(false, seed);
        let (k, kp) = prey_only_survivors(true, seed);
        println!("ECO_D seed={seed} baseline_alive={b} (peak {bp}) keys_alive={k} (peak {kp})");
        base_alive += usize::from(b > 0);
        key_alive += usize::from(k > 0);
    }
    println!("ECO_D seeds_surviving baseline={base_alive} keys={key_alive}");
    assert!(
        key_alive >= base_alive,
        "the keys lost seeds: baseline {base_alive}, keys {key_alive}"
    );
}

// ---- (e) ---------------------------------------------------------------------------------------

fn both_keys() -> InitialConditionSet {
    initial(&[
        ("live.locomotion", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
    ])
}

#[test]
fn eco_child_entry_point() {
    if std::env::var(CHILD_ROLE).is_err() {
        return;
    }
    println!("{MARKER} {:#010x}", checksum_after(&both_keys(), 2, 2_000));
}

fn checksum_from_child() -> u32 {
    let output = Command::new(std::env::current_exe().expect("current_exe"))
        .args(["--exact", "eco_child_entry_point", "--nocapture"])
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
fn e_checksum_agrees_across_three_processes_with_both_keys_on() {
    let sums: Vec<u32> = (0..3).map(|_| checksum_from_child()).collect();
    println!("ECO_CHECKSUMS {sums:x?}");
    assert!(sums.iter().all(|s| *s == sums[0]), "{sums:x?}");
    assert_ne!(sums[0], 0);
}

#[test]
fn e_closed_eu_stays_closed_with_both_keys_on() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut a = build(&both_keys(), 2);
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
    for _ in 1..2_000 {
        a.run_schedule_once();
        worst = worst.max((obs(&a) - start).abs() / start.abs().max(1.0));
    }
    println!("ECO_EU start={start} worst_relative_drift={worst}");
    assert!(worst < 1e-6, "closed EU drifted by {worst}");
}

#[test]
fn the_keys_are_registered_and_validated() {
    assert!(LIVE_KEYS.contains(&LIVE_KEY_DRIVE_FLOOR));
    assert!(LIVE_KEYS.contains(&LIVE_KEY_THIRST_STEERING));
    assert_eq!(LIVE_KEY_DRIVE_FLOOR, "live.drive_floor");
    assert_eq!(LIVE_KEY_THIRST_STEERING, "live.thirst_steering");
    let d = LiveWorldConfig::default();
    assert_eq!((d.drive_floor, d.thirst_steering), (0.0, false));
    let on = LiveWorldConfig::from_initial_conditions(&initial(&[
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
    ]))
    .expect("parses");
    assert_eq!((on.drive_floor, on.thirst_steering), (0.5, true));
    for bad in [-0.1, 1.1] {
        assert!(
            LiveWorldConfig::from_initial_conditions(&initial(&[("live.drive_floor", bad)]))
                .is_err()
        );
    }
    assert!(
        LiveWorldConfig::from_initial_conditions(&initial(&[("live.thirst_steering", 0.5)]))
            .is_err()
    );
}
