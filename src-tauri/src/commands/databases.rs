//! Database management commands. We shell out to the bundled/system
//! `mariadb` client — this avoids pulling a MySQL driver dependency and
//! keeps the binary small.

use crate::core::paths;
use crate::core::AppState;
use crate::error::{ForageError, ForageResult};
use serde::Serialize;
use std::process::Command;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct Database {
    pub name: String,
    pub tables_count: u64,
    pub size_bytes: u64,
}

#[tauri::command]
pub fn list_databases(state: State<'_, AppState>) -> Result<Vec<Database>, String> {
    let client = find_mariadb_client().map_err(|e| e.to_string())?;
    let socket = paths::run_dir().join("mariadb.sock");
    let out = Command::new(&client)
        .arg("--socket").arg(&socket)
        .arg("--user").arg(&state.settings.get().mariadb.app_user)
        .arg("--password").arg(&state.settings.get().mariadb.app_password)
        .arg("--batch")
        .arg("--skip-column-names")
        .arg("-e")
        .arg("SELECT schema_name FROM information_schema.schemata WHERE schema_name NOT IN ('information_schema','performance_schema','mysql','sys')")
        .output()
        .map_err(|e| ForageError::Database(format!("mariadb client failed: {}", e)))
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "list_databases failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut dbs = Vec::new();
    for line in stdout.lines() {
        let name = line.trim().to_string();
        if name.is_empty() {
            continue;
        }
        let (tables, size) = query_db_stats(&client, &socket, &state, &name).unwrap_or((0, 0));
        dbs.push(Database {
            name,
            tables_count: tables,
            size_bytes: size,
        });
    }
    Ok(dbs)
}

#[tauri::command]
pub fn create_database(state: State<'_, AppState>, name: String) -> Result<(), String> {
    paths::validate_name(&name).map_err(|e| e.to_string())?;
    let client = find_mariadb_client().map_err(|e| e.to_string())?;
    let socket = paths::run_dir().join("mariadb.sock");
    let s = state.settings.get();
    let sql = format!(
        "CREATE DATABASE IF NOT EXISTS `{}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;",
        name
    );
    run_sql(&client, &socket, &s.mariadb.app_user, &s.mariadb.app_password, &sql)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn drop_database(state: State<'_, AppState>, name: String) -> Result<(), String> {
    paths::validate_name(&name).map_err(|e| e.to_string())?;
    let client = find_mariadb_client().map_err(|e| e.to_string())?;
    let socket = paths::run_dir().join("mariadb.sock");
    let s = state.settings.get();
    let sql = format!("DROP DATABASE IF EXISTS `{}`;", name);
    run_sql(&client, &socket, &s.mariadb.app_user, &s.mariadb.app_password, &sql)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn query_database(
    state: State<'_, AppState>,
    db: String,
    sql: String,
) -> Result<String, String> {
    paths::validate_name(&db).map_err(|e| e.to_string())?;
    let client = find_mariadb_client().map_err(|e| e.to_string())?;
    let socket = paths::run_dir().join("mariadb.sock");
    let s = state.settings.get();
    let out = Command::new(&client)
        .arg("--socket").arg(&socket)
        .arg("--user").arg(&s.mariadb.app_user)
        .arg("--password").arg(&s.mariadb.app_password)
        .arg("--database").arg(&db)
        .arg("--batch")
        .arg("--table")
        .arg("-e").arg(&sql)
        .output()
        .map_err(|e| ForageError::Database(format!("mariadb client failed: {}", e)))
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[tauri::command]
pub fn open_phpmyadmin(state: State<'_, AppState>) -> Result<(), String> {
    let url = state.settings.get().phpmyadmin_url();
    crate::commands::projects::open_project_in_browser(state, "pma".into())
        .or_else(|_| {
            // Fall back to direct URL open if pma project doesn't exist.
            open_url(&url)
        })
}

fn find_mariadb_client() -> ForageResult<std::path::PathBuf> {
    // Check download dir.
    let bin_dir = paths::bin_dir();
    if let Ok(entries) = std::fs::read_dir(&bin_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let candidates = [
                    path.join("bin").join("mariadb"),
                    path.join("bin").join("mariadb.exe"),
                    path.join("bin").join("mysql"),
                    path.join("bin").join("mysql.exe"),
                ];
                for c in candidates {
                    if c.exists() {
                        return Ok(c);
                    }
                }
            }
        }
    }
    if let Ok(p) = which::which("mariadb") {
        return Ok(p);
    }
    if let Ok(p) = which::which("mysql") {
        return Ok(p);
    }
    Err(ForageError::BinaryNotFound {
        name: "mariadb".into(),
        locations: "searched bin/, PATH".into(),
    })
}

fn run_sql(
    client: &std::path::Path,
    socket: &std::path::Path,
    user: &str,
    pass: &str,
    sql: &str,
) -> ForageResult<()> {
    let out = Command::new(client)
        .arg("--socket").arg(socket)
        .arg("--user").arg(user)
        .arg("--password").arg(pass)
        .arg("-e").arg(sql)
        .output()?;
    if !out.status.success() {
        return Err(ForageError::Database(format!(
            "SQL failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(())
}

fn query_db_stats(
    client: &std::path::Path,
    socket: &std::path::Path,
    state: &AppState,
    db: &str,
) -> ForageResult<(u64, u64)> {
    let s = state.settings.get();
    let sql = format!(
        "SELECT COUNT(*), COALESCE(SUM(data_length + index_length), 0) FROM information_schema.tables WHERE table_schema = '{}';",
        db
    );
    let out = Command::new(client)
        .arg("--socket").arg(socket)
        .arg("--user").arg(&s.mariadb.app_user)
        .arg("--password").arg(&s.mariadb.app_password)
        .arg("--batch")
        .arg("--skip-column-names")
        .arg("-e").arg(&sql)
        .output()?;
    if !out.status.success() {
        return Err(ForageError::Database(format!(
            "stats failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    let line = String::from_utf8_lossy(&out.stdout);
    let mut parts = line.trim().split('\t');
    let tables: u64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let size: u64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    Ok((tables, size))
}

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}
