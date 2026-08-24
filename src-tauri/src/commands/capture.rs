//! IPC for the in-process tick profiler ([`crate::core::tick_capture`]).
//!
//! Four commands, and nothing else: start, status, stop, export. They write to and read from the
//! engine's [`SharedTickCapture`](crate::core::tick_capture::SharedTickCapture) handle rather than
//! reaching into the simulation world, for the same reason `set_lod_focus` does — the world belongs
//! to the simulation thread, and a command that borrowed it would have to stop it first.
//!
//! Starting a capture on an engine that is not running is allowed and does nothing until it is: the
//! capture consumes ticks, so a stopped engine simply produces none. The status command reports
//! `ticks_observed = 0` in that case, which is the honest answer and distinguishable from "the
//! engine is fast".

use crate::core::tick_capture::{
    CaptureAccounting, CaptureConfig, CaptureExport, CaptureStatus, WorkloadContext,
};
use crate::AppState;
use tauri::State;

/// What a capture is doing, without the cost of summarising it.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct CaptureStatusReport {
    pub status: CaptureStatus,
    pub config: CaptureConfig,
    pub accounting: CaptureAccounting,
    /// Samples currently held in the ring — never more than `config.capacity`.
    pub samples_held: usize,
    /// The world the capture is measuring, as read off the running world rather than configured.
    pub workload: WorkloadContext,
}

/// Begin a capture, discarding whatever the previous one held.
///
/// Returns the configuration's rejection message rather than clamping a bad value into a good one:
/// a capacity of zero silently corrected to one produces a report that looks like a measurement and
/// is not.
#[tauri::command]
pub fn start_tick_capture(state: State<'_, AppState>, config: CaptureConfig) -> Result<(), String> {
    state
        .engine
        .tick_capture
        .start(config)
        .map_err(|e| e.to_string())
}

/// Stop recording. Idempotent, and keeps everything captured so far so it can still be exported.
#[tauri::command]
pub fn stop_tick_capture(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.tick_capture.stop();
    Ok(())
}

/// What the capture is doing right now.
#[tauri::command]
pub fn get_tick_capture_status(state: State<'_, AppState>) -> Result<CaptureStatusReport, String> {
    let capture = &state.engine.tick_capture;
    Ok(CaptureStatusReport {
        status: capture.status(),
        config: capture.config(),
        accounting: capture.accounting(),
        samples_held: capture.sample_count(),
        workload: capture.export().workload,
    })
}

/// Summarise the capture, and optionally write it to a named file under the app's data directory.
///
/// `file_name` follows the same contract as a save name — a name, never a path. See
/// [`crate::commands::save_paths`] for why a frontend-supplied string is not treated as a path.
/// Passing `None` returns the document without touching the filesystem.
#[tauri::command]
pub fn export_tick_capture(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    file_name: Option<String>,
) -> Result<CaptureExport, String> {
    let capture = &state.engine.tick_capture;
    let Some(name) = file_name else {
        return Ok(capture.export());
    };
    let target =
        crate::commands::save_paths::resolve_save_path(&captures_dir(&app_handle)?, &name)?;
    capture.export_to(&target).map_err(|e| e.to_string())
}

/// Write a completed capture under the app data directory, from outside the IPC layer.
///
/// The engine thread calls this when `ANIMA_TICK_CAPTURE_OUT` is set and a capture reaches
/// `Complete` — the only route out of a capture on a release build, which has no DevTools to invoke
/// [`export_tick_capture`] from. Generic over the runtime because [`crate::core::simulation_loop`]
/// is, and returns the path it wrote so the caller can say where the file went.
///
/// `name` goes through the same [`crate::commands::save_paths`] resolution as an IPC-supplied save
/// name. The value arrives from the environment rather than a webview, which is a different threat
/// model but not a licence to treat it as a path: the rule is the same so there is only one rule.
pub(crate) fn export_capture_to_app_data<R: tauri::Runtime>(
    capture: &crate::core::tick_capture::SharedTickCapture,
    app_handle: Option<&tauri::AppHandle<R>>,
    name: &str,
) -> Result<std::path::PathBuf, String> {
    let handle = app_handle.ok_or("no app handle: this engine was started headless")?;
    let target = crate::commands::save_paths::resolve_save_path(&captures_dir(handle)?, name)?;
    capture.export_to(&target).map_err(|e| e.to_string())?;
    Ok(target)
}

/// Directory this app keeps capture exports in, created on demand. Beside `saves/` rather than
/// inside it, so a profiler artefact never turns up in a save picker.
fn captures_dir<R: tauri::Runtime>(
    app_handle: &tauri::AppHandle<R>,
) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    let dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("cannot locate the app data directory: {e}"))?
        .join("captures");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("cannot create the capture directory {}: {e}", dir.display()))?;
    Ok(dir)
}
