//! A probe, not a gate: print one run's fingerprint so the *same binary* can be executed many times
//! and the results compared across processes.
//!
//! Bevy orders two systems with no declared edge between them by topological sort, and that sort is
//! not a declared property of the schedule — so a property that holds within a process can still
//! vary between processes. Every existing determinism gate compares two runs *inside* one process
//! (`the_same_seed_and_manifest_give_the_same_live_checksum`), which is exactly the comparison that
//! cannot see this. This test always passes; its value is the line it prints.
//!
//! Run it many times and compare:
//!
//! ```text
//! cargo test --features desktop --test live_cross_process_probe -- --nocapture
//! ```

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    live_observables, LiveExperimentAdapter, LIVE_OBSERVABLE_IDS,
};

const SEED: u64 = 999_983;
const TICKS: u64 = 600;

#[test]
fn print_one_runs_fingerprint_for_cross_process_comparison() {
    let initial = InitialConditionSet::new(vec![
        ("live.founders".to_string(), 10.0),
        ("live.predator_fraction".to_string(), 0.3),
        ("live.trees".to_string(), 8.0),
        ("live.lakes".to_string(), 2.0),
        ("live.food_cap".to_string(), 50.0),
    ]);
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        SEED,
        (16, 16),
        TICKS,
    )
    .expect("the live world builds");
    for _ in 0..TICKS {
        a.run_schedule_once();
    }
    let checksum = a.checksum();
    let obs = {
        let mut world = a.world();
        live_observables(&mut world)
    };
    // Every observable, printed to full `f64` precision — `{:?}` on an `f64` is shortest
    // round-trip, so two processes agreeing on this line agree bit for bit. A subset would let a
    // divergence hide in the observable nobody thought to print, which is how the census bug
    // survived: the one gate that could have seen it compared a checksum that does not cover
    // `EcosystemBiomass`.
    assert_eq!(
        obs.len(),
        LIVE_OBSERVABLE_IDS.len(),
        "the probe must cover every observable the adapter emits"
    );
    let body: Vec<String> = obs.iter().map(|(k, v)| format!("{k}={v:?}")).collect();
    println!("PROBE checksum={checksum} {}", body.join(" "));
}
