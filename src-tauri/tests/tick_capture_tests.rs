//! In-process tick capture, against the **real** live schedule.
//!
//! `core::tick_capture`'s own unit tests cover the ring, the percentile convention, the malformed
//! configurations and the adversarial clock inputs — all of which are reachable without a world.
//! What needs a world is the claim that matters most: **measuring a run does not change it.** These
//! tests drive `simulation_schedule::build_tick_schedule` — the function `SimulationEngine::start`
//! calls — through `LiveExperimentAdapter`, with capture off and on, and compare the world's
//! checksum, its RNG stream position and its observables.
//!
//! They also check the other half: that a capture of the real schedule actually produces samples
//! whose phases add up, rather than a well-formed document full of zeroes.

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::LiveExperimentAdapter;
use anima_engine_lib::core::tick_capture::{
    CaptureConfig, CaptureStatus, PhaseMask, SharedTickCapture, TickPhase,
};

const SEED: u64 = 90210;
const TICKS: u64 = 40;

fn initial() -> InitialConditionSet {
    InitialConditionSet::new(vec![
        ("live.founders".to_string(), 4.0),
        ("live.trees".to_string(), 3.0),
        ("live.lakes".to_string(), 1.0),
    ])
}

fn adapter() -> LiveExperimentAdapter {
    LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial(),
        &[],
        SEED,
        (16, 16),
        TICKS,
    )
    .expect("the live world must build")
}

/// The world's RNG stream position — the single most sensitive witness to "something consumed
/// randomness that should not have".
fn stream_pos(adapter: &LiveExperimentAdapter) -> u128 {
    adapter
        .world()
        .get_resource::<anima_engine_lib::core::resources::SimRng>()
        .map(|r| r.stream_pos())
        .expect("the live world always has a seeded stream")
}

fn config(capacity: usize) -> CaptureConfig {
    CaptureConfig {
        warmup_ticks: 0,
        capacity,
        max_samples: None,
        sample_every: 1,
        groups: PhaseMask::all(),
    }
}

#[test]
fn capture_does_not_change_the_live_trajectory() {
    // Baseline: no capture at all, so not even the sink resource exists.
    let mut plain = adapter();
    for _ in 0..TICKS {
        plain.run_schedule_once();
    }
    let plain_checksum = plain.checksum();
    let plain_observables = plain.observables();
    let plain_stream = stream_pos(&plain);

    // A sink is present but idle — the state the app is in whenever nobody has started a capture.
    let mut idle = adapter();
    idle.install_tick_capture(SharedTickCapture::new());
    for _ in 0..TICKS {
        idle.run_schedule_once();
    }
    assert_eq!(
        plain_checksum,
        idle.checksum(),
        "an idle capture sink changed the world"
    );
    assert_eq!(plain_stream, stream_pos(&idle));

    // Recording, every tick, every phase.
    let shared = SharedTickCapture::new();
    shared.start(config(TICKS as usize)).expect("valid config");
    let mut measured = adapter();
    measured.install_tick_capture(shared.clone());
    for _ in 0..TICKS {
        measured.run_schedule_once();
    }

    assert_eq!(
        plain_checksum,
        measured.checksum(),
        "recording a capture changed the world's trajectory"
    );
    assert_eq!(
        plain_observables,
        measured.observables(),
        "recording a capture changed what the world reports"
    );
    assert_eq!(
        plain_stream,
        stream_pos(&measured),
        "the capture consumed randomness"
    );
    assert_eq!(
        shared.accounting().samples_recorded,
        TICKS,
        "the capture must have actually recorded the run it did not change"
    );
}

#[test]
fn a_capture_of_the_real_schedule_produces_phases_that_add_up() {
    let shared = SharedTickCapture::new();
    shared.start(config(TICKS as usize)).expect("valid config");
    let mut live = adapter();
    live.install_tick_capture(shared.clone());
    for _ in 0..TICKS {
        live.run_schedule_once();
    }

    let acc = shared.accounting();
    assert_eq!(acc.ticks_observed, TICKS);
    assert_eq!(
        acc.dropped_incomplete, 0,
        "every checkpoint of the real schedule must have run"
    );
    assert_eq!(
        acc.dropped_out_of_order, 0,
        "the single-threaded executor must run the checkpoints in their declared order"
    );
    assert_eq!(acc.dropped_overflow, 0);
    assert_eq!(acc.samples_overwritten, 0, "the ring was sized for the run");

    for sample in shared.samples() {
        let ns = sample.phase_ns;
        let parts = ns[TickPhase::SensorBrain.index()]
            + ns[TickPhase::PhysicsMovement.index()]
            + ns[TickPhase::EcologyResources.index()]
            + ns[TickPhase::ScheduleTail.index()];
        assert_eq!(
            parts,
            ns[TickPhase::Schedule.index()],
            "the checkpoint-bounded phases must partition the schedule exactly"
        );
        assert_eq!(
            ns[TickPhase::FullTick.index()],
            ns[TickPhase::Schedule.index()] + ns[TickPhase::TelemetryPublish.index()]
        );
        assert!(
            ns[TickPhase::Schedule.index()] > 0,
            "a tick of the real schedule cannot take zero time"
        );
        assert_eq!(
            sample.agent_count, 4,
            "the workload must be the founding population, read off the world"
        );
    }

    let doc = shared.export();
    assert_eq!(doc.executor, "single-threaded");
    assert!(
        doc.workload.dimensions_measured,
        "the world's real field dimensions must be measured, not assumed"
    );
    assert_eq!(
        (doc.workload.world_width, doc.workload.world_height),
        (256, 256),
        "the live world runs at MapSettings::default(), not at any benchmark constant"
    );
    // A headless run publishes nothing over IPC, and says so with a zero rather than a guess.
    let telemetry = doc
        .phases
        .iter()
        .find(|p| p.phase == "telemetry_publish")
        .expect("row");
    assert_eq!(telemetry.max_ns, 0);
    assert!(telemetry.exact);

    let schedule = doc
        .phases
        .iter()
        .find(|p| p.phase == "schedule")
        .expect("row");
    assert_eq!(schedule.count, TICKS as usize);
    assert!(schedule.p50_ns > 0);
    assert!(schedule.p50_ns <= schedule.p95_ns);
    assert!(schedule.p95_ns <= schedule.p99_ns);
    assert!(schedule.p99_ns <= schedule.max_ns);
    assert!(schedule.min_ns <= schedule.p50_ns);
    assert_eq!(
        schedule.mean_ns_per_agent,
        Some(schedule.mean_ns / 4.0),
        "the per-agent figure must divide by the agents that were actually there"
    );
    assert!(
        doc.unavailable.iter().any(|u| u.id == "plant_soil_weather"),
        "a phase this build cannot measure must be named, not silently missing"
    );
}

#[test]
fn a_capture_smaller_than_the_run_keeps_the_newest_ticks_and_says_how_many_it_dropped() {
    let shared = SharedTickCapture::new();
    shared.start(config(8)).expect("valid config");
    let mut live = adapter();
    live.install_tick_capture(shared.clone());
    for _ in 0..TICKS {
        live.run_schedule_once();
    }

    assert_eq!(shared.sample_count(), 8);
    let acc = shared.accounting();
    assert_eq!(acc.samples_recorded, TICKS);
    assert_eq!(acc.samples_overwritten, TICKS - 8);

    let ticks: Vec<u64> = shared.samples().iter().map(|s| s.tick).collect();
    assert_eq!(
        ticks,
        ((TICKS - 7)..=TICKS).collect::<Vec<_>>(),
        "the ring must hold the newest window, oldest first"
    );
    let doc = shared.export();
    assert_eq!(doc.first_tick, Some(TICKS - 7));
    assert_eq!(doc.last_tick, Some(TICKS));
}

#[test]
fn warmup_and_rate_apply_to_a_real_run() {
    let shared = SharedTickCapture::new();
    shared
        .start(CaptureConfig {
            warmup_ticks: 10,
            capacity: 64,
            max_samples: None,
            sample_every: 3,
            groups: PhaseMask::of(&[TickPhase::Schedule, TickPhase::FullTick]),
        })
        .expect("valid config");
    let mut live = adapter();
    live.install_tick_capture(shared.clone());
    for _ in 0..TICKS {
        live.run_schedule_once();
    }

    let acc = shared.accounting();
    assert_eq!(acc.ticks_observed, TICKS);
    assert_eq!(acc.warmup_discarded, 10);
    assert_eq!(acc.samples_recorded + acc.rate_skipped, TICKS - 10);
    // First post-warm-up tick is 11, then every third.
    let ticks: Vec<u64> = shared.samples().iter().map(|s| s.tick).collect();
    assert_eq!(ticks.first(), Some(&11));
    assert!(ticks.windows(2).all(|w| w[1] - w[0] == 3));

    // The mask narrows what is summarised, never what was measured.
    let doc = shared.export();
    assert_eq!(doc.phases.len(), 2);
    assert!(doc.phases.iter().all(|p| p.count == ticks.len()));
}

#[test]
fn stopping_a_capture_mid_run_keeps_what_it_had_and_ignores_the_rest() {
    let shared = SharedTickCapture::new();
    shared.start(config(64)).expect("valid config");
    let mut live = adapter();
    live.install_tick_capture(shared.clone());
    for _ in 0..10 {
        live.run_schedule_once();
    }
    shared.stop();
    let held = shared.sample_count();
    assert_eq!(held, 10);

    for _ in 0..10 {
        live.run_schedule_once();
    }
    assert_eq!(shared.status(), CaptureStatus::Stopped);
    assert_eq!(
        shared.sample_count(),
        held,
        "a stopped capture kept sampling"
    );
    assert_eq!(shared.accounting().ticks_observed, 10);

    // And the world carried on regardless.
    assert!(live.observables().iter().all(|(_, v)| v.is_finite()));
}

#[test]
fn an_export_of_a_live_capture_round_trips_through_a_file() {
    let shared = SharedTickCapture::new();
    shared.start(config(16)).expect("valid config");
    let mut live = adapter();
    live.install_tick_capture(shared.clone());
    for _ in 0..12 {
        live.run_schedule_once();
    }

    let dir = std::env::temp_dir().join(format!("anima_capture_live_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("capture.json");
    let written = shared.export_to(&path).expect("export");

    let bytes = std::fs::read(&path).expect("read");
    let back: anima_engine_lib::core::tick_capture::CaptureExport =
        serde_json::from_slice(&bytes).expect("parse");
    // The file must be *exactly* what `export_to` returned — no tolerance.
    //
    // This assertion was briefly weakened to a relative tolerance on the means, on the belief that
    // serde_json could not round-trip an f64. It could: it writes with `ryu`, which is exact. The
    // lossy half was the default **parser**, and `Cargo.toml` now enables `float_roundtrip`. The
    // difference is worth stating because a tolerance here would also have hidden the other
    // candidate cause — an `export_to` that summarised twice and returned a document that was never
    // the one it wrote.
    assert_eq!(back, written, "the file is not what export_to returned");
    assert_eq!(back.accounting.samples_recorded, 12);
    // Two exports of an unchanged capture are identical, which is what makes the assertion above a
    // statement about serialization rather than about timing.
    assert_eq!(
        shared.export(),
        written,
        "export is not a pure function of the capture"
    );
    assert!(back
        .hardware
        .not_measured
        .contains(&"cpu_model".to_string()));

    let _ = std::fs::remove_dir_all(&dir);
}
