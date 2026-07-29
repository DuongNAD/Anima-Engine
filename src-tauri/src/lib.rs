#![allow(
    clippy::too_many_arguments,
    clippy::collapsible_match,
    clippy::type_complexity
)]

pub mod ai;
pub mod commands;
pub mod core;
pub mod evolution;
pub mod physics;

use crate::core::engine::SimulationEngine;
use std::sync::Arc;
use tauri::Manager;

pub struct AppState {
    pub engine: Arc<SimulationEngine>,
    pub evolution_settings: Arc<std::sync::Mutex<commands::EvolutionSettings>>,
    pub evolution_running: Arc<std::sync::atomic::AtomicBool>,
    pub map_elites_grid: Arc<std::sync::Mutex<commands::MapElitesGridState>>,
    /// The one route a human's write takes into the running world (ADR-0004 C3).
    ///
    /// The four handles above and beside it are still here because `SimulationEngine::start` takes
    /// them as arguments — so this is enforcement by *construction of the write*, not yet by
    /// visibility. See [`ObserverSeam`](core::observer::ObserverSeam) for what that does and does not
    /// close.
    pub seam: core::observer::ObserverSeam,
}
pub fn run() {
    let initial_grid = std::collections::HashMap::new();

    // Hoisted out of the `manage(..)` literal so the seam can be handed the same handles the rest of
    // the app reads through, rather than a second set that would drift from them.
    let engine = Arc::new(SimulationEngine::new());
    let evolution_settings = Arc::new(std::sync::Mutex::new(commands::EvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    }));
    let evolution_running = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seam = core::observer::ObserverSeam::new(
        engine.observer_actions.clone(),
        Arc::clone(&evolution_settings),
        Arc::clone(&evolution_running),
        Arc::clone(&engine.sharding_config),
        engine.manual_migration_trigger.clone(),
    );

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            engine,
            evolution_settings,
            evolution_running,
            map_elites_grid: Arc::new(std::sync::Mutex::new(commands::MapElitesGridState {
                grid: initial_grid,
                grid_resolution: 50,
            })),
            seam,
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
            commands::set_lod_focus,
            commands::get_lod_focus,
            commands::get_lod_bands,
            commands::set_sharding_config,
            commands::get_sharding_config,
            commands::get_migration_handoff_diagnostics,
            commands::trigger_migration,
            commands::get_test_rabbit_state,
            commands::save_simulation_state,
            commands::load_simulation_state,
            commands::get_terrain_map,
            commands::get_ecosystem_state,
            commands::save_world_artifact,
            commands::list_legacy_saves,
            commands::import_legacy_save,
            commands::start_tick_capture,
            commands::stop_tick_capture,
            commands::get_tick_capture_status,
            commands::export_tick_capture
        ])
        .setup(|app| {
            let app_state = app.state::<AppState>();
            if let Ok(app_data_dir) = app.path().app_data_dir() {
                // One autosave, under the same contract as every other save.
                //
                // This used to read `app_data_dir/default_save.json` — a bare `serde_json` dump,
                // outside the `saves/` directory the load command can reach, with no envelope, no
                // checksum and no schema version. Two persistence contracts existed side by side
                // and only one of them was versioned.
                //
                // The autosave now lives at `saves/autosave.json` and goes through
                // `snapshot::read`, which verifies the checksum and migrates a pre-envelope file
                // forward. The old location is read **once**, if the new one is absent, so an
                // existing user's world is adopted rather than stranded; nothing writes back to it.
                let saves_dir = app_data_dir.join("saves");
                let autosave = saves_dir.join(crate::commands::save_paths::AUTOSAVE_NAME);
                let legacy = app_data_dir.join(crate::commands::save_paths::LEGACY_AUTOSAVE_FILE);
                let source = if autosave.exists() {
                    Some(autosave)
                } else if legacy.exists() {
                    Some(legacy)
                } else {
                    None
                };

                if let Some(path) = source {
                    match crate::core::snapshot::read(&path) {
                        Ok(loaded_state) => {
                            *app_state
                                .evolution_settings
                                .lock()
                                .unwrap_or_else(|e| e.into_inner()) =
                                loaded_state.evolution_settings.clone();
                            *app_state
                                .map_elites_grid
                                .lock()
                                .unwrap_or_else(|e| e.into_inner()) =
                                loaded_state.map_elites_grid.clone();

                            *app_state
                                .engine
                                .pending_load_state
                                .lock()
                                .unwrap_or_else(|e| e.into_inner()) = Some(loaded_state);
                            app_state.engine.start(
                                Some(app.handle().clone()),
                                Arc::clone(&app_state.evolution_settings),
                                Arc::clone(&app_state.evolution_running),
                                Arc::clone(&app_state.map_elites_grid),
                            );
                        }
                        // A corrupt or unreadable autosave is not a reason to refuse to launch —
                        // the app starts on a fresh world instead. But the previous code swallowed
                        // this with `if let Ok(..)` and said nothing, so a user whose world failed
                        // to load saw an empty simulation and no explanation. The file is left in
                        // place, so it can still be inspected or recovered.
                        Err(e) => {
                            eprintln!(
                                "[anima] autosave at {} could not be read ({e}); starting a fresh \
                                 world. The file has been left untouched.",
                                path.display()
                            );
                        }
                    }
                }
            }

            // A fresh world starts running only when asked. Placed after the autosave branch and
            // guarded on `running`, so resuming a save and starting from zero cannot both fire and
            // spawn two engines over one world.
            //
            // The condition this closes: before it, the *only* way an engine started without a
            // human clicking Start was resuming an autosave — so a run "from zero" and a run
            // "unattended" were mutually exclusive.
            if crate::core::resources::autostart_from_env()
                && !app_state
                    .engine
                    .running
                    .load(std::sync::atomic::Ordering::SeqCst)
            {
                eprintln!("[anima] ANIMA_AUTOSTART is set; simulating from genesis");
                app_state.engine.start(
                    Some(app.handle().clone()),
                    Arc::clone(&app_state.evolution_settings),
                    Arc::clone(&app_state.evolution_running),
                    Arc::clone(&app_state.map_elites_grid),
                );
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            let state = app_handle.state::<AppState>();
            let engine = &state.engine;

            if engine.running.load(std::sync::atomic::Ordering::SeqCst) {
                let (tx, rx) = std::sync::mpsc::channel();
                if engine.save_request_tx.send(tx).is_ok() {
                    // `Ok(Ok(_))`: the thread answered, and it did not refuse. A refusal on exit is
                    // dropped deliberately — the autosave is best-effort and there is no one left
                    // to tell, whereas the explicit save command surfaces the reason.
                    if let Ok(Ok(saved_state)) = rx.recv_timeout(std::time::Duration::from_secs(2))
                    {
                        if let Ok(app_data_dir) = app_handle.path().app_data_dir() {
                            // Same directory, same envelope, same atomic write as an explicit
                            // save. The old form was `to_string_pretty` into `fs::write`, which
                            // truncates the destination before writing a byte — so a crash or a
                            // full disk during exit destroyed the autosave the user already had in
                            // order to fail at producing a new one. It also wrote a bare state with
                            // no schema version, which the load command could not read back.
                            let saves = app_data_dir.join("saves");
                            if let Err(error) = std::fs::create_dir_all(&saves) {
                                eprintln!(
                                    "exit autosave failed: could not create {}: {error}",
                                    saves.display()
                                );
                            } else {
                                let target = saves.join(crate::commands::save_paths::AUTOSAVE_NAME);
                                match crate::core::snapshot::SnapshotEnvelope::seal(saved_state) {
                                    Ok(envelope) => {
                                        if let Err(error) =
                                            crate::core::snapshot::write_atomic(&target, &envelope)
                                        {
                                            eprintln!(
                                                "exit autosave failed while writing {}: {error}",
                                                target.display()
                                            );
                                        }
                                    }
                                    Err(error) => eprintln!(
                                        "exit autosave refused invalid checkpoint state: {error}"
                                    ),
                                }
                            }
                        }
                    }
                }
                engine.stop();
            }
        }
    });
}
