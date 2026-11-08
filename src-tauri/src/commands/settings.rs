//! Settings commands. Get/set the user settings, regenerate configs.

use crate::core::config_gen;
use crate::core::settings::{BinaryStrategy, PhpConfig, Settings, Theme};
use crate::core::AppState;
use serde_json::Value;
use tauri::State;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.get()
}

/// Update a single top-level field. The frontend sends `{ key: "ports.nginx_http", value: 8080 }`
/// and we apply it via a deep merge. This avoids sending the whole settings
/// object on every keystroke.
#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    key: String,
    value: Value,
) -> Result<Settings, String> {
    state
        .settings
        .update(|s| {
            apply_key(s, &key, value.clone());
        })
        .map_err(|e| e.to_string())?;

    // Regenerate configs so they reflect the new settings.
    let s = state.settings.get();
    config_gen::generate_all(&s).map_err(|e| e.to_string())?;
    Ok(s)
}

/// Reset to defaults.
#[tauri::command]
pub async fn reset_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let defaults = Settings::default();
    state.settings.save(&defaults).map_err(|e| e.to_string())?;
    config_gen::generate_all(&defaults).map_err(|e| e.to_string())?;
    Ok(defaults)
}

#[tauri::command]
pub fn is_initialized() -> bool {
    crate::core::paths::is_initialized()
}

fn apply_key(s: &mut Settings, key: &str, value: Value) {
    let parts: Vec<&str> = key.split('.').collect();
    match parts.as_slice() {
        ["ports", "nginx_http"] => {
            if let Some(v) = value.as_u64() { s.ports.nginx_http = v as u16; }
        }
        ["ports", "nginx_https"] => {
            if let Some(v) = value.as_u64() { s.ports.nginx_https = v as u16; }
        }
        ["ports", "mariadb"] => {
            if let Some(v) = value.as_u64() { s.ports.mariadb = v as u16; }
        }
        ["versions", "php"] => {
            if let Some(v) = value.as_str() { s.versions.php = v.into(); }
        }
        ["versions", "mariadb"] => {
            if let Some(v) = value.as_str() { s.versions.mariadb = v.into(); }
        }
        ["versions", "nginx"] => {
            if let Some(v) = value.as_str() { s.versions.nginx = v.into(); }
        }
        ["versions", "phpmyadmin"] => {
            if let Some(v) = value.as_str() { s.versions.phpmyadmin = v.into(); }
        }
        ["versions", "wordpress"] => {
            if let Some(v) = value.as_str() { s.versions.wordpress = v.into(); }
        }
        ["php", "memory_limit"] => {
            if let Some(v) = value.as_str() { s.php.memory_limit = v.into(); }
        }
        ["php", "upload_max_filesize"] => {
            if let Some(v) = value.as_str() { s.php.upload_max_filesize = v.into(); }
        }
        ["php", "post_max_size"] => {
            if let Some(v) = value.as_str() { s.php.post_max_size = v.into(); }
        }
        ["php", "max_execution_time"] => {
            if let Some(v) = value.as_u64() { s.php.max_execution_time = v as u32; }
        }
        ["php", "display_errors"] => {
            if let Some(v) = value.as_bool() { s.php.display_errors = v; }
        }
        ["mariadb", "innodb_buffer_pool_size"] => {
            if let Some(v) = value.as_str() { s.mariadb.innodb_buffer_pool_size = v.into(); }
        }
        ["mariadb", "max_connections"] => {
            if let Some(v) = value.as_u64() { s.mariadb.max_connections = v as u32; }
        }
        ["nginx", "client_max_body_size"] => {
            if let Some(v) = value.as_str() { s.nginx.client_max_body_size = v.into(); }
        }
        ["nginx", "worker_processes"] => {
            if let Some(v) = value.as_str() { s.nginx.worker_processes = v.into(); }
        }
        ["binary_strategy"] => {
            if let Some(v) = value.as_str() {
                s.binary_strategy = match v {
                    "auto" => BinaryStrategy::Auto,
                    "prefer_system" => BinaryStrategy::PreferSystem,
                    "prefer_bundled" => BinaryStrategy::PreferBundled,
                    "prefer_download" => BinaryStrategy::PreferDownload,
                    _ => BinaryStrategy::Auto,
                };
            }
        }
        ["conflict_policy"] => {
            if let Some(v) = value.as_str() {
                s.conflict_policy = match v {
                    "coexist" => crate::core::settings::ConflictPolicy::Coexist,
                    "use_system" => crate::core::settings::ConflictPolicy::UseSystem,
                    "replace" => crate::core::settings::ConflictPolicy::Replace,
                    _ => crate::core::settings::ConflictPolicy::Coexist,
                };
            }
        }
        ["theme"] => {
            if let Some(v) = value.as_str() {
                s.theme = match v {
                    "light" => Theme::Light,
                    "dark" => Theme::Dark,
                    _ => Theme::Auto,
                };
            }
        }
        ["auto_start_services"] => {
            if let Some(v) = value.as_bool() { s.auto_start_services = v; }
        }
        ["open_browser_on_start"] => {
            if let Some(v) = value.as_bool() { s.open_browser_on_start = v; }
        }
        _ => {
            tracing::warn!(key = %key, "unknown settings key, ignored");
        }
    }
}
