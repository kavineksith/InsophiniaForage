//! Detection report — what binaries are already on the system, what
//! versions, and what the app would do for each (use system / use
//! bundled / download new / coexist).
//!
//! The report is the foundation of the conflict resolution policy:
//! before downloading anything, the app asks "what's already here?"
//! and shows the user a clear plan.

use crate::binaries::detect;
use crate::core::paths::{BinarySource, ResolvedBinary};
use crate::core::settings::{BinaryStrategy, Settings};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

/// One row in the detection report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionRow {
    /// Component name: "php", "nginx", "mariadb", "composer", "phpmyadmin".
    pub component: String,
    /// What the app would do for this component.
    pub action: PlannedAction,
    /// What was found on the system (if anything).
    pub system: Option<DetectedInstall>,
    /// What was found bundled (if anything).
    pub bundled: Option<DetectedInstall>,
    /// What was found downloaded (if anything).
    pub downloaded: Option<DetectedInstall>,
    /// Recommended version (from settings).
    pub recommended_version: String,
    /// Compatibility warnings (filled in by the compatibility checker).
    pub warnings: Vec<String>,
}

/// What the app found at a particular source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedInstall {
    pub path: String,
    pub version: Option<String>,
    /// File size in bytes (for downloaded/bundled). 0 for system.
    pub size_bytes: u64,
}

/// What the app plans to do with this component.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlannedAction {
    /// Use the system-installed binary as-is.
    UseSystem,
    /// Use the bundled binary (shipped in resources/).
    UseBundled,
    /// Use the already-downloaded binary in ~/.insophinia-forage/bin/.
    UseDownloaded,
    /// Download the binary on first run.
    Download,
    /// Nothing to do — already satisfied.
    Nothing,
    /// Skip — no portable build available for this OS.
    Skip,
}

/// Build a full detection report for all components, given the current
/// settings and binary strategy.
pub fn build_report(settings: &Settings) -> Vec<DetectionRow> {
    let mut rows = Vec::new();

    for component in ["php", "nginx", "mariadb", "composer", "phpmyadmin"] {
        let recommended = recommended_version(component, settings);
        let system = detect_system(component);
        let bundled = detect_bundled(component);
        let downloaded = detect_downloaded(component, &recommended);
        let action = plan_action(component, settings.binary_strategy, &system, &bundled, &downloaded);
        let warnings = Vec::new(); // filled in by compatibility module

        info!(
            component = %component,
            action = ?action,
            has_system = system.is_some(),
            has_bundled = bundled.is_some(),
            has_downloaded = downloaded.is_some(),
            "detection row"
        );

        rows.push(DetectionRow {
            component: component.to_string(),
            action,
            system,
            bundled,
            downloaded,
            recommended_version: recommended,
            warnings,
        });
    }

    rows
}

/// Look up the recommended version for a component from settings.
fn recommended_version(component: &str, settings: &Settings) -> String {
    match component {
        "php" => settings.versions.php.clone(),
        "nginx" => settings.versions.nginx.clone(),
        "mariadb" => settings.versions.mariadb.clone(),
        "phpmyadmin" => settings.versions.phpmyadmin.clone(),
        "composer" => "stable".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Detect a system-installed binary via PATH.
fn detect_system(component: &str) -> Option<DetectedInstall> {
    let name = match component {
        "php" => "php",
        "nginx" => "nginx",
        "mariadb" => "mariadbd", // server binary; we also check `mysqld` fallback
        "composer" => "composer",
        "phpmyadmin" => return None, // phpMyAdmin is a PHP app, not a binary
        _ => return None,
    };

    let path = which::which(name).ok().or_else(|| {
        if component == "mariadb" {
            which::which("mysqld").ok()
        } else {
            None
        }
    })?;

    let version = detect::query_version(&path);
    Some(DetectedInstall {
        path: path.to_string_lossy().into_owned(),
        version,
        size_bytes: 0,
    })
}

/// Detect a bundled binary (shipped in resources/).
fn detect_bundled(component: &str) -> Option<DetectedInstall> {
    if let Some(rb) = detect::find_bundled(component).ok().flatten() {
        let size = std::fs::metadata(&rb.path).map(|m| m.len()).unwrap_or(0);
        Some(DetectedInstall {
            path: rb.path.to_string_lossy().into_owned(),
            version: rb.version,
            size_bytes: size,
        })
    } else {
        None
    }
}

/// Detect a previously-downloaded binary in `~/.insophinia-forage/bin/`.
fn detect_downloaded(component: &str, recommended: &str) -> Option<DetectedInstall> {
    let bin_dir = crate::core::paths::bin_dir();
    if !bin_dir.exists() {
        return None;
    }

    // Look for a folder named `<component>-<version>`.
    let prefix = match component {
        "phpmyadmin" => "phpmyadmin",
        _ => component,
    };

    let mut best: Option<(PathBuf, String)> = None;
    if let Ok(entries) = std::fs::read_dir(&bin_dir) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with(prefix) {
                continue;
            }
            // Extract version from folder name: "php-8.3.14" -> "8.3.14".
            let version = name
                .strip_prefix(&format!("{}-", prefix))
                .unwrap_or("")
                .to_string();
            // Prefer the recommended version, else the highest.
            let score = if version == recommended {
                1000
            } else {
                // Naive: parse leading digits.
                version
                    .split('.')
                    .filter_map(|p| p.parse::<u32>().ok())
                    .take(3)
                    .fold(0u32, |acc, p| acc * 1000 + p)
            };
            let new_score = score;
            let old_score = best.as_ref().map(|(_, v)| {
                if v == recommended {
                    1000
                } else {
                    v.split('.')
                        .filter_map(|p| p.parse::<u32>().ok())
                        .take(3)
                        .fold(0u32, |acc, p| acc * 1000 + p)
                }
            }).unwrap_or(0);
            if new_score >= old_score {
                best = Some((entry.path(), version));
            }
        }
    }

    // Special case: composer.phar is a single file, not a folder.
    if component == "composer" {
        let phar = bin_dir.join("composer.phar");
        if phar.exists() {
            return Some(DetectedInstall {
                path: phar.to_string_lossy().into_owned(),
                version: None,
                size_bytes: std::fs::metadata(&phar).map(|m| m.len()).unwrap_or(0),
            });
        }
    }

    // Special case: phpMyAdmin lives in www/pma/, not bin/.
    if component == "phpmyadmin" {
        let pma = crate::core::paths::www_dir().join("pma").join("index.php");
        if pma.exists() {
            return Some(DetectedInstall {
                path: pma.parent().unwrap().to_string_lossy().into_owned(),
                version: None,
                size_bytes: 0,
            });
        }
        return None;
    }

    best.and_then(|(dir, version)| {
        // Find the actual binary inside.
        let exe_name = match component {
            "php" => "php",
            "nginx" => "nginx",
            "mariadb" => "mariadbd",
            _ => component,
        };
        for candidate in [
            dir.join(exe_name),
            dir.join("bin").join(exe_name),
            dir.join("sbin").join(exe_name),
        ] {
            if candidate.exists() {
                let v = detect::query_version(&candidate).or(Some(version.clone()));
                return Some(DetectedInstall {
                    path: candidate.to_string_lossy().into_owned(),
                    version: v,
                    size_bytes: std::fs::metadata(&candidate).map(|m| m.len()).unwrap_or(0),
                });
            }
        }
        None
    })
}

/// Decide what to do with a component based on strategy + what's already
/// present. This is the heart of the conflict resolution policy.
fn plan_action(
    component: &str,
    strategy: BinaryStrategy,
    system: &Option<DetectedInstall>,
    bundled: &Option<DetectedInstall>,
    downloaded: &Option<DetectedInstall>,
) -> PlannedAction {
    // nginx on Linux/macOS has no portable build — always use system.
    if component == "nginx" && bundled.is_none() && downloaded.is_none() {
        if system.is_some() {
            return PlannedAction::UseSystem;
        }
        return PlannedAction::Skip;
    }

    let order: [PlannedAction; 3] = match strategy {
        BinaryStrategy::Auto | BinaryStrategy::PreferSystem => [
            PlannedAction::UseSystem,
            PlannedAction::UseBundled,
            PlannedAction::UseDownloaded,
        ],
        BinaryStrategy::PreferBundled => [
            PlannedAction::UseBundled,
            PlannedAction::UseDownloaded,
            PlannedAction::UseSystem,
        ],
        BinaryStrategy::PreferDownload => [
            PlannedAction::UseDownloaded,
            PlannedAction::UseBundled,
            PlannedAction::UseSystem,
        ],
    };

    for action in order {
        let present = match action {
            PlannedAction::UseSystem => system.is_some(),
            PlannedAction::UseBundled => bundled.is_some(),
            PlannedAction::UseDownloaded => downloaded.is_some(),
            _ => false,
        };
        if present {
            return action;
        }
    }

    // Nothing found — we need to download.
    PlannedAction::Download
}

/// After the wizard runs, compute which actions actually need a download
/// vs. which are already satisfied. Useful for showing a "what will
/// happen" preview before the user commits.
pub fn pending_downloads(report: &[DetectionRow]) -> Vec<String> {
    report
        .iter()
        .filter(|r| r.action == PlannedAction::Download)
        .map(|r| r.component.clone())
        .collect()
}

/// Compute how much disk space the pending downloads will use (rough
/// estimate based on known sizes per component).
pub fn estimated_download_size(components: &[String]) -> u64 {
    let mut total = 0u64;
    for c in components {
        total += match c.as_str() {
            "php" => 30 * 1024 * 1024,       // 30 MB (static build)
            "nginx" => 5 * 1024 * 1024,      // 5 MB (Windows only)
            "mariadb" => 120 * 1024 * 1024,  // 120 MB
            "phpmyadmin" => 12 * 1024 * 1024, // 12 MB
            "composer" => 2 * 1024 * 1024,   // 2 MB
            _ => 0,
        };
    }
    total
}

/// Convert a planned action to a human-readable label.
pub fn action_label(action: PlannedAction) -> &'static str {
    match action {
        PlannedAction::UseSystem => "Use system install",
        PlannedAction::UseBundled => "Use bundled binary",
        PlannedAction::UseDownloaded => "Use previously downloaded",
        PlannedAction::Download => "Will download (~size shown below)",
        PlannedAction::Nothing => "Already satisfied",
        PlannedAction::Skip => "Not available for this OS",
    }
}

/// Convenience: build a report and the resolved-binary cache at once.
/// Used by the wizard's "Detection" step.
pub fn build_report_and_resolve(
    settings: &Settings,
) -> (Vec<DetectionRow>, Vec<ResolvedBinary>) {
    let report = build_report(settings);
    let resolved: Vec<ResolvedBinary> = report
        .iter()
        .filter_map(|r| {
            let path = match r.action {
                PlannedAction::UseSystem => r.system.as_ref().map(|d| d.path.clone()),
                PlannedAction::UseBundled => r.bundled.as_ref().map(|d| d.path.clone()),
                PlannedAction::UseDownloaded => r.downloaded.as_ref().map(|d| d.path.clone()),
                _ => None,
            }?;
            Some(ResolvedBinary {
                name: r.component.clone(),
                path: PathBuf::from(path),
                source: match r.action {
                    PlannedAction::UseSystem => BinarySource::System,
                    PlannedAction::UseBundled => BinarySource::Bundled,
                    PlannedAction::UseDownloaded => BinarySource::Downloaded,
                    _ => return None,
                },
                version: r
                    .system
                    .as_ref()
                    .or(r.bundled.as_ref())
                    .or(r.downloaded.as_ref())
                    .and_then(|d| d.version.clone()),
            })
        })
        .collect();
    (report, resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_action_prefers_system_in_auto_mode() {
        let system = Some(DetectedInstall {
            path: "/usr/bin/php".into(),
            version: Some("8.3.14".into()),
            size_bytes: 0,
        });
        let action = plan_action(
            "php",
            BinaryStrategy::Auto,
            &system,
            &None,
            &None,
        );
        assert_eq!(action, PlannedAction::UseSystem);
    }

    #[test]
    fn plan_action_falls_back_to_download_when_nothing_found() {
        let action = plan_action(
            "php",
            BinaryStrategy::Auto,
            &None,
            &None,
            &None,
        );
        assert_eq!(action, PlannedAction::Download);
    }

    #[test]
    fn plan_action_skips_nginx_on_linux_when_no_portable() {
        let action = plan_action(
            "nginx",
            BinaryStrategy::Auto,
            &None,
            &None,
            &None,
        );
        assert_eq!(action, PlannedAction::Skip);
    }

    #[test]
    fn estimated_download_size_is_nonzero_for_known_components() {
        let size = estimated_download_size(&["php".into(), "mariadb".into()]);
        assert!(size > 0);
    }
}
