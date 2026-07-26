use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn trigger_migration(state: State<'_, AppState>, target_port: u16) -> Result<(), String> {
    // ADR-0004 C3. Recorded before the send, so the record cannot lag the effect.
    state
        .engine
        .observer_actions
        .push(crate::core::observer::ObserverAction::MigrationTriggered { target_port });
    state
        .engine
        .manual_migration_trigger
        .send(target_port)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_sharding_config(
    state: State<'_, AppState>,
    config: crate::core::ecs::ShardingConfig,
) -> Result<(), String> {
    state.engine.observer_actions.push(
        crate::core::observer::ObserverAction::ShardingConfigChanged {
            local_port: config.local_port,
        },
    );
    let mut sharding_config = state
        .engine
        .sharding_config
        .write()
        .map_err(|e| e.to_string())?;
    *sharding_config = config;
    Ok(())
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
