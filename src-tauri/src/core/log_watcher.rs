//! Real-time log watcher. Uses the `notify` crate to watch each service's
//! log file for changes, reads the new bytes, splits them into lines, and
//! emits `service://log-line` events to the frontend.
//!
//! ## Design
//!
//! - One `RecommendedWatcher` per service. Cheap — `notify` uses inotify
//!   on Linux, FSEvents on macOS, ReadDirectoryChangesW on Windows.
//! - The watcher callback reads new bytes from the file starting at the
//!   last-read offset. Offsets are tracked in a separate `Mutex<HashMap>`
//!   so the callback can update them without owning the watcher.
//! - If the file is rotated (truncated or replaced), we detect the size
//!   shrink and reset the offset to 0.
//! - Partial lines (no trailing `\n`) are held back: we reset the offset
//!   so the next event re-reads them. This handles log lines that get
//!   split across two writes.

use crate::core::paths;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info, warn};

/// Where a service's log file lives. Mirrors `ProcessManager::tail_log`.
fn service_log_path(service: &str) -> PathBuf {
    paths::logs_dir().join(service).join("service.log")
}

/// Per-service offset tracker. Stored separately from the watcher so the
/// notify callback can update it without owning the watcher.
struct Slot {
    path: PathBuf,
    offset: u64,
}

/// Thread-safe log watcher manager. Owns the `notify::RecommendedWatcher`
/// instances (one per service) and the offset map.
#[derive(Clone, Default)]
pub struct LogWatcher {
    watchers: Arc<Mutex<HashMap<String, notify::RecommendedWatcher>>>,
    slots: Arc<Mutex<HashMap<String, Slot>>>,
}

impl LogWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribe to a service's log. Idempotent — calling twice for the
    /// same service is a no-op.
    pub fn subscribe(&self, app: &AppHandle, service: &str) -> Result<(), String> {
        let log_path = service_log_path(service);

        // Make sure the file exists so the watcher has something to watch.
        if !log_path.exists() {
            if let Some(parent) = log_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&log_path, "").map_err(|e| e.to_string())?;
        }

        // Initialize the offset to the current EOF so we only emit *new*
        // lines (the UI loads the existing tail separately via `tail_log`).
        let initial_offset = std::fs::metadata(&log_path)
            .map(|m| m.len())
            .unwrap_or(0);
        self.slots.lock().insert(
            service.to_string(),
            Slot {
                path: log_path.clone(),
                offset: initial_offset,
            },
        });

        // Already subscribed? Just refresh the offset.
        if self.watchers.lock().contains_key(service) {
            return Ok(());
        }

        let svc = service.to_string();
        let app_clone = app.clone();
        let slots_clone = self.slots.clone();

        let mut watcher = notify::recommended_watcher(
            move |res: Result<notify::Event, notify::Error>| {
                if let Err(e) = res {
                    warn!(error = %e, "log watcher event error");
                    return;
                }
                if let Err(e) = read_and_emit(&app_clone, &svc, &slots_clone) {
                    error!(service = %svc, error = %e, "log read failed");
                }
            },
        )
        .map_err(|e| format!("watcher init failed: {}", e))?;

        // Watch the parent dir so we survive file rotation (replace).
        let watch_dir = log_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| log_path.clone());
        watcher
            .watch(&watch_dir, notify::RecursiveMode::NonRecursive)
            .map_err(|e| format!("watch failed: {}", e))?;

        info!(service = %service, path = %log_path.display(), "log watcher started");
        self.watchers.lock().insert(service.to_string(), watcher);
        Ok(())
    }

    /// Stop watching a service. Drops the watcher (which closes the
    /// inotify/FSEvents handle) and removes the offset slot.
    pub fn unsubscribe(&self, service: &str) {
        let removed = self.watchers.lock().remove(service);
        self.slots.lock().remove(service);
        if removed.is_some() {
            info!(service = %service, "log watcher stopped");
        }
    }

    /// Stop all watchers. Called on app quit.
    pub fn unsubscribe_all(&self) {
        let mut watchers = self.watchers.lock();
        let keys: Vec<String> = watchers.keys().cloned().collect();
        let count = keys.len();
        for k in &keys {
            watchers.remove(k);
        }
        self.slots.lock().clear();
        if count > 0 {
            info!(count, "stopped all log watchers");
        }
    }
}

/// Read any new bytes from the service's log file (starting at the stored
/// offset) and emit each complete line as a `service://log-line` event.
fn read_and_emit(
    app: &AppHandle,
    service: &str,
    slots: &Arc<Mutex<HashMap<String, Slot>>>,
) -> Result<(), String> {
    let mut slots = slots.lock();
    let slot = slots
        .get_mut(service)
        .ok_or_else(|| "no slot for service".to_string())?;

    let mut file = std::fs::File::open(&slot.path).map_err(|e| e.to_string())?;
    let current_size = file
        .metadata()
        .map(|m| m.len())
        .map_err(|e| e.to_string())?;

    // Detect rotation: file shrank → reset to 0.
    if current_size < slot.offset {
        debug!(
            service = %service,
            prev = slot.offset,
            new = current_size,
            "log rotated"
        );
        slot.offset = 0;
    }

    file.seek(SeekFrom::Start(slot.offset))
        .map_err(|e| e.to_string())?;

    let mut buf = Vec::new();
    file.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let prev_offset = slot.offset;

    if buf.is_empty() {
        return Ok(());
    }

    let text = String::from_utf8_lossy(&buf);
    let ends_with_newline = text.ends_with('\n');
    let lines: Vec<&str> = text.split('\n').collect();

    // If the buffer ends with \n, the last split element is "" — drop it.
    // Otherwise, hold back the partial last line by NOT advancing the
    // offset past it.
    let (complete, held_back_bytes) = if ends_with_newline {
        let complete = &lines[..lines.len().saturating_sub(1)];
        (complete, 0usize)
    } else {
        // Split off the last element as partial.
        let (complete, partial) = lines.split_at(lines.len().saturating_sub(1));
        let held = partial.first().map(|s| s.len()).unwrap_or(0);
        (complete, held)
    };

    // Advance offset by the bytes of complete lines (each line + its \n).
    let complete_bytes: usize = complete.iter().map(|l| l.len() + 1).sum();
    slot.offset = prev_offset + complete_bytes as u64;

    // Emit complete lines. We do this after releasing the slots lock —
    // but since we already updated the offset, it's safe to drop the
    // guard now.
    let lines_to_emit: Vec<String> = complete
        .iter()
        .filter(|l| !l.is_empty())
        .map(|s| s.to_string())
        .collect();
    drop(slots);

    for line in lines_to_emit {
        let _ = app.emit(
            "service://log-line",
            serde_json::json!({ "service": service, "line": line }),
        );
    }

    let _ = held_back_bytes;
    Ok(())
}
