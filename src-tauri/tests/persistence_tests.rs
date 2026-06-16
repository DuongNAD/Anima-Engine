use std::sync::{Arc, Mutex, atomic::AtomicBool};
use anima_engine_lib::core::simulation_lifecycle::{SimulationEngine, SavedSimulationState};
use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};

#[test]
fn test_saved_simulation_state_serialization() {
    let state = SavedSimulationState {
        tick_count: 42,
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
        pheromone_grid: anima_engine_lib::core::simulation_lifecycle::SerializedPheromoneGrid {
            values: vec![0.1; 16384],
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
        agents: vec![],
        evolution_settings: EvolutionSettings {
            mutation_rate: 0.2,
            selection_bias: 1.2,
            grid_resolution: 30,
        },
        map_elites_grid: MapElitesGridState {
            grid: std::collections::HashMap::new(),
            grid_resolution: 30,
        },
        chronicle_history: vec![],
        lineage_nodes: vec![],
        lineage_relations: vec![],
        lakes: vec![],
        trees: vec![],
    };

    let serialized = serde_json::to_string(&state);
    assert!(serialized.is_ok());
    let json_str = serialized.unwrap();

    let deserialized = serde_json::from_str::<SavedSimulationState>(&json_str);
    assert!(deserialized.is_ok());
    let state_back = deserialized.unwrap();

    assert_eq!(state_back.tick_count, 42);
    assert_eq!(state_back.food_spawn_settings.max_food_count, 15);
    assert_eq!(state_back.map_bounds.min.x, -50.0);
    assert_eq!(state_back.epoch_manager.current_epoch, 3);
    assert_eq!(state_back.pheromone_grid.diffusion_rate, 0.12);
    assert_eq!(state_back.foods[0].position.z, 2.0);
}

#[test]
fn test_engine_save_load_lifecycle() {
    let engine = SimulationEngine::new();
    let evo_settings = Arc::new(Mutex::new(EvolutionSettings {
        mutation_rate: 0.1,
        selection_bias: 1.0,
        grid_resolution: 40,
    }));
    let evo_running = Arc::new(AtomicBool::new(false));
    let map_elites_grid = Arc::new(Mutex::new(MapElitesGridState {
        grid: std::collections::HashMap::new(),
        grid_resolution: 40,
    }));

    // Start simulation
    engine.start::<tauri::test::MockRuntime>(
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_elites_grid),
    );

    // Wait a short moment for ticks to happen
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Perform save request
    let (tx, rx) = std::sync::mpsc::channel();
    let send_res = engine.save_request_tx.send(tx);
    assert!(send_res.is_ok());

    let saved_state = rx.recv_timeout(std::time::Duration::from_secs(2));
    assert!(saved_state.is_ok());
    let saved_state = saved_state.unwrap();

    assert!(saved_state.tick_count > 0);
    assert_eq!(saved_state.agents.len(), 10);

    // Stop engine
    engine.stop();

    // Modify some state to load
    let mut modified_state = saved_state.clone();
    modified_state.tick_count = 1000;
    modified_state.epoch_manager.current_epoch = 99;

    // Put into pending load state
    *engine.pending_load_state.lock().unwrap() = Some(modified_state);

    // Restart engine
    engine.start::<tauri::test::MockRuntime>(
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_elites_grid),
    );

    std::thread::sleep(std::time::Duration::from_millis(200));

    // Perform save again to verify loading was successful
    let (tx2, rx2) = std::sync::mpsc::channel();
    let _ = engine.save_request_tx.send(tx2);
    let loaded_state = rx2.recv_timeout(std::time::Duration::from_secs(2));
    assert!(loaded_state.is_ok());
    let loaded_state = loaded_state.unwrap();

    assert!(loaded_state.tick_count >= 1000);
    assert_eq!(loaded_state.epoch_manager.current_epoch, 99);
    assert_eq!(loaded_state.agents.len(), 10);

    engine.stop();
}
