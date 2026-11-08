//! Project management commands. A "project" is just a folder under
//! `~/.insophinia-forage/www/<name>/` accessible at `http://<name>:<port>`.

use crate::core::paths;
use crate::core::AppState;
use crate::error::{ForageError, ForageResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    Plain,    // empty PHP project
    WordPress,
    Laravel,
    PhpMyAdmin,
    Custom,
}

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub kind: ProjectKind,
    pub url: String,
    pub size_bytes: u64,
    pub last_modified: Option<chrono::DateTime<chrono::Utc>>,
}

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> Vec<Project> {
    let www = paths::www_dir();
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&www) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let kind = detect_kind(&path);
            let url = state.settings.get().project_url(&name);
            let size = dir_size(&path);
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| {
                    chrono::DateTime::<chrono::Utc>::from_timestamp(d.as_secs() as i64, 0)
                        .unwrap_or_default()
                });
            out.push(Project {
                name,
                path,
                kind,
                url,
                size_bytes: size,
                last_modified: mtime,
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[tauri::command]
pub fn create_project(
    state: State<'_, AppState>,
    name: String,
    kind: ProjectKind,
) -> Result<Project, String> {
    paths::validate_name(&name).map_err(|e| e.to_string())?;
    let root = paths::project_root(&name).map_err(|e| e.to_string())?;
    if root.exists() {
        return Err(format!("project '{}' already exists", name));
    }
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;

    match kind {
        ProjectKind::Plain => {
            std::fs::write(
                root.join("index.php"),
                "<?php\nphpinfo();\n",
            )
            .map_err(|e| e.to_string())?;
        }
        ProjectKind::PhpMyAdmin => {
            return Err("phpMyAdmin is installed via the wizard, not create_project".into());
        }
        ProjectKind::WordPress | ProjectKind::Laravel | ProjectKind::Custom => {
            // Stub a placeholder. The wizard / dedicated commands handle
            // the actual scaffolding.
            std::fs::write(
                root.join("index.php"),
                format!("<?php\n// {} project — scaffold via the WordPress/Laravel tab\n", match kind {
                    ProjectKind::WordPress => "WordPress",
                    ProjectKind::Laravel => "Laravel",
                    _ => "Custom",
                }),
            )
            .map_err(|e| e.to_string())?;
        }
    }

    let url = state.settings.get().project_url(&name);
    Ok(Project {
        name,
        path: root,
        kind,
        url,
        size_bytes: 0,
        last_modified: Some(chrono::Utc::now()),
    })
}

#[tauri::command]
pub fn delete_project(name: String) -> Result<(), String> {
    paths::validate_name(&name).map_err(|e| e.to_string())?;
    let root = paths::project_root(&name).map_err(|e| e.to_string())?;
    if !root.exists() {
        return Err(format!("project '{}' not found", name));
    }
    // Safety: project_root() already validated the name and joined it onto
    // www_dir(), so this can't escape www/.
    std::fs::remove_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn open_project_in_browser(state: State<'_, AppState>, name: String) -> Result<(), String> {
    paths::validate_name(&name).map_err(|e| e.to_string())?;
    let url = state.settings.get().project_url(&name);
    open_url(&url)
}

#[tauri::command]
pub fn open_project_in_file_manager(name: String) -> Result<(), String> {
    paths::validate_name(&name).map_err(|e| e.to_string())?;
    let root = paths::project_root(&name).map_err(|e| e.to_string())?;
    if !root.exists() {
        return Err(format!("project '{}' not found", name));
    }
    open_in_file_manager(&root)
}

fn detect_kind(path: &PathBuf) -> ProjectKind {
    if path.join("wp-config.php").exists() || path.join("wp-config-sample.php").exists() {
        return ProjectKind::WordPress;
    }
    if path.join("artisan").exists() {
        return ProjectKind::Laravel;
    }
    if path.join("index.php").exists() && path.parent().map(|p| p.file_name()).flatten()
        == Some(std::ffi::OsStr::new("www"))
        && path.file_name() == Some(std::ffi::OsStr::new("pma"))
    {
        return ProjectKind::PhpMyAdmin;
    }
    if path.join("index.php").exists() {
        return ProjectKind::Plain;
    }
    ProjectKind::Custom
}

fn dir_size(path: &PathBuf) -> u64 {
    let mut total = 0;
    let mut stack = vec![path.clone()];
    while let Some(dir) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let ft = match entry.file_type() {
                    Ok(ft) => ft,
                    Err(_) => continue,
                };
                if ft.is_dir() {
                    stack.push(entry.path());
                } else if ft.is_file() {
                    if let Ok(meta) = entry.metadata() {
                        total += meta.len();
                    }
                }
            }
        }
    }
    total
}

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn open_in_file_manager(path: &PathBuf) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
