#![allow(clippy::too_many_arguments, clippy::collapsible_match, clippy::type_complexity)]

pub mod ai;
pub mod core;
pub mod evolution;
pub mod physics;
pub mod commands;

use crate::core::engine::SimulationEngine;
use std::sync::Arc;
use tauri::Manager;

pub struct AppState {
    pub engine: Arc<SimulationEngine>,
    pub evolution_settings: Arc<std::sync::Mutex<commands::EvolutionSettings>>,
    pub evolution_running: Arc<std::sync::atomic::AtomicBool>,
    pub map_elites_grid: Arc<std::sync::Mutex<commands::MapElitesGridState>>,
}
pub fn run() {
    let initial_grid = std::collections::HashMap::new();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            engine: Arc::new(SimulationEngine::new()),
            evolution_settings: Arc::new(std::sync::Mutex::new(commands::EvolutionSettings {
                mutation_rate: 0.15,
                selection_bias: 1.5,
                grid_resolution: 50,
            })),
            evolution_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            map_elites_grid: Arc::new(std::sync::Mutex::new(commands::MapElitesGridState {
                grid: initial_grid,
                grid_resolution: 50,
            })),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_simulation_status,
            commands::toggle_simulation,
            commands::get_map_elites_grid,
            commands::update_evolution_settings,
            commands::toggle_evolution,
            commands::get_pheromone_grid,
            commands::get_environmental_elements,
            commands::get_active_raycasts,
            commands::get_lineage_graph,
            commands::get_chronicle_history,
            commands::set_sharding_config,
            commands::get_sharding_config,
            commands::trigger_migration,
            commands::get_test_rabbit_state,
            commands::save_simulation_state,
            commands::load_simulation_state
        ])
        .setup(|app| {
            use crate::core::simulation_lifecycle::SavedSimulationState;
            let app_state = app.state::<AppState>();
            if let Ok(app_data_dir) = app.path().app_data_dir() {
                let default_save_path = app_data_dir.join("default_save.json");
                if default_save_path.exists() {
                    if let Ok(json_str) = std::fs::read_to_string(&default_save_path) {
                        if let Ok(loaded_state) = serde_json::from_str::<SavedSimulationState>(&json_str) {
                            *app_state.evolution_settings.lock().unwrap() = loaded_state.evolution_settings.clone();
                            *app_state.map_elites_grid.lock().unwrap() = loaded_state.map_elites_grid.clone();
                            
                            *app_state.engine.pending_load_state.lock().unwrap() = Some(loaded_state);
                            app_state.engine.start(
                                Some(app.handle().clone()),
                                Arc::clone(&app_state.evolution_settings),
                                Arc::clone(&app_state.evolution_running),
                                Arc::clone(&app_state.map_elites_grid),
                            );
                        }
                    }
                }
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| match event {
        tauri::RunEvent::ExitRequested { .. } => {
            let state = app_handle.state::<AppState>();
            let engine = &state.engine;
            
            if engine.running.load(std::sync::atomic::Ordering::SeqCst) {
                let (tx, rx) = std::sync::mpsc::channel();
                if engine.save_request_tx.send(tx).is_ok() {
                    if let Ok(saved_state) = rx.recv_timeout(std::time::Duration::from_secs(2)) {
                        if let Ok(app_data_dir) = app_handle.path().app_data_dir() {
                            let _ = std::fs::create_dir_all(&app_data_dir);
                            let default_save_path = app_data_dir.join("default_save.json");
                            if let Ok(json_str) = serde_json::to_string_pretty(&saved_state) {
                                let _ = std::fs::write(default_save_path, json_str);
                            }
                        }
                    }
                }
                engine.stop();
            }
        }
        _ => {}
    });
}
