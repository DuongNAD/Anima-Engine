use tauri::State;
use crate::AppState;
use crate::core::simulation_lifecycle::ChronicleEvent;

#[tauri::command]
pub fn get_pheromone_grid(state: State<'_, AppState>) -> Result<crate::ai::pheromone::PheromoneGridState, String> {
    let shared = state.engine.pheromone_grid_state.read().unwrap_or_else(|e| e.into_inner());
    Ok(shared.clone())
}

#[tauri::command]
pub fn get_environmental_elements(state: State<'_, AppState>) -> Result<crate::core::ecs::EnvironmentalState, String> {
    let shared = state.engine.environmental_state.read().unwrap_or_else(|e| e.into_inner());
    Ok(shared.clone())
}

#[tauri::command]
pub fn get_active_raycasts(state: State<'_, AppState>) -> Result<Vec<crate::core::ecs::RaycastTelemetry>, String> {
    let shared = state.engine.active_raycasts.read().unwrap_or_else(|e| e.into_inner());
    Ok(shared.clone())
}

#[tauri::command]
pub fn get_chronicle_history(state: State<'_, AppState>) -> Result<Vec<ChronicleEvent>, String> {
    let history = state.engine.chronicle_history.read().unwrap_or_else(|e| e.into_inner());
    Ok(history.clone())
}
