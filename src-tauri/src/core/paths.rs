//! Centralized path resolution. Every path the app reads or writes goes
//! through this module so we have one place to audit for path-traversal
//! bugs and one place to mock in tests.
//!
//! Layout on disk (per-user, no root required):
//! ```text
//! ~/.insophinia-forage/
//! ├── bin/                # downloaded/bundled binaries (php, nginx, mariadb)
//! │   ├── php-8.3/
//! │   ├── nginx-1.27/
//! │   ├── mariadb-11.4/
//! │   └── composer.phar
//! ├── conf/               # generated config files
//! │   ├── nginx.conf
//! │   ├── php-fpm.conf
//! │   ├── my.cnf
//! │   └── php.ini
//! ├── data/
//! │   ├── mysql/          # MariaDB data dir
//! │   └── sessions/       # PHP session save path
//! ├── logs/
//! │   ├── app/            # tracing-appender logs (rotated)
//! │   ├── nginx/
//! │   ├── php/
//! │   └── mariadb/
//! ├── www/                # web roots (one folder per project)
//! │   ├── pma/            # phpMyAdmin
//! │   └── <project>/
//! ├── cache/              # downloads, extracted archives
//! ├── run/                # PID files, unix sockets
//! └── settings.json       # user settings
//! ```

use crate::error::{ForageError, ForageResult};
use std::path::{Path, PathBuf};

/// Root directory for all app data. Defaults to `~/.insophinia-forage`.
/// Can be overridden by setting `INSOPHINIA_FORAGE_HOME` for tests.
pub fn home_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("INSOPHINIA_FORAGE_HOME") {
        return PathBuf::from(custom);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".insophinia-forage")
}

// ---- Subdirectory helpers ---------------------------------------------------

pub fn bin_dir() -> PathBuf {
    home_dir().join("bin")
}
pub fn conf_dir() -> PathBuf {
    home_dir().join("conf")
}
pub fn data_dir() -> PathBuf {
    home_dir().join("data")
}
pub fn logs_dir() -> PathBuf {
    home_dir().join("logs")
}
pub fn www_dir() -> PathBuf {
    home_dir().join("www")
}
pub fn cache_dir() -> PathBuf {
    home_dir().join("cache")
}
pub fn run_dir() -> PathBuf {
    home_dir().join("run")
}
pub fn settings_file() -> PathBuf {
    home_dir().join("settings.json")
}

// ---- Service-specific paths -------------------------------------------------

pub fn mysql_data_dir() -> PathBuf {
    data_dir().join("mysql")
}
pub fn php_sessions_dir() -> PathBuf {
    data_dir().join("sessions")
}
pub fn nginx_log_dir() -> PathBuf {
    logs_dir().join("nginx")
}
pub fn php_log_dir() -> PathBuf {
    logs_dir().join("php")
}
pub fn mariadb_log_dir() -> PathBuf {
    logs_dir().join("mariadb")
}
pub fn app_log_dir() -> PathBuf {
    logs_dir().join("app")
}

/// Unix socket for PHP-FPM. Kept under `run/` so it survives restarts but
/// never collides with system sockets.
pub fn php_fpm_socket() -> PathBuf {
    run_dir().join("php-fpm.sock")
}

/// PID file for a given service. Used to detect orphan processes after a
/// crash.
pub fn pid_file(service: &str) -> PathBuf {
    run_dir().join(format!("{}.pid", service))
}

// ---- Binary paths (resolved at runtime) ------------------------------------

/// A binary that has been resolved to a concrete path on disk, along with
/// its source — useful for showing the user where each tool came from.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResolvedBinary {
    pub name: String,
    pub path: PathBuf,
    pub source: BinarySource,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BinarySource {
    /// Bundled in the installer (resources/).
    Bundled,
    /// Downloaded on first run into ~/.insophinia-forage/bin/.
    Downloaded,
    /// Detected on the system PATH (e.g. `/usr/bin/php`).
    System,
}

// ---- Path safety ------------------------------------------------------------

/// Reject any path component that could escape the app root. We use this on
/// every project name, database name, or user-supplied identifier before
/// joining it onto a real path.
pub fn validate_name(name: &str) -> ForageResult<()> {
    if name.is_empty() {
        return Err(ForageError::InvalidInput("name cannot be empty".into()));
    }
    if name.len() > 64 {
        return Err(ForageError::InvalidInput(
            "name cannot be longer than 64 characters".into(),
        ));
    }
    // Allow [a-zA-Z0-9._-] only. No slashes, no parent-dir escapes.
    let valid = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-');
    if !valid {
        return Err(ForageError::InvalidInput(format!(
            "'{}' contains invalid characters. Allowed: a-z A-Z 0-9 . _ -",
            name
        )));
    }
    // Reject dotfiles that would be hidden / confusing.
    if name.starts_with('.') {
        return Err(ForageError::InvalidInput(
            "name cannot start with a dot".into(),
        ));
    }
    Ok(())
}

/// Project web root: `~/.insophinia-forage/www/<name>`.
pub fn project_root(name: &str) -> ForageResult<PathBuf> {
    validate_name(name)?;
    Ok(www_dir().join(name))
}

/// True if the app has been initialized (settings.json exists).
pub fn is_initialized() -> bool {
    settings_file().exists()
}

/// Ensure all required directories exist. Idempotent.
pub fn ensure_dirs() -> ForageResult<()> {
    for dir in [
        home_dir(),
        bin_dir(),
        conf_dir(),
        data_dir(),
        mysql_data_dir(),
        php_sessions_dir(),
        logs_dir(),
        nginx_log_dir(),
        php_log_dir(),
        mariadb_log_dir(),
        app_log_dir(),
        www_dir(),
        cache_dir(),
        run_dir(),
    ] {
        if !dir.exists() {
            std::fs::create_dir_all(&dir).map_err(|e| {
                ForageError::Config(format!(
                    "failed to create directory {}: {}",
                    dir.display(),
                    e
                ))
            })?;
        }
        // Restrict permissions on data dir so other users can't read DB files.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if dir == mysql_data_dir() || dir == home_dir() {
                let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
            }
        }
    }
    Ok(())
}

/// Normalize a user-supplied path. Expands `~`, resolves `..`, and verifies
/// the result is absolute. Used when the user picks a custom project folder
/// or imports an existing project.
pub fn normalize_user_path(p: &str) -> ForageResult<PathBuf> {
    let expanded = if p.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            home.join(&p[2..])
        } else {
            PathBuf::from(p)
        }
    } else {
        PathBuf::from(p)
    };
    let canonical = expanded
        .canonicalize()
        .unwrap_or_else(|_| expanded.clone());
    if !canonical.is_absolute() {
        return Err(ForageError::InvalidInput(format!(
            "path must be absolute after normalization: {}",
            p
        )));
    }
    Ok(canonical)
}

/// True if `child` is inside `parent`. Used to prevent path-traversal when
/// deleting projects or purging databases.
pub fn is_inside(parent: &Path, child: &Path) -> bool {
    child.starts_with(parent)
}
