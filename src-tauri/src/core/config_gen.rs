//! Config generator. Reads templates from `src/templates/`, substitutes
//! `{{placeholders}}` with values from settings, and writes the result to
//! `~/.insophinia-forge/conf/`.
//!
//! Templates are embedded with `include_str!` so they ship inside the
//! binary — no external file dependencies at runtime.

use crate::core::paths;
use crate::core::php_versions;
use crate::core::settings::Settings;
use crate::error::{ForageError, ForageResult};
use std::path::PathBuf;

const NGINX_TEMPLATE: &str = include_str!("../templates/nginx.conf.template");
const PHP_FPM_TEMPLATE: &str = include_str!("../templates/php-fpm.conf.template");
const PHP_INI_TEMPLATE: &str = include_str!("../templates/php.ini.template");
const MY_CNF_TEMPLATE: &str = include_str!("../templates/my.cnf.template");

/// Generate all config files. Idempotent. Includes:
///   - nginx.conf (with per-project PHP version routing)
///   - php-fpm.conf (default version)
///   - php-fpm-<version>.conf (one per project override)
///   - php.ini
///   - my.cnf
pub fn generate_all(settings: &Settings) -> ForageResult<()> {
    paths::ensure_dirs()?;
    write_conf("nginx.conf", &render_nginx(settings))?;
    write_conf("php-fpm.conf", &render_php_fpm(settings, None))?;
    write_conf("php.ini", &render_php_ini(settings))?;
    write_conf("my.cnf", &render_my_cnf(settings))?;

    // Generate per-version php-fpm configs for projects with overrides.
    let active_versions = php_versions::collect_active_php_versions(&settings.versions.php);
    for version in &active_versions {
        if version == &settings.versions.php {
            continue; // already handled by the default config
        }
        let conf_name = format!("php-fpm-{}.conf", version);
        write_conf(&conf_name, &render_php_fpm(settings, Some(version)))?;
    }

    Ok(())
}

fn write_conf(name: &str, content: &str) -> ForageResult<()> {
    let path = paths::conf_dir().join(name);
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, &path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Tiny mustache-like renderer. Replaces `{{key}}` (with optional inner
/// whitespace) with `value`. Unknown keys are left untouched and surface
/// as config syntax errors when the service starts — that's better than
/// silently producing a broken config.
fn render(template: &str, mut vars: Vec<(&str, String)>) -> String {
    let mut out = template.to_string();
    // Sort by length descending so `{{foo_bar}}` doesn't get partially
    // matched by `{{foo}}`.
    vars.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    for (key, val) in &vars {
        let needle_no_ws = format!("{{{{{}}}}}", key.trim());
        let needle_ws = format!("{{{{ {} }}}}", key.trim());
        out = out.replace(&needle_no_ws, val).replace(&needle_ws, val);
    }
    out
}

fn render_nginx(s: &Settings) -> String {
    // Build the `map $project $php_socket` directive. The default points
    // to the default PHP-FPM socket; each project with an override gets
    // its own line. This is how we route per-project PHP versions.
    let default_socket = paths::php_fpm_socket();
    let mut map_lines = vec![format!("    default        \"{}\";", default_socket.display())];

    let overrides = php_versions::list_project_php_overrides();
    for (project, version) in &overrides {
        if let Some(v) = version {
            let socket = php_versions::php_socket_for_version(v);
            map_lines.push(format!("    \"{}\"  \"{}\";", project, socket.display()));
        }
    }
    let map_directive = format!(
        "map $project $php_socket {{\n{}\n}}\n",
        map_lines.join("\n")
    );

    let rendered = render(
        NGINX_TEMPLATE,
        vec![
            ("worker_processes", s.nginx.worker_processes.clone()),
            ("error_log", paths::nginx_log_dir().join("error.log").display().to_string()),
            ("access_log", paths::nginx_log_dir().join("access.log").display().to_string()),
            ("pid_file", paths::pid_file("nginx").display().to_string()),
            ("client_max_body_size", s.nginx.client_max_body_size.clone()),
            ("nginx_http", s.ports.nginx_http.to_string()),
            ("www_dir", paths::www_dir().display().to_string()),
            ("php_fpm_socket", default_socket.display().to_string()),
        ],
    );

    // Inject the map directive as the first thing inside the http block.
    // nginx requires `map` to be inside `http`, not at the top level.
    rendered.replacen("http {", &format!("http {{\n{}", map_directive), 1)
}

/// Render the PHP-FPM config. If `version` is `None`, uses the default
/// socket and pool name `www`. Otherwise uses the per-version socket
/// and pool name `www_<version_with_underscores>`.
fn render_php_fpm(s: &Settings, version: Option<&str>) -> String {
    let (user, group) = current_user_group();
    let socket = match version {
        None => paths::php_fpm_socket(),
        Some(v) => php_versions::php_socket_for_version(v),
    };
    let pool_name = match version {
        None => "www".to_string(),
        Some(v) => format!("www_{}", v.replace('.', "_")),
    };
    let pid_file = match version {
        None => paths::pid_file("php-fpm"),
        Some(v) => paths::pid_file(&format!("php-fpm-{}", v)),
    };

    let rendered = render(
        PHP_FPM_TEMPLATE,
        vec![
            ("pid_file", pid_file.display().to_string()),
            ("error_log", paths::php_log_dir().join("fpm-error.log").display().to_string()),
            ("user", user),
            ("group", group),
            ("php_fpm_socket", socket.display().to_string()),
            ("slowlog", paths::php_log_dir().join("slow.log").display().to_string()),
            ("php_error_log", paths::php_log_dir().join("php-error.log").display().to_string()),
            ("sessions_dir", paths::php_sessions_dir().display().to_string()),
            ("upload_tmp_dir", paths::data_dir().join("uploads").display().to_string()),
            ("sys_temp_dir", paths::data_dir().join("tmp").display().to_string()),
        ],
    );

    // Replace the pool name. The template uses [www]; we substitute with
    // [www_8_3] etc. for per-version configs.
    rendered.replacen("[www]", &format!("[{}]", pool_name), 1)
}

fn render_php_ini(s: &Settings) -> String {
    render(
        PHP_INI_TEMPLATE,
        vec![
            ("memory_limit", s.php.memory_limit.clone()),
            ("upload_max_filesize", s.php.upload_max_filesize.clone()),
            ("post_max_size", s.php.post_max_size.clone()),
            ("max_execution_time", s.php.max_execution_time.to_string()),
            ("display_errors", if s.php.display_errors { "On" } else { "Off" }.into()),
            ("php_error_log", paths::php_log_dir().join("php-error.log").display().to_string()),
            ("sessions_dir", paths::php_sessions_dir().display().to_string()),
            ("upload_tmp_dir", paths::data_dir().join("uploads").display().to_string()),
        ],
    )
}

fn render_my_cnf(s: &Settings) -> String {
    let (user, _) = current_user_group();
    render(
        MY_CNF_TEMPLATE,
        vec![
            ("mariadb_port", s.ports.mariadb.to_string()),
            ("socket", paths::run_dir().join("mariadb.sock").display().to_string()),
            ("pid_file", paths::pid_file("mariadbd").display().to_string()),
            ("basedir", mariadb_basedir().display().to_string()),
            ("data_dir", paths::mysql_data_dir().display().to_string()),
            ("tmpdir", paths::cache_dir().join("mariadb-tmp").display().to_string()),
            ("user", user),
            ("innodb_buffer_pool_size", s.mariadb.innodb_buffer_pool_size.clone()),
            ("max_connections", s.mariadb.max_connections.to_string()),
            ("error_log", paths::mariadb_log_dir().join("error.log").display().to_string()),
        ],
    )
}

fn mariadb_basedir() -> PathBuf {
    let bin_dir = paths::bin_dir();
    if let Ok(entries) = std::fs::read_dir(&bin_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with("mariadb-") {
                return entry.path();
            }
        }
    }
    PathBuf::from("/usr")
}

/// Get the current user/group name. PHP-FPM runs as this user so we never
/// need root. On Linux Mint this is normally `<username>` with a matching
/// group; on macOS the user's primary group is usually `staff`.
fn current_user_group() -> (String, String) {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "nobody".to_string());

    #[cfg(unix)]
    {
        let group = std::env::var("GROUP").unwrap_or_else(|_| {
            // Common Linux Mint / Ubuntu default: a group matching the user.
            // On macOS the primary group is usually `staff`.
            if cfg!(target_os = "macos") {
                "staff".to_string()
            } else {
                user.clone()
            }
        });
        (user, group)
    }
    #[cfg(not(unix))]
    {
        (user.clone(), user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_replaces_known_keys() {
        let out = render("hello {{name}}!", vec![("name", "world".into())]);
        assert_eq!(out, "hello world!");
    }

    #[test]
    fn render_handles_inner_whitespace() {
        let out = render("hello {{ name }}!", vec![("name", "world".into())]);
        assert_eq!(out, "hello world!");
    }

    #[test]
    fn render_does_not_partial_match() {
        let out = render("a={{foo}} b={{foo_bar}}", vec![
            ("foo_bar", "B".into()),
            ("foo", "A".into()),
        ]);
        assert_eq!(out, "a=A b=B");
    }
}
