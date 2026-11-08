//! PHP version management commands. Per-project overrides + listing
//! installed versions + installing new versions.

use crate::core::php_versions;
use crate::core::AppState;
use serde::Serialize;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Serialize)]
pub struct PhpVersionInfo {
    pub version: String,
    pub installed: bool,
    pub path: Option<String>,
    pub source: Option<String>,
}

#[tauri::command]
pub fn list_php_versions() -> Vec<PhpVersionInfo> {
    let installed = php_versions::list_installed_php_versions();
    let known = ["8.1", "8.2", "8.3", "8.4"];

    let mut out = Vec::new();
    for v in installed {
        let path = php_versions::find_php_binary_for_version(&v).ok();
        let source = path.as_ref().and_then(|p| {
            let s = p.to_string_lossy();
            if s.contains(".insophinia-forage") {
                Some("downloaded".to_string())
            } else if s.starts_with("/usr") || s.starts_with("/opt") {
                Some("system".to_string())
            } else {
                Some("bundled".to_string())
            }
        });
        out.push(PhpVersionInfo {
            version: v.clone(),
            installed: true,
            path: path.map(|p| p.to_string_lossy().into_owned()),
            source,
        });
    }

    // Add known-but-not-installed versions.
    for v in known {
        if !out.iter().any(|i| i.version == v) {
            out.push(PhpVersionInfo {
                version: v.into(),
                installed: false,
                path: None,
                source: None,
            });
        }
    }

    // Sort by version descending.
    out.sort_by(|a, b| b.version.cmp(&a.version));
    out
}

#[tauri::command]
pub fn get_project_php_version(project_name: String) -> Result<Option<String>, String> {
    let cfg = php_versions::read_project_config(&project_name).map_err(|e| e.to_string())?;
    Ok(cfg.php_version)
}

#[tauri::command]
pub fn set_project_php_version(
    state: State<'_, AppState>,
    project_name: String,
    version: Option<String>,
) -> Result<(), String> {
    php_versions::set_php_version(&project_name, version.as_deref()).map_err(|e| e.to_string())?;

    // Regenerate configs so the change takes effect on next service start.
    let settings = state.settings.get();
    crate::core::config_gen::generate_all(&settings).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn list_project_php_overrides() -> HashMap<String, Option<String>> {
    php_versions::list_project_php_overrides()
}

/// Download and install a new PHP version. Wraps the existing
/// `download_binary` flow but ensures the version is registered.
#[tauri::command]
pub async fn install_php_version(
    app: AppHandle,
    state: State<'_, AppState>,
    version: String,
) -> Result<(), String> {
    let spec = crate::binaries::download::spec_for("php", &version).map_err(|e| e.to_string())?;
    let app_clone = app.clone();
    let v_clone = version.clone();
    crate::binaries::download::download_and_extract(&spec, move |done, total| {
        let _ = app_clone.emit(
            "binary://download-progress",
            serde_json::json!({ "name": format!("php-{}", v_clone), "done": done, "total": total }),
        );
    })
    .await
    .map_err(|e| e.to_string())?;

    state.refresh_binaries().map_err(|e| e.to_string())?;
    let _ = app.emit("binary://refreshed", "php");
    Ok(())
}
