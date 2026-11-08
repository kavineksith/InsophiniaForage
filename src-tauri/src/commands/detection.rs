//! Detection + compatibility commands. Used by the wizard's "Detection"
//! step and the Settings page's compatibility checker.

use crate::core::compatibility::{self, CompatibilityIssue};
use crate::core::detection::{self, DetectionRow, PlannedAction};
use crate::core::latest_versions::{self, AvailableVersions};
use crate::core::settings::ConflictPolicy;
use crate::core::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct DetectionReport {
    pub rows: Vec<DetectionRow>,
    pub pending_downloads: Vec<String>,
    pub estimated_download_bytes: u64,
    pub conflict_policy: ConflictPolicy,
    pub compatibility_issues: Vec<CompatibilityIssue>,
}

/// Build a fresh detection report. Called by the wizard before saving,
/// and by Settings to refresh the display.
#[tauri::command]
pub fn get_detection_report(state: State<'_, AppState>) -> Result<DetectionReport, String> {
    let settings = state.settings.get();
    let rows = detection::build_report(&settings);
    let pending = detection::pending_downloads(&rows);
    let size = detection::estimated_download_size(&pending);
    let issues = compatibility::validate_settings(&settings);

    Ok(DetectionReport {
        rows,
        pending_downloads: pending,
        estimated_download_bytes: size,
        conflict_policy: settings.conflict_policy,
        compatibility_issues: issues,
    })
}

/// Run only the compatibility checks against the current settings. Used
/// by the Settings page to show warnings/errors in real time.
#[tauri::command]
pub fn check_compatibility(state: State<'_, AppState>) -> Vec<CompatibilityIssue> {
    let settings = state.settings.get();
    compatibility::validate_settings(&settings)
}

/// Run compatibility checks against a hypothetical settings change.
/// The wizard uses this to validate before saving — pass the proposed
/// versions, get back issues.
#[tauri::command]
pub fn check_compatibility_for(
    state: State<'_, AppState>,
    php: String,
    mariadb: String,
    nginx: String,
    phpmyadmin: String,
    wordpress: String,
) -> Vec<CompatibilityIssue> {
    let mut s = state.settings.get();
    s.versions.php = php;
    s.versions.mariadb = mariadb;
    s.versions.nginx = nginx;
    s.versions.phpmyadmin = phpmyadmin;
    s.versions.wordpress = wordpress;
    compatibility::validate_settings(&s)
}

/// Set the conflict policy. Called from Settings → General.
#[tauri::command]
pub async fn set_conflict_policy(
    state: State<'_, AppState>,
    policy: String,
) -> Result<(), String> {
    let p = match policy.as_str() {
        "coexist" => ConflictPolicy::Coexist,
        "use_system" => ConflictPolicy::UseSystem,
        "replace" => ConflictPolicy::Replace,
        _ => return Err(format!("unknown conflict policy: {}", policy)),
    };
    state
        .settings
        .update(|s| s.conflict_policy = p)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// True if the conflict policy is `Replace` AND a system install was
/// detected for the given component. The UI uses this to require explicit
/// confirmation before allowing a download that would replace a system
/// package.
#[tauri::command]
pub fn would_replace_system_install(
    state: State<'_, AppState>,
    component: String,
) -> bool {
    let settings = state.settings.get();
    if settings.conflict_policy != ConflictPolicy::Replace {
        return false;
    }
    let report = detection::build_report(&settings);
    report
        .iter()
        .find(|r| r.component == component)
        .map(|r| r.system.is_some())
        .unwrap_or(false)
}

/// Convenience: human-readable label for a planned action.
#[tauri::command]
pub fn action_label(action: PlannedAction) -> String {
    detection::action_label(action).to_string()
}

/// Fetch the latest available versions from official sources (php.net,
/// nginx.org, mariadb.org, phpmyadmin.net, wordpress.org). Best-effort —
/// on network failure, returns the hardcoded fallback lists.
///
/// The wizard and Settings page call this on mount to populate the
/// version dropdowns dynamically.
#[tauri::command]
pub async fn fetch_available_versions() -> Result<AvailableVersions, String> {
    tracing::info!("fetching available versions from official sources");
    let result = latest_versions::fetch_available_versions().await;
    if !result.fetched {
        tracing::warn!(
            error = ?result.error,
            "version fetch failed — using hardcoded fallbacks"
        );
    } else {
        tracing::info!(
            php = ?result.php,
            nginx = ?result.nginx,
            mariadb = ?result.mariadb,
            phpmyadmin = ?result.phpmyadmin,
            wordpress = ?result.wordpress,
            "fetched available versions"
        );
    }
    Ok(result)
}
