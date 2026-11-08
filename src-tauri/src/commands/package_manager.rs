//! Package manager install commands. Lets the user install PHP/nginx
//! from official package manager sources (apt, dnf, brew) via copy-paste
//! commands. The app never runs `sudo` — it generates the exact commands
//! for the user to run in their terminal.

use crate::core::package_manager::{self, InstallPlan, PackageManager};

/// Detect which package manager is on the system.
#[tauri::command]
pub fn detect_package_manager() -> PackageManager {
    package_manager::detect_package_manager()
}

/// Build the install plan for a component+version on the current system.
/// Returns the ordered list of shell commands to run, plus notes.
#[tauri::command]
pub fn get_install_plan(component: String, version: String) -> Result<InstallPlan, String> {
    package_manager::build_install_plan(&component, &version)
}

/// After the user runs the install commands in their terminal, this
/// verifies the binary is now on the system. Returns the path if found.
#[tauri::command]
pub fn verify_system_install(component: String, version: String) -> Option<String> {
    package_manager::verify_system_install(&component, &version)
        .map(|p| p.to_string_lossy().into_owned())
}
