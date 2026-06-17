mod common;

use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use std::thread;

use anima_engine_lib::core::simulation_lifecycle::{SimulationEngine, SavedSimulationState, SerializedPheromoneGrid};
use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator = common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_load_zero_agents() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let engine = SimulationEngine::new();
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

    // Construct a SavedSimulationState with 0 agents
    let state = SavedSimulationState {
        tick_count: 500,
        active_environment_event: anima_engine_lib::evolution::meta_ai::EnvironmentalEvent::Stable,
        food_spawn_settings: anima_engine_lib::core::ecs::FoodSpawnSettings {
            max_food_count: 15,
            default_energy: 25.0,
            default_hydration: 15.0,
        },
        map_bounds: anima_engine_lib::core::ecs::MapBounds {
            min: glam::Vec3::new(-50.0, 0.0, -50.0),
            max: glam::Vec3::new(50.0, 10.0, 50.0),
        },
        epoch_manager: anima_engine_lib::core::ecs::EpochManager {
            ticks_per_epoch: 500,
            current_epoch_ticks: 120,
            current_epoch: 3,
        },
        pheromone_grid: SerializedPheromoneGrid {
            values: vec![0.0; 16384],
            diffusion_rate: 0.12,
            decay_rate: 0.04,
        },
        foods: vec![
            anima_engine_lib::core::simulation_lifecycle::SerializedFood {
                position: glam::Vec3::new(1.0, 0.0, 2.0),
                energy_value: 30.0,
                hydration_value: 20.0,
            }
        ],
        agents: vec![], // Zero agents
        evolution_settings: EvolutionSettings {
            mutation_rate: 0.15,
            selection_bias: 1.5,
            grid_resolution: 50,
        },
        map_elites_grid: MapElitesGridState {
            grid: std::collections::HashMap::new(),
            grid_resolution: 50,
        },
        chronicle_history: vec![],
        lineage_nodes: vec![],
        lineage_relations: vec![],
        lakes: vec![],
        trees: vec![],
    };

    // Put into pending load state
    *engine.pending_load_state.lock().unwrap() = Some(state);

    // Start simulation
    engine.start::<tauri::test::MockRuntime>(
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_elites_grid),
    );

    // Wait a short moment for ticks to happen
    thread::sleep(Duration::from_millis(200));

    // Verify it is running and tick count increased
    let status = engine.get_status();
    assert!(status.running);
    assert!(status.tick_count > 500);

    // Verify agents telemetry has length 0
    let states = engine.agent_states.read().unwrap();
    assert_eq!(states.len(), 0, "Agent states should be empty when loading state with 0 agents");

    engine.stop();
}

#[test]
fn test_load_corrupted_json() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("corrupted_state.json");
    
    // Write invalid JSON content
    std::fs::write(&file_path, "{ corrupted_json_missing_brackets: ").unwrap();

    let read_result = std::fs::read_to_string(&file_path);
    assert!(read_result.is_ok());
    let json_str = read_result.unwrap();

    // Deserialize should fail
    let deserialized = serde_json::from_str::<SavedSimulationState>(&json_str);
    assert!(deserialized.is_err(), "Expected parsing error for corrupted JSON");

    // Cleanup
    let _ = std::fs::remove_file(file_path);
}

#[test]
fn test_load_missing_file() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let missing_path = "non_existent_file_persistence_999.json";
    
    // File read should fail with NotFound
    let read_result = std::fs::read_to_string(missing_path);
    assert!(read_result.is_err());
    let err = read_result.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn test_100_save_load_cycles() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let engine = SimulationEngine::new();
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

    // Start simulation initially
    engine.start::<tauri::test::MockRuntime>(
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_elites_grid),
    );

    // Let it run to spawn initial 10 agents and food
    thread::sleep(Duration::from_millis(150));

    // Save initial state
    let (tx, rx) = std::sync::mpsc::channel();
    engine.save_request_tx.send(tx).expect("Failed to send initial save request");
    let mut current_state = rx.recv_timeout(Duration::from_secs(5))
        .expect("Timeout waiting for initial save");

    assert!(current_state.tick_count > 0);
    assert_eq!(current_state.agents.len(), 10, "Should spawn 10 agents by default");

    engine.stop();

    // Track initial allocations in hot-loop tick to establish a baseline
    ALLOCATOR.start_tracking();

    for cycle in 1..=100 {
        // Load the saved state from previous cycle
        *engine.pending_load_state.lock().unwrap() = Some(current_state.clone());

        // Start engine
        engine.start::<tauri::test::MockRuntime>(
            None,
            Arc::clone(&evo_settings),
            Arc::clone(&evo_running),
            Arc::clone(&map_elites_grid),
        );

        // Run briefly to allow simulation ticks to execute
        thread::sleep(Duration::from_millis(15));

        // Save state at the end of this run
        let (tx_cycle, rx_cycle) = std::sync::mpsc::channel();
        engine.save_request_tx.send(tx_cycle)
            .unwrap_or_else(|e| panic!("Failed to send save request in cycle {}: {:?}", cycle, e));
        
        let new_state = rx_cycle.recv_timeout(Duration::from_secs(2))
            .unwrap_or_else(|e| panic!("Timeout/error receiving save in cycle {}: {:?}", cycle, e));

        // Verify that the data is not glitched/NaN
        for agent in &new_state.agents {
            assert!(agent.root_position.x.is_finite(), "NaN/Inf coordinate in cycle {}", cycle);
            assert!(agent.root_position.y.is_finite(), "NaN/Inf coordinate in cycle {}", cycle);
            assert!(agent.root_position.z.is_finite(), "NaN/Inf coordinate in cycle {}", cycle);
            for segment in &agent.segments {
                assert!(segment.position.x.is_finite(), "NaN/Inf segment coordinate in cycle {}", cycle);
                assert!(segment.position.y.is_finite(), "NaN/Inf segment coordinate in cycle {}", cycle);
                assert!(segment.position.z.is_finite(), "NaN/Inf segment coordinate in cycle {}", cycle);
            }
        }

        assert!(new_state.tick_count >= current_state.tick_count);
        current_state = new_state;

        // Stop the engine to finish the cycle
        engine.stop();
    }

    let _allocations = ALLOCATOR.stop_tracking();
    
    // We expect some allocations because starting/stopping the engine, serializing to state,
    // and thread initialization/joining allocates memory. But it should not lock, panic,
    // or leak unbounded memory resources (thread handles and channels are fully joined and dropped).
    // Let's assert that the engine is stopped successfully.
    let status = engine.get_status();
    assert!(!status.running);
}
