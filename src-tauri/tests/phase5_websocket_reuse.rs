use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};
use anima_engine_lib::core::engine::SimulationEngine;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const READY_DEADLINE: Duration = Duration::from_secs(10);

fn await_listener(port: u16) {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let start = Instant::now();

    loop {
        match TcpStream::connect_timeout(&address, Duration::from_millis(100)) {
            Ok(stream) => {
                drop(stream);
                return;
            }
            Err(error) => {
                let waited = start.elapsed();
                assert!(
                    waited < READY_DEADLINE,
                    "websocket listener on {address} was not reachable after {waited:.2?}; \
                     last error: {error}"
                );
            }
        }

        thread::sleep(Duration::from_millis(5));
    }
}

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

    // Ask the OS for a free port once, then require the engine to bind and release that same port
    // on every cycle. This keeps the reuse property without colliding with unrelated local tools.
    let reservation = TcpListener::bind("127.0.0.1:0").expect("reserve a test port");
    let test_port = reservation.local_addr().expect("reserved address").port();
    drop(reservation);

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
        assert!(
            engine.running.load(std::sync::atomic::Ordering::SeqCst),
            "Engine should be running on iteration {}",
            i
        );

        await_listener(test_port);

        // Stop the engine (joins threads, closes the websocket server, drains channels)
        engine.stop();

        // Assert it is stopped
        assert!(
            !engine.running.load(std::sync::atomic::Ordering::SeqCst),
            "Engine should be stopped on iteration {}",
            i
        );

        // Verify that the threads Option is cleared and joined
        {
            let threads_lock = engine.threads.lock().unwrap();
            assert!(
                threads_lock.is_none(),
                "Threads should be joined and None on iteration {}",
                i
            );
        }

        // The next iteration can only test address reuse if this iteration actually released it.
        let rebound = TcpListener::bind(("127.0.0.1", test_port)).unwrap_or_else(|error| {
            panic!("port {test_port} not reusable on iteration {i}: {error}")
        });
        drop(rebound);
    }
}
