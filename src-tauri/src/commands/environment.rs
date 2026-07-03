use crate::core::simulation_lifecycle::ChronicleEvent;
use crate::AppState;
use tauri::State;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TerrainMapState {
    pub width: usize,
    pub height: usize,
    pub biomes: Vec<u8>,
    pub elevations: Vec<f32>,
    pub moistures: Vec<f32>,
    pub temperatures: Vec<f32>,
    pub bounds: crate::core::resources::MapBounds,
    pub pois: Vec<(usize, usize)>,
}

impl TerrainMapState {
    pub fn from_resource(
        terrain_map: &crate::core::terrain::TerrainMap,
        bounds: &crate::core::resources::MapBounds,
    ) -> Self {
        Self {
            width: terrain_map.width,
            height: terrain_map.height,
            biomes: terrain_map.biomes.clone(),
            elevations: terrain_map.elevations.clone(),
            moistures: terrain_map.moistures.clone(),
            temperatures: terrain_map.temperatures.clone(),
            bounds: *bounds,
            pois: terrain_map.pois.clone(),
        }
    }
}

/// Live snapshot of the closed ecosystem for the dashboard: the three energy compartments of
/// the conserved biomass ledger (which should sum ~constant), the population split, and the
/// biodiversity indices. Published once per tick by the simulation thread.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct EcosystemState {
    pub detritus: f64,
    pub plants: f64,
    pub animals: f64,
    pub total: f64,
    pub prey_count: u32,
    pub predator_count: u32,
    pub shannon: f32,
    pub simpson: f32,
}

#[tauri::command]
pub fn get_ecosystem_state(state: State<'_, AppState>) -> Result<EcosystemState, String> {
    let shared = state
        .engine
        .ecosystem_state
        .read()
        .unwrap_or_else(|e| e.into_inner());
    Ok(shared.clone())
}

#[tauri::command]
pub fn get_terrain_map(state: State<'_, AppState>) -> Result<TerrainMapState, String> {
    let shared = state
        .engine
        .terrain_map
        .read()
        .unwrap_or_else(|e| e.into_inner());
    shared
        .clone()
        .ok_or_else(|| "Terrain map not initialized".to_string())
}

#[tauri::command]
pub fn get_pheromone_grid(
    state: State<'_, AppState>,
) -> Result<crate::ai::pheromone::PheromoneGridState, String> {
    let shared = state
        .engine
        .pheromone_grid_state
        .read()
        .unwrap_or_else(|e| e.into_inner());
    Ok(shared.clone())
}

#[tauri::command]
pub fn get_environmental_elements(
    state: State<'_, AppState>,
) -> Result<crate::core::ecs::EnvironmentalState, String> {
    let shared = state
        .engine
        .environmental_state
        .read()
        .unwrap_or_else(|e| e.into_inner());
    Ok(shared.clone())
}

#[tauri::command]
pub fn get_active_raycasts(
    state: State<'_, AppState>,
) -> Result<Vec<crate::core::ecs::RaycastTelemetry>, String> {
    let shared = state
        .engine
        .active_raycasts
        .read()
        .unwrap_or_else(|e| e.into_inner());
    Ok(shared.clone())
}

#[tauri::command]
pub fn get_chronicle_history(state: State<'_, AppState>) -> Result<Vec<ChronicleEvent>, String> {
    let history = state
        .engine
        .chronicle_history
        .read()
        .unwrap_or_else(|e| e.into_inner());
    Ok(history.clone())
}
