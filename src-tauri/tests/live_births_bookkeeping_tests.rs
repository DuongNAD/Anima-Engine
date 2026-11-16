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

/// Control: with reproduction disabled (the adapter default) nothing is born and `live.births`
/// stays 0, and two independent runs of the disabled path agree.
///
/// The edit cannot reach this path: `autonomous_reproduction_system` returns before its loop when
/// the policy is present and disabled. That the trajectory is unchanged was measured, not pinned:
/// on macOS arm64 the checksum after 1,000 ticks was 0x94692e63 on the parent commit (a47d534)
/// and on the fix. An absolute world checksum is platform-dependent (CI measured other values on
/// Windows and Linux for the same commit), so it is recorded here and never asserted.
#[test]
fn disabled_reproduction_keeps_live_births_zero() {
    let run = || {
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
        (summed, adapter.checksum())
    };
    let (summed, checksum) = run();
    let (_, again) = run();
    println!("F039 control live_births_summed={summed} checksum={checksum:#010x}");
    assert_eq!(summed, 0.0, "nothing is born with reproduction disabled");
    assert_eq!(
        checksum, again,
        "the disabled path replays within one process"
    );
}
