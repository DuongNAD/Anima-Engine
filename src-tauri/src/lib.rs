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

    // --- Rabbit Test Experiment Command ---
    #[derive(Clone, Copy, Debug)]
    #[repr(C)]
    pub struct AdvancedRabbitPart {
        pub x: f32,
        pub y: f32,
        pub z: f32,
        pub rx: f32,
        pub ry: f32,
        pub rz: f32,
        pub sx: f32,
        pub sy: f32,
        pub sz: f32,
        pub r: f32,
        pub g: f32,
        pub b: f32,
        pub part_type: f32,
    }

    pub fn generate_dynamic_rabbit(
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
        _breathing_offset: f32,
        is_eating: bool,
    ) -> Vec<AdvancedRabbitPart> {
        let elapsed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64() as f32;
        let speed_multiplier = 1.2;
        let t = elapsed * speed_multiplier;

        let breathing = (t * 4.0).sin() * 0.04;
        let hop_height = (t * 2.0).sin().max(0.0) * 0.6;
        let hop_rotation = (t * 2.0).sin() * 0.08;

        let cur_x = x + (t * 0.5).sin() * 2.0;
        let cur_y = y + hop_height - 0.5;
        let cur_rot = rotation + hop_rotation;

        let mut parts = Vec::with_capacity(12);
        let cos_r = cur_rot.cos();
        let sin_r = cur_rot.sin();

        let local_to_world = |lx: f32, ly: f32, lz: f32| -> (f32, f32, f32) {
            (
                cur_x + lx * cos_r - ly * sin_r,
                cur_y + lx * sin_r + ly * cos_r,
                z + lz,
            )
        };

        // 0. Body (part_type: 0.0)
        let body_scale = 2.0 + breathing;
        parts.push(AdvancedRabbitPart {
            x: cur_x,
            y: cur_y,
            z,
            rx: 0.0,
            ry: 0.0,
            rz: cur_rot,
            sx: body_scale * 1.6,
            sy: body_scale * 1.0,
            sz: body_scale * 1.0,
            r: 0.9,
            g: 0.9,
            b: 0.9,
            part_type: 0.0,
        });

        // 1. Head (part_type: 1.0)
        let (head_x, head_y, head_z) = local_to_world(1.8, 0.0, 0.0);
        let head_scale = 1.2 + breathing * 0.5;
        parts.push(AdvancedRabbitPart {
            x: head_x,
            y: head_y,
            z: head_z,
            rx: 0.0,
            ry: 0.0,
            rz: cur_rot,
            sx: head_scale * 1.1,
            sy: head_scale * 0.9,
            sz: head_scale * 0.95,
            r: 0.95,
            g: 0.95,
            b: 0.95,
            part_type: 1.0,
        });

        // 2. Left Ear (part_type: 2.0)
        let ear_breathing = (t * 6.0).sin() * 0.12;
        let (ear_l_x, ear_l_y, ear_l_z) = local_to_world(2.0, 0.8, 0.5);
        parts.push(AdvancedRabbitPart {
            x: ear_l_x,
            y: ear_l_y,
            z: ear_l_z,
            rx: 0.0,
            ry: 0.0,
            rz: cur_rot + 0.3 + ear_breathing,
            sx: 0.8 * 2.8,
            sy: 0.8 * 0.35,
            sz: 0.8 * 0.2,
            r: 0.85,
            g: 0.75,
            b: 0.75,
            part_type: 2.0,
        });

        // 3. Right Ear (part_type: 3.0)
        let (ear_r_x, ear_r_y, ear_r_z) = local_to_world(2.0, -0.8, -0.5);
        parts.push(AdvancedRabbitPart {
            x: ear_r_x,
            y: ear_r_y,
            z: ear_r_z,
            rx: 0.0,
            ry: 0.0,
            rz: cur_rot - 0.3 - ear_breathing,
            sx: 0.8 * 2.8,
            sy: 0.8 * 0.35,
            sz: 0.8 * 0.2,
            r: 0.85,
            g: 0.75,
            b: 0.75,
            part_type: 3.0,
        });

        // 4. Front-Left Leg (part_type: 4.0)
        let (fl_leg_x, fl_leg_y, fl_leg_z) = local_to_world(0.8 + (t * 4.0 + std::f32::consts::PI).sin() * 0.15, -0.8 - hop_height * 0.35, 0.5);
        parts.push(AdvancedRabbitPart {
            x: fl_leg_x,
            y: fl_leg_y,
            z: fl_leg_z,
            rx: 0.0,
            ry: 0.0,
            rz: cur_rot + (t * 4.0 + std::f32::consts::PI).sin() * 0.25 - hop_height * 0.3,
            sx: 0.8 * 1.0,
            sy: 0.8 * 1.3,
            sz: 0.8 * 1.0,
            r: 0.82,
            g: 0.82,
            b: 0.82,
            part_type: 4.0,
        });

        // 5. Front-Right Leg (part_type: 5.0)
        let (fr_leg_x, fr_leg_y, fr_leg_z) = local_to_world(0.8 + (t * 4.0).sin() * 0.15, -0.8 - hop_height * 0.35, -0.5);
        parts.push(AdvancedRabbitPart {
            x: fr_leg_x,
            y: fr_leg_y,
            z: fr_leg_z,
            rx: 0.0,
            ry: 0.0,
            rz: cur_rot + (t * 4.0).sin() * 0.25 - hop_height * 0.3,
            sx: 0.8 * 1.0,
            sy: 0.8 * 1.3,
            sz: 0.8 * 1.0,
            r: 0.82,
            g: 0.82,
            b: 0.82,
            part_type: 5.0,
        });

        // 6. Hind-Left Leg (part_type: 6.0)
        let (hl_leg_x, hl_leg_y, hl_leg_z) = local_to_world(-1.2 - hop_height * 0.1 + (t * 4.0).sin() * 0.1, -0.6 - hop_height * 0.4, 0.6);
        parts.push(AdvancedRabbitPart {
            x: hl_leg_x,
            y: hl_leg_y,
            z: hl_leg_z,
            rx: 0.0,
            ry: 0.0,
            rz: cur_rot + (t * 4.0).sin() * 0.15 - hop_height * 0.3,
            sx: 1.4 * 1.0,
            sy: 1.4 * 1.3,
            sz: 1.4 * 1.0,
            r: 0.8,
            g: 0.8,
            b: 0.8,
            part_type: 6.0,
        });

        // 7. Hind-Right Leg (part_type: 7.0)
        let (hr_leg_x, hr_leg_y, hr_leg_z) = local_to_world(-1.2 - hop_height * 0.1 + (t * 4.0 + std::f32::consts::PI).sin() * 0.1, -0.6 - hop_height * 0.4, -0.6);
        parts.push(AdvancedRabbitPart {
            x: hr_leg_x,
            y: hr_leg_y,
            z: hr_leg_z,
            rx: 0.0,
            ry: 0.0,
            rz: cur_rot + (t * 4.0 + std::f32::consts::PI).sin() * 0.15 - hop_height * 0.3,
            sx: 1.4 * 1.0,
            sy: 1.4 * 1.3,
            sz: 1.4 * 1.0,
            r: 0.8,
            g: 0.8,
            b: 0.8,
            part_type: 7.0,
        });

        // 8. Tail (part_type: 8.0)
        let (tail_x, tail_y, tail_z) = local_to_world(-2.0, 0.0, 0.0);
        let tail_wiggle = breathing * 1.5;
        parts.push(AdvancedRabbitPart {
            x: tail_x,
            y: tail_y,
            z: tail_z,
            rx: 0.0,
            ry: 0.0,
            rz: cur_rot + tail_wiggle,
            sx: 0.5,
            sy: 0.5,
            sz: 0.5,
            r: 1.0,
            g: 1.0,
            b: 1.0,
            part_type: 8.0,
        });

        // 9. Mouth (part_type: 9.0)
        let chewing_offset = if is_eating { (t * 15.0).sin() * 0.08 } else { 0.0 };
        let (mouth_x, mouth_y, mouth_z) = local_to_world(2.3, -0.4 + chewing_offset, 0.0);
        parts.push(AdvancedRabbitPart {
            x: mouth_x,
            y: mouth_y,
            z: mouth_z,
            rx: 0.0,
            ry: 0.0,
            rz: cur_rot,
            sx: 0.3,
            sy: 0.2,
            sz: 0.3,
            r: 0.9,
            g: 0.7,
            b: 0.7,
            part_type: 9.0,
        });

        // 10. Left Eye (part_type: 7.0)
        parts.push(AdvancedRabbitPart {
            x: 0.35,
            y: 0.15,
            z: 0.35,
            rx: 0.0,
            ry: 0.0,
            rz: 0.0,
            sx: 1.0,
            sy: 1.0,
            sz: 1.0,
            r: 0.118,
            g: 0.161,
            b: 0.231,
            part_type: 7.0,
        });

        // 11. Right Eye (part_type: 7.0)
        parts.push(AdvancedRabbitPart {
            x: 0.35,
            y: 0.15,
            z: -0.35,
            rx: 0.0,
            ry: 0.0,
            rz: 0.0,
            sx: 1.0,
            sy: 1.0,
            sz: 1.0,
            r: 0.118,
            g: 0.161,
            b: 0.231,
            part_type: 7.0,
        });

        parts
    }

    #[tauri::command]
    pub fn get_test_rabbit_state() -> tauri::ipc::Response {
        let rabbit_parts = generate_dynamic_rabbit(0.0, 0.0, 0.0, 0.785, 0.0, true);
        let mut buffer = Vec::with_capacity(rabbit_parts.len() * 52);
        for part in rabbit_parts {
            buffer.extend_from_slice(&part.x.to_le_bytes());
            buffer.extend_from_slice(&part.y.to_le_bytes());
            buffer.extend_from_slice(&part.z.to_le_bytes());
            buffer.extend_from_slice(&part.rx.to_le_bytes());
            buffer.extend_from_slice(&part.ry.to_le_bytes());
            buffer.extend_from_slice(&part.rz.to_le_bytes());
            buffer.extend_from_slice(&part.sx.to_le_bytes());
            buffer.extend_from_slice(&part.sy.to_le_bytes());
            buffer.extend_from_slice(&part.sz.to_le_bytes());
            buffer.extend_from_slice(&part.r.to_le_bytes());
            buffer.extend_from_slice(&part.g.to_le_bytes());
            buffer.extend_from_slice(&part.b.to_le_bytes());
            buffer.extend_from_slice(&part.part_type.to_le_bytes());
        }
        tauri::ipc::Response::new(buffer)
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
            commands::trigger_migration,
            commands::get_test_rabbit_state
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
