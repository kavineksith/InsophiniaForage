//! Logging setup. Uses `tracing` + `tracing-appender` so logs rotate daily
//! and never grow unbounded. We keep 7 days of logs by default — old files
//! are deleted by a background sweeper that runs once on startup.
//!
//! Two layers:
//! 1. A **rolling file appender** under `~/.insophinia-forage/logs/app/`
//!    for forensic debugging.
//! 2. A **console appender** (stderr) for `tauri dev` visibility.
//!
//! We intentionally do NOT log secrets. The DB password is stored in the
//! settings file with restricted permissions and never written to logs.

use crate::core::paths;
use crate::error::ForageResult;
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

/// Initialize the global tracing subscriber. Returns a `WorkerGuard` that
/// must be held for the lifetime of the app — dropping it flushes and stops
/// the file appender.
pub fn init() -> ForageResult<(WorkerGuard, WorkerGuard)> {
    let log_dir = paths::app_log_dir();
    std::fs::create_dir_all(&log_dir)?;

    // File appender: one file per day, named like `app.2026-10-26.log`.
    let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "app.log");
    let (file_nb, file_guard) = tracing_appender::non_blocking(file_appender);

    // Console appender: pretty-printed to stderr for `tauri dev`.
    let stderr_nb = std::io::stderr();
    let (stderr_nb, stderr_guard) = tracing_appender::non_blocking(stderr_nb);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,insophinia_forage_lib=debug"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(file_nb)
                .with_ansi(false)
                .with_target(true)
                .json(),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(stderr_nb)
                .with_ansi(true)
                .with_target(false)
                .compact(),
        )
        .try_init()
        .map_err(|e| crate::error::ForageError::Config(format!("tracing init failed: {}", e)))?;

    // Sweep old logs in the background — fire and forget.
    let sweep_dir = log_dir.clone();
    std::thread::spawn(move || {
        if let Err(e) = sweep_old_logs(&sweep_dir, 7) {
            tracing::warn!(error = %e, "log sweep failed");
        }
    });

    Ok((file_guard, stderr_guard))
}

/// Delete log files older than `keep_days` days.
fn sweep_old_logs(dir: &PathBuf, keep_days: u32) -> std::io::Result<()> {
    let cutoff = std::time::SystemTime::now()
        - std::time::Duration::from_secs(60 * 60 * 24 * keep_days as u64);

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if !meta.is_file() {
            continue;
        }
        if let Ok(modified) = meta.modified() {
            if modified < cutoff {
                let _ = std::fs::remove_file(entry.path());
                tracing::info!(path = %entry.path().display(), "swept old log");
            }
        }
    }
    Ok(())
}
