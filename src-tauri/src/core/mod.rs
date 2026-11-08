//! Core modules — the parts of the backend that are not Tauri commands.

pub mod compatibility;
pub mod config_gen;
pub mod detection;
pub mod latest_versions;
pub mod log_watcher;
pub mod logging;
pub mod package_manager;
pub mod paths;
pub mod php_versions;
pub mod ports;
pub mod process;
pub mod settings;
pub mod source_build;
pub mod spawn;

use parking_lot::RwLock;
use std::sync::Arc;

/// Shared app state. Injected into Tauri via `.manage(AppState::new())`
/// and accessed from commands via `State<'_, AppState>`.
pub struct AppState {
    pub settings: settings::SettingsManager,
    pub processes: process::ProcessManager,
    pub log_watcher: log_watcher::LogWatcher,
    /// Cached list of which binaries we resolved at startup.
    pub binaries: RwLock<Binaries>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Binaries {
    pub php: Option<paths::ResolvedBinary>,
    pub nginx: Option<paths::ResolvedBinary>,
    pub mariadb: Option<paths::ResolvedBinary>,
    pub composer: Option<paths::ResolvedBinary>,
}

impl AppState {
    pub fn new() -> crate::error::ForageResult<Self> {
        Ok(Self {
            settings: settings::SettingsManager::load()?,
            processes: process::ProcessManager::new(),
            log_watcher: log_watcher::LogWatcher::new(),
            binaries: RwLock::new(Binaries::default()),
        })
    }

    /// Refresh the resolved-binary cache from settings + on-disk state.
    pub fn refresh_binaries(&self) -> crate::error::ForageResult<Binaries> {
        let s = self.settings.get();
        let php = crate::binaries::resolve("php", &s, Some(&s.versions.php)).ok();
        let nginx = crate::binaries::resolve("nginx", &s, Some(&s.versions.nginx)).ok();
        let mariadb = crate::binaries::resolve("mariadbd", &s, Some(&s.versions.mariadb))
            .or_else(|_| crate::binaries::resolve("mariadb", &s, Some(&s.versions.mariadb)))
            .ok();
        let composer = crate::binaries::resolve("composer", &s, None).ok();
        let snapshot = Binaries {
            php,
            nginx,
            mariadb,
            composer,
        };
        *self.binaries.write() = snapshot.clone();
        Ok(snapshot)
    }

    /// Snapshot of the binary cache.
    pub fn binaries(&self) -> Binaries {
        self.binaries.read().clone()
    }
}

/// Type alias for `Arc<AppState>` — used when we need to share state across
/// async tasks (e.g. log streaming).
pub type SharedState = Arc<AppState>;
