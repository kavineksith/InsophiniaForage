//! Service commands: start/stop/status for nginx, PHP-FPM, MariaDB.

use crate::core::config_gen;
use crate::core::php_versions;
use crate::core::process::ServiceStatus;
use crate::core::spawn;
use crate::core::AppState;
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{info, warn};

#[tauri::command]
pub async fn start_service(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    info!(service = %name, "start_service command");
    do_start_service(&app, state.inner(), &name).await.map_err(|e| e.to_string())?;
    let _ = app.emit("service://status-changed", &name);
    Ok(())
}

#[tauri::command]
pub async fn stop_service(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    info!(service = %name, "stop_service command");
    if name == "all" {
        state.processes.stop_all();
        let _ = app.emit("service://status-changed", "all");
        return Ok(());
    }
    state.processes.stop(&name).map_err(|e| e.to_string())?;
    let _ = app.emit("service://status-changed", &name);
    Ok(())
}

#[tauri::command]
pub fn service_status(state: State<'_, AppState>) -> Vec<ServiceStatus> {
    state.processes.list()
}

#[tauri::command]
pub fn is_service_running(state: State<'_, AppState>, name: String) -> bool {
    state.processes.is_running(&name)
}

/// Public entry point for the tray icon's "Start all" action.
pub async fn start_all_services(app: &AppHandle) -> Result<(), crate::error::ForageError> {
    let state: tauri::State<'_, AppState> = app.state();
    let state = state.inner();
    do_start_service(app, state, "all").await?;
    let _ = app.emit("service://status-changed", "all");
    Ok(())
}

async fn do_start_service(
    app: &AppHandle,
    state: &AppState,
    name: &str,
) -> Result<(), crate::error::ForageError> {
    let valid = matches!(name, "nginx" | "php" | "mariadb" | "all");
    if !valid {
        return Err(crate::error::ForageError::InvalidInput(format!(
            "unknown service: {}",
            name
        )));
    }

    if name == "all" {
        // Order matters: DB first, then PHP (all versions), then nginx.
        if let Err(e) = do_start_service(app, state, "mariadb").await {
            warn!(service = "mariadb", error = %e, "failed during start_all");
        }
        if let Err(e) = do_start_service(app, state, "php").await {
            warn!(service = "php", error = %e, "failed during start_all");
        }
        if let Err(e) = do_start_service(app, state, "nginx").await {
            warn!(service = "nginx", error = %e, "failed during start_all");
        }
        return Ok(());
    }

    let settings = state.settings.get();
    config_gen::generate_all(&settings)?;

    if name == "mariadb" {
        if let Err(e) = spawn::init_mariadb(&settings) {
            warn!(error = %e, "mariadb init failed");
            return Err(e);
        }
    }

    if name == "php" {
        // Start one php-fpm process per active PHP version. The default
        // version is always started; overridden versions are started only
        // if at least one project uses them.
        let active_versions = php_versions::collect_active_php_versions(&settings.versions.php);
        info!(versions = ?active_versions, "starting php-fpm for versions");
        let specs = spawn::php_fpm_specs_for_versions(&settings, &active_versions)?;
        for spec in specs {
            state.processes.spawn(spec)?;
        }
        return Ok(());
    }

    let spec = match name {
        "nginx" => spawn::nginx_spec(&settings)?,
        "mariadb" => spawn::mariadb_spec(&settings)?,
        _ => unreachable!(),
    };

    state.processes.spawn(spec)?;

    if name == "mariadb" {
        // Give MariaDB a moment to bind the socket.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if let Err(e) = spawn::setup_dev_db(&settings) {
            warn!(error = %e, "dev db setup failed (will retry on next start)");
        }
    }

    Ok(())
}
