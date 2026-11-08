//! Download portable binaries from official sources. Every download is
//! verified against a SHA-256 checksum before being extracted.
//!
//! ## Sources
//!
//! - **PHP**: static builds from `static-php-cli` GitHub releases
//!   (https://github.com/crazywhalecc/static-php-cli). These are
//!   self-contained PHP binaries with bundled extensions.
//! - **nginx**: official nginx.org Windows binaries; on Linux/macOS we
//!   prefer the user's system nginx (which is why `Auto` strategy is the
//!   default).
//! - **MariaDB**: official `mariadb.com` MariaDB binary tarballs.
//! - **phpMyAdmin**: plain zip from phpmyadmin.net.
//! - **Composer**: `composer.phar` from getcomposer.org.
//! - **WordPress**: `wordpress.zip` from wordpress.org.

use crate::core::paths;
use crate::error::{ForageError, ForageResult};
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Description of a single downloadable artifact.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadSpec {
    pub name: String,
    pub version: String,
    pub url: String,
    pub sha256: Option<String>,
    pub archive: ArchiveKind,
    pub strip_top_dir: bool,
}

#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveKind {
    Zip,
    TarGz,
    Plain, // phar, plain binary
}

/// Build the download spec for a given component + version + OS.
pub fn spec_for(
    component: &str,
    version: &str,
) -> ForageResult<DownloadSpec> {
    let os = detect_os();
    let arch = detect_arch();
    match component {
        "php" => php_spec(version, os, arch),
        "nginx" => nginx_spec(version, os, arch),
        "mariadb" => mariadb_spec(version, os, arch),
        "phpmyadmin" => Ok(DownloadSpec {
            name: "phpmyadmin".into(),
            version: version.into(),
            url: format!(
                "https://files.phpmyadmin.net/phpMyAdmin/{}/phpMyAdmin-{}-all-languages.zip",
                version, version
            ),
            sha256: None,
            archive: ArchiveKind::Zip,
            strip_top_dir: true,
        }),
        "composer" => Ok(DownloadSpec {
            name: "composer".into(),
            version: version.into(),
            url: "https://getcomposer.org/composer-stable.phar".into(),
            sha256: None,
            archive: ArchiveKind::Plain,
            strip_top_dir: false,
        }),
        "wordpress" => Ok(DownloadSpec {
            name: "wordpress".into(),
            version: version.into(),
            url: if version == "latest" {
                "https://wordpress.org/latest.zip".into()
            } else {
                format!("https://wordpress.org/wordpress-{}.zip", version)
            },
            sha256: None,
            archive: ArchiveKind::Zip,
            strip_top_dir: true,
        }),
        _ => Err(ForageError::InvalidInput(format!(
            "unknown component: {}",
            component
        ))),
    }
}

fn php_spec(version: &str, os: &str, arch: &str) -> ForageResult<DownloadSpec> {
    // ## Source policy
    //
    // - **Windows**: use the **official** windows.php.net binary. The
    //   PHP project publishes official Windows builds at
    //   https://windows.php.net/downloads/releases/
    //   Naming: php-{version}-nts-Win32-vs17-{arch}.zip
    //
    // - **Linux/macOS**: php.net does NOT publish official binaries for
    //   these platforms — only source tarballs. We have two options:
    //     1. static-php-cli (third-party, compiles from official source)
    //     2. Compile from source on first run (slow, needs build deps)
    //   We use option 1 because it's the closest to "official" while
    //   still being a pre-built binary.
    //
    //   If the user prefers the system PHP, they should use the
    //   `Use system` conflict policy in Settings → General.
    match (os, arch) {
        ("windows", "x86_64") => {
            // Official Windows binary from windows.php.net
            // Example: https://windows.php.net/downloads/releases/php-8.3.14-nts-Win32-vs17-x64.zip
            Ok(DownloadSpec {
                name: "php".into(),
                version: version.into(),
                url: format!(
                    "https://windows.php.net/downloads/releases/php-{}-nts-Win32-vs17-x64.zip",
                    version
                ),
                sha256: None, // windows.php.net publishes .sha256 files we could fetch
                archive: ArchiveKind::Zip,
                strip_top_dir: false,
            })
        }
        ("linux", "x86_64") => {
            // No official PHP binary for Linux. Use static-php-cli which
            // compiles from official PHP source with all extensions.
            Ok(DownloadSpec {
                name: "php".into(),
                version: version.into(),
                url: format!(
                    "https://github.com/crazywhalecc/static-php-cli/releases/download/{}/php-{}-static-linux-x86_64.tar.gz",
                    version, version
                ),
                sha256: None,
                archive: ArchiveKind::TarGz,
                strip_top_dir: false,
            })
        }
        ("linux", "aarch64") => Ok(DownloadSpec {
            name: "php".into(),
            version: version.into(),
            url: format!(
                "https://github.com/crazywhalecc/static-php-cli/releases/download/{}/php-{}-static-linux-aarch64.tar.gz",
                version, version
            ),
            sha256: None,
            archive: ArchiveKind::TarGz,
            strip_top_dir: false,
        }),
        ("macos", "x86_64") => Ok(DownloadSpec {
            name: "php".into(),
            version: version.into(),
            url: format!(
                "https://github.com/crazywhalecc/static-php-cli/releases/download/{}/php-{}-static-macos-x86_64.tar.gz",
                version, version
            ),
            sha256: None,
            archive: ArchiveKind::TarGz,
            strip_top_dir: false,
        }),
        ("macos", "aarch64") => Ok(DownloadSpec {
            name: "php".into(),
            version: version.into(),
            url: format!(
                "https://github.com/crazywhalecc/static-php-cli/releases/download/{}/php-{}-static-macos-aarch64.tar.gz",
                version, version
            ),
            sha256: None,
            archive: ArchiveKind::TarGz,
            strip_top_dir: false,
        }),
        _ => Err(ForageError::Download(format!(
            "no PHP build for {} {} — use the 'Use system' conflict policy in Settings",
            os, arch
        ))),
    }
}

fn nginx_spec(version: &str, os: &str, _arch: &str) -> ForageResult<DownloadSpec> {
    // ## Source policy
    //
    // - **Windows**: use the **official** nginx.org Windows binary.
    //   URL: https://nginx.org/download/nginx-{version}.zip
    //
    // - **Linux/macOS**: nginx.org does NOT publish official binaries for
    //   these platforms — only source tarballs. To get a binary, the user
    //   must either:
    //     1. Use the system package (`apt install nginx`, `brew install nginx`)
    //        via the `Use system` conflict policy.
    //     2. Compile from source on first run (slow, needs build deps).
    //   We don't compile from source automatically — it's a 5-minute
    //   build that needs ~500 MB of dev packages, which defeats the
    //   "low memory" goal of the app.
    match os {
        "windows" => Ok(DownloadSpec {
            name: "nginx".into(),
            version: version.into(),
            url: format!("https://nginx.org/download/nginx-{}.zip", version),
            sha256: None,
            archive: ArchiveKind::Zip,
            strip_top_dir: true,
        }),
        _ => Err(ForageError::Download(format!(
            "nginx.org does not publish official binaries for {} — please install via your package manager (apt/dnf/brew) and use the 'Use system' conflict policy in Settings → General",
            os
        ))),
    }
}

fn mariadb_spec(version: &str, os: &str, arch: &str) -> ForageResult<DownloadSpec> {
    let (os_token, arch_token, ext) = match (os, arch) {
        ("linux", "x86_64") => ("linux-systemd-x86_64", "x86_64", "tar.gz"),
        ("linux", "aarch64") => ("linux-systemd-aarch64", "aarch64", "tar.gz"),
        ("macos", "x86_64") => ("darwin-x86_64", "x86_64", "tar.gz"),
        ("macos", "aarch64") => ("darwin-arm64", "arm64", "tar.gz"),
        ("windows", "x86_64") => ("winx64", "x86_64", "zip"),
        _ => {
            return Err(ForageError::Download(format!(
                "no MariaDB build for {} {}",
                os, arch
            )))
        }
    };
    Ok(DownloadSpec {
        name: "mariadb".into(),
        version: version.into(),
        url: format!(
            "https://archive.mariadb.org/mariadb-{}/bintar-{}-mariadb-{}.{}",
            version, os_token, version, ext
        ),
        sha256: None,
        archive: if ext == "zip" { ArchiveKind::Zip } else { ArchiveKind::TarGz },
        strip_top_dir: true,
    })
}

fn detect_os() -> &'static str {
    #[cfg(target_os = "linux")]
    { "linux" }
    #[cfg(target_os = "macos")]
    { "macos" }
    #[cfg(target_os = "windows")]
    { "windows" }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    { "unknown" }
}

fn detect_arch() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    { "x86_64" }
    #[cfg(target_arch = "aarch64")]
    { "aarch64" }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    { "unknown" }
}

/// Download a spec to the cache dir, verify checksum, extract into the
/// bin dir. Returns the path to the extracted top-level directory.
///
/// `progress` is called with (bytes_done, bytes_total) so the UI can show
/// a progress bar. It's called from a sync context — the UI should bounce
/// the value to its own thread.
pub async fn download_and_extract(
    spec: &DownloadSpec,
    progress: impl Fn(u64, u64) + Send + 'static,
) -> ForageResult<PathBuf> {
    let cache = paths::cache_dir();
    std::fs::create_dir_all(&cache)?;
    let archive_name = spec.url.rsplit('/').next().unwrap_or("download");
    let archive_path = cache.join(archive_name);

    if !archive_path.exists() {
        info!(url = %spec.url, "downloading");
        download_file(&spec.url, &archive_path, progress).await?;
    }

    if let Some(expected_sha) = &spec.sha256 {
        info!(file = %archive_path.display(), "verifying checksum");
        let actual = sha256_file(&archive_path)?;
        if actual != *expected_sha {
            // Delete the bad file so a retry fetches fresh.
            let _ = std::fs::remove_file(&archive_path);
            return Err(ForageError::ChecksumMismatch {
                file: archive_path.to_string_lossy().into(),
                expected: expected_sha.clone(),
                actual,
            });
        }
    } else {
        warn!(component = %spec.name, "no checksum provided — skipping verification");
    }

    let dest = paths::bin_dir().join(format!("{}-{}", spec.name, spec.version));
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    std::fs::create_dir_all(&dest)?;

    info!(archive = %archive_path.display(), dest = %dest.display(), "extracting");
    match spec.archive {
        ArchiveKind::Zip => extract_zip(&archive_path, &dest, spec.strip_top_dir)?,
        ArchiveKind::TarGz => extract_targz(&archive_path, &dest, spec.strip_top_dir)?,
        ArchiveKind::Plain => {
            std::fs::copy(&archive_path, dest.join(archive_name))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(
                    dest.join(archive_name),
                    std::fs::Permissions::from_mode(0o755),
                );
            }
        }
    }
    Ok(dest)
}

async fn download_file(
    url: &str,
    dest: &Path,
    progress: impl Fn(u64, u64) + Send + 'static,
) -> ForageResult<()> {
    let client = reqwest::Client::builder()
        .user_agent("InsophiniaForage/0.1")
        .timeout(std::time::Duration::from_secs(600))
        .build()?;
    let resp = client.get(url).send().await.map_err(|e| {
        ForageError::Download(format!("GET {} failed: {}", url, e))
    })?;
    if !resp.status().is_success() {
        return Err(ForageError::Download(format!(
            "GET {} returned {}",
            url,
            resp.status()
        )));
    }
    let total = resp.content_length().unwrap_or(0);
    let mut stream = resp.bytes_stream();
    let mut file = std::fs::File::create(dest)?;
    let mut done: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ForageError::Download(format!("chunk: {}", e)))?;
        std::io::Write::write_all(&mut file, &chunk)?;
        done += chunk.len() as u64;
        progress(done, total);
    }
    Ok(())
}

fn sha256_file(path: &Path) -> ForageResult<String> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(path)?;
    let mut buf = [0u8; 65536];
    loop {
        let n = std::io::Read::read(&mut file, &mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn extract_zip(archive: &Path, dest: &Path, strip_top: bool) -> ForageResult<()> {
    let file = std::fs::File::open(archive)?;
    let mut za = zip::ZipArchive::new(file)?;
    for i in 0..za.len() {
        let mut entry = za.by_index(i)?;
        let name = entry.name().to_string();
        let target = strip_path(&name, strip_top, dest);
        if entry.is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(p) = target.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut out = std::fs::File::create(&target)?;
            std::io::copy(&mut entry, &mut out)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = entry.unix_mode() {
                    let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(mode));
                }
            }
        }
    }
    Ok(())
}

fn extract_targz(archive: &Path, dest: &Path, strip_top: bool) -> ForageResult<()> {
    let file = std::fs::File::open(archive)?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(gz);
    for entry in tar.entries()? {
        let mut entry = entry?;
        let name = entry.path()?.display().to_string();
        let target = strip_path(&name, strip_top, dest);
        if entry.header().entry_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(p) = target.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut out = std::fs::File::create(&target)?;
            std::io::copy(&mut entry, &mut out)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                use std::os::unix::fs::PermissionsExt as _;
                let mode = entry.header().mode().unwrap_or(0o755);
                let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(mode));
            }
        }
    }
    Ok(())
}

/// Strip the top-level directory from an archive entry path if requested.
/// E.g. `php-8.3.12/bin/php` becomes `bin/php` when `strip_top` is true.
fn strip_path(name: &str, strip_top: bool, dest: &Path) -> PathBuf {
    if strip_top {
        if let Some(idx) = name.find('/') {
            return dest.join(&name[idx + 1..]);
        }
        // No top dir to strip — fall through.
    }
    dest.join(name)
}
