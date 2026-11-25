//! Card R1c Condition 3: schema-7 Burn 0.13 learner migration and schema-8 burnpack round-trip.

use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};
use anima_engine_lib::core::components::CognitiveState;
use anima_engine_lib::core::simulation_lifecycle::{SavedSimulationState, SimulationEngine};
use anima_engine_lib::core::snapshot::{
    self, read_with_warnings, SnapshotEnvelope, SnapshotMigrationWarning, SCHEMA_VERSION,
};
use anima_engine_lib::core::training::{check_saved_learner_worker, LearnerRecordError};
use std::path::Path;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use std::time::{Duration, Instant};

const READY_DEADLINE: Duration = Duration::from_secs(10);

fn default_evo_args() -> (
    Arc<Mutex<EvolutionSettings>>,
    Arc<AtomicBool>,
    Arc<Mutex<MapElitesGridState>>,
) {
    (
        Arc::new(Mutex::new(EvolutionSettings {
            mutation_rate: 0.1,
            selection_bias: 1.0,
            grid_resolution: 40,
        })),
        Arc::new(AtomicBool::new(false)),
        Arc::new(Mutex::new(MapElitesGridState {
            grid: std::collections::HashMap::new(),
            grid_resolution: 40,
        })),
    )
}

fn await_running_past_tick(engine: &SimulationEngine, tick_floor: u64) {
    let start = Instant::now();
    loop {
        let status = engine.get_status();
        if status.running && status.tick_count > tick_floor {
            return;
        }
        assert!(
            start.elapsed() < READY_DEADLINE,
            "timed out waiting for simulation past tick {tick_floor}; last running={}, tick={}",
            status.running,
            status.tick_count
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn assert_world_state_matches(raw: &SavedSimulationState, migrated: &SavedSimulationState) {
    assert_eq!(migrated.tick_count, raw.tick_count);
    assert_eq!(migrated.sim_rng_seed, raw.sim_rng_seed);
    assert_eq!(migrated.sim_rng_pos, raw.sim_rng_pos);
    assert_eq!(migrated.eco_detritus.to_bits(), raw.eco_detritus.to_bits());
    assert_eq!(migrated.eco_plants.to_bits(), raw.eco_plants.to_bits());
    assert_eq!(migrated.eco_animals.to_bits(), raw.eco_animals.to_bits());
    assert_eq!(migrated.resource_field_r.len(), raw.resource_field_r.len());
    assert_eq!(migrated.foods.len(), raw.foods.len());
    assert_eq!(migrated.trees.len(), raw.trees.len());
    assert_eq!(migrated.lakes.len(), raw.lakes.len());
    assert_eq!(migrated.agents.len(), raw.agents.len());
    for (m_agent, r_agent) in migrated.agents.iter().zip(raw.agents.iter()) {
        assert_eq!(m_agent.lineage_id, r_agent.lineage_id);
        assert_eq!(m_agent.root_position, r_agent.root_position);
        assert_eq!(
            m_agent.homeostatic_state.energy,
            r_agent.homeostatic_state.energy
        );
        assert_eq!(m_agent.segments.len(), r_agent.segments.len());
        assert!(!matches!(
            m_agent.cognitive_state,
            CognitiveState::PendingInference(_)
        ));
    }
}

#[test]
fn pre_upgrade_schema7_snapshot_restores_world_resets_learner_and_reports_named_error() {
    assert_eq!(SCHEMA_VERSION, 8);
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/snapshots/schema7_burn013_shared_learner.json");
    let raw_bytes = std::fs::read(&fixture_path).expect("schema-7 fixture file must exist");
    let envelope: SnapshotEnvelope =
        serde_json::from_slice(&raw_bytes).expect("fixture must parse as SnapshotEnvelope");
    assert_eq!(envelope.schema_version, 7);
    envelope
        .verify()
        .expect("pre-upgrade schema-7 fixture checksum must verify");

    let raw_state = envelope
        .parse_state()
        .expect("pre-upgrade raw state must parse");
    let raw_shared = raw_state
        .shared_learning
        .as_ref()
        .expect("pre-upgrade schema-7 fixture must carry shared_learning");
    assert!(raw_shared.model_update_diagnostics.published >= 1);
    assert!(!raw_shared.learner.training_model_record.is_empty());
    assert!(!raw_shared.learner.optimizer_record.is_empty());

    // Direct validation against Burn 0.22 rejects Burn 0.13 BinBytesRecorder payloads cleanly.
    let direct_check = check_saved_learner_worker(&raw_shared.learner, raw_state.sim_rng_seed);
    assert!(
        matches!(
            direct_check,
            Err(LearnerRecordError::ModelRecordUnreadable(_))
                | Err(LearnerRecordError::ModelLoadFailed(_))
                | Err(LearnerRecordError::OptimizerRecordUnreadable(_))
        ),
        "expected named Burn 0.13 decode rejection, got {direct_check:?}"
    );

    // Loading via snapshot::read_with_warnings succeeds, emits the named error warning, restores
    // the world, and clears shared_learning so the learner starts fresh.
    let (migrated, warnings) =
        read_with_warnings(&fixture_path).expect("schema-7 snapshot must migrate cleanly");
    assert_eq!(migrated.loaded_from_schema, 7);
    assert!(
        migrated.shared_learning.is_none(),
        "schema-7 Burn 0.13 learner state must be dropped so the learner starts fresh"
    );
    assert_eq!(
        warnings,
        vec![SnapshotMigrationWarning::Schema7LearnerReset(
            LearnerRecordError::Schema7Burn013Unsupported {
                model_record_bytes: raw_shared.learner.training_model_record.len(),
                optimizer_record_bytes: raw_shared.learner.optimizer_record.len(),
            }
        )]
    );
    assert!(
        warnings[0]
            .to_string()
            .contains("LearnerRecordError::Schema7Burn013Unsupported"),
        "unexpected warning text: {}",
        warnings[0]
    );
    assert_world_state_matches(&raw_state, &migrated);

    // Booting SimulationEngine from the migrated schema-7 snapshot starts a fresh Burn 0.22
    // learner and advances ticks without error. First assert that the frozen legacy v1 fixture
    // mismatches the switched-over C01 World v2 desktop identity, then align the in-memory
    // identity to the active desktop world so learner migration runs without modifying the fixture.
    let active_identity =
        anima_engine_lib::core::world_artifact::active_desktop_world_identity(None)
            .expect("active desktop world identity must resolve");
    assert!(
        migrated
            .world_identity
            .mismatch_against(&active_identity)
            .is_some(),
        "frozen v1 schema-7 fixture must mismatch switched-over C01 desktop world"
    );
    let mut migrated = migrated;
    migrated.world_identity = active_identity;

    let resume_tick = migrated.tick_count;
    let engine = SimulationEngine::new();
    *engine.pending_load_state.lock().unwrap() = Some(migrated);
    let (evo_settings, evo_running, map_elites_grid) = default_evo_args();
    std::env::set_var("ANIMA_USE_GPU", "0");
    engine.start::<tauri::test::MockRuntime>(None, evo_settings, evo_running, map_elites_grid);
    await_running_past_tick(&engine, resume_tick + 2);

    let (tx, rx) = std::sync::mpsc::channel();
    engine.save_request_tx.send(tx).expect("send save request");
    let saved_after = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("save reply")
        .expect("migrated simulation saves cleanly");
    engine.stop();

    assert!(saved_after.tick_count > resume_tick);
    let new_shared = saved_after
        .shared_learning
        .as_ref()
        .expect("resumed engine produces a schema-8 shared_learning state");
    assert!(
        new_shared.model_update_diagnostics.published
            < raw_shared.model_update_diagnostics.published,
        "shared learner diagnostics must restart fresh rather than keeping schema-7 published={}",
        raw_shared.model_update_diagnostics.published
    );
    check_saved_learner_worker(&new_shared.learner, saved_after.sim_rng_seed)
        .expect("freshly saved schema-8 learner records must validate under Burn 0.22");
}

#[test]
fn schema8_snapshot_round_trips_burn022_learner_and_resumes_cleanly() {
    std::env::set_var("ANIMA_DETERMINISTIC", "1");
    std::env::set_var("ANIMA_SIM_SEED", "424242");
    std::env::set_var("ANIMA_USE_GPU", "0");

    let engine = SimulationEngine::new();
    let (evo_settings, evo_running, map_elites_grid) = default_evo_args();
    engine.start::<tauri::test::MockRuntime>(
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_elites_grid),
    );

    let deadline = Instant::now() + Duration::from_secs(15);
    let mut active_save = None;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
        let status = engine.get_status();
        if !status.running || status.tick_count < 15 {
            continue;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        if engine.save_request_tx.send(tx).is_err() {
            continue;
        }
        if let Ok(Ok(saved)) = rx.recv_timeout(Duration::from_secs(2)) {
            if let Some(shared) = saved.shared_learning.as_ref() {
                if shared.model_update_diagnostics.published >= 1
                    && !shared.learner.training_model_record.is_empty()
                    && !shared.learner.optimizer_record.is_empty()
                {
                    active_save = Some(saved);
                    break;
                }
            }
        }
    }
    engine.stop();

    let saved = active_save.expect("schema-8 simulation must publish at least one learner update");
    let orig_shared = saved.shared_learning.clone().unwrap();

    let temp_dir = std::env::temp_dir().join(format!("anima_schema8_rt_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");
    let snap_path = temp_dir.join("schema8.json");
    let envelope = SnapshotEnvelope::seal(saved.clone()).expect("seal schema-8 snapshot");
    assert_eq!(envelope.schema_version, 8);
    snapshot::write_atomic(&snap_path, &envelope).expect("write schema-8 snapshot");

    let (restored, warnings) = read_with_warnings(&snap_path).expect("read schema-8 snapshot");
    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(
        warnings.is_empty(),
        "schema-8 must emit no migration warning"
    );
    assert_eq!(restored.loaded_from_schema, 8);
    let restored_shared = restored
        .shared_learning
        .as_ref()
        .expect("schema-8 snapshot must preserve shared_learning");
    assert_eq!(
        restored_shared.learner.training_model_record,
        orig_shared.learner.training_model_record
    );
    assert_eq!(
        restored_shared.learner.optimizer_record,
        orig_shared.learner.optimizer_record
    );
    assert_eq!(
        restored_shared.learner.partial_batch,
        orig_shared.learner.partial_batch
    );
    assert_eq!(
        restored_shared.inference_weights,
        orig_shared.inference_weights
    );
    assert_eq!(
        restored_shared.model_update_diagnostics,
        orig_shared.model_update_diagnostics
    );
    check_saved_learner_worker(&restored_shared.learner, restored.sim_rng_seed)
        .expect("round-tripped schema-8 learner records must validate");

    let resume_tick = restored.tick_count;
    let engine2 = SimulationEngine::new();
    *engine2.pending_load_state.lock().unwrap() = Some(restored);
    let (evo_settings2, evo_running2, map_elites_grid2) = default_evo_args();
    engine2.start::<tauri::test::MockRuntime>(None, evo_settings2, evo_running2, map_elites_grid2);
    await_running_past_tick(&engine2, resume_tick + 2);
    engine2.stop();
}

#[test]
fn simulation_loop_handles_raw_schema7_shared_learning_state_without_aborting() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/snapshots/schema7_burn013_shared_learner.json");
    let raw_bytes = std::fs::read(&fixture_path).expect("fixture exists");
    let envelope: SnapshotEnvelope = serde_json::from_slice(&raw_bytes).unwrap();
    let mut unmigrated = envelope.parse_state().unwrap();
    unmigrated.loaded_from_schema = 7;
    assert!(unmigrated.shared_learning.is_some());
    let active_identity =
        anima_engine_lib::core::world_artifact::active_desktop_world_identity(None)
            .expect("active desktop world identity must resolve");
    assert!(
        unmigrated
            .world_identity
            .mismatch_against(&active_identity)
            .is_some(),
        "frozen v1 schema-7 fixture must mismatch switched-over C01 desktop world"
    );
    unmigrated.world_identity = active_identity;

    let tick_before = unmigrated.tick_count;
    let engine = SimulationEngine::new();
    *engine.pending_load_state.lock().unwrap() = Some(unmigrated);
    let (evo_settings, evo_running, map_elites_grid) = default_evo_args();
    std::env::set_var("ANIMA_USE_GPU", "0");
    engine.start::<tauri::test::MockRuntime>(None, evo_settings, evo_running, map_elites_grid);
    await_running_past_tick(&engine, tick_before + 1);
    engine.stop();
}
