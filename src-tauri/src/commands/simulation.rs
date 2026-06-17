use tauri::State;
use std::sync::Arc;
use crate::AppState;
use crate::core::simulation_lifecycle::{SimulationStatus, SavedSimulationState};

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
pub fn save_simulation_state(
    state: State<'_, AppState>,
    file_path: String,
) -> Result<bool, String> {
    let engine = &state.engine;
    if !engine.running.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("Simulation is not running".to_string());
    }

    let (tx, rx) = std::sync::mpsc::channel::<SavedSimulationState>();
    engine.save_request_tx.send(tx)
        .map_err(|e| format!("Failed to send save request: {}", e))?;

    let saved_state = rx.recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| "Timeout waiting for simulation thread to serialize".to_string())?;

    let json_str = serde_json::to_string_pretty(&saved_state)
        .map_err(|e| format!("Serialization error: {}", e))?;
    std::fs::write(&file_path, json_str)
        .map_err(|e| format!("File writing error: {}", e))?;

    Ok(true)
}

#[tauri::command]
pub fn load_simulation_state(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    file_path: String,
) -> Result<bool, String> {
    let json_str = std::fs::read_to_string(&file_path)
        .map_err(|e| format!("File read error: {}", e))?;
    let loaded_state = serde_json::from_str::<SavedSimulationState>(&json_str)
        .map_err(|e| format!("Parsing error: {}", e))?;

    let engine = &state.engine;
    let was_running = engine.running.load(std::sync::atomic::Ordering::SeqCst);
    if was_running {
        engine.stop();
    }

    *state.evolution_settings.lock().unwrap() = loaded_state.evolution_settings.clone();
    *state.map_elites_grid.lock().unwrap() = loaded_state.map_elites_grid.clone();

    *engine.pending_load_state.lock().unwrap() = Some(loaded_state);
    
    engine.start(
        Some(app_handle),
        Arc::clone(&state.evolution_settings),
        Arc::clone(&state.evolution_running),
        Arc::clone(&state.map_elites_grid),
    );

    Ok(true)
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
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
