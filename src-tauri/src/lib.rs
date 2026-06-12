#![allow(clippy::too_many_arguments, clippy::collapsible_match, clippy::type_complexity)]

pub mod ai;
pub mod core;
pub mod evolution;
pub mod physics;

use crate::core::engine::SimulationEngine;
use std::sync::Arc;

pub struct AppState {
    pub engine: Arc<SimulationEngine>,
    pub evolution_settings: Arc<std::sync::Mutex<commands::EvolutionSettings>>,
    pub evolution_running: Arc<std::sync::atomic::AtomicBool>,
    pub map_elites_grid: Arc<std::sync::Mutex<commands::MapElitesGridState>>,
}

pub mod commands {
    use super::AppState;
    use crate::core::engine::SimulationStatus;
    use tauri::State;
    use std::sync::Arc;
    use crate::evolution::lineage::LineageTracker;

    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
    pub struct EvolutionSettings {
        pub mutation_rate: f64,
        pub selection_bias: f64,
        pub grid_resolution: u32,
    }

    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
    pub struct EliteIndividualState {
        pub fitness: f64,
        pub features: Vec<f64>,
    }

    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
    pub struct MapElitesGridState {
        pub grid: std::collections::HashMap<String, EliteIndividualState>,
        pub grid_resolution: u32,
    }

    #[tauri::command]
    pub fn get_simulation_status(state: State<'_, AppState>) -> Result<SimulationStatus, String> {
        Ok(state.engine.get_status())
    }

    #[tauri::command]
    pub fn toggle_simulation(
        state: State<'_, AppState>,
        app_handle: tauri::AppHandle,
    ) -> Result<bool, String> {
        let engine = &state.engine;
        if engine.running.load(std::sync::atomic::Ordering::SeqCst) {
            engine.stop();
            Ok(false)
        } else {
            engine.start(
                Some(app_handle),
                Arc::clone(&state.evolution_settings),
                Arc::clone(&state.evolution_running),
                Arc::clone(&state.map_elites_grid),
            );
            Ok(true)
        }
    }

    #[tauri::command]
    pub fn get_map_elites_grid(
        state: State<'_, AppState>,
    ) -> Result<MapElitesGridState, String> {
        let grid = state.map_elites_grid.lock().unwrap();
        Ok(grid.clone())
    }

    #[tauri::command]
    pub fn update_evolution_settings(
        state: State<'_, AppState>,
        settings: EvolutionSettings,
    ) -> Result<bool, String> {
        if settings.mutation_rate < 0.0
            || settings.mutation_rate > 1.0
            || settings.selection_bias <= 0.0
        {
            return Err("Invalid settings".to_string());
        }
        let mut evolution_settings = state.evolution_settings.lock().unwrap();
        *evolution_settings = settings;
        Ok(true)
    }

    #[tauri::command]
    pub fn toggle_evolution(
        state: State<'_, AppState>,
        _app_handle: tauri::AppHandle,
    ) -> Result<bool, String> {
        let running = &state.evolution_running;
        let was_running = running.load(std::sync::atomic::Ordering::SeqCst);
        let new_running = !was_running;
        running.store(new_running, std::sync::atomic::Ordering::SeqCst);
        Ok(new_running)
    }

    #[tauri::command]
    pub fn get_pheromone_grid(state: State<'_, AppState>) -> Result<crate::ai::pheromone::PheromoneGridState, String> {
        let shared = state.engine.pheromone_grid_state.read().unwrap_or_else(|e| e.into_inner());
        Ok(shared.clone())
    }

    #[tauri::command]
    pub fn get_active_raycasts(state: State<'_, AppState>) -> Result<Vec<crate::core::ecs::RaycastTelemetry>, String> {
        let shared = state.engine.active_raycasts.read().unwrap_or_else(|e| e.into_inner());
        Ok(shared.clone())
    }

    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
    pub struct LineageNodePayload {
        pub id: String,
        pub generation: u32,
        pub parent_id: Option<String>,
        pub fitness: f64,
        pub mutations_count: u32,
    }

    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
    pub struct LineageLinkPayload {
        pub source: String,
        pub target: String,
    }

    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
    pub struct LineageGraphPayload {
        pub nodes: Vec<LineageNodePayload>,
        pub links: Vec<LineageLinkPayload>,
        pub db_connected: bool,
    }

    #[tauri::command]
    pub fn get_lineage_graph(
        state: State<'_, AppState>,
    ) -> Result<LineageGraphPayload, String> {
        let (nodes, relations) = state.engine.lineage_tracker.get_lineage_graph()?;
        let db_connected = state.engine.lineage_tracker.is_online();

        let mut payload_nodes = Vec::with_capacity(nodes.len());
        let mut payload_links = Vec::with_capacity(relations.len());

        for rel in &relations {
            payload_links.push(LineageLinkPayload {
                source: rel.source_id.clone(),
                target: rel.target_id.clone(),
            });
        }

        let mut parent_map = std::collections::HashMap::new();
        for rel in &relations {
            parent_map.entry(rel.target_id.clone())
                .or_insert_with(Vec::new)
                .push((rel.source_id.clone(), rel.relation_type));
        }

        let mut mutations_map = std::collections::HashMap::new();

        fn get_mutations_count(
            node_id: &str,
            parent_map: &std::collections::HashMap<String, Vec<(String, crate::evolution::lineage::RelationType)>>,
            memo: &mut std::collections::HashMap<String, u32>,
        ) -> u32 {
            if let Some(&val) = memo.get(node_id) {
                return val;
            }
            let mut count = 0;
            if let Some(parents) = parent_map.get(node_id) {
                let mut max_parent_mutations = 0;
                let mut is_mutation = false;
                for (parent_id, rel_type) in parents {
                    let parent_mut = get_mutations_count(parent_id, parent_map, memo);
                    if parent_mut > max_parent_mutations {
                        max_parent_mutations = parent_mut;
                    }
                    if *rel_type == crate::evolution::lineage::RelationType::Mutate {
                        is_mutation = true;
                    }
                }
                count = max_parent_mutations + if is_mutation { 1 } else { 0 };
            }
            memo.insert(node_id.to_string(), count);
            count
        }

        for node in &nodes {
            let parent_id = parent_map.get(&node.id)
                .and_then(|parents| parents.first())
                .map(|(p_id, _)| p_id.clone());

            let mutations_count = get_mutations_count(&node.id, &parent_map, &mut mutations_map);
            let fitness = node.genotype.as_ref().map(|g| g.nodes.len() as f64).unwrap_or(0.0);

            payload_nodes.push(LineageNodePayload {
                id: node.id.clone(),
                generation: node.generation,
                parent_id,
                fitness,
                mutations_count,
            });
        }

        Ok(LineageGraphPayload {
            nodes: payload_nodes,
            links: payload_links,
            db_connected,
        })
    }

    #[tauri::command]
    pub fn get_chronicle_history(state: State<'_, AppState>) -> Result<Vec<crate::core::engine::ChronicleEvent>, String> {
        let history = state.engine.chronicle_history.read().unwrap_or_else(|e| e.into_inner());
        Ok(history.clone())
    }

    #[tauri::command]
    pub fn trigger_migration(
        state: State<'_, AppState>,
        target_port: u16,
    ) -> Result<(), String> {
        state.engine.manual_migration_trigger.send(target_port).map_err(|e| e.to_string())
    }

    #[tauri::command]
    pub fn set_sharding_config(
        state: State<'_, AppState>,
        config: crate::core::ecs::ShardingConfig,
    ) -> Result<(), String> {
        let mut sharding_config = state.engine.sharding_config.write().map_err(|e| e.to_string())?;
        *sharding_config = config;
        Ok(())
    }

    #[tauri::command]
    pub fn get_sharding_config(
        state: State<'_, AppState>,
    ) -> Result<crate::core::ecs::ShardingConfig, String> {
        let sharding_config = state.engine.sharding_config.read().map_err(|e| e.to_string())?;
        Ok(sharding_config.clone())
    }
}

pub fn run() {
    let initial_grid = std::collections::HashMap::new();

    tauri::Builder::default()
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
            commands::get_active_raycasts,
            commands::get_lineage_graph,
            commands::get_chronicle_history,
            commands::set_sharding_config,
            commands::get_sharding_config,
            commands::trigger_migration
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
