//! F-039 — continuous reproduction recorded births only in `DemographicTracker`, so the
//! `live.births` observable (which drains `VitalStatistics`) read 0 while the animals bred.
//!
//! `autonomous_reproduction_system` called `DemographicTracker::record_birth` but never
//! `VitalStatistics::record_birth`; the only caller of the latter was the evolutionary-replacement
//! path. The adapter forces `ReproductionPolicy::disabled()` today (until QD-4), so the bug is
//! latent on the trunk and goes live the moment reproduction is enabled, as this test does.
//!
//! The world is built through `LiveExperimentAdapter::from_manifest` and driven by the app's own
//! schedule (`run_schedule_once` runs `build_tick_schedule`), sampled the way the runner does.

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    live_observables, LiveExperimentAdapter, LIVE_KEY_MORTALITY,
};
use anima_engine_lib::core::resources::ReproductionPolicy;
use anima_engine_lib::core::telemetry_export::DemographicTracker;

const SEED: u64 = 1;
const TICKS: u32 = 1_000;

fn build() -> LiveExperimentAdapter {
    let initials = InitialConditionSet::new(vec![(LIVE_KEY_MORTALITY.to_string(), 1.0)]);
    LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initials,
        &[],
        SEED,
        (16, 16),
        0,
    )
    .expect("a live world with a death rule builds")
}

fn live_births(adapter: &LiveExperimentAdapter) -> f64 {
    let mut world = adapter.world();
    live_observables(&mut world)
        .into_iter()
        .find_map(|(n, v)| (n == "live.births").then_some(v))
        .expect("live.births exists")
}

#[test]
fn continuous_births_reach_live_births() {
    let mut adapter = build();
    {
        let mut world = adapter.world();
        world.insert_resource(DemographicTracker::default());
        world.insert_resource(ReproductionPolicy { enabled: true });
    }
    let mut summed = 0.0;
    for _ in 0..TICKS {
        adapter.run_schedule_once();
        summed += live_births(&adapter);
    }
    let tracker = adapter
        .world()
        .resource::<DemographicTracker>()
        .total_births();
    println!("F039 ticks={TICKS} seed={SEED} tracker_births={tracker} live_births_summed={summed}");
    assert!(
        tracker > 0,
        "the world must actually breed for this test to mean anything"
    );
    assert_eq!(
        summed, tracker as f64,
        "live.births must equal the births that happened (F-039)"
    );
}

/// Control: with reproduction disabled (the adapter default) nothing is born, `live.births`
/// stays 0, and the trajectory is bit-identical to the parent commit's.
#[test]
fn disabled_reproduction_keeps_live_births_zero_and_checksum_unchanged() {
    let mut adapter = build();
    assert_eq!(
        *adapter.world().resource::<ReproductionPolicy>(),
        ReproductionPolicy::disabled()
    );
    let mut summed = 0.0;
    for _ in 0..TICKS {
        adapter.run_schedule_once();
        summed += live_births(&adapter);
    }
    let checksum = adapter.checksum();
    println!("F039 control live_births_summed={summed} checksum={checksum:#010x}");
    assert_eq!(summed, 0.0);
    assert_eq!(
        checksum, CONTROL_CHECKSUM,
        "the F-039 edit must not change the disabled path"
    );
}

/// Measured on the parent commit (a47d534) before the edit.
const CONTROL_CHECKSUM: u32 = 0x9469_2e63;
