use tauri::State;
use crate::AppState;

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
