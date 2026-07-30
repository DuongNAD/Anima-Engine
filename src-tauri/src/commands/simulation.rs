use crate::core::simulation_lifecycle::{SavedSimulationState, SimulationStatus};
use crate::AppState;
use std::sync::Arc;
use tauri::State;

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

/// Directory this app is allowed to keep saves in, created on demand.
///
/// Every save path is built from here plus a validated name — see [`crate::commands::save_paths`]
/// for why a frontend-supplied string is never treated as a path.
fn saves_dir(app_handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    let dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("cannot locate the app data directory: {e}"))?
        .join("saves");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("cannot create the save directory {}: {e}", dir.display()))?;
    Ok(dir)
}

#[tauri::command]
pub fn save_simulation_state(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    file_path: String,
) -> Result<bool, String> {
    // `file_path` keeps its name for IPC compatibility but is now a save *name*, not a path. It
    // used to go straight to `write_atomic`, so anything that could reach `invoke` could write a
    // file anywhere this process has permission.
    let target =
        crate::commands::save_paths::resolve_save_path(&saves_dir(&app_handle)?, &file_path)?;

    let engine = &state.engine;
    if !engine.running.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("Simulation is not running".to_string());
    }

    let (tx, rx) = std::sync::mpsc::channel::<Result<SavedSimulationState, String>>();
    crate::core::simulation_loop::enqueue_save_request(&engine.save_request_tx, tx)
        .map_err(|e| format!("Failed to enqueue save request: {e}"))?;

    // Two different failures, kept apart: the sim thread never answered, versus it answered that
    // this world cannot be saved. The second is a refusal with a reason the user can act on, so it
    // is passed through verbatim rather than flattened into a generic save error.
    // The simulation may be finishing a bounded Meta-AI/evolution batch before it can freeze the
    // worker at a consistent checkpoint boundary. The worker's own deadline is 30 seconds; this
    // outer wait must be longer or the UI could time out while the engine is still completing a
    // valid request.
    let saved_state = rx
        .recv_timeout(std::time::Duration::from_secs(35))
        .map_err(|_| "Timeout waiting for simulation thread to serialize".to_string())??;

    // G1.2: wrap in a versioned, checksummed envelope and write it atomically. The old path was
    // `to_string_pretty` into `fs::write`, which truncates the destination before writing a byte —
    // a crash or a full disk destroyed the save you already had in order to fail at writing a new
    // one.
    let envelope = crate::core::snapshot::SnapshotEnvelope::seal(saved_state)
        .map_err(|e| format!("Serialization error: {e}"))?;
    crate::core::snapshot::write_atomic(&target, &envelope)
        .map_err(|e| format!("File writing error: {e}"))?;

    Ok(true)
}

#[tauri::command]
pub fn load_simulation_state(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    file_path: String,
) -> Result<bool, String> {
    // Same contract as save: a name resolved under app-data, never a path from the webview. Reading
    // an arbitrary path was the more dangerous half of the old behaviour — it pulled attacker-chosen
    // bytes into the running world, and surfaced parse failures containing file contents back to the
    // caller.
    let source =
        crate::commands::save_paths::resolve_save_path(&saves_dir(&app_handle)?, &file_path)?;

    // Verifies the checksum and migrates a pre-envelope save (schema 1 or 2) forward. A corrupt or
    // truncated file is refused here with a message naming the problem, instead of deserializing
    // into a plausible-looking world.
    let loaded_state = crate::core::snapshot::read(&source).map_err(|e| e.to_string())?;

    let engine = &state.engine;
    let was_running = engine.running.load(std::sync::atomic::Ordering::SeqCst);
    if was_running {
        engine.stop();
    }

    *state
        .evolution_settings
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = loaded_state.evolution_settings.clone();
    *state
        .map_elites_grid
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = loaded_state.map_elites_grid.clone();

    *engine
        .pending_load_state
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(loaded_state);

    engine.start(
        Some(app_handle),
        Arc::clone(&state.evolution_settings),
        Arc::clone(&state.evolution_running),
        Arc::clone(&state.map_elites_grid),
    );

    Ok(true)
}

/// The directory a user drops a pre-confinement save into. See `save_paths::LEGACY_IMPORT_DIR`.
fn legacy_import_dir(app_handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    let dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("cannot locate the app data directory: {e}"))?
        .join(crate::commands::save_paths::LEGACY_IMPORT_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| {
        format!(
            "cannot create the legacy import directory {}: {e}",
            dir.display()
        )
    })?;
    Ok(dir)
}

/// What the user can import, and where to put it.
#[derive(serde::Serialize, ts_rs::TS)]
// Same target as every other generated type. `src/core/*.rs` and this file are at the same depth,
// so the string is identical rather than re-derived — an `export_to` that resolves somewhere else
// produces a binding nothing imports and a drift gate that never sees it.
#[ts(export, export_to = "../../src/types/generated/")]
pub struct LegacyImportListing {
    /// Absolute path of the drop directory, so the UI can tell the user where to copy the file.
    pub directory: String,
    /// Names present there now, each valid as an argument to `import_legacy_save`.
    pub names: Vec<String>,
    /// Files that are present but cannot be imported, so the UI can say so instead of hiding them.
    ///
    /// A user who drops `My Save (old).sav` in and sees an empty list has been told nothing. The
    /// listing is what the UI renders, so the reason a file is missing belongs in it.
    pub ignored: Vec<String>,
}

/// List the importable saves in `dir`, and the files that are there but are not importable.
///
/// # Why a name is only listed when it is already canonical
///
/// `sanitize_save_name` *normalises*: it appends `.json` when the name lacks it, so `old.txt`
/// sanitises to `old.txt.json`. Listing the raw directory entry for anything that merely *passes*
/// sanitisation therefore produced names the importer could not resolve — the listing said
/// `old.txt`, `import_legacy_save("old.txt")` looked for `legacy-import/old.txt.json`, and that file
/// does not exist. The listing and the resolver disagreed about what a file was called.
///
/// The fix is the fixed point: a name is listed only when sanitising it returns the name itself. That
/// is exactly the set of names for which "what the listing calls it" and "what the importer opens"
/// are the same string, so the round trip cannot fail. In practice it means `*.json` — which is what
/// the save command has always written — and everything else is reported as ignored rather than
/// silently dropped.
///
/// Split from the command so it can be tested against a real directory without a Tauri app.
pub fn list_legacy_saves_in(dir: &std::path::Path) -> Result<LegacyImportListing, String> {
    let mut names = Vec::new();
    let mut ignored = Vec::new();
    for entry in
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?
    {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        match crate::commands::save_paths::sanitize_save_name(&name) {
            Ok(canonical) if canonical == name => names.push(name),
            _ => ignored.push(name),
        }
    }
    names.sort();
    ignored.sort();
    Ok(LegacyImportListing {
        directory: dir.to_string_lossy().into_owned(),
        names,
        ignored,
    })
}

/// List the saves available for legacy import.
///
/// Reads a directory this app owns and returns names only. It cannot be pointed anywhere else, so
/// there is nothing here for a compromised webview to aim.
#[tauri::command]
pub fn list_legacy_saves(app_handle: tauri::AppHandle) -> Result<LegacyImportListing, String> {
    list_legacy_saves_in(&legacy_import_dir(&app_handle)?)
}

/// Copy one legacy save forward from `import_dir` into `saves_dir`, reading only from the first.
///
/// The whole of `import_legacy_save` except for locating the two directories, so a test can supply
/// two real temporary roots and assert what actually happened on disk: that the destination is a
/// current [`SnapshotEnvelope`], and that the source file's bytes are unchanged.
///
/// Taking the two roots as separate parameters is the structural half of "the import never writes to
/// its source". A single-root version could not express the property; with two, every write in the
/// body demonstrably targets the one derived from `saves_dir`.
pub fn import_legacy_save_into(
    import_dir: &std::path::Path,
    saves_dir: &std::path::Path,
    legacy_name: &str,
    save_as: &str,
) -> Result<String, String> {
    let source = crate::commands::save_paths::resolve_legacy_import_path(import_dir, legacy_name)?;
    if !source.is_file() {
        return Err(format!(
            "no file named {legacy_name:?} in the legacy import directory. Copy the old save there \
             first — this command cannot read from anywhere else."
        ));
    }

    // Resolve the destination before reading, so an unusable `save_as` fails without having done any
    // work — and, more to the point, so the destination is fixed by `saves_dir` and cannot be
    // influenced by anything found inside the source file.
    let target = crate::commands::save_paths::resolve_save_path(saves_dir, save_as)?;

    // The same reader the load command uses: it verifies the checksum of an enveloped save and
    // migrates a pre-envelope one (schema 1 or 2) forward. A legacy file is exactly the second case.
    let state = crate::core::snapshot::read(&source).map_err(|e| e.to_string())?;

    // Re-seal into the current envelope and write it where saves live. The import is a copy
    // forward, so the imported world gets the checksum and versioning every other save has.
    let envelope = crate::core::snapshot::SnapshotEnvelope::seal(state)
        .map_err(|e| format!("could not re-seal the imported save: {e}"))?;
    crate::core::snapshot::write_atomic(&target, &envelope)
        .map_err(|e| format!("could not write the imported save: {e}"))?;

    Ok(target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| save_as.to_string()))
}

/// Import a save written before path confinement, **read-only**, into the app's save directory.
///
/// # Why this is not "load an absolute path"
///
/// The accepted design promises pre-confinement saves stay loadable through an explicitly opt-in
/// migration. Implementing that as a command taking an absolute path would hand back exactly the
/// capability the confinement removed — a compromised webview would call it with an SSH key and
/// read the parse error.
///
/// So the authorising act is one the page cannot perform: the user copies the old file into
/// `<app data>/legacy-import/` with their own file manager. This command addresses it by name,
/// through the same allow-list as every other save name, reads it, and copies it forward into
/// `saves/` under a name the user chose. The legacy file is never written to, never truncated, and
/// never deleted — if the import produces a world the user does not want, the original is still
/// where they put it.
#[tauri::command]
pub fn import_legacy_save(
    app_handle: tauri::AppHandle,
    legacy_name: String,
    save_as: String,
) -> Result<String, String> {
    import_legacy_save_into(
        &legacy_import_dir(&app_handle)?,
        &saves_dir(&app_handle)?,
        &legacy_name,
        &save_as,
    )
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
    let (fl_leg_x, fl_leg_y, fl_leg_z) = local_to_world(
        0.8 + (t * 4.0 + std::f32::consts::PI).sin() * 0.15,
        -0.8 - hop_height * 0.35,
        0.5,
    );
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
    let (fr_leg_x, fr_leg_y, fr_leg_z) =
        local_to_world(0.8 + (t * 4.0).sin() * 0.15, -0.8 - hop_height * 0.35, -0.5);
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
    let (hl_leg_x, hl_leg_y, hl_leg_z) = local_to_world(
        -1.2 - hop_height * 0.1 + (t * 4.0).sin() * 0.1,
        -0.6 - hop_height * 0.4,
        0.6,
    );
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
    let (hr_leg_x, hr_leg_y, hr_leg_z) = local_to_world(
        -1.2 - hop_height * 0.1 + (t * 4.0 + std::f32::consts::PI).sin() * 0.1,
        -0.6 - hop_height * 0.4,
        -0.6,
    );
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
    let chewing_offset = if is_eating {
        (t * 15.0).sin() * 0.08
    } else {
        0.0
    };
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

/// Point simulation detail at the observer, or turn the targeting off.
///
/// `enabled: false` returns the engine to uniform detail — every agent `Hot`, thinking every tick,
/// which is what an engine built before simulation LOD did. That is the default, and it is the
/// rollback path: nothing here is sticky.
///
/// Writing is all this does. The value is picked up by `sync_lod_focus_system` on the next tick
/// rather than reaching into the world from the UI thread, because the world belongs to the
/// simulation thread and a command that borrowed it would have to stop it first.
#[tauri::command]
pub fn set_lod_focus(
    state: State<'_, AppState>,
    focus: crate::core::simulation_lod::LodFocus,
) -> Result<(), String> {
    let mut shared = state
        .engine
        .lod_focus
        .0
        .write()
        .map_err(|e| e.to_string())?;
    *shared = focus;
    Ok(())
}

/// The focus the engine is currently using.
#[tauri::command]
pub fn get_lod_focus(
    state: State<'_, AppState>,
) -> Result<crate::core::simulation_lod::LodFocus, String> {
    let shared = state.engine.lod_focus.0.read().map_err(|e| e.to_string())?;
    Ok(*shared)
}

/// The tier boundaries this build uses.
///
/// A caller needs these to decide whether setting a focus is even appropriate: the agent viewport
/// only turns tiering on when everything it is showing fits inside the hot radius, because
/// degrading an agent the user is looking at is worse than paying for it. Exposing the number
/// rather than letting the frontend hardcode `50.0` keeps that decision tied to the one definition
/// in `LodBands::default`.
///
/// Returns the default rather than reading the world's resource because nothing changes it at
/// runtime — the default *is* the live value — and reaching into the simulation thread's world for
/// a constant would mean stopping it.
#[tauri::command]
pub fn get_lod_bands() -> crate::core::simulation_lod::LodBands {
    crate::core::simulation_lod::LodBands::default()
}
