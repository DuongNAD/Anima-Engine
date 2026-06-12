use std::sync::Arc;
use std::time::Duration;
use std::thread;
use anima_engine_lib::core::engine::SimulationEngine;
use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};

#[test]
fn test_engine_websocket_address_reuse() {
    let engine = SimulationEngine::new();

    let evolution_settings = Arc::new(std::sync::Mutex::new(EvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    }));
    let evolution_running = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let map_elites_grid = Arc::new(std::sync::Mutex::new(MapElitesGridState {
        grid: std::collections::HashMap::new(),
        grid_resolution: 50,
    }));

    // Choose a fixed port for testing socket address reuse
    let test_port = 25983;

    // Run 15 rapid start-stop toggle cycles using a FIXED port
    for i in 0..15 {
        {
            let mut sharding_config = engine.sharding_config.write().unwrap();
            sharding_config.local_port = test_port;
        }

        // Start the engine
        engine.start::<tauri::test::MockRuntime>(
            None,
            Arc::clone(&evolution_settings),
            Arc::clone(&evolution_running),
            Arc::clone(&map_elites_grid),
        );

        // Assert it is running
        assert!(engine.running.load(std::sync::atomic::Ordering::SeqCst), "Engine should be running on iteration {}", i);

        // Sleep briefly to let the websocket server bind and start listening
        thread::sleep(Duration::from_millis(100));

        // Stop the engine (joins threads, closes the websocket server, drains channels)
        engine.stop();

        // Assert it is stopped
        assert!(!engine.running.load(std::sync::atomic::Ordering::SeqCst), "Engine should be stopped on iteration {}", i);

        // Verify that the threads Option is cleared and joined
        {
            let threads_lock = engine.threads.lock().unwrap();
            assert!(threads_lock.is_none(), "Threads should be joined and None on iteration {}", i);
        }
    }
}
