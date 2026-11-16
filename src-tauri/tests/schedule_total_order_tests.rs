//! F-041 gate: the schedule the app runs, driven through `build_tick_schedule`, gives one world per
//! seed in every *process*, in the configuration where it used to give two (mortality plus
//! continuous reproduction).
//!
//! The child is the instrument (see `determinism_gate_tests.rs` for why a same-process comparison
//! cannot see a per-process hash seed). A child is this same test binary re-executed with
//! `ANIMA_F041_CONFIG` naming the scenario and `ANIMA_F041_TICKS` its length; it prints one line
//! `F041-CHECKSUM 0x........`. With `ANIMA_F041_ORDER=1` it also prints the executed system order
//! once (`F041-ORDER a,b,c,...`), which is how the pairs that flipped between processes were found.

use std::process::Command;

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::LiveExperimentAdapter;
use anima_engine_lib::core::resources::ReproductionPolicy;

const CONFIG: &str = "ANIMA_F041_CONFIG";
const TICKS_VAR: &str = "ANIMA_F041_TICKS";
const ORDER_VAR: &str = "ANIMA_F041_ORDER";
const MARKER: &str = "F041-CHECKSUM";

fn kv(pairs: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect())
}

/// `(initial conditions, seed, continuous reproduction on)` for a named scenario.
fn scenario(name: &str) -> (InitialConditionSet, u64, bool) {
    match name {
        "default" => (kv(&[]), 1, false),
        "e2" => (
            kv(&[
                ("live.founders", 10.0),
                ("live.predator_fraction", 0.3),
                ("live.trees", 8.0),
                ("live.lakes", 2.0),
                ("live.food_cap", 50.0),
            ]),
            700_001,
            false,
        ),
        "mortality60" => (
            kv(&[("live.mortality", 1.0), ("live.founders", 60.0)]),
            1,
            false,
        ),
        "turnover" => (
            kv(&[("live.mortality", 1.0), ("live.founders", 10.0)]),
            1,
            true,
        ),
        other => panic!("unknown scenario {other}"),
    }
}

fn build(name: &str) -> LiveExperimentAdapter {
    let (initial, seed, repro) = scenario(name);
    let a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        seed,
        (16, 16),
        0,
    )
    .expect("the live world builds");
    if repro {
        a.world()
            .insert_resource(ReproductionPolicy { enabled: true });
    }
    a
}

/// Child entry point. Without the env var it returns at once, so a plain `cargo test` passes it trivially (not `#[ignore]`d, so the test-target policy needs no entry).
#[test]
fn f041_child_entry_point() {
    let Ok(name) = std::env::var(CONFIG) else {
        return;
    };
    let ticks: u64 = std::env::var(TICKS_VAR)
        .ok()
        .and_then(|v| v.parse().ok())
        .expect("ANIMA_F041_TICKS");
    let mut a = build(&name);
    for _ in 0..ticks {
        a.run_schedule_once();
        if std::env::var(ORDER_VAR).is_ok() && a.tick() == 1 {
            let order: Vec<String> = a
                .schedule_mut()
                .systems()
                .expect("initialized")
                .map(|(_, s)| {
                    let n = s.name();
                    n.rsplit("::").next().unwrap_or(&n).to_string()
                })
                .collect();
            println!("F041-ORDER {}", order.join(","));
        }
    }
    println!("{MARKER} {:#010x}", a.checksum());
}

fn checksum_from_child(name: &str, ticks: u64) -> u32 {
    let exe = std::env::current_exe().expect("current_exe");
    let output = Command::new(exe)
        .args(["--exact", "f041_child_entry_point", "--nocapture"])
        .env(CONFIG, name)
        .env(TICKS_VAR, ticks.to_string())
        .env("ANIMA_DETERMINISTIC", "1")
        .env("ANIMA_USE_GPU", "0")
        .output()
        .expect("failed to spawn child");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|l| l.trim_start().starts_with(MARKER))
        .unwrap_or_else(|| {
            panic!(
                "child ({name}) printed no checksum.\n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
    let hex = line.trim().rsplit(' ').next().expect("checksum token");
    u32::from_str_radix(hex.trim_start_matches("0x"), 16).expect("hex checksum")
}

fn assert_one_world(name: &str, ticks: u64, processes: usize) {
    let sums: Vec<u32> = (0..processes)
        .map(|_| checksum_from_child(name, ticks))
        .collect();
    assert!(
        sums.iter().all(|c| *c == sums[0]),
        "F-041: scenario `{name}` ({ticks} ticks, `build_tick_schedule`) gave different worlds in \
         different processes: {sums:#010x?}\nA pair of systems with no declared order was sorted \
         differently per process. Run the ambiguity gate in `schedule_ambiguity_tests.rs` to see \
         which."
    );
}

/// Mortality and continuous reproduction both on: the configuration that gave `0x5bb50ab7` in ten
/// processes and `0x99190d0a` in two before the schedule had a total order.
#[test]
fn turnover_world_is_one_world_in_six_processes() {
    assert_one_world("turnover", 3_000, 6);
}

/// Large population, mortality on: the configuration whose per-tick trajectory differed between
/// processes from tick 2 while its final checksum happened to converge.
#[test]
fn crowded_world_is_one_world_in_four_processes() {
    assert_one_world("mortality60", 600, 4);
}

/// The two configurations that were already process-invariant must stay so (the fix must not trade
/// one variance for another).
#[test]
fn default_and_e2_worlds_stay_one_world_in_three_processes() {
    assert_one_world("default", 1_500, 3);
    assert_one_world("e2", 1_500, 3);
}
