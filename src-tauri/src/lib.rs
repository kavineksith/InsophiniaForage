//! InsophiniaForage — backend library entry point.
//!
//! Tauri 2 splits the binary into a `main.rs` (thin) and `lib.rs` (real
//! logic) so the same code can be reused from tests.

mod binaries;
mod commands;
mod core;
mod error;
mod tray;

use commands::*;
use core::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging first — every subsequent step is observable.
    let _guards = core::logging::init().expect("logging init failed");
    tracing::info!("InsophiniaForage starting up");

    // Load app state. If settings.json doesn't exist, defaults are used
    // and the wizard will be shown.
    let state = match AppState::new() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "failed to initialize app state");
            std::process::exit(1);
        }
    };

    // Ensure data directories exist (idempotent).
    if let Err(e) = core::paths::ensure_dirs() {
        tracing::warn!(error = %e, "failed to ensure data dirs");
    }

    // Refresh binary cache (best-effort — services may not be installed yet).
    let _ = state.refresh_binaries();

    // Generate configs if we have settings (always — even defaults produce
    // valid configs).
    if let Err(e) = core::config_gen::generate_all(&state.settings.get()) {
        tracing::warn!(error = %e, "failed to generate initial configs");
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .manage(state)
        .setup(|app| {
            // Resolve and cache the Tauri resource dir so `find_bundled`
            // can locate bundled binaries at runtime.
            let resource_dir = app
                .path()
                .resource_dir()
                .ok();
            crate::binaries::detect::set_resource_dir(resource_dir);

            // System tray.
            if let Err(e) = tray::setup(app.handle()) {
                tracing::warn!(error = %e, "tray setup failed");
            }

            // On window close, hide instead of quitting so the tray icon
            // stays alive. We do NOT stop services on close — only on Quit.
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        // On Linux Mint + GNOME, hiding instead of closing
                        // is the standard tray-app behavior. The user can
                        // fully quit via the tray menu.
                        api.prevent_close();
                        let _ = w.hide();
                    }
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Catch the actual app-exit path ( Quit tray menu or Cmd+Q).
            if let tauri::WindowEvent::Destroyed = event {
                let state: tauri::State<'_, AppState> = window.app_handle().state();
                state.processes.stop_all();
                state.log_watcher.unsubscribe_all();
                tracing::info!("all services stopped on exit");
            }
        })
        .invoke_handler(tauri::generate_handler![
            // services
            services::start_service,
            services::stop_service,
            services::service_status,
            services::is_service_running,
            // projects
            projects::list_projects,
            projects::create_project,
            projects::delete_project,
            projects::open_project_in_browser,
            projects::open_project_in_file_manager,
            // databases
            databases::list_databases,
            databases::create_database,
            databases::drop_database,
            databases::query_database,
            databases::open_phpmyadmin,
            // wordpress
            wordpress::install_wordpress,
            // composer
            composer::composer_run,
            // settings
            settings::get_settings,
            settings::update_settings,
            settings::reset_settings,
            settings::is_initialized,
            // binaries
            binaries::list_binaries,
            binaries::download_binary,
            binaries::refresh_binaries,
            binaries::resolve_binary,
            // logs
            logs::tail_log,
            logs::subscribe_log,
            logs::unsubscribe_log,
            // php versions
            php_versions::list_php_versions,
            php_versions::get_project_php_version,
            php_versions::set_project_php_version,
            php_versions::list_project_php_overrides,
            php_versions::install_php_version,
            // detection + compatibility
            detection::get_detection_report,
            detection::check_compatibility,
            detection::check_compatibility_for,
            detection::set_conflict_policy,
            detection::would_replace_system_install,
            detection::action_label,
            detection::fetch_available_versions,
            // source compilation (Linux/macOS only)
            source_build::check_php_build_deps,
            source_build::is_source_compile_supported,
            source_build::compile_php_from_source,
            source_build::compile_nginx_from_source,
            // package manager install (apt/dnf/brew)
            package_manager::detect_package_manager,
            package_manager::get_install_plan,
            package_manager::verify_system_install,
            // wizard
            wizard::validate_wizard_input,
            wizard::save_wizard_input,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    // The line above blocks until the app exits. Drop guards here.
    drop(app);
    tracing::info!("InsophiniaForage shut down cleanly");
}
