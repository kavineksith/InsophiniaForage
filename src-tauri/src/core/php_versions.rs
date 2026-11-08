//! Per-project PHP version overrides. Each project can choose a specific
//! PHP version (e.g. "8.1" for a legacy WordPress site, "8.3" for a new
//! Laravel app). The override is stored in
//! `~/.insophinia-forage/www/<name>/.forge.json` so it travels with the
//! project folder if the user copies it.
//!
//! At service start time, the spawn logic:
//!   1. Reads all project overrides.
//!   2. For each unique PHP version, ensures a php-fpm pool config exists.
//!   3. Spawns one php-fpm process per version, each on its own socket
//!      (`run/php-fpm-<version>.sock`).
//!   4. The default php-fpm (no override) uses `run/php-fpm.sock`.
//!
//! nginx config uses a `map $project $php_socket` directive to route
//! each project to the right socket.

use crate::core::paths;
use crate::error::{ForageError, ForageResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

/// Per-project override file. Lives inside the project root so it travels
/// with the project if the user copies it.
const OVERRIDE_FILE: &str = ".forge.json";

/// Schema for `.forge.json`. We use a struct so future fields (memory
/// limit, error display, etc.) can be added without breaking old files.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectConfig {
    /// PHP version override. `None` means "use the global default".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub php_version: Option<String>,
    /// Per-project PHP memory limit. `None` = use global setting.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_limit: Option<String>,
}

/// Read a project's override file. Returns `Default::default()` if the
/// file doesn't exist (no override).
pub fn read_project_config(project_name: &str) -> ForageResult<ProjectConfig> {
    paths::validate_name(project_name)?;
    let root = paths::project_root(project_name)?;
    let path = root.join(OVERRIDE_FILE);
    if !path.exists() {
        return Ok(ProjectConfig::default());
    }
    let raw = std::fs::read_to_string(&path)?;
    let cfg: ProjectConfig = serde_json::from_str(&raw).map_err(|e| {
        ForageError::Project(format!(
            "failed to parse {} for '{}': {}",
            OVERRIDE_FILE, project_name, e
        ))
    })?;
    Ok(cfg)
}

/// Write a project's override file. Atomic via tmp+rename.
pub fn write_project_config(project_name: &str, cfg: &ProjectConfig) -> ForageResult<()> {
    paths::validate_name(project_name)?;
    let root = paths::project_root(project_name)?;
    if !root.exists() {
        return Err(ForageError::Project(format!(
            "project '{}' does not exist",
            project_name
        )));
    }
    let path = root.join(OVERRIDE_FILE);
    let tmp = root.join(format!("{}.tmp", OVERRIDE_FILE));
    let json = serde_json::to_string_pretty(cfg)?;
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Set a project's PHP version override. Pass `None` to clear.
pub fn set_php_version(project_name: &str, version: Option<&str>) -> ForageResult<()> {
    let mut cfg = read_project_config(project_name)?;
    cfg.php_version = version.map(|s| s.to_string());
    write_project_config(project_name, &cfg)
}

/// Discover all installed PHP versions by scanning `~/.insophinia-forage/bin/`
/// for `php-<version>/` folders, plus bundled binaries, plus system PHP.
///
/// Returns a sorted list of version strings (e.g. `["8.1", "8.3", "8.4"]`).
pub fn list_installed_php_versions() -> Vec<String> {
    let mut versions: BTreeSet<String> = BTreeSet::new();

    // 1. Downloaded / user-installed.
    let bin_dir = paths::bin_dir();
    if let Ok(entries) = std::fs::read_dir(&bin_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(v) = name.strip_prefix("php-") {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    versions.insert(v.to_string());
                }
            }
        }
    }

    // 2. Bundled (resources/bin/<platform>/php-<version>/).
    if let Some(rd) = crate::binaries::detect::resource_dir() {
        let platform_dir = rd.join("bin");
        if let Ok(entries) = std::fs::read_dir(&platform_dir) {
            for entry in entries.flatten() {
                if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                if let Ok(inner) = std::fs::read_dir(entry.path()) {
                    for e in inner.flatten() {
                        let name = e.file_name();
                        let name = name.to_string_lossy();
                        if let Some(v) = name.strip_prefix("php-") {
                            versions.insert(v.to_string());
                        }
                    }
                }
            }
        }
    }

    // 3. System PHP (if any) — query its version.
    if let Ok(p) = which::which("php") {
        if let Some(v) = crate::binaries::detect::query_version(&p) {
            versions.insert(v);
        }
    }

    versions.into_iter().collect()
}

/// Map a project name to its PHP-FPM socket path. Uses the override if
/// set, else the default socket.
pub fn php_socket_for_project(project_name: &str, default_version: &str) -> PathBuf {
    let cfg = read_project_config(project_name).unwrap_or_default();
    let version = cfg.php_version.as_deref().unwrap_or(default_version);
    php_socket_for_version(version)
}

/// Socket path for a given PHP version. Always under `run/`.
pub fn php_socket_for_version(version: &str) -> PathBuf {
    // Normalize: "8.3" -> "8.3". Empty/default -> "default".
    let v = if version.is_empty() { "default" } else { version };
    // Replace dots with underscores in the filename to avoid confusion.
    let safe = v.replace('.', "_");
    paths::run_dir().join(format!("php-fpm-{}.sock", safe))
}

/// Compute the set of unique PHP versions currently in use across all
/// projects. Used by the spawn logic to decide how many php-fpm
/// processes to start.
pub fn collect_active_php_versions(default_version: &str) -> Vec<String> {
    let mut versions: BTreeSet<String> = BTreeSet::new();
    versions.insert(default_version.to_string());

    let www = paths::www_dir();
    if let Ok(entries) = std::fs::read_dir(&www) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            if let Ok(cfg) = read_project_config(&name) {
                if let Some(v) = cfg.php_version {
                    versions.insert(v);
                }
            }
        }
    }

    versions.into_iter().collect()
}

/// Find the PHP binary path for a specific version. Looks in:
///   1. `~/.insophinia-forage/bin/php-<version>/php`
///   2. Bundled `resources/bin/<platform>/php-<version>/php`
///   3. System `php` (only if version matches)
pub fn find_php_binary_for_version(version: &str) -> ForageResult<PathBuf> {
    let version_dir_name = format!("php-{}", version);

    // 1. Downloaded.
    let bin_dir = paths::bin_dir();
    let candidate_dir = bin_dir.join(&version_dir_name);
    if candidate_dir.is_dir() {
        for exe in ["php", "bin/php", "sbin/php"] {
            let p = candidate_dir.join(exe);
            if p.exists() {
                return Ok(p);
            }
        }
    }

    // 2. Bundled.
    if let Some(rd) = crate::binaries::detect::resource_dir() {
        let bin_root = rd.join("bin");
        if let Ok(platform_dirs) = std::fs::read_dir(&bin_root) {
            for pd in platform_dirs.flatten() {
                let p = pd.path().join(&version_dir_name);
                if p.is_dir() {
                    for exe in ["php", "bin/php", "sbin/php"] {
                        let candidate = p.join(exe);
                        if candidate.exists() {
                            return Ok(candidate);
                        }
                    }
                }
            }
        }
    }

    // 3. System PHP — only if its version matches.
    if let Ok(p) = which::which("php") {
        if let Some(sys_ver) = crate::binaries::detect::query_version(&p) {
            // Allow both exact match and prefix match (e.g. "8.3" matches "8.3.14").
            if sys_ver == version || sys_ver.starts_with(&format!("{}.", version)) {
                return Ok(p);
            }
        }
    }

    Err(ForageError::BinaryNotFound {
        name: format!("php-{}", version),
        locations: format!(
            "searched {}/, bundled resources/, PATH",
            bin_dir.display()
        ),
    })
}

/// Snapshot of all per-project overrides. Returned by the
/// `list_project_php_overrides` command for the UI.
pub fn list_project_php_overrides() -> HashMap<String, Option<String>> {
    let mut out = HashMap::new();
    let www = paths::www_dir();
    if let Ok(entries) = std::fs::read_dir(&www) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let cfg = read_project_config(&name).unwrap_or_default();
            out.insert(name, cfg.php_version);
        }
    }
    out
}
