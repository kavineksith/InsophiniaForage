//! Log streaming. The UI calls `tail_log` once on mount to get the
//! existing log content, then `subscribe_log` to receive real-time
//! updates via `service://log-line` events. The `notify` crate watches
//! the log file; we read new bytes on every event and emit each complete
//! line. See `core/log_watcher.rs` for the implementation.

use crate::core::log_watcher::LogWatcher;
use crate::core::process::ProcessManager;
use crate::core::AppState;
use tauri::{AppHandle, Emitter, State};

/// Get the last `n` lines of a service log. Used for the initial load
/// before subscribing to live updates.
#[tauri::command]
pub fn tail_log(service: String, n: Option<usize>) -> Result<Vec<String>, String> {
    ProcessManager::tail_log(&service, n.unwrap_or(500)).map_err(|e| e.to_string())
}

/// Start watching a service's log for live updates. Emits
/// `service://log-line` events with `{ service, line }` payload.
#[tauri::command]
pub fn subscribe_log(
    app: AppHandle,
    state: State<'_, AppState>,
    service: String,
) -> Result<(), String> {
    state.log_watcher.subscribe(&app, &service)
}

/// Stop watching a service's log. Frees the inotify/FSEvents handle.
#[tauri::command]
pub fn unsubscribe_log(state: State<'_, AppState>, service: String) -> Result<(), String> {
    state.log_watcher.unsubscribe(&service);
    Ok(())
}

/// Emit a log-line event to the frontend. Useful for tests or for
/// emitting synthetic lines from elsewhere in the backend.
#[allow(dead_code)]
pub fn emit_log_event(app: &AppHandle, service: &str, line: &str) {
    let _ = app.emit(
        "service://log-line",
        serde_json::json!({ "service": service, "line": line }),
    );
}

/// Re-export so other modules can call `LogWatcher::unsubscribe_all` on
/// app exit.
pub use crate::core::log_watcher::LogWatcher as LogWatcherHandle;
