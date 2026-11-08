//! Composer runner. Runs `php composer.phar install` (or any composer
//! command) inside a project directory.

use crate::core::paths;
use crate::error::ForageResult;
use serde::Serialize;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct ComposerResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[tauri::command]
pub async fn composer_run(
    project_name: String,
    args: Vec<String>,
) -> Result<ComposerResult, String> {
    paths::validate_name(&project_name).map_err(|e| e.to_string())?;
    let project_root = paths::project_root(&project_name).map_err(|e| e.to_string())?;
    if !project_root.exists() {
        return Err(format!("project '{}' not found", project_name));
    }

    // Find PHP and composer.
    let php = find_php().map_err(|e| e.to_string())?;
    let composer = find_composer().map_err(|e| e.to_string())?;

    let mut cmd = Command::new(php);
    cmd.arg(composer).args(&args).current_dir(&project_root);
    cmd.env("COMPOSER_HOME", paths::cache_dir().join("composer"));

    let out = cmd.output().map_err(|e| e.to_string())?;
    Ok(ComposerResult {
        exit_code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

fn find_php() -> ForageResult<std::path::PathBuf> {
    let bin_dir = paths::bin_dir();
    if let Ok(entries) = std::fs::read_dir(&bin_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.file_name().map(|n| n.to_string_lossy().starts_with("php-")).unwrap_or(false) {
                for c in [path.join("php"), path.join("bin").join("php"), path.join("php.exe")] {
                    if c.exists() {
                        return Ok(c);
                    }
                }
            }
        }
    }
    if let Ok(p) = which::which("php") {
        return Ok(p);
    }
    Err(crate::error::ForageError::BinaryNotFound {
        name: "php".into(),
        locations: "searched bin/, PATH".into(),
    })
}

fn find_composer() -> ForageResult<std::path::PathBuf> {
    // Look for composer.phar in our bin dir, else system composer.
    let local = paths::bin_dir().join("composer.phar");
    if local.exists() {
        return Ok(local);
    }
    if let Ok(p) = which::which("composer") {
        return Ok(p);
    }
    Err(crate::error::ForageError::BinaryNotFound {
        name: "composer".into(),
        locations: format!("searched {}, PATH", local.display()),
    })
}
