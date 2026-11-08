//! Service spawn helpers. Each function builds a `SpawnSpec` for one
//! service using the current settings and resolved binaries.

use crate::core::paths;
use crate::core::process::SpawnSpec;
use crate::core::settings::Settings;
use crate::error::{ForageError, ForageResult};
use std::path::PathBuf;

/// Build the spawn spec for nginx.
pub fn nginx_spec(settings: &Settings) -> ForageResult<SpawnSpec> {
    let conf = paths::conf_dir().join("nginx.conf");
    if !conf.exists() {
        return Err(ForageError::Config(
            "nginx.conf not generated yet — call config_gen::generate_all first".into(),
        ));
    }
    // Find nginx binary — must be resolved by AppState first.
    let nginx_path = find_cached_binary("nginx")?;
    Ok(SpawnSpec {
        service: "nginx".into(),
        program: nginx_path,
        args: vec![
            "-c".into(),
            conf.to_string_lossy().into(),
            "-p".into(),
            paths::home_dir().to_string_lossy().into(),
        ],
        working_dir: Some(paths::home_dir()),
        env: vec![],
        log_file: paths::nginx_log_dir().join("service.log"),
    })
}

/// Build the spawn spec for PHP-FPM (the default version).
pub fn php_fpm_spec(settings: &Settings) -> ForageResult<SpawnSpec> {
    let conf = paths::conf_dir().join("php-fpm.conf");
    if !conf.exists() {
        return Err(ForageError::Config(
            "php-fpm.conf not generated yet".into(),
        ));
    }
    let php_path = find_cached_binary("php-fpm")
        .or_else(|_| find_cached_binary("php"))?;
    Ok(SpawnSpec {
        service: "php".into(),
        program: php_path,
        // -F = foreground (don't daemonize); -y = config file
        args: vec![
            "-F".into(),
            "-y".into(),
            conf.to_string_lossy().into(),
        ],
        working_dir: Some(paths::home_dir()),
        env: vec![
            ("PHP_INI_SCAN_DIR".into(), "".into()),
        ],
        log_file: paths::php_log_dir().join("service.log"),
    })
}

/// Build spawn specs for ALL PHP-FPM versions currently in use across
/// projects. Returns one spec per unique version. The default version
/// (no override) gets the service name "php"; overridden versions get
/// "php-<version>" (e.g. "php-8.1").
pub fn php_fpm_specs_for_versions(
    settings: &Settings,
    versions: &[String],
) -> ForageResult<Vec<SpawnSpec>> {
    let mut specs = Vec::new();
    for version in versions {
        let is_default = version == &settings.versions.php;
        let service_name = if is_default {
            "php".to_string()
        } else {
            format!("php-{}", version)
        };

        // Find the PHP binary for this version.
        let php_path = if is_default {
            find_cached_binary("php-fpm").or_else(|_| find_cached_binary("php"))?
        } else {
            // Try php-fpm first within the version dir, fall back to php
            // (some static builds ship a single binary that handles both).
            crate::core::php_versions::find_php_binary_for_version(version)
                .and_then(|p| {
                    // If it's just `php`, we can still run it as fpm via `-f` flag.
                    if p.file_name().map(|n| n == "php").unwrap_or(false) {
                        Ok(p)
                    } else {
                        Ok(p)
                    }
                })?
        };

        // Find the per-version config file.
        let conf = if is_default {
            paths::conf_dir().join("php-fpm.conf")
        } else {
            paths::conf_dir().join(format!("php-fpm-{}.conf", version))
        };
        if !conf.exists() {
            return Err(ForageError::Config(format!(
                "{} not generated — call config_gen::generate_php_fpm_conf for version {}",
                conf.display(),
                version
            )));
        }

        specs.push(SpawnSpec {
            service: service_name,
            program: php_path,
            args: vec![
                "-F".into(),
                "-y".into(),
                conf.to_string_lossy().into(),
            ],
            working_dir: Some(paths::home_dir()),
            env: vec![("PHP_INI_SCAN_DIR".into(), "".into())],
            log_file: if is_default {
                paths::php_log_dir().join("service.log")
            } else {
                paths::php_log_dir().join(format!("service-{}.log", version))
            },
        });
    }
    Ok(specs)
}

/// Build the spawn spec for MariaDB.
pub fn mariadb_spec(settings: &Settings) -> ForageResult<SpawnSpec> {
    let conf = paths::conf_dir().join("my.cnf");
    if !conf.exists() {
        return Err(ForageError::Config(
            "my.cnf not generated yet".into(),
        ));
    }
    // Initialize data dir on first run.
    let data_dir = paths::mysql_data_dir();
    if !data_dir.exists() || std::fs::read_dir(&data_dir).map(|mut d| d.next().is_none()).unwrap_or(true) {
        return Err(ForageError::Config(
            "MariaDB data dir not initialized — call init_mariadb first".into(),
        ));
    }
    let mariadbd_path = find_cached_binary("mariadbd")
        .or_else(|_| find_cached_binary("mysqld"))?;
    Ok(SpawnSpec {
        service: "mariadb".into(),
        program: mariadbd_path,
        args: vec![
            format!("--defaults-file={}", conf.to_string_lossy()),
            "--skip-grant-tables".into(), // dev-only; safe because we bind to 127.0.0.1 only
        ],
        working_dir: Some(paths::home_dir()),
        env: vec![],
        log_file: paths::mariadb_log_dir().join("service.log"),
    })
}

/// Look up a binary in the AppState cache. We can't pass AppState here
/// directly (would create a cycle), so we read from a thread-local set by
/// the commands layer.
///
/// In practice this just searches the cached download dir + system PATH
/// again — it's cheap and avoids the cycle.
fn find_cached_binary(name: &str) -> ForageResult<PathBuf> {
    // Check download dir first.
    let bin_dir = paths::bin_dir();
    if let Ok(entries) = std::fs::read_dir(&bin_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let candidate = find_in_subdir(&path, name);
                if let Some(p) = candidate {
                    return Ok(p);
                }
            }
        }
    }
    // Check bundled next to current exe.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidates = [
                parent.join("bin").join(name),
                parent.join("bin").join(format!("{}.exe", name)),
                parent.join("resources").join("bin").join(name),
            ];
            for c in candidates {
                if c.exists() {
                    return Ok(c);
                }
            }
        }
    }
    // Fall back to PATH.
    if let Ok(p) = which::which(name) {
        return Ok(p);
    }
    Err(ForageError::BinaryNotFound {
        name: name.to_string(),
        locations: format!("searched: {}, exe dir, PATH", bin_dir.display()),
    })
}

fn find_in_subdir(dir: &PathBuf, name: &str) -> Option<PathBuf> {
    let candidates = [
        dir.join(name),
        dir.join("bin").join(name),
        dir.join("sbin").join(name),
        dir.join(format!("{}.exe", name)),
        dir.join("bin").join(format!("{}.exe", name)),
    ];
    candidates.into_iter().find(|c| c.exists())
}

/// Initialize the MariaDB data directory. Run once on first start.
pub fn init_mariadb(settings: &Settings) -> ForageResult<()> {
    let data_dir = paths::mysql_data_dir();
    if data_dir.exists() && std::fs::read_dir(&data_dir).map(|mut d| d.next().is_some()).unwrap_or(false) {
        return Ok(()); // already initialized
    }
    std::fs::create_dir_all(&data_dir)?;
    let mariadb_install_db = find_cached_binary("mariadb-install-db")
        .or_else(|_| find_cached_binary("mysql_install_db"))?;
    let basedir = mariadb_install_db.parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/usr"));
    let mut cmd = std::process::Command::new(&mariadb_install_db);
    cmd.arg("--datadir").arg(&data_dir)
        .arg("--basedir").arg(&basedir)
        .arg("--auth-root-authentication-method=normal");
    let out = cmd.output().map_err(|e| ForageError::Service {
        service: "mariadb".into(),
        message: format!("mariadb-install-db failed: {}", e),
    })?;
    if !out.status.success() {
        return Err(ForageError::Service {
            service: "mariadb".into(),
            message: format!(
                "mariadb-install-db failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ),
        });
    }
    Ok(())
}

/// Set up the dev DB user + a default `forge` database. Called after the
/// first successful MariaDB start.
pub fn setup_dev_db(settings: &Settings) -> ForageResult<()> {
    let mariadb_client = find_cached_binary("mariadb")
        .or_else(|_| find_cached_binary("mysql"))?;
    let socket = paths::run_dir().join("mariadb.sock");
    let mut cmd = std::process::Command::new(&mariadb_client);
    cmd.arg("--socket").arg(&socket)
        .arg("--user=root");
    let sql = format!(
        "CREATE USER IF NOT EXISTS '{}'@'localhost' IDENTIFIED BY '{}';\
         GRANT ALL ON *.* TO '{}'@'localhost' WITH GRANT OPTION;\
         CREATE DATABASE IF NOT EXISTS forge;\
         FLUSH PRIVILEGES;",
        settings.mariadb.app_user,
        settings.mariadb.app_password,
        settings.mariadb.app_user,
    );
    cmd.arg("-e").arg(&sql);
    let out = cmd.output().map_err(|e| ForageError::Database(format!(
        "mariadb client failed: {}", e
    )))?;
    if !out.status.success() {
        return Err(ForageError::Database(format!(
            "dev user setup failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(())
}
