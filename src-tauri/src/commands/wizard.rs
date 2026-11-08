//! First-run wizard commands. The wizard walks the user through:
//! 1. Version selection (PHP, MariaDB, nginx)
//! 2. Port selection (with conflict detection)
//! 3. Binary strategy (Auto / PreferSystem / etc.)
//! 4. Theme
//! 5. Save settings + generate initial configs + optionally download binaries.

use crate::binaries::download;
use crate::core::config_gen;
use crate::core::paths;
use crate::core::ports;
use crate::core::settings::{BinaryStrategy, Settings, Theme};
use crate::core::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WizardInput {
    pub versions: VersionsInput,
    pub ports: PortsInput,
    pub binary_strategy: String,
    pub theme: String,
    pub auto_start_services: bool,
    pub download_binaries: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionsInput {
    pub php: String,
    pub mariadb: String,
    pub nginx: String,
    pub phpmyadmin: String,
    pub wordpress: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortsInput {
    pub nginx_http: u16,
    pub nginx_https: u16,
    pub mariadb: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct WizardResult {
    pub success: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[tauri::command]
pub fn validate_wizard_input(input: WizardInput) -> WizardResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Port validation.
    for (name, port) in [
        ("nginx_http", input.ports.nginx_http),
        ("nginx_https", input.ports.nginx_https),
        ("mariadb", input.ports.mariadb),
    ] {
        if !(1024..=65535).contains(&port) {
            errors.push(format!("Port {} ({}) must be between 1024 and 65535", name, port));
        } else if !ports::is_port_free(port) {
            warnings.push(format!("Port {} ({}) is currently in use — start will fail", name, port));
        }
    }

    // PHP version validation.
    let known_php = ["8.1", "8.2", "8.3", "8.4"];
    if !known_php.contains(&input.versions.php.as_str()) {
        errors.push(format!(
            "PHP version {} not in known list {}",
            input.versions.php,
            known_php.join(", ")
        ));
    }

    WizardResult {
        success: errors.is_empty(),
        errors,
        warnings,
    }
}

#[tauri::command]
pub async fn save_wizard_input(
    app: AppHandle,
    state: State<'_, AppState>,
    input: WizardInput,
) -> Result<WizardResult, String> {
    let validation = validate_wizard_input(input.clone());
    if !validation.success {
        return Ok(validation);
    }

    // Run compatibility checks against the proposed versions BEFORE
    // saving. If there are blocking errors, reject the save.
    let compat_issues = crate::core::compatibility::validate_settings(&Settings {
        versions: crate::core::settings::VersionsConfig {
            php: input.versions.php.clone(),
            mariadb: input.versions.mariadb.clone(),
            nginx: input.versions.nginx.clone(),
            phpmyadmin: input.versions.phpmyadmin.clone(),
            wordpress: input.versions.wordpress.clone(),
        },
        ..Settings::default()
    });
    let blocking: Vec<_> = compat_issues
        .iter()
        .filter(|i| i.severity == crate::core::compatibility::Severity::Error)
        .collect();
    if !blocking.is_empty() {
        return Ok(WizardResult {
            success: false,
            errors: blocking.iter().map(|i| i.message.clone()).collect(),
            warnings: vec![],
        });
    }

    let _ = state.settings.update(|s| {
        s.versions.php = input.versions.php.clone();
        s.versions.mariadb = input.versions.mariadb.clone();
        s.versions.nginx = input.versions.nginx.clone();
        s.versions.phpmyadmin = input.versions.phpmyadmin.clone();
        s.versions.wordpress = input.versions.wordpress.clone();
        s.ports.nginx_http = input.ports.nginx_http;
        s.ports.nginx_https = input.ports.nginx_https;
        s.ports.mariadb = input.ports.mariadb;
        s.binary_strategy = match input.binary_strategy.as_str() {
            "auto" => BinaryStrategy::Auto,
            "prefer_system" => BinaryStrategy::PreferSystem,
            "prefer_bundled" => BinaryStrategy::PreferBundled,
            "prefer_download" => BinaryStrategy::PreferDownload,
            _ => BinaryStrategy::Auto,
        };
        s.theme = match input.theme.as_str() {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => Theme::Auto,
        };
        s.auto_start_services = input.auto_start_services;
    });

    // Regenerate configs.
    let s = state.settings.get();
    config_gen::generate_all(&s).map_err(|e| e.to_string())?;

    // Refresh binary cache.
    state.refresh_binaries().map_err(|e| e.to_string())?;

    // Optionally download missing binaries. We use the detection report
    // to decide what actually needs downloading — no redundant downloads
    // for components already on the system (subject to conflict_policy).
    let mut warnings = validation.warnings.clone();
    warnings.extend(
        compat_issues
            .iter()
            .filter(|i| i.severity != crate::core::compatibility::Severity::Error)
            .map(|i| i.message.clone()),
    );

    if input.download_binaries {
        let report = crate::core::detection::build_report(&s);
        let pending = crate::core::detection::pending_downloads(&report);
        tracing::info!(?pending, "wizard: pending downloads after detection");

        for component in &pending {
            let version = match component.as_str() {
                "php" => &s.versions.php,
                "mariadb" => &s.versions.mariadb,
                "phpmyadmin" => &s.versions.phpmyadmin,
                "composer" => "stable",
                "nginx" => &s.versions.nginx,
                _ => continue,
            };
            let spec = match download::spec_for(component, version) {
                Ok(s) => s,
                Err(e) => {
                    warnings.push(format!(
                        "No download available for {} {}: {}",
                        component, version, e
                    ));
                    continue;
                }
            };
            let app_clone = app.clone();
            let comp_name = component.to_string();
            if let Err(e) = download::download_and_extract(&spec, move |done, total| {
                let _ = app_clone.emit(
                    "binary://download-progress",
                    serde_json::json!({ "name": &comp_name, "done": done, "total": total }),
                );
            }).await {
                warnings.push(format!("Download failed for {}: {}", component, e));
            }
        }
        // Install phpMyAdmin into www/pma.
        let pma_dir = paths::bin_dir().join(format!("phpmyadmin-{}", s.versions.phpmyadmin));
        if pma_dir.exists() && !paths::www_dir().join("pma").exists() {
            let _ = std::fs::create_dir_all(paths::www_dir().join("pma"));
            let _ = copy_dir_recursive(&pma_dir, &paths::www_dir().join("pma"));
        }
        // Move composer.phar into bin/.
        let composer_phar = paths::cache_dir().join("composer-stable.phar");
        if composer_phar.exists() {
            let _ = std::fs::rename(&composer_phar, paths::bin_dir().join("composer.phar"));
        }
        state.refresh_binaries().ok();
    }

    let _ = app.emit("wizard//completed", ());

    Ok(WizardResult {
        success: true,
        errors: vec![],
        warnings,
    })
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let path = entry.path();
        let name = entry.file_name();
        let target = dst.join(name);
        if ft.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else if ft.is_file() {
            std::fs::copy(&path, &target)?;
        }
    }
    Ok(())
}
