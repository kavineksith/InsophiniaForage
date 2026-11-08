//! Source-build commands. Lets the user compile PHP/nginx from official
//! source code on Linux/macOS. Useful when:
//!   - The user wants a specific version not in their distro's package manager
//!   - The user doesn't want to use third-party static builds
//!   - The user is OK with a 5–10 minute build
//!
//! The build itself does NOT require root — it installs to
//! `~/.insophinia-forage/bin/<component>-<version>-source/`. But installing
//! build deps (gcc, libxml2-dev, etc.) requires root via apt/dnf/brew.
//! We never run `sudo` from the app — we show the user the exact command
//! to copy-paste into their terminal.

use crate::core::source_build::{self, BuildDepsReport};
use tauri::AppHandle;

/// Check whether the system has the build deps required to compile PHP
/// from source. Returns a report listing what's missing and the exact
/// command to run to install them.
#[tauri::command]
pub async fn check_php_build_deps() -> Result<BuildDepsReport, String> {
    source_build::check_php_build_deps()
        .await
        .map_err(|e| e.to_string())
}

/// True if the current OS supports source compilation (Linux or macOS).
/// Windows doesn't — different toolchain, no pkg-config. The UI uses
/// this to hide the "Compile from source" option on Windows.
#[tauri::command]
pub fn is_source_compile_supported() -> bool {
    source_build::is_source_compile_supported()
}

/// Compile PHP from official source. Downloads from php.net, runs
/// `./configure && make && make install` to a user-writable prefix.
///
/// Emits `build://progress` events with `{ stage, line }` payload as
/// the build progresses. The frontend shows these in a build-progress
/// dialog.
///
/// This is a long-running operation (5–10 minutes). The frontend should
/// show a non-blocking progress dialog and allow the user to dismiss it
/// (the build continues in the background).
#[tauri::command]
pub async fn compile_php_from_source(
    app: AppHandle,
    version: String,
) -> Result<String, String> {
    let prefix = source_build::compile_php_from_source(&app, &version)
        .await
        .map_err(|e| e.to_string())?;
    Ok(prefix.to_string_lossy().into_owned())
}

/// Compile nginx from official source. Same flow as PHP but simpler
/// (fewer deps, faster build — ~1 minute).
#[tauri::command]
pub async fn compile_nginx_from_source(
    app: AppHandle,
    version: String,
) -> Result<String, String> {
    let prefix = source_build::compile_nginx_from_source(&app, &version)
        .await
        .map_err(|e| e.to_string())?;
    Ok(prefix.to_string_lossy().into_owned())
}
