//! Regression and negative-control tests for F-068:
//! live morphology `AddNode` mutations with paused evolution worker must keep
//! `SavedEvolutionWorkerState.node_id_counter` and `NextNodeId` synchronized across save/load
//! cycles without weakening `evolution_worker_resume_state`.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};
use anima_engine_lib::core::ecs::AgentClass;
use anima_engine_lib::core::simulation_lifecycle::{SavedSimulationState, SimulationEngine};
use anima_engine_lib::core::simulation_state::{
    evolution_worker_resume_state, restored_next_node_id,
};

const READY_DEADLINE: Duration = Duration::from_secs(10);
const READY_POLL: Duration = Duration::from_millis(5);

fn await_running_past_tick(engine: &SimulationEngine, tick_floor: u64) {
    let start = Instant::now();
    loop {
        let status = engine.get_status();
        if status.running && status.tick_count > tick_floor {
            return;
        }
        assert!(
            start.elapsed() < READY_DEADLINE,
            "timed out waiting for simulation engine to tick past {tick_floor} \
             (running={}, tick_count={})",
            status.running,
            status.tick_count
        );
        thread::sleep(READY_POLL);
    }
}

fn save_state(engine: &SimulationEngine, context: &str) -> SavedSimulationState {
    let (tx, rx) = std::sync::mpsc::channel();
    engine
        .save_request_tx
        .send(tx)
        .unwrap_or_else(|e| panic!("failed to send save request ({context}): {e}"));
    rx.recv_timeout(Duration::from_secs(5))
        .unwrap_or_else(|e| panic!("timeout/channel error waiting for save ({context}): {e}"))
        .unwrap_or_else(|why| panic!("save refused ({context}): {why}"))
}

fn greatest_saved_node_id(state: &SavedSimulationState) -> u32 {
    state
        .agents
        .iter()
        .flat_map(|agent| agent.genotype.nodes.iter().map(|node| node.id))
        .chain(
            state
                .dormant_agents
                .iter()
                .flat_map(|dormant| dormant.agent.genotype.nodes.iter().map(|node| node.id)),
        )
        .chain(
            state
                .lineage_nodes
                .iter()
                .filter_map(|node| node.genotype.as_ref())
                .flat_map(|genotype| genotype.nodes.iter().map(|node| node.id)),
        )
        .max()
        .unwrap_or(2)
}

#[test]
fn f068_live_add_node_with_evo_paused_survives_save_load_cycles() {
    let evo_settings = Arc::new(Mutex::new(EvolutionSettings {
        mutation_rate: 1.0,
        selection_bias: 1.5,
        grid_resolution: 50,
    }));
    // Keep the evolution worker paused, matching `test_100_save_load_cycles`.
    let evo_running = Arc::new(AtomicBool::new(false));
    let map_elites_grid = Arc::new(Mutex::new(MapElitesGridState {
        grid: std::collections::HashMap::new(),
        grid_resolution: 50,
    }));

    let engine = SimulationEngine::new();
    engine.start::<tauri::test::MockRuntime>(
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_elites_grid),
    );
    await_running_past_tick(&engine, 0);
    let mut current_state = save_state(&engine, "initial genesis");
    engine.stop();

    let mut observed_add_node = greatest_saved_node_id(&current_state) >= 3;
    let mut post_mutation_cycles = 0usize;

    for cycle in 1..=40usize {
        if !observed_add_node {
            // Re-arm prey reserves between cycles until at least one live birth executes
            // `MorphologyMutation::AddNode` while `evo_running == false`.
            for agent in &mut current_state.agents {
                if matches!(agent.class, AgentClass::Prey) {
                    agent.homeostatic_state.energy = 95.0;
                }
            }
        }

        let prev_tick = current_state.tick_count;
        *engine.pending_load_state.lock().unwrap() = Some(current_state.clone());
        engine.start::<tauri::test::MockRuntime>(
            None,
            Arc::clone(&evo_settings),
            Arc::clone(&evo_running),
            Arc::clone(&map_elites_grid),
        );
        await_running_past_tick(&engine, prev_tick);
        thread::sleep(Duration::from_millis(10));
        let next_state = save_state(&engine, &format!("cycle {cycle}"));
        engine.stop();

        let max_node_id = greatest_saved_node_id(&next_state);
        let worker = next_state
            .evolution_worker
            .as_ref()
            .expect("saved state must carry evolution worker checkpoint");
        assert!(
            worker.node_id_counter > max_node_id,
            "cycle {cycle}: worker.node_id_counter ({}) must exceed greatest_saved_node_id ({max_node_id})",
            worker.node_id_counter
        );
        assert!(
            restored_next_node_id(&next_state) > max_node_id,
            "cycle {cycle}: restored_next_node_id ({}) must exceed greatest_saved_node_id ({max_node_id})",
            restored_next_node_id(&next_state)
        );
        // Verify restore check accepts the checkpoint cleanly.
        evolution_worker_resume_state(Some(&next_state), next_state.sim_rng_seed)
            .unwrap_or_else(|err| panic!("cycle {cycle} checkpoint rejected on restore: {err}"));

        current_state = next_state;
        if max_node_id >= 3 {
            observed_add_node = true;
            post_mutation_cycles += 1;
            if post_mutation_cycles >= 5 {
                break;
            }
        }
    }

    assert!(
        observed_add_node,
        "test did not observe any live AddNode mutation (max node id stayed <= 2); cannot verify F-068 non-vacuously"
    );
    assert!(
        post_mutation_cycles >= 5,
        "expected at least 5 clean save/restore cycles after live AddNode mutation"
    );
}

#[test]
fn f068_corrupt_morphology_cursor_is_still_rejected() {
    let evo_settings = Arc::new(Mutex::new(EvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    }));
    let evo_running = Arc::new(AtomicBool::new(false));
    let map_elites_grid = Arc::new(Mutex::new(MapElitesGridState {
        grid: std::collections::HashMap::new(),
        grid_resolution: 50,
    }));

    let engine = SimulationEngine::new();
    engine.start::<tauri::test::MockRuntime>(
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_elites_grid),
    );
    await_running_past_tick(&engine, 0);
    let mut state = save_state(&engine, "corrupt-cursor baseline");
    engine.stop();

    // Even with a corrupted worker cursor behind saved genotypes, restore must still refuse.
    let min_required = greatest_saved_node_id(&state) + 1;
    let worker = state
        .evolution_worker
        .as_mut()
        .expect("saved state must carry evolution worker checkpoint");
    assert!(worker.node_id_counter >= min_required);
    worker.node_id_counter = min_required - 1;

    let err = evolution_worker_resume_state(Some(&state), state.sim_rng_seed)
        .expect_err("corrupted cursor preceding saved genotypes must still be rejected");
    assert!(
        err.contains("evolution morphology-node cursor precedes saved genotypes"),
        "unexpected error: {err}"
    );
}
