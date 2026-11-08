//! Binary resolution. Implements the hybrid strategy:
//!
//! 1. **Detect** system-installed binaries on PATH (`/usr/bin/php`, etc).
//! 2. **Bundle** binaries shipped with the installer (resources/bin/).
//! 3. **Download** portable binaries into `~/.insophinia-forage/bin/`.
//!
//! The order is determined by the user's `BinaryStrategy` setting. By
//! default (`Auto`), we prefer system > bundled > download — this minimizes
//! download size and matches what the user already has.

pub mod detect;
pub mod download;

use crate::core::paths::{BinarySource, ResolvedBinary};
use crate::core::settings::{BinaryStrategy, Settings};
use crate::error::{ForageError, ForageResult};
use std::path::PathBuf;

/// Resolve a binary by name (e.g. "php", "nginx", "mariadbd").
///
/// `version_hint` is used for download fallback only — we'll fetch the
/// closest matching version from the download registry.
pub fn resolve(
    name: &str,
    settings: &Settings,
    version_hint: Option<&str>,
) -> ForageResult<ResolvedBinary> {
    let strategy = settings.binary_strategy;
    let candidates: [BinarySource; 3] = match strategy {
        BinaryStrategy::Auto | BinaryStrategy::PreferSystem => {
            [BinarySource::System, BinarySource::Bundled, BinarySource::Downloaded]
        }
        BinaryStrategy::PreferBundled => {
            [BinarySource::Bundled, BinarySource::System, BinarySource::Downloaded]
        }
        BinaryStrategy::PreferDownload => {
            [BinarySource::Downloaded, BinarySource::Bundled, BinarySource::System]
        }
    };

    let mut tried: Vec<String> = Vec::new();
    for source in candidates {
        let result = match source {
            BinarySource::System => detect::find_system(name),
            BinarySource::Bundled => detect::find_bundled(name),
            BinarySource::Downloaded => find_downloaded(name, version_hint),
        };
        match result {
            Ok(Some(mut b)) => {
                if let Some(v) = detect::query_version(&b.path) {
                    b.version = Some(v);
                }
                tracing::info!(
                    name = %name,
                    source = ?b.source,
                    path = %b.path.display(),
                    "resolved binary"
                );
                return Ok(b);
            }
            Ok(None) => tried.push(format!("{:?}: not found", source)),
            Err(e) => tried.push(format!("{:?}: {}", source, e)),
        }
    }
    Err(ForageError::BinaryNotFound {
        name: name.to_string(),
        locations: tried.join("; "),
    })
}

fn find_downloaded(name: &str, version_hint: Option<&str>) -> ForageResult<Option<ResolvedBinary>> {
    let bin_dir = crate::core::paths::bin_dir();
    // We store downloaded binaries in versioned folders: bin/php-8.3/php,
    // bin/nginx-1.27/nginx, bin/mariadb-11.4/bin/mariadbd.
    let prefix = match name {
        "php" | "php-fpm" => "php",
        "nginx" => "nginx",
        "mariadbd" | "mariadb" => "mariadb",
        "composer" => "composer",
        _ => name,
    };
    let mut entries: Vec<PathBuf> = Vec::new();
    if bin_dir.exists() {
        for entry in std::fs::read_dir(&bin_dir)? {
            let entry = entry?;
            let fname = entry.file_name();
            let fname = fname.to_string_lossy();
            if fname.starts_with(prefix) {
                entries.push(entry.path());
            }
        }
    }
    // Prefer the version hinted, else the highest version found.
    entries.sort_by(|a, b| b.cmp(a));
    if let Some(version_hint) = version_hint {
        if let Some(matched) = entries
            .iter()
            .find(|p| p.to_string_lossy().contains(version_hint))
        {
            return resolve_in_dir(name, matched).map(Some);
        }
    }
    if let Some(latest) = entries.first() {
        return resolve_in_dir(name, latest).map(Some);
    }
    Ok(None)
}

fn resolve_in_dir(name: &str, dir: &PathBuf) -> ForageResult<ResolvedBinary> {
    // On Windows we look for .exe; on Unix, no extension.
    #[cfg(windows)]
    let exe = format!("{}.exe", name);
    #[cfg(not(windows))]
    let exe = name.to_string();

    // Try dir/<name> first (flat layout), then dir/bin/<name>.
    let candidates = [
        dir.join(&exe),
        dir.join("bin").join(&exe),
        dir.join("sbin").join(&exe),
    ];
    for c in &candidates {
        if c.exists() {
            return Ok(ResolvedBinary {
                name: name.to_string(),
                path: c.clone(),
                source: BinarySource::Downloaded,
                version: None,
            });
        }
    }
    Ok(None)
}
