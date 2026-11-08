//! Package manager install commands. Generates the exact shell commands
//! to install PHP/nginx from official package manager sources:
//!
//! - **macOS**: Homebrew (`brew install php@8.5`)
//! - **Debian/Ubuntu**: Ondřej Surý's PPA (`packages.sury.org/php`) —
//!   he's the official Debian PHP maintainer
//! - **Fedora**: Remi Collet's repo (`rpms.remirepo.net`) — he's the
//!   official Fedora PHP maintainer
//! - **nginx on Debian/Ubuntu**: official `nginx.org/packages/...` apt repo
//! - **nginx on Fedora/RHEL**: official `nginx.org/packages/.../centos/` yum repo
//! - **nginx on macOS**: `brew install nginx`
//!
//! ## Why this is the best option
//!
//! - Binaries are built and signed by official maintainers
//! - Auto-updated by `apt upgrade` / `dnf upgrade` / `brew upgrade`
//! - All PHP extensions available as separate packages
//! - Properly integrated with the system
//!
//! ## Root access
//!
//! - **apt/dnf** (Linux): requires `sudo` for the install commands.
//!   The app **never runs `sudo`** — it shows the user the exact
//!   commands to copy-paste into their terminal.
//! - **brew** (macOS): does NOT require sudo. The app could run these
//!   directly, but we still show them as copy-paste for safety.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Which package manager we detected on the system.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageManager {
    Apt,
    Dnf,
    Brew,
    Unknown,
}

/// A single shell command + a description of what it does.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellStep {
    /// The command to run.
    pub command: String,
    /// Human-readable description.
    pub description: String,
    /// True if this command requires sudo. The UI shows a warning.
    pub requires_sudo: bool,
}

/// A full install plan for one component+version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPlan {
    pub component: String,
    pub version: String,
    pub package_manager: PackageManager,
    /// Ordered list of commands to run. The user should run them in
    /// order, top to bottom.
    pub steps: Vec<ShellStep>,
    /// Where the binary will land after install.
    pub expected_path: String,
    /// Notes / caveats shown to the user.
    pub notes: Vec<String>,
}

/// Detect the package manager on the current system.
pub fn detect_package_manager() -> PackageManager {
    if std::path::Path::new("/usr/bin/apt").exists() {
        PackageManager::Apt
    } else if std::path::Path::new("/usr/bin/dnf").exists() {
        PackageManager::Dnf
    } else if std::path::Path::new("/opt/homebrew/bin/brew").exists()
        || std::path::Path::new("/usr/local/bin/brew").exists()
    {
        PackageManager::Brew
    } else {
        PackageManager::Unknown
    }
}

/// Build the install plan for a component+version on the current system.
pub fn build_install_plan(component: &str, version: &str) -> Result<InstallPlan, String> {
    let pm = detect_package_manager();
    match (component, pm) {
        ("php", PackageManager::Brew) => Ok(php_brew_plan(version)),
        ("php", PackageManager::Apt) => Ok(php_apt_sury_plan(version)),
        ("php", PackageManager::Dnf) => Ok(php_dnf_remi_plan(version)),
        ("nginx", PackageManager::Brew) => Ok(nginx_brew_plan(version)),
        ("nginx", PackageManager::Apt) => Ok(nginx_apt_official_plan(version)),
        ("nginx", PackageManager::Dnf) => Ok(nginx_dnf_official_plan(version)),
        _ => Err(format!(
            "no install plan for {} on {:?}",
            component, pm
        )),
    }
}

// ---- PHP plans -------------------------------------------------------------

/// macOS: `brew install php@8.5 && brew link --force --overwrite php@8.5`
fn php_brew_plan(version: &str) -> InstallPlan {
    let short_ver = short_version(version); // "8.5" -> "8.5", "8.5.14" -> "8.5"
    InstallPlan {
        component: "php".into(),
        version: version.into(),
        package_manager: PackageManager::Brew,
        steps: vec![
            ShellStep {
                command: format!("brew install php@{}", short_ver),
                description: format!(
                    "Install PHP {} from official Homebrew formula",
                    short_ver
                ),
                requires_sudo: false,
            },
            ShellStep {
                command: format!("brew link --force --overwrite php@{}", short_ver),
                description: "Link this PHP version so it's the default `php` on PATH".into(),
                requires_sudo: false,
            },
        ],
        expected_path: "/opt/homebrew/bin/php".into(),
        notes: vec![
            "Homebrew PHP includes FPM, CLI, and most extensions by default.".into(),
            "After install, the FPM binary is at /opt/homebrew/sbin/php-fpm".into(),
            format!("Run `brew services start php@{}` to start FPM as a launchd service.", short_ver),
        ],
    }
}

/// Debian/Ubuntu: Ondřej Surý's PPA (packages.sury.org/php).
/// He is the official Debian PHP maintainer — this is the canonical
/// source for newer PHP versions on Debian/Ubuntu.
fn php_apt_sury_plan(version: &str) -> InstallPlan {
    let short_ver = short_version(version); // "8.5" -> "8.5"
    InstallPlan {
        component: "php".into(),
        version: version.into(),
        package_manager: PackageManager::Apt,
        steps: vec![
            ShellStep {
                command: "sudo apt update".into(),
                description: "Update apt index".into(),
                requires_sudo: true,
            },
            ShellStep {
                command: "sudo apt install -y lsb-release ca-certificates curl".into(),
                description: "Install prerequisites for adding the Sury repo".into(),
                requires_sudo: true,
            },
            ShellStep {
                command: "sudo curl -sSLo /tmp/debsuryorg-archive-keyring.deb https://packages.sury.org/debsuryorg-archive-keyring.deb".into(),
                description: "Download the Sury repo signing key".into(),
                requires_sudo: true,
            },
            ShellStep {
                command: "sudo dpkg -i /tmp/debsuryorg-archive-keyring.deb".into(),
                description: "Install the Sury signing key".into(),
                requires_sudo: true,
            },
            ShellStep {
                command: "sudo tee /etc/apt/sources.list.d/php.sources <<EOF\nTypes: deb\nURIs: https://packages.sury.org/php/\nSuites: $(lsb_release -sc)\nComponents: main\nSigned-By: /usr/share/keyrings/debsuryorg-archive-keyring.gpg\nEOF".into(),
                description: "Add the Sury PHP repository".into(),
                requires_sudo: true,
            },
            ShellStep {
                command: "sudo apt update".into(),
                description: "Update apt index with the new repo".into(),
                requires_sudo: true,
            },
            ShellStep {
                command: format!("sudo apt install -y php{}-fpm php{}-cli php{}-mysql php{}-gd php{}-mbstring php{}-xml php{}-curl php{}-zip", short_ver, short_ver, short_ver, short_ver, short_ver, short_ver, short_ver, short_ver),
                description: format!("Install PHP {} + FPM + common extensions", short_ver),
                requires_sudo: true,
            },
        ],
        expected_path: format!("/usr/sbin/php{}-fpm", short_ver),
        notes: vec![
            "Ondřej Surý is the official Debian PHP maintainer. packages.sury.org is the canonical source for newer PHP versions on Debian/Ubuntu.".into(),
            "This installs PHP-FPM (the FastCGI process manager the app uses) + the CLI + the extensions WordPress/Laravel/common apps need.".into(),
            format!("The FPM binary lands at /usr/sbin/php{}-fpm. The CLI lands at /usr/bin/php{}.", short_ver, short_ver),
            format!("To get more extensions later: `apt search php{}-` and `sudo apt install php{}-<ext>`.", short_ver, short_ver),
        ],
    }
}

/// Fedora: Remi Collet's repo (rpms.remirepo.net).
/// Remi is the official Fedora PHP maintainer.
fn php_dnf_remi_plan(version: &str) -> InstallPlan {
    let short_ver = short_version(version); // "8.5" -> "8.5"
    let no_dot = short_ver.replace('.', ""); // "85" for the dnf package name
    InstallPlan {
        component: "php".into(),
        version: version.into(),
        package_manager: PackageManager::Dnf,
        steps: vec![
            ShellStep {
                command: "sudo dnf install -y dnf-plugins-core".into(),
                description: "Install dnf plugin support".into(),
                requires_sudo: true,
            },
            ShellStep {
                command: "sudo dnf install -y https://rpms.remirepo.net/fedora/remi-release-$(rpm -E %fedora).rpm".into(),
                description: "Enable Remi's RPM repository".into(),
                requires_sudo: true,
            },
            ShellStep {
                command: format!("sudo dnf module reset -y php && sudo dnf module enable -y php:remi-{}", short_ver),
                description: format!("Enable the PHP {} module from Remi's repo", short_ver),
                requires_sudo: true,
            },
            ShellStep {
                command: format!("sudo dnf install -y php php-fpm php-cli php-mysqlnd php-gd php-mbstring php-xml php-curl php-zip"),
                description: "Install PHP + FPM + common extensions".into(),
                requires_sudo: true,
            },
        ],
        expected_path: "/usr/sbin/php-fpm".into(),
        notes: vec![
            "Remi Collet is the official Fedora PHP maintainer. rpms.remirepo.net is the canonical source for newer PHP versions on Fedora/RHEL.".into(),
            "The `dnf module enable` line picks the Remi PHP version. Without it, dnf would install whatever version Fedora ships by default.".into(),
            "PHP-FPM lands at /usr/sbin/php-fpm. The CLI lands at /usr/bin/php.".into(),
            "For more extensions: `dnf search php-` and `sudo dnf install php-<ext>`.".into(),
        ],
    }
}

// ---- nginx plans -----------------------------------------------------------

/// macOS: `brew install nginx`
fn nginx_brew_plan(_version: &str) -> InstallPlan {
    InstallPlan {
        component: "nginx".into(),
        version: _version.into(),
        package_manager: PackageManager::Brew,
        steps: vec![
            ShellStep {
                command: "brew install nginx".into(),
                description: "Install nginx from official Homebrew formula".into(),
                requires_sudo: false,
            },
        ],
        expected_path: "/opt/homebrew/bin/nginx".into(),
        notes: vec![
            "Homebrew's nginx formula builds from official nginx source. The version is whatever Homebrew currently ships — usually latest stable.".into(),
            "nginx lands at /opt/homebrew/bin/nginx (Apple Silicon) or /usr/local/bin/nginx (Intel).".into(),
            "Config file lands at /opt/homebrew/etc/nginx/nginx.conf.".into(),
            "Run `brew services start nginx` to run it as a launchd service.".into(),
        ],
    }
}

/// Debian/Ubuntu: official nginx.org apt repo.
fn nginx_apt_official_plan(_version: &str) -> InstallPlan {
    InstallPlan {
        component: "nginx".into(),
        version: _version.into(),
        package_manager: PackageManager::Apt,
        steps: vec![
            ShellStep {
                command: "sudo apt install -y curl gnupg2 ca-certificates lsb-release ubuntu-keyring".into(),
                description: "Install prerequisites for adding the nginx repo".into(),
                requires_sudo: true,
            },
            ShellStep {
                command: "curl https://nginx.org/keys/nginx_signing.key | gpg --dearmor | sudo tee /usr/share/keyrings/nginx-archive-keyring.gpg >/dev/null".into(),
                description: "Import the official nginx signing key".into(),
                requires_sudo: true,
            },
            ShellStep {
                command: "echo \"deb [signed-by=/usr/share/keyrings/nginx-archive-keyring.gpg] https://nginx.org/packages/ubuntu $(lsb_release -cs) nginx\" | sudo tee /etc/apt/sources.list.d/nginx.list".into(),
                description: "Add the official nginx.org apt repo (stable). Use 'mainline' in the URL instead of 'packages' for mainline builds.".into(),
                requires_sudo: true,
            },
            ShellStep {
                command: "echo -e \"Package: *\\nPin: origin nginx.org\\nPin: release o=nginx\\nPin-Priority: 900\\n\" | sudo tee /etc/apt/preferences.d/99nginx".into(),
                description: "Pin apt to prefer nginx.org packages over distro packages".into(),
                requires_sudo: true,
            },
            ShellStep {
                command: "sudo apt update".into(),
                description: "Update apt index with the new repo".into(),
                requires_sudo: true,
            },
            ShellStep {
                command: "sudo apt install -y nginx".into(),
                description: "Install nginx".into(),
                requires_sudo: true,
            },
        ],
        expected_path: "/usr/sbin/nginx".into(),
        notes: vec![
            "This installs the official nginx.org build of nginx. It's signed by the nginx team and auto-updated via apt.".into(),
            "If you're on Debian (not Ubuntu), replace 'ubuntu' with 'debian' in the apt repo URL.".into(),
            "The 'mainline' branch has the latest features; 'stable' is more conservative. Use mainline for dev.".into(),
            "nginx lands at /usr/sbin/nginx. Config at /etc/nginx/nginx.conf.".into(),
        ],
    }
}

/// Fedora/RHEL: official nginx.org yum repo.
fn nginx_dnf_official_plan(_version: &str) -> InstallPlan {
    InstallPlan {
        component: "nginx".into(),
        version: _version.into(),
        package_manager: PackageManager::Dnf,
        steps: vec![
            ShellStep {
                command: "sudo dnf install -y dnf-plugins-core".into(),
                description: "Install dnf plugin support".into(),
                requires_sudo: true,
            },
            ShellStep {
                command: "sudo dnf config-manager --add-repo https://nginx.org/packages/centos/$(rpm -E %rhel)/$basearch/".into(),
                description: "Add the official nginx.org yum repo".into(),
                requires_sudo: true,
            },
            ShellStep {
                command: "sudo dnf install -y nginx".into(),
                description: "Install nginx".into(),
                requires_sudo: true,
            },
        ],
        expected_path: "/usr/sbin/nginx".into(),
        notes: vec![
            "This installs the official nginx.org build of nginx for Fedora/RHEL/CentOS.".into(),
            "nginx lands at /usr/sbin/nginx. Config at /etc/nginx/nginx.conf.".into(),
            "Enable + start the systemd service: `sudo systemctl enable --now nginx`.".into(),
        ],
    }
}

// ---- Helpers ---------------------------------------------------------------

/// Extract the major.minor version from a longer version string.
/// "8.5" -> "8.5", "8.5.14" -> "8.5", "8.5-RC1" -> "8.5"
fn short_version(version: &str) -> String {
    // Strip any "-RC1" / "-beta" suffix first.
    let base = version.split('-').next().unwrap_or(version);
    let parts: Vec<&str> = base.split('.').collect();
    if parts.len() >= 2 {
        format!("{}.{}", parts[0], parts[1])
    } else {
        version.to_string()
    }
}

/// Check if a component is now installed on the system after the user ran
/// the install commands. Returns the path if found.
pub fn verify_system_install(component: &str, _version: &str) -> Option<PathBuf> {
    let name = match component {
        "php" => "php",
        "nginx" => "nginx",
        _ => return None,
    };
    which::which(name).ok().or_else(|| {
        // Also check common FPM/server paths.
        let extra = match component {
            "php" => vec!["/usr/sbin/php-fpm", "/usr/sbin/php8.5-fpm", "/opt/homebrew/sbin/php-fpm"],
            "nginx" => vec!["/usr/sbin/nginx", "/opt/homebrew/bin/nginx", "/usr/local/bin/nginx"],
            _ => vec![],
        };
        for p in extra {
            if std::path::Path::new(p).exists() {
                return Some(PathBuf::from(p));
            }
        }
        None
    })
}
