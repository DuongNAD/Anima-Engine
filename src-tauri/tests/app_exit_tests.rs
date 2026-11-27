//! Regression test for `APP-EXIT`: closing the final desktop window must stop the
//! simulation engine on `CloseRequested` / `Destroyed`, before Tauri tears down the
//! window and `ICoreWebView2Controller` while `EMIT` continues flooding the UI thread.

use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};
use anima_engine_lib::core::engine::SimulationEngine;
use anima_engine_lib::core::observer::ObserverSeam;
use anima_engine_lib::core::resources::LaunchPersistencePolicy;
use anima_engine_lib::core::thread_supervisor as sup;
use anima_engine_lib::{handle_run_event, AppState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Manager;

const EXPECTED_THREADS: [&str; 5] = [sup::EMIT, sup::EVO, sup::LEARN, sup::NET, sup::SIM];

#[derive(Default)]
struct CloseObservation {
    close_requested_seen: bool,
    running_after_close_requested: bool,
    status_running_after_close_requested: bool,
    live_after_close_requested: Vec<&'static str>,
    elapsed: Duration,
}

#[test]
fn closing_the_last_window_stops_the_simulation_engine_on_close_requested() {
    let engine = Arc::new(SimulationEngine::new());
    let evolution_settings = Arc::new(Mutex::new(EvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    }));
    let evolution_running = Arc::new(AtomicBool::new(false));
    let map_elites_grid = Arc::new(Mutex::new(MapElitesGridState {
        grid: HashMap::new(),
        grid_resolution: 50,
    }));
    let seam = ObserverSeam::new(
        engine.observer_actions.clone(),
        Arc::clone(&evolution_settings),
        Arc::clone(&evolution_running),
        Arc::clone(&engine.sharding_config),
        engine.manual_migration_trigger.clone(),
    );

    let app = tauri::test::mock_builder()
        .manage(AppState {
            engine: Arc::clone(&engine),
            evolution_settings: Arc::clone(&evolution_settings),
            evolution_running: Arc::clone(&evolution_running),
            map_elites_grid: Arc::clone(&map_elites_grid),
            seam,
        })
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build mock tauri app");

    let main_window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build main mock window");
    assert_eq!(app.webview_windows().len(), 1);

    engine.start(
        Some(app.handle().clone()),
        Arc::clone(&evolution_settings),
        Arc::clone(&evolution_running),
        Arc::clone(&map_elites_grid),
    );
    assert_eq!(engine.supervisor.live(), EXPECTED_THREADS.to_vec());

    let (ready_tx, ready_rx) = mpsc::channel::<()>();
    let close_initiated_at = Arc::new(Mutex::new(None::<Instant>));
    let close_initiated_for_thread = Arc::clone(&close_initiated_at);
    let closer = std::thread::spawn(move || {
        ready_rx.recv().expect("mock runtime sends RunEvent::Ready");
        *close_initiated_for_thread
            .lock()
            .expect("close timestamp lock") = Some(Instant::now());
        main_window.close().expect("request main window close");
    });

    let observation = Arc::new(Mutex::new(CloseObservation::default()));
    let obs_for_loop = Arc::clone(&observation);
    let close_initiated_for_loop = Arc::clone(&close_initiated_at);
    let ready_tx = Mutex::new(Some(ready_tx));

    let exit_code = app.run_return(move |app_handle, event| {
        if matches!(event, tauri::RunEvent::Ready) {
            if let Some(tx) = ready_tx.lock().expect("ready_tx lock").take() {
                let _ = tx.send(());
            }
        }

        handle_run_event(
            app_handle,
            &event,
            LaunchPersistencePolicy::for_fresh_start(true),
        );

        if matches!(
            event,
            tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::CloseRequested { .. },
                ..
            }
        ) {
            let state = app_handle.state::<AppState>();
            let started = close_initiated_for_loop
                .lock()
                .expect("close timestamp lock")
                .unwrap_or_else(Instant::now);
            let mut obs = obs_for_loop.lock().expect("observation lock");
            obs.close_requested_seen = true;
            obs.running_after_close_requested = state.engine.running.load(Ordering::SeqCst);
            obs.status_running_after_close_requested = state.engine.get_status().running;
            obs.live_after_close_requested = state.engine.supervisor.live();
            obs.elapsed = started.elapsed();
        }
    });

    closer.join().expect("closer thread join");
    // Safety cleanup so a pre-fix failure leaves zero supervised worker threads alive.
    engine.stop();

    assert_eq!(exit_code, 0, "mock runtime returned non-zero exit code");
    let obs = observation.lock().expect("observation lock");
    assert!(
        obs.close_requested_seen,
        "RunEvent::WindowEvent(CloseRequested) was never delivered by MockRuntime"
    );
    assert!(
        !obs.running_after_close_requested,
        "APP-EXIT: engine.running remained true after final window CloseRequested (live threads: {:?})",
        obs.live_after_close_requested
    );
    assert!(
        obs.live_after_close_requested.is_empty(),
        "APP-EXIT: supervised worker threads remained alive after final window CloseRequested: {:?}",
        obs.live_after_close_requested
    );
    assert!(
        !obs.status_running_after_close_requested,
        "APP-EXIT: engine status still reported running=true after CloseRequested"
    );
    assert!(
        obs.elapsed < Duration::from_secs(5),
        "APP-EXIT: shutdown on CloseRequested took {:?} (expected < 5s)",
        obs.elapsed
    );
}
