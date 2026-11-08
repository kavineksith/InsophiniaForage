//! Compile PHP and nginx from official source code. Installs to a
//! user-writable directory under `~/.insophinia-forage/bin/` — no root
//! required for the build itself.
//!
//! ## When to use this
//!
//! - Linux/macOS user wants a specific PHP/nginx version that's not in
//!   their distro's package manager.
//! - User doesn't want to use the third-party `static-php-cli` builds.
//! - User is OK with a 5–10 minute build on first run.
//!
//! ## What requires root
//!
//! - **Installing build deps** (`gcc`, `make`, `libxml2-dev`, etc.) —
//!   requires root, ONE TIME ONLY. The app shows the exact command to
//!   run; the user runs it themselves. We never run `sudo`.
//! - **The actual compile** — does NOT require root. We install to
//!   `~/.insophinia-forage/bin/php-<version>-source/`, which is user-writable.
//!
//! ## Flow
//!
//! 1. `check_build_deps()` — verify gcc/make/pkg-config + required libs
//!    are installed. If missing, return the exact apt/dnf/brew command.
//! 2. `compile_php(version)` — download source from php.net, extract,
//!    `./configure --prefix=...`, `make`, `make install`. Stream stdout
//!    + stderr to the UI via events.
//! 3. The compiled binary lands at
//!    `~/.insophinia-forage/bin/php-<version>-source/bin/php`.
//! 4. The hybrid resolver picks it up on next refresh.

use crate::core::paths;
use crate::error::{ForageError, ForageResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{info, warn};

/// Result of a build-deps check. Tells the UI what's missing and what
/// command to run to fix it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildDepsReport {
    /// True if all required tools and libraries are present.
    pub all_present: bool,
    /// Missing tools (gcc, make, pkg-config, etc.).
    pub missing_tools: Vec<String>,
    /// Missing libraries (libxml2, libcurl, etc.) — checked via pkg-config.
    pub missing_libs: Vec<String>,
    /// The exact command the user should run to install the missing deps.
    /// Different per OS / package manager.
    pub install_command: String,
    /// Which OS / package manager we detected.
    pub package_manager: String,
}

/// Check whether the system has the build deps required to compile PHP
/// from source. Returns a report with the exact install command if not.
pub async fn check_php_build_deps() -> ForageResult<BuildDepsReport> {
    let mut missing_tools = Vec::new();
    let mut missing_libs = Vec::new();

    // 1. Required tools.
    for tool in ["gcc", "make", "pkg-config", "autoconf", "bison"] {
        if which::which(tool).is_err() {
            missing_tools.push(tool.to_string());
        }
    }

    // 2. Required libraries (via pkg-config).
    for lib in [
        "libxml-2.0",
        "sqlite3",
        "zlib",
        "libcurl",
        "openssl",
        "libpng",
        "libpcre2-8",
    ] {
        let result = Command::new("pkg-config")
            .args(["--exists", lib])
            .output()
            .await;
        match result {
            Ok(out) => {
                if !out.status.success() {
                    missing_libs.push(lib.to_string());
                }
            }
            Err(_) => {
                // pkg-config itself missing — already in missing_tools.
            }
        }
    }

    // 3. Detect package manager and produce the install command.
    let (package_manager, install_command) = detect_package_manager_and_install_command(
        &missing_tools,
        &missing_libs,
    );

    let all_present = missing_tools.is_empty() && missing_libs.is_empty();

    Ok(BuildDepsReport {
        all_present,
        missing_tools,
        missing_libs,
        install_command,
        package_manager,
    })
}

/// Detect the package manager and produce the exact install command for
/// the missing build deps.
///
/// The command is shown to the user — they copy-paste it into their
/// terminal. We never run `sudo` from the app.
fn detect_package_manager_and_install_command(
    missing_tools: &[String],
    missing_libs: &[String],
) -> (String, String) {
    // Map pkg-config library names to package names per distro.
    let apt_libs = map_libs_to_packages(missing_libs, "apt");
    let dnf_libs = map_libs_to_packages(missing_libs, "dnf");
    let brew_libs = map_libs_to_packages(missing_libs, "brew");

    // Detect package manager. Order matters: check apt first (most common
    // on Linux Mint/Ubuntu), then dnf (Fedora), then brew (macOS).
    if std::path::Path::new("/usr/bin/apt").exists() {
        let mut pkgs: Vec<String> = Vec::new();
        pkgs.extend(missing_tools.iter().map(|t| match t.as_str() {
            "gcc" => "gcc".to_string(),
            "make" => "make".to_string(),
            "pkg-config" => "pkg-config".to_string(),
            "autoconf" => "autoconf".to_string(),
            "bison" => "bison".to_string(),
            _ => t.clone(),
        }));
        pkgs.extend(apt_libs);
        let cmd = format!("sudo apt update && sudo apt install -y {}", pkgs.join(" "));
        return ("apt".into(), cmd);
    }

    if std::path::Path::new("/usr/bin/dnf").exists() {
        let mut pkgs: Vec<String> = Vec::new();
        pkgs.extend(missing_tools.iter().map(|t| match t.as_str() {
            "gcc" => "gcc".to_string(),
            "make" => "make".to_string(),
            "pkg-config" => "pkgconf-pkg-config".to_string(),
            "autoconf" => "autoconf".to_string(),
            "bison" => "bison".to_string(),
            _ => t.clone(),
        }));
        pkgs.extend(dnf_libs);
        let cmd = format!("sudo dnf install -y {}", pkgs.join(" "));
        return ("dnf".into(), cmd);
    }

    if std::path::Path::new("/opt/homebrew/bin/brew").exists()
        || std::path::Path::new("/usr/local/bin/brew").exists()
    {
        let mut pkgs: Vec<String> = Vec::new();
        pkgs.extend(missing_tools.iter().map(|t| match t.as_str() {
            "gcc" => "gcc".to_string(),
            "make" => "make".to_string(),
            "pkg-config" => "pkg-config".to_string(),
            "autoconf" => "autoconf".to_string(),
            "bison" => "bison".to_string(),
            _ => t.clone(),
        }));
        pkgs.extend(brew_libs);
        let cmd = format!("brew install {}", pkgs.join(" "));
        return ("brew".into(), cmd);
    }

    ("unknown".into(), String::new())
}

/// Map pkg-config library names to package manager package names.
fn map_libs_to_packages(libs: &[String], pm: &str) -> Vec<String> {
    libs.iter()
        .filter_map(|lib| match (lib.as_str(), pm) {
            ("libxml-2.0", "apt") => Some("libxml2-dev".into()),
            ("libxml-2.0", "dnf") => Some("libxml2-devel".into()),
            ("libxml-2.0", "brew") => Some("libxml2".into()),

            ("sqlite3", "apt") => Some("libsqlite3-dev".into()),
            ("sqlite3", "dnf") => Some("sqlite-devel".into()),
            ("sqlite3", "brew") => Some("sqlite".into()),

            ("zlib", "apt") => Some("zlib1g-dev".into()),
            ("zlib", "dnf") => Some("zlib-devel".into()),
            ("zlib", "brew") => Some("zlib".into()),

            ("libcurl", "apt") => Some("libcurl4-openssl-dev".into()),
            ("libcurl", "dnf") => Some("libcurl-devel".into()),
            ("libcurl", "brew") => Some("curl".into()),

            ("openssl", "apt") => Some("libssl-dev".into()),
            ("openssl", "dnf") => Some("openssl-devel".into()),
            ("openssl", "brew") => Some("openssl@3".into()),

            ("libpng", "apt") => Some("libpng-dev".into()),
            ("libpng", "dnf") => Some("libpng-devel".into()),
            ("libpng", "brew") => Some("libpng".into()),

            ("libpcre2-8", "apt") => Some("libpcre2-dev".into()),
            ("libpcre2-8", "dnf") => Some("pcre2-devel".into()),
            ("libpcre2-8", "brew") => Some("pcre2".into()),

            _ => None,
        })
        .collect()
}

/// Compile PHP from official source. Downloads the source tarball,
/// extracts, runs `./configure && make && make install` to a
/// user-writable prefix under `~/.insophinia-forage/bin/`.
///
/// Progress is emitted to the frontend via `build://progress` events
/// with `{ stage, line }` payload. The frontend shows these in a
/// build-progress dialog.
///
/// This is a long-running operation (5–10 minutes). The frontend should
/// call it from a non-blocking context.
pub async fn compile_php_from_source(
    app: &AppHandle,
    version: &str,
) -> ForageResult<PathBuf> {
    // Validate version format (defense against path traversal).
    if !version
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.')
        || version.is_empty()
    {
        return Err(ForageError::InvalidInput(format!(
            "invalid PHP version: {}",
            version
        )));
    }

    emit_progress(app, "init", &format!("Compiling PHP {} from source…", version));

    // 1. Check build deps.
    emit_progress(app, "check-deps", "Checking build dependencies…");
    let report = check_php_build_deps().await?;
    if !report.all_present {
        return Err(ForageError::Config(format!(
            "Build deps missing: {}. Run this command first:\n\n{}",
            report
                .missing_tools
                .iter()
                .chain(report.missing_libs.iter())
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            report.install_command
        )));
    }
    emit_progress(app, "check-deps", "✓ All build deps present");

    // 2. Download source tarball from php.net.
    let url = format!("https://www.php.net/distributions/php-{}.tar.gz", version);
    let cache_dir = paths::cache_dir().join("source");
    std::fs::create_dir_all(&cache_dir)?;
    let archive_path = cache_dir.join(format!("php-{}.tar.gz", version));

    if !archive_path.exists() {
        emit_progress(app, "download", &format!("Downloading {}…", url));
        download_file(&url, &archive_path).await?;
        emit_progress(
            app,
            "download",
            &format!("✓ Downloaded ({} MB)", archive_path.metadata()?.len() / 1024 / 1024),
        );
    } else {
        emit_progress(app, "download", "✓ Using cached source tarball");
    }

    // 3. Extract to a build dir.
    let build_dir = cache_dir.join(format!("php-{}-build", version));
    if build_dir.exists() {
        std::fs::remove_dir_all(&build_dir)?;
    }
    std::fs::create_dir_all(&build_dir)?;
    emit_progress(app, "extract", &format!("Extracting to {}…", build_dir.display()));
    extract_targz(&archive_path, &build_dir, true)?;
    emit_progress(app, "extract", "✓ Extracted");

    // The extracted dir is named `php-<version>`.
    let src_dir = build_dir.join(format!("php-{}", version));
    if !src_dir.exists() {
        return Err(ForageError::Config(format!(
            "expected source dir {} not found after extraction",
            src_dir.display()
        )));
    }

    // 4. Configure install prefix.
    let prefix = paths::bin_dir().join(format!("php-{}-source", version));
    if prefix.exists() {
        emit_progress(
            app,
            "init",
            &format!("Removing previous build at {}…", prefix.display()),
        );
        std::fs::remove_dir_all(&prefix)?;
    }
    std::fs::create_dir_all(&prefix)?;

    // 5. Run ./configure.
    let configure_args = vec![
        format!("--prefix={}", prefix.display()),
        "--enable-fpm".into(),
        "--with-fpm-user=".into(),  // empty = current user
        "--with-fpm-group=".into(),
        "--enable-mbstring".into(),
        "--enable-xml".into(),
        "--enable-bcmath".into(),
        "--enable-pcntl".into(),
        "--enable-sockets".into(),
        "--enable-opcache".into(),
        "--with-curl".into(),
        "--with-openssl".into(),
        "--with-zlib".into(),
        "--with-pcre2".into(),
        "--with-mysqli=mysqlnd".into(),
        "--with-pdo-mysql=mysqlnd".into(),
        "--with-sqlite3".into(),
        "--with-pdo-sqlite".into(),
        "--with-gd".into(),
        "--with-zip".into(),
    ];
    emit_progress(app, "configure", "Running ./configure…");
    run_command_streamed(
        app,
        "configure",
        "./configure",
        &configure_args,
        &src_dir,
    )
    .await?;

    // 6. Run make.
    let nproc = detect_cpu_count();
    emit_progress(
        app,
        "make",
        &format!("Running make -j{}… (this takes 5–10 minutes)", nproc),
    );
    run_command_streamed(app, "make", "make", &[format!("-j{}", nproc)], &src_dir).await?;

    // 7. Run make install.
    emit_progress(app, "install", "Running make install…");
    run_command_streamed(app, "install", "make", &["install".into()], &src_dir).await?;

    // 8. Verify the binary exists.
    let php_bin = prefix.join("bin").join("php");
    let php_fpm_bin = prefix.join("sbin").join("php-fpm");
    if !php_bin.exists() {
        return Err(ForageError::Config(format!(
            "PHP binary not found at {} after make install",
            php_bin.display()
        )));
    }
    emit_progress(
        app,
        "done",
        &format!("✓ PHP {} compiled to {}", version, prefix.display()),
    );

    // Clean up build dir to save disk.
    let _ = std::fs::remove_dir_all(&build_dir);

    Ok(prefix)
}

/// Compile nginx from official source. Same flow as PHP.
pub async fn compile_nginx_from_source(
    app: &AppHandle,
    version: &str,
) -> ForageResult<PathBuf> {
    if !version
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.')
        || version.is_empty()
    {
        return Err(ForageError::InvalidInput(format!(
            "invalid nginx version: {}",
            version
        )));
    }

    emit_progress(app, "init", &format!("Compiling nginx {} from source…", version));

    // Check minimal build deps (nginx needs less than PHP).
    let mut missing = Vec::new();
    for tool in ["gcc", "make", "pkg-config"] {
        if which::which(tool).is_err() {
            missing.push(tool.to_string());
        }
    }
    // nginx needs libpcre2, zlib, openssl.
    for lib in ["libpcre2-8", "zlib", "openssl"] {
        let result = Command::new("pkg-config")
            .args(["--exists", lib])
            .output()
            .await;
        if let Ok(out) = result {
            if !out.status.success() {
                missing.push(lib.to_string());
            }
        }
    }
    if !missing.is_empty() {
        let report = check_php_build_deps().await?;
        return Err(ForageError::Config(format!(
            "Build deps missing: {}. Run this command first:\n\n{}",
            missing.join(", "),
            report.install_command
        )));
    }

    let url = format!("https://nginx.org/download/nginx-{}.tar.gz", version);
    let cache_dir = paths::cache_dir().join("source");
    std::fs::create_dir_all(&cache_dir)?;
    let archive_path = cache_dir.join(format!("nginx-{}.tar.gz", version));

    if !archive_path.exists() {
        emit_progress(app, "download", &format!("Downloading {}…", url));
        download_file(&url, &archive_path).await?;
    }

    let build_dir = cache_dir.join(format!("nginx-{}-build", version));
    if build_dir.exists() {
        std::fs::remove_dir_all(&build_dir)?;
    }
    std::fs::create_dir_all(&build_dir)?;
    emit_progress(app, "extract", "Extracting…");
    extract_targz(&archive_path, &build_dir, true)?;

    let src_dir = build_dir.join(format!("nginx-{}", version));
    if !src_dir.exists() {
        return Err(ForageError::Config(format!(
            "expected source dir {} not found",
            src_dir.display()
        )));
    }

    let prefix = paths::bin_dir().join(format!("nginx-{}-source", version));
    if prefix.exists() {
        std::fs::remove_dir_all(&prefix)?;
    }
    std::fs::create_dir_all(&prefix)?;

    emit_progress(app, "configure", "Running ./configure…");
    run_command_streamed(
        app,
        "configure",
        "./configure",
        &[
            format!("--prefix={}", prefix.display()),
            "--with-http_ssl_module".into(),
            "--with-http_v2_module".into(),
            "--with-http_realip_module".into(),
            "--with-http_gzip_static_module".into(),
        ],
        &src_dir,
    )
    .await?;

    let nproc = detect_cpu_count();
    emit_progress(app, "make", &format!("Running make -j{}…", nproc));
    run_command_streamed(app, "make", "make", &[format!("-j{}", nproc)], &src_dir).await?;

    emit_progress(app, "install", "Running make install…");
    run_command_streamed(app, "install", "make", &["install".into()], &src_dir).await?;

    let nginx_bin = prefix.join("sbin").join("nginx");
    if !nginx_bin.exists() {
        return Err(ForageError::Config(format!(
            "nginx binary not found at {} after make install",
            nginx_bin.display()
        )));
    }

    emit_progress(
        app,
        "done",
        &format!("✓ nginx {} compiled to {}", version, prefix.display()),
    );

    let _ = std::fs::remove_dir_all(&build_dir);
    Ok(prefix)
}

// ---- Helpers ---------------------------------------------------------------

fn detect_cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
}

async fn download_file(url: &str, dest: &PathBuf) -> ForageResult<()> {
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
    let bytes = resp.bytes().await.map_err(|e| {
        ForageError::Download(format!("body read failed: {}", e))
    })?;
    std::fs::write(dest, &bytes)?;
    Ok(())
}

fn extract_targz(archive: &PathBuf, dest: &PathBuf, strip_top: bool) -> ForageResult<()> {
    let file = std::fs::File::open(archive)?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(gz);
    for entry in tar.entries()? {
        let mut entry = entry?;
        let name = entry.path()?.display().to_string();
        let target = if strip_top {
            if let Some(idx) = name.find('/') {
                dest.join(&name[idx + 1..])
            } else {
                continue;
            }
        } else {
            dest.join(&name)
        };
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
                let mode = entry.header().mode().unwrap_or(0o755);
                let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(mode));
            }
        }
    }
    Ok(())
}

/// Run a command, streaming stdout + stderr line-by-line to the frontend
/// via `build://progress` events.
async fn run_command_streamed(
    app: &AppHandle,
    stage: &str,
    program: &str,
    args: &[String],
    cwd: &PathBuf,
) -> ForageResult<()> {
    info!(program, ?args, cwd = %cwd.display(), "running build command");
    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd.current_dir(cwd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true); // if the future is dropped, kill the child

    let mut child = cmd.spawn().map_err(|e| ForageError::Service {
        service: "source-build".into(),
        message: format!("{} spawn failed: {}", program, e),
    })?;

    let stdout = child.stdout.take().ok_or_else(|| ForageError::Service {
        service: "source-build".into(),
        message: "stdout pipe failed".into(),
    })?;
    let stderr = child.stderr.take().ok_or_else(|| ForageError::Service {
        service: "source-build".into(),
        message: "stderr pipe failed".into(),
    })?;

    let app_out = app.clone();
    let app_err = app.clone();
    let stage_out = stage.to_string();
    let stage_err = stage.to_string();

    // Spawn two tasks to drain stdout + stderr concurrently.
    let out_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            emit_progress(&app_out, &stage_out, &line);
        }
    });
    let err_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            emit_progress(&app_err, &stage_err, &line);
        }
    });

    let status = child.wait().await.map_err(|e| ForageError::Service {
        service: "source-build".into(),
        message: format!("{} wait failed: {}", program, e),
    })?;

    let _ = out_task.await;
    let _ = err_task.await;

    if !status.success() {
        return Err(ForageError::Service {
            service: "source-build".into(),
            message: format!("{} exited with status {}", program, status),
        });
    }
    Ok(())
}

fn emit_progress(app: &AppHandle, stage: &str, line: &str) {
    let _ = app.emit(
        "build://progress",
        serde_json::json!({ "stage": stage, "line": line }),
    );
}

/// True if the current OS supports source compilation. Windows doesn't
/// (different toolchain, no pkg-config).
pub fn is_source_compile_supported() -> bool {
    cfg!(any(target_os = "linux", target_os = "macos"))
}
