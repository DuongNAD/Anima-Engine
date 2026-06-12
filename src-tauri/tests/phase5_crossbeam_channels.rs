use std::sync::Arc;
use std::time::Duration;
use std::thread;
use anima_engine_lib::core::engine::SimulationEngine;
use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};

#[test]
fn test_engine_toggle_stress_channels() {
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

    // Run 25 rapid start-stop toggle cycles sequentially
    for i in 0..25 {
        // Set dynamic port to 0 to automatically bind to any free port and avoid port conflicts
        {
            let mut sharding_config = engine.sharding_config.write().unwrap();
            sharding_config.local_port = 0;
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

        // Sleep briefly to let threads spawn, execute channel requests
        thread::sleep(Duration::from_millis(50));

        // Stop the engine (joins threads and drains channels)
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
