//! WR1 no-feedback gate (`C8` DONE WHEN #1 and ADR-0008 H1):
//!
//! 1. Same seed, recording ON vs OFF yields identical `world_checksum` at ticks
//!    `0, 180, 1800, 18000` on BOTH:
//!    - the default world (`InitialConditionSet::default()`);
//!    - an active world with lifetime learning + autonomous reproduction + mortality enabled.
//! 2. Capturing `LiveInterventions` and `ObserverTrace` inputs never perturbs `world_checksum`.
//! 3. Sealed segments carry bit-exact `end_checksum` matching `LiveExperimentAdapter::checksum()`
//!    and pass full cryptographic chain verification.

use anima_engine_lib::core::causal::{CAUSE_OBSERVER};
use anima_engine_lib::core::components::AgentBrain;
use anima_engine_lib::core::ecs::Agent;
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::intervention::{
    Curve, InterventionCommand, InterventionKind, Region,
};
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LIVE_KEY_DRIVE_FLOOR, LIVE_KEY_EVOLVED_BRAINS, LIVE_KEY_FOOD_CAP,
    LIVE_KEY_FOUNDERS, LIVE_KEY_LAKES, LIVE_KEY_LAKE_LAYOUT, LIVE_KEY_LOCOMOTION,
    LIVE_KEY_MORTALITY, LIVE_KEY_PREDATOR_FRACTION, LIVE_KEY_PREY_CAP, LIVE_KEY_REPRODUCTION,
    LIVE_KEY_THIRST_STEERING, LIVE_KEY_TREES,
};
use anima_engine_lib::core::observer::{ObserverAction, ObserverActionRecord, ObserverTrace};
use anima_engine_lib::core::resources::{BrainPolicy, LifetimeLearning, VitalStatistics};
use anima_engine_lib::core::world_record::{verify_record, WorldRecordRecorder};
use bevy_ecs::prelude::With;

const CHECKPOINTS: [u64; 4] = [0, 180, 1_800, 18_000];
const SEGMENT_TICKS: u64 = 180;

fn kv(pairs: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect())
}

fn enable_learning_and_build(seed: u64, with_recorder: bool) -> LiveExperimentAdapter {
    let initial = kv(&[
        (LIVE_KEY_FOUNDERS, 8.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.25),
        (LIVE_KEY_PREY_CAP, 16.0),
        (LIVE_KEY_TREES, 4.0),
        (LIVE_KEY_LAKES, 2.0),
        (LIVE_KEY_LAKE_LAYOUT, 1.0),
        (LIVE_KEY_FOOD_CAP, 30.0),
        (LIVE_KEY_MORTALITY, 1.0),
        (LIVE_KEY_REPRODUCTION, 1.0),
        (LIVE_KEY_EVOLVED_BRAINS, 1.0),
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_DRIVE_FLOOR, 0.5),
        (LIVE_KEY_THIRST_STEERING, 1.0),
    ]);
    let adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        seed,
        (16, 16),
        CHECKPOINTS[3],
    )
    .expect("learning + reproduction world builds");

    {
        let mut world = adapter.world();
        if let Some(mut policy) = world.get_resource_mut::<BrainPolicy>() {
            policy.evolved = true;
            policy.lifetime_learning = LifetimeLearning {
                enabled: true,
                learning_rate: 0.05,
                discount: 0.99,
                interval: 60,
                active_radius: f32::INFINITY,
            };
        }
        if with_recorder {
            let pairs = initial.entries.clone();
            let recorder = WorldRecordRecorder::from_world(
                &mut world,
                seed,
                SEGMENT_TICKS,
                true,
                pairs,
                vec!["gpu_microstate".to_string()],
            )
            .expect("recorder initializes");
            world.insert_resource(recorder);
        }
    }
    adapter
}

fn run_until_checkpoints(adapter: &mut LiveExperimentAdapter) -> Vec<(u64, u32)> {
    let mut results = Vec::with_capacity(CHECKPOINTS.len());
    results.push((0, adapter.checksum()));
    for target in CHECKPOINTS.into_iter().skip(1) {
        while adapter.tick() < target {
            adapter.run_schedule_once();
        }
        results.push((target, adapter.checksum()));
    }
    results
}

#[test]
fn recording_has_zero_feedback_on_default_world_across_18k_ticks() {
    const SEED: u64 = 700_001;
    let initial = InitialConditionSet::default();

    let mut off = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        SEED,
        (16, 16),
        CHECKPOINTS[3],
    )
    .expect("default world builds (OFF)");

    let mut on = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        SEED,
        (16, 16),
        CHECKPOINTS[3],
    )
    .expect("default world builds (ON)");
    {
        let mut world = on.world();
        let recorder = WorldRecordRecorder::from_world(
            &mut world,
            SEED,
            SEGMENT_TICKS,
            true,
            initial.entries.clone(),
            vec![],
        )
        .expect("recorder initializes");
        world.insert_resource(recorder);
    }

    let off_checks = run_until_checkpoints(&mut off);
    let on_checks = run_until_checkpoints(&mut on);
    assert_eq!(
        on_checks, off_checks,
        "WR1 recording must not perturb default world checksums at 0, 180, 1800, 18000"
    );

    let world = on.world();
    let recorder = world
        .get_resource::<WorldRecordRecorder>()
        .expect("recorder still present");
    assert_eq!(recorder.last_seal_error(), None);
    let record = recorder.record();
    assert_eq!(record.segments.len(), 100);
    for checkpoint_tick in [180u64, 1_800, 18_000] {
        let seg_idx = (checkpoint_tick / SEGMENT_TICKS - 1) as usize;
        let expected_csum = on_checks
            .iter()
            .find(|(t, _)| *t == checkpoint_tick)
            .map(|(_, c)| *c)
            .unwrap();
        assert_eq!(
            record.segments[seg_idx].body.end_checksum, expected_csum,
            "sealed segment checksum must match world_checksum at tick {checkpoint_tick}"
        );
    }
    let report = verify_record(record).expect("default world record verifies");
    assert_eq!(report.segment_count, 100);
    assert_eq!(report.first_tick, Some(1));
    assert_eq!(report.last_tick, Some(18_000));
    assert!(report.is_complete());
    assert!(report.replayable);
}

#[test]
fn recording_has_zero_feedback_with_learning_and_reproduction_enabled() {
    const SEED: u64 = 700_002;
    let mut off = enable_learning_and_build(SEED, false);
    let mut on = enable_learning_and_build(SEED, true);

    let off_checks = run_until_checkpoints(&mut off);
    let on_checks = run_until_checkpoints(&mut on);
    assert_eq!(
        on_checks, off_checks,
        "WR1 recording must not perturb learning + reproduction checksums at 0, 180, 1800, 18000"
    );

    // Non-vacuity: verify reproduction, mortality, and in-life learning all occurred.
    let mut world = on.world();
    let vitals = world
        .get_resource::<VitalStatistics>()
        .expect("vitals present")
        .clone();
    assert!(
        vitals.total_births > 0,
        "non-vacuity: reproduction must produce >= 1 birth (got 0)"
    );
    assert!(
        vitals.total_deaths > 0,
        "non-vacuity: mortality must produce >= 1 death (got 0)"
    );

    let mut learned_count = 0usize;
    {
        let mut q = world.query_filtered::<&AgentBrain, With<Agent>>();
        for brain in q.iter(&world) {
            if brain.learned.is_some() {
                learned_count += 1;
            }
        }
    }
    assert!(
        learned_count > 0,
        "non-vacuity: at least one agent must carry updated learned weights"
    );

    let recorder = world
        .get_resource::<WorldRecordRecorder>()
        .expect("recorder present");
    assert_eq!(recorder.last_seal_error(), None);
    let report = verify_record(recorder.record()).expect("active world record verifies");
    assert_eq!(report.segment_count, 100);
    assert!(
        report.total_events > 0,
        "non-vacuity: active run must emit lifecycle WorldEvent entries"
    );
    assert!(report.is_complete());
}

#[test]
fn intervention_and_observer_input_capture_does_not_perturb_trajectory() {
    const SEED: u64 = 700_003;
    let rain_cmd = InterventionCommand {
        id: 1,
        cause_id: 1,
        kind: InterventionKind::RainfallDelta,
        region: Region::Global,
        start_tick: 1,
        duration_ticks: 90,
        intensity: 0.25,
        signed_negative: false,
        curve: Curve::Step,
        reversible: true,
    };
    let obs_rec = ObserverActionRecord {
        tick: 5,
        action: ObserverAction::EvolutionToggled { running: false },
        cause_id: CAUSE_OBSERVER,
    };

    let build_pair = |with_recorder: bool| -> LiveExperimentAdapter {
        let a = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &InitialConditionSet::default(),
            std::slice::from_ref(&rain_cmd),
            SEED,
            (16, 16),
            120,
        )
        .expect("builds");
        {
            let mut world = a.world();
            if with_recorder {
                let rec =
                    WorldRecordRecorder::from_world(&mut world, SEED, 60, true, vec![], vec![])
                        .unwrap();
                world.insert_resource(rec);
            }
        }
        a
    };

    let mut off = build_pair(false);
    let mut on = build_pair(true);

    let interventions = [rain_cmd.clone()];
    let mut sim_rng_off = anima_engine_lib::core::resources::SimRng::for_world(SEED as u32);
    let mut sim_rng_on = anima_engine_lib::core::resources::SimRng::for_world(SEED as u32);
    let mut ledger_off = anima_engine_lib::core::causal::CausalLedger::new();
    let mut ledger_on = anima_engine_lib::core::causal::CausalLedger::new();
    use rand::SeedableRng;
    let _ = (&mut sim_rng_off, &mut sim_rng_on);
    let mut std_rng_off = rand::rngs::StdRng::seed_from_u64(SEED);
    let mut std_rng_on = rand::rngs::StdRng::seed_from_u64(SEED);

    for t in 1u64..=120 {
        if t == 5 {
            off.world()
                .get_resource_mut::<ObserverTrace>()
                .unwrap()
                .record_action(obs_rec);
            on.world()
                .get_resource_mut::<ObserverTrace>()
                .unwrap()
                .record_action(obs_rec);
        }
        let clock = anima_engine_lib::core::sim_clock::SimClock { tick: t };
        let active: Vec<&InterventionCommand> = interventions
            .iter()
            .filter(|c| c.active_at(t))
            .collect();
        off.step(&clock, &active, &mut ledger_off, &mut std_rng_off);
        on.step(&clock, &active, &mut ledger_on, &mut std_rng_on);
        assert_eq!(off.checksum(), on.checksum(), "diverged at tick {t}");
    }

    let world = on.world();
    let rec = world.get_resource::<WorldRecordRecorder>().unwrap().record();
    let report = verify_record(rec).unwrap();
    assert_eq!(report.segment_count, 2);
    assert_eq!(report.total_inputs, 2);
}
