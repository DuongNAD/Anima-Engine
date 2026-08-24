use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn trigger_migration(state: State<'_, AppState>, target_port: u16) -> Result<(), String> {
    // ADR-0004 C3. Record and send are one call, and the record survives a failed send — "they asked
    // and it failed" is a different fact from "they never asked".
    state.seam.trigger_migration(target_port)
}

#[tauri::command]
pub fn set_sharding_config(
    state: State<'_, AppState>,
    config: crate::core::ecs::ShardingConfig,
) -> Result<(), String> {
    state.seam.set_sharding_config(config)
}

#[tauri::command]
pub fn get_sharding_config(
    state: State<'_, AppState>,
) -> Result<crate::core::ecs::ShardingConfig, String> {
    let sharding_config = state
        .engine
        .sharding_config
        .read()
        .map_err(|e| e.to_string())?;
    Ok(sharding_config.clone())
}

#[tauri::command]
pub fn get_migration_handoff_diagnostics(
    state: State<'_, AppState>,
) -> Result<crate::core::resources::MigrationHandoffSnapshot, String> {
    Ok(state.engine.migration_handoff_diagnostics.snapshot())
}
