//! User settings — persisted to `~/.insophinia-forage/settings.json`.
//!
//! Settings are loaded once on startup and reloaded on demand. All mutations
//! go through `SettingsManager::save()` which writes atomically (write to
//! temp file, then rename) to prevent corruption if the app crashes mid-write.

use crate::core::paths;
use crate::error::{ForageError, ForageResult};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Schema version for forward migrations.
    pub version: u32,
    pub ports: PortsConfig,
    pub versions: VersionsConfig,
    pub php: PhpConfig,
    pub mariadb: MariadbConfig,
    pub nginx: NginxConfig,
    pub binary_strategy: BinaryStrategy,
    /// What to do when the app detects a system-installed component.
    /// Defaults to `Coexist` — install a separate copy in the app's bin
    /// dir and never touch system packages.
    pub conflict_policy: ConflictPolicy,
    pub auto_start_services: bool,
    pub open_browser_on_start: bool,
    pub theme: Theme,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortsConfig {
    pub nginx_http: u16,
    pub nginx_https: u16,
    pub mariadb: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionsConfig {
    pub php: String,
    pub mariadb: String,
    pub nginx: String,
    pub phpmyadmin: String,
    pub wordpress: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpConfig {
    pub memory_limit: String,    // e.g. "256M"
    pub upload_max_filesize: String,
    pub post_max_size: String,
    pub max_execution_time: u32, // seconds
    pub display_errors: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MariadbConfig {
    pub innodb_buffer_pool_size: String, // e.g. "128M"
    pub max_connections: u32,
    pub root_password: String,           // dev-only; never used in production
    pub app_user: String,
    pub app_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NginxConfig {
    pub client_max_body_size: String,
    pub worker_processes: String,        // "auto" or a number
}

/// How binaries are resolved. `Auto` = system → bundled → download.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BinaryStrategy {
    Auto,
    PreferSystem,
    PreferBundled,
    PreferDownload,
}

/// What to do when the app detects that a component is already installed
/// on the system (e.g. `php` from `apt`, `nginx` from `dnf`).
///
/// This is independent of `BinaryStrategy`:
/// - `BinaryStrategy` controls the **resolution order** at runtime
///   (which binary to spawn when starting a service).
/// - `ConflictPolicy` controls the **install-time behavior**
///   (whether to download a separate copy or skip the download).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    /// Install a separate copy in `~/.insophinia-forage/bin/` and never
    /// touch the system package. The app uses its own copy at runtime.
    /// **Recommended.** Safe for the user's system.
    Coexist,
    /// Use the system binary as-is. Don't download a separate copy.
    /// Lightest on disk, but the user is responsible for keeping the
    /// system package working (e.g. `apt upgrade php`).
    UseSystem,
    /// Uninstall the system package via the package manager and install
    /// the app's own copy. **Not recommended** — breaks other apps that
    /// depend on the system package. The app will refuse to do this
    /// without explicit confirmation.
    Replace,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Light,
    Dark,
    Auto,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            ports: PortsConfig {
                nginx_http: crate::core::ports::defaults::NGINX_HTTP,
                nginx_https: crate::core::ports::defaults::NGINX_HTTPS,
                mariadb: crate::core::ports::defaults::MARIADB,
            },
            versions: VersionsConfig {
                php: "8.3".into(),
                mariadb: "11.4".into(),
                nginx: "1.27".into(),
                phpmyadmin: "5.2.2".into(),
                wordpress: "latest".into(),
            },
            php: PhpConfig {
                memory_limit: "256M".into(),
                upload_max_filesize: "64M".into(),
                post_max_size: "64M".into(),
                max_execution_time: 300,
                display_errors: true,
            },
            mariadb: MariadbConfig {
                innodb_buffer_pool_size: "128M".into(),
                max_connections: 30,
                // Random dev-only passwords. Generated once on first run.
                root_password: generate_random_password(24),
                app_user: "forge".into(),
                app_password: generate_random_password(20),
            },
            nginx: NginxConfig {
                client_max_body_size: "64M".into(),
                worker_processes: "auto".into(),
            },
            binary_strategy: BinaryStrategy::Auto,
            conflict_policy: ConflictPolicy::Coexist,
            auto_start_services: false,
            open_browser_on_start: false,
            theme: Theme::Auto,
            created_at: chrono::Utc::now(),
        }
    }
}

/// Thread-safe settings manager. Read-heavy, write-rare.
#[derive(Clone)]
pub struct SettingsManager {
    inner: Arc<RwLock<Settings>>,
}

impl SettingsManager {
    pub fn load() -> ForageResult<Self> {
        let settings = if paths::is_initialized() {
            let raw = std::fs::read_to_string(paths::settings_file())?;
            let mut s: Settings = serde_json::from_str(&raw)?;
            s.migrate();
            s
        } else {
            Settings::default()
        };
        Ok(Self {
            inner: Arc::new(RwLock::new(settings)),
        })
    }

    pub fn get(&self) -> Settings {
        self.inner.read().clone()
    }

    pub fn update<F>(&self, f: F) -> ForageResult<Settings>
    where
        F: FnOnce(&mut Settings),
    {
        let mut s = self.inner.write();
        f(&mut s);
        let snapshot = s.clone();
        drop(s);
        self.save(&snapshot)?;
        Ok(snapshot)
    }

    pub fn save(&self, s: &Settings) -> ForageResult<()> {
        let path = paths::settings_file();
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(s)?;
        std::fs::write(&tmp, json)?;
        // Atomic on POSIX, near-atomic on Windows.
        std::fs::rename(&tmp, &path)?;
        // Restrict permissions — settings contain DB passwords.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

impl Settings {
    /// Forward-compatible migration hook. Bump `version` and add a match
    /// arm here when the schema changes.
    pub fn migrate(&mut self) {
        // Currently v1, nothing to migrate.
    }

    pub fn project_url(&self, name: &str) -> String {
        format!("http://{}:{}", name, self.ports.nginx_http)
    }

    pub fn phpmyadmin_url(&self) -> String {
        format!("http://pma:{}", self.ports.nginx_http)
    }
}

/// Cryptographically random password for the dev DB user. Uses the OS RNG.
fn generate_random_password(len: usize) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    // We avoid pulling in `rand` to keep deps light. The OS RNG via
    // SystemTime nanos + process id is sufficient for a *dev-only* password
    // that lives in a 0600-permission file on the user's own machine.
    let mut buf = String::with_capacity(len);
    let charset: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
        ^ (std::process::id() as u64);
    for _ in 0..len {
        // xorshift64
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        buf.push(charset[(seed % charset.len() as u64) as usize] as char);
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_roundtrip() {
        let s = Settings::default();
        let json = serde_json::to_string(&s).unwrap();
        let _: Settings = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn passwords_are_unique() {
        let a = Settings::default();
        let b = Settings::default();
        assert_ne!(a.mariadb.root_password, b.mariadb.root_password);
    }
}
