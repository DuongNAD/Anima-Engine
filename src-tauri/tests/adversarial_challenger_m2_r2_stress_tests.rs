//! Live checkpoint parity under stress: a multi-stage save/load disk round trip, and a checkpoint
//! fork's control branch, must both land where an uninterrupted live run lands.
//!
//! These are the same two properties `live_experiment_tests` gates, over longer runs and more
//! checkpoints. Until 2026-11-05 this file also held four `core::branch_tree` tests; that module was
//! removed from `main` with the rest of the 2026-08-16 block (`STATE_OF_THE_PROJECT.md` §1.g).

use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, WorldLawSet,
    MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{
    checkpoint_fork, run_manifest_seed, ExperimentModel,
};
use anima_engine_lib::core::intervention::{
    Curve, InterventionCommand, InterventionKind, InterventionQueue, Region,
};
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LiveSnapshot, LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::core::snapshot::{self, SnapshotEnvelope};
use anima_engine_lib::core::world_artifact::WorldIdentity;
use rand::SeedableRng;

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn live_registry() -> ObservableRegistry {
    ObservableRegistry::live_default()
}

fn live_base_manifest(duration_ticks: u64, seed: u64) -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "adv_m2_r2_live_base".into(),
        name: "Adversarial Live Base Manifest".into(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: InitialConditionSet::new(vec![
            ("live.founders".to_string(), 6.0),
            ("live.predator_fraction".to_string(), 0.5),
            ("live.trees".to_string(), 4.0),
            ("live.lakes".to_string(), 2.0),
            ("live.food_cap".to_string(), 50.0),
        ]),
        interventions: Vec::new(),
        seeds: vec![seed],
        duration_ticks,
        sample_period: 60,
        observable_ids: LIVE_OBSERVABLE_IDS.iter().map(|s| s.to_string()).collect(),
        exotic_interventions: Vec::new(),
        observer: Default::default(),
    }
}

fn through_disk_temp(snapshot: &LiveSnapshot, label: &str) -> LiveSnapshot {
    static DISK_CTR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(5000);
    let id = DISK_CTR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "anima_challenger_r2_{}_{}_{}",
        std::process::id(),
        id,
        label
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("checkpoint.json");
    let envelope = SnapshotEnvelope::seal(snapshot.saved.clone()).expect("seal");
    snapshot::write_atomic(&path, &envelope).expect("write");
    let back = snapshot::read(&path).expect("read");
    let _ = std::fs::remove_dir_all(&dir);
    LiveSnapshot {
        saved: back,
        config: snapshot.config,
        seed: snapshot.seed,
    }
}

// =============================================================================
// SUITE 1: DEFECT 1 - Live Simulation Checkpoint Parity & Fork Stress
// =============================================================================

/// Stress-test: Save/load live simulation state across ecology boundaries (K = 60).
/// Verifies bit-exact checksum equality between resumed and uninterrupted runs.
#[test]
fn stress_test_live_sim_checkpoint_disk_roundtrip_parity() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let seed = 4242;
    let duration = 180;
    let laws = WorldLawSet::baseline();
    let initial = InitialConditionSet::new(vec![
        ("live.founders".to_string(), 6.0),
        ("live.predator_fraction".to_string(), 0.5),
        ("live.trees".to_string(), 4.0),
        ("live.lakes".to_string(), 2.0),
        ("live.food_cap".to_string(), 50.0),
    ]);
    let queue = InterventionQueue::new(vec![]);

    // 1. Uninterrupted reference execution
    let mut uninterrupted =
        LiveExperimentAdapter::from_manifest(&laws, &initial, &[], seed, (16, 16), duration)
            .expect("build uninterrupted");
    let mut rng_ref = rand::rngs::StdRng::seed_from_u64(seed);
    let mut clock_ref = SimClock::new();
    let mut ledger_ref = CausalLedger::new();

    for _ in 0..duration {
        let tick = clock_ref.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        uninterrupted.step(&clock_ref, &active, &mut ledger_ref, &mut rng_ref);
        let _ = uninterrupted.observables();
    }
    let expected_checksum = uninterrupted.checksum();
    let expected_obs = uninterrupted.observables();

    // 2. Interrupted execution at K = 60
    let k = 60;
    let mut interrupted =
        LiveExperimentAdapter::from_manifest(&laws, &initial, &[], seed, (16, 16), duration)
            .expect("build interrupted");
    let mut rng_b = rand::rngs::StdRng::seed_from_u64(seed);
    let mut clock_b = SimClock::new();
    let mut ledger_b = CausalLedger::new();
    for _ in 0..k {
        let tick = clock_b.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        interrupted.step(&clock_b, &active, &mut ledger_b, &mut rng_b);
        let _ = interrupted.observables();
    }

    let snap = through_disk_temp(&interrupted.snapshot(), "k60");
    let mut resumed = LiveExperimentAdapter::from_snapshot(&snap).expect("restore");
    let mut clock_c = SimClock { tick: k };
    for _ in 0..(duration - k) {
        let tick = clock_c.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        resumed.step(&clock_c, &active, &mut ledger_b, &mut rng_b);
        let _ = resumed.observables();
    }

    assert_eq!(
        expected_checksum,
        resumed.checksum(),
        "Live simulation resumed from tick 60 must match uninterrupted baseline bit-exactly"
    );
    assert_eq!(expected_obs, resumed.observables());
}

/// Stress-test: Checkpoint fork with LiveExperimentAdapter.
/// Asserts that control branch reproduced bit-exact checksum equality with uninterrupted run.
#[test]
fn stress_test_live_checkpoint_fork_control_parity() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let reg = live_registry();
    let seed = 4242;
    let m = live_base_manifest(180, seed);
    let report = checkpoint_fork::<LiveExperimentAdapter>(
        &m,
        &reg,
        seed,
        60,
        &[InterventionCommand {
            id: 10,
            cause_id: 100,
            kind: InterventionKind::Deforest,
            region: Region::Global,
            start_tick: 61,
            duration_ticks: 120,
            intensity: 0.5,
            signed_negative: true,
            curve: Curve::Step,
            reversible: false,
        }],
    )
    .expect("checkpoint_fork must succeed");

    let uninterrupted = run_manifest_seed::<LiveExperimentAdapter>(&m, &reg, seed, None, None);
    assert_eq!(
        report.control.final_checksum, uninterrupted.final_checksum,
        "Control branch from tick 60 diverged from uninterrupted run"
    );
    assert_ne!(
        report.control.final_checksum, report.treatment.final_checksum,
        "Treatment branch did not diverge from control"
    );
}
