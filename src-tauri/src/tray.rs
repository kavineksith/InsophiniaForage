//! System tray icon with Start All / Stop All / Open Panel / Quit.

use crate::core::AppState;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, State,
};

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let start_all = MenuItem::with_id(app, "start_all", "Start all", true, None::<&str>)?;
    let stop_all = MenuItem::with_id(app, "stop_all", "Stop all", true, None::<&str>)?;
    let open_panel = MenuItem::with_id(app, "open_panel", "Open panel", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&start_all, &stop_all, &open_panel, &separator, &quit])?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("InsophiniaForage")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "start_all" => {
                let app_clone = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = crate::commands::services::start_all_services(&app_clone).await {
                        tracing::warn!(error = %e, "tray start_all failed");
                    }
                });
            }
            "stop_all" => {
                let state: State<'_, AppState> = app.state();
                state.processes.stop_all();
            }
            "open_panel" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                let state: State<'_, AppState> = app.state();
                state.processes.stop_all();
                state.log_watcher.unsubscribe_all();
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
