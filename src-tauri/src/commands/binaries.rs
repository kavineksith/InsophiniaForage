//! Binary management commands. Resolve, list, download.

use crate::binaries::{self, download};
use crate::core::AppState;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Serialize)]
pub struct BinaryStatus {
    pub name: String,
    pub resolved: bool,
    pub path: Option<String>,
    pub source: Option<String>,
    pub version: Option<String>,
    pub download_url: Option<String>,
    pub available_versions: Vec<String>,
}

#[tauri::command]
pub fn list_binaries(state: State<'_, AppState>) -> Vec<BinaryStatus> {
    let cached = state.binaries();
    vec![
        binary_status("php", cached.php.clone()),
        binary_status("nginx", cached.nginx.clone()),
        binary_status("mariadb", cached.mariadb.clone()),
        binary_status("composer", cached.composer.clone()),
    ]
}

fn binary_status(
    name: &str,
    resolved: Option<crate::core::paths::ResolvedBinary>,
) -> BinaryStatus {
    BinaryStatus {
        name: name.to_string(),
        resolved: resolved.is_some(),
        path: resolved.as_ref().map(|b| b.path.to_string_lossy().into_owned()),
        source: resolved.as_ref().map(|b| format!("{:?}", b.source).to_lowercase()),
        version: resolved.as_ref().and_then(|b| b.version.clone()),
        download_url: None,
        available_versions: available_versions_for(name),
    }
}

fn available_versions_for(name: &str) -> Vec<String> {
    match name {
        "php" => vec!["8.4".into(), "8.3".into(), "8.2".into(), "8.1".into()],
        "nginx" => vec!["1.27".into(), "1.26".into()],
        "mariadb" => vec!["11.6".into(), "11.4".into(), "10.11".into()],
        "composer" => vec!["stable".into(), "preview".into()],
        _ => vec![],
    }
}

#[tauri::command]
pub async fn download_binary(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    version: String,
) -> Result<(), String> {
    let spec = download::spec_for(&name, &version).map_err(|e| e.to_string())?;
    let app_handle = app.clone();
    let _ = download::download_and_extract(&spec, move |done, total| {
        let _ = app_handle.emit(
            "binary://download-progress",
            serde_json::json!({ "name": &name, "done": done, "total": total }),
        );
    })
    .await
    .map_err(|e| e.to_string())?;

    // Refresh the cached binary list.
    state.refresh_binaries().map_err(|e| e.to_string())?;
    let _ = app.emit("binary://refreshed", &name);
    Ok(())
}

#[tauri::command]
pub fn refresh_binaries(state: State<'_, AppState>) -> Result<(), String> {
    state.refresh_binaries().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn resolve_binary(state: State<'_, AppState>, name: String) -> Result<crate::core::paths::ResolvedBinary, String> {
    let s = state.settings.get();
    let version_hint = match name.as_str() {
        "php" => Some(s.versions.php.as_str()),
        "nginx" => Some(s.versions.nginx.as_str()),
        "mariadb" | "mariadbd" => Some(s.versions.mariadb.as_str()),
        _ => None,
    };
    binaries::resolve(&name, &s, version_hint).map_err(|e| e.to_string())
}
