//! System + bundled binary detection.

use crate::core::paths::{BinarySource, ResolvedBinary};
use crate::error::ForageResult;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// The Tauri resource dir, set once at startup by `lib.rs`. Bundled
/// binaries live under `<resource_dir>/bin/<platform>-<arch>/`.
static RESOURCE_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Called from `lib.rs` setup hook. Stores the Tauri resource dir so
/// `find_bundled` can use it without needing an AppHandle.
pub fn set_resource_dir(path: Option<PathBuf>) {
    let _ = RESOURCE_DIR.set(path);
}

/// Get the resource dir, if it was set.
pub fn resource_dir() -> Option<&'static PathBuf> {
    RESOURCE_DIR.get().and_then(|o| o.as_ref())
}

/// Look for `name` on PATH. Returns `None` (not an error) if not found.
pub fn find_system(name: &str) -> ForageResult<Option<ResolvedBinary>> {
    let exe = which::which(name).ok();
    Ok(exe.map(|path| ResolvedBinary {
        name: name.to_string(),
        path,
        source: BinarySource::System,
        version: None,
    }))
}

/// Look for `name` in the bundled resources directory.
///
/// Layout (set by `scripts/bundle-binaries.py`):
/// ```text
/// resources/bin/<os>-<arch>/<component>-<version>/...
/// ```
///
/// We look for the binary at:
///   - `<resource>/bin/<platform>/<component>-*/<name>`
///   - `<resource>/bin/<platform>/<component>-*/bin/<name>`
///   - `<exe_dir>/resources/bin/...` (portable / dev mode)
pub fn find_bundled(name: &str) -> ForageResult<Option<ResolvedBinary>> {
    for c in bundled_candidates(name) {
        if c.exists() {
            return Ok(Some(ResolvedBinary {
                name: name.to_string(),
                path: c,
                source: BinarySource::Bundled,
                version: None,
            }));
        }
    }
    Ok(None)
}

fn bundled_candidates(name: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();

    #[cfg(unix)]
    let exe = name.to_string();
    #[cfg(windows)]
    let exe = format!("{}.exe", name);

    let platform = platform_token();

    // 1. Tauri resource dir (set at startup).
    if let Some(rd) = resource_dir() {
        let bin_root = rd.join("bin");
        // Walk each component dir.
        if let Ok(entries) = std::fs::read_dir(&bin_root) {
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_dir() {
                    continue;
                }
                // The dir name may be "<platform>-<arch>" or just "<platform>".
                let dir_name = p.file_name().to_string_lossy().to_string();
                if dir_name.starts_with(&platform) || dir_name == "shared" {
                    push_subdirs(&mut out, &p, name, &exe);
                }
            }
        }
        // Also look at top-level resource dir for shared binaries.
        push_subdirs(&mut out, &bin_root, name, &exe);
    }

    // 2. Portable / dev mode: next to the current executable.
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            out.push(dir.join("bin").join(&exe));
            out.push(dir.join("resources").join("bin").join(&exe));
            // Also walk the resources/bin/<platform>/ tree.
            let rb = dir.join("resources").join("bin");
            if rb.exists() {
                push_subdirs(&mut out, &rb, name, &exe);
            }
        }
    }

    // 3. User-local install dir (for downloads that haven't been moved yet).
    if let Some(home) = dirs::home_dir() {
        let bin_dir = home.join(".insophinia-forage").join("bin");
        if bin_dir.exists() {
            push_subdirs(&mut out, &bin_dir, name, &exe);
        }
    }

    out
}

fn push_subdirs(out: &mut Vec<PathBuf>, parent: &PathBuf, name: &str, exe: &str) {
    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let dir = entry.path();
            // Only consider dirs named "<component>-<version>" — e.g.
            // "php-8.3.14", "mariadb-11.4.4". This avoids matching
            // unrelated subdirs.
            let dir_name = dir.file_name().to_string_lossy().to_string();
            if !dir_name.starts_with(&format!("{}-", name.split('-').next().unwrap_or(name)))
                && !dir_name.starts_with(name)
            {
                continue;
            }
            // Try common layouts.
            for candidate in [
                dir.join(exe),
                dir.join("bin").join(exe),
                dir.join("sbin").join(exe),
            ] {
                out.push(candidate);
            }
        }
    }
}

fn platform_token() -> String {
    #[cfg(target_os = "linux")]
    let os = "linux";
    #[cfg(target_os = "macos")]
    let os = "macos";
    #[cfg(target_os = "windows")]
    let os = "windows";
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let os = "unknown";

    #[cfg(target_arch = "x86_64")]
    let arch = "x86_64";
    #[cfg(target_arch = "aarch64")]
    let arch = "aarch64";
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    let arch = "unknown";

    format!("{}-{}", os, arch)
}

/// Run `<binary> --version` (or `-v` for nginx) and parse the first line.
pub fn query_version(path: &PathBuf) -> Option<String> {
    let fname = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let (flag, parser): (&str, fn(&str) -> Option<String>) = if fname.contains("nginx") {
        ("-v", |s: &str| {
            // nginx version: nginx/1.27.2
            s.split("nginx/").nth(1).map(|v| v.trim().to_string())
        })
    } else if fname.contains("php") {
        ("-v", |s: &str| {
            // PHP 8.3.12 (cli) ...
            s.split_whitespace().nth(1).map(|v| v.to_string())
        })
    } else if fname.contains("mariadb") || fname.contains("mysql") {
        ("--version", |s: &str| {
            // mariadb  Ver 15.1 Distrib 11.4.3-MariaDB ...
            s.split_whitespace()
                .find(|w| w.chars().any(|c| c.is_ascii_digit()))
                .map(|v| v.to_string())
        })
    } else {
        ("--version", |s: &str| {
            s.split_whitespace().next().map(|v| v.to_string())
        })
    };

    let out = Command::new(path).arg(flag).output().ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Some tools (nginx) print to stderr.
    let combined = if stdout.is_empty() {
        stderr.to_string()
    } else {
        stdout.to_string()
    };
    let first_line = combined.lines().next()?;
    parser(first_line)
}
