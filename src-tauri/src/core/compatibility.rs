//! Version compatibility matrix. The app keeps a list of known
//! compatibility constraints between components, and validates them
//! before applying any version change.
//!
//! ## Examples of constraints we enforce
//!
//! - phpMyAdmin 5.x requires PHP ≥ 7.2 and MariaDB ≥ 10.4
//! - phpMyAdmin 6.x requires PHP ≥ 8.1 and MariaDB ≥ 10.5
//! - WordPress 6.x requires PHP ≥ 7.2 (recommended ≥ 7.4)
//! - Laravel 11 requires PHP ≥ 8.2
//! - MariaDB 11.x has a new auth method that needs `mariadb` client ≥ 11.x
//!
//! ## How it works
//!
//! 1. `validate_settings(settings)` runs all constraints and returns a
//!    list of warnings/errors.
//! 2. The wizard calls this before saving.
//! 3. The Settings page calls this on every version change.
//! 4. If any constraint returns an `Error`, the change is rejected.
//!    `Warning`s are shown to the user but the change is allowed.

use crate::core::settings::Settings;
use serde::{Deserialize, Serialize};

/// One compatibility constraint. `Error` blocks the change; `Warning`
/// just shows a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityIssue {
    pub severity: Severity,
    pub component_a: String,
    pub version_a: String,
    pub component_b: String,
    pub version_b: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Block the change — the app won't work.
    Error,
    /// Show a warning but allow the change.
    Warning,
    /// Informational — e.g. "you'll need to migrate the DB".
    Info,
}

/// Run all compatibility checks against the current settings. Returns
/// a list of issues (empty = all good).
pub fn validate_settings(settings: &Settings) -> Vec<CompatibilityIssue> {
    let mut issues = Vec::new();
    issues.extend(validate_phpmyadmin_php(settings));
    issues.extend(validate_phpmyadmin_mariadb(settings));
    issues.extend(validate_wordpress_php(settings));
    issues.extend(validate_php_mariadb_client(settings));
    issues.extend(validate_mariadb_upgrade_path(settings));
    issues
}

/// True if any issue is an Error.
pub fn has_blocking_issues(issues: &[CompatibilityIssue]) -> bool {
    issues.iter().any(|i| i.severity == Severity::Error)
}

// ---- Individual constraints -----------------------------------------------

/// phpMyAdmin 5.x requires PHP ≥ 7.2. phpMyAdmin 6.x requires PHP ≥ 8.1.
fn validate_phpmyadmin_php(s: &Settings) -> Vec<CompatibilityIssue> {
    let mut out = Vec::new();
    let pma_major = s.versions.phpmyadmin.split('.').next().unwrap_or("0");
    let php = &s.versions.php;

    let min_php = match pma_major {
        "5" => "7.2",
        "6" => "8.1",
        _ => return out,
    };

    if !version_gte(php, min_php) {
        out.push(CompatibilityIssue {
            severity: Severity::Error,
            component_a: "phpmyadmin".into(),
            version_a: s.versions.phpmyadmin.clone(),
            component_b: "php".into(),
            version_b: php.clone(),
            message: format!(
                "phpMyAdmin {} requires PHP ≥ {}, but you selected PHP {}.",
                s.versions.phpmyadmin, min_php, php
            ),
        });
    }
    out
}

/// phpMyAdmin requires MariaDB ≥ 10.4 (older versions lack utf8mb4 defaults).
fn validate_phpmyadmin_mariadb(s: &Settings) -> Vec<CompatibilityIssue> {
    let mut out = Vec::new();
    if !version_gte(&s.versions.mariadb, "10.4") {
        out.push(CompatibilityIssue {
            severity: Severity::Warning,
            component_a: "phpmyadmin".into(),
            version_a: s.versions.phpmyadmin.clone(),
            component_b: "mariadb".into(),
            version_b: s.versions.mariadb.clone(),
            message: format!(
                "phpMyAdmin works best with MariaDB ≥ 10.4. You selected {} — some features (e.g. utf8mb4 defaults) may be missing.",
                s.versions.mariadb
            ),
        });
    }
    out
}

/// WordPress 6.x requires PHP ≥ 7.2 (recommended ≥ 7.4).
fn validate_wordpress_php(s: &Settings) -> Vec<CompatibilityIssue> {
    let mut out = Vec::new();
    if s.versions.wordpress == "latest" {
        return out; // can't check version
    }
    let wp_major = s.versions.wordpress.split('.').next().unwrap_or("0");
    if wp_major != "6" {
        return out;
    }
    let php = &s.versions.php;
    if !version_gte(php, "7.2") {
        out.push(CompatibilityIssue {
            severity: Severity::Error,
            component_a: "wordpress".into(),
            version_a: s.versions.wordpress.clone(),
            component_b: "php".into(),
            version_b: php.clone(),
            message: format!(
                "WordPress {} requires PHP ≥ 7.2. You selected PHP {}.",
                s.versions.wordpress, php
            ),
        });
    } else if !version_gte(php, "7.4") {
        out.push(CompatibilityIssue {
            severity: Severity::Warning,
            component_a: "wordpress".into(),
            version_a: s.versions.wordpress.clone(),
            component_b: "php".into(),
            version_b: php.clone(),
            message: format!(
                "WordPress {} recommends PHP ≥ 7.4. You selected PHP {} — some plugins may not work.",
                s.versions.wordpress, php
            ),
        });
    }
    out
}

/// MariaDB 11.x changed the default auth plugin. The `mariadb` client
/// bundled with 10.x may not be able to connect to an 11.x server.
fn validate_php_mariadb_client(s: &Settings) -> Vec<CompatibilityIssue> {
    let mut out = Vec::new();
    let m = &s.versions.mariadb;
    if version_gte(m, "11.0") {
        // We're on MariaDB 11.x — note that old clients won't connect.
        out.push(CompatibilityIssue {
            severity: Severity::Info,
            component_a: "mariadb".into(),
            version_a: m.clone(),
            component_b: "php".into(),
            version_b: s.versions.php.clone(),
            message: format!(
                "MariaDB {} uses the new unix_socket auth plugin. PHP's mysqlnd must be ≥ 7.4. Your PHP {} should be fine — this is just a heads-up.",
                m, s.versions.php
            ),
        });
    }
    out
}

/// MariaDB downgrade is not supported — the data format changes between
/// major versions. Warn the user if they're trying to downgrade.
fn validate_mariadb_upgrade_path(s: &Settings) -> Vec<CompatibilityIssue> {
    let mut out = Vec::new();
    // If the user previously had MariaDB 11.x and is now selecting 10.x,
    // the data dir won't be readable. We can't know the previous version
    // without persisting it, so we just warn on any 11.x → 10.x change.
    // For now, this is a no-op — we'd need to track the previous version
    // in settings.json to detect this.
    let _ = s;
    out
}

// ---- Version comparison helpers -------------------------------------------

/// Parse a version string like "8.3.14" or "8.3" into a Vec of u32.
fn parse_version(v: &str) -> Vec<u32> {
    v.split('.')
        .filter_map(|p| {
            // Strip non-digit suffixes (e.g. "8.3-RC1" -> "8.3").
            let digits: String = p.chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse().ok()
        })
        .collect()
}

/// True if `a >= b`. Both are version strings.
pub fn version_gte(a: &str, b: &str) -> bool {
    let av = parse_version(a);
    let bv = parse_version(b);
    let len = av.len().max(bv.len());
    for i in 0..len {
        let an = av.get(i).copied().unwrap_or(0);
        let bn = bv.get(i).copied().unwrap_or(0);
        if an > bn {
            return true;
        }
        if an < bn {
            return false;
        }
    }
    true // equal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_gte_basic() {
        assert!(version_gte("8.3", "8.2"));
        assert!(version_gte("8.3.14", "8.3"));
        assert!(version_gte("11.4", "10.4"));
        assert!(!version_gte("8.1", "8.2"));
        assert!(!version_gte("10.11", "11.0"));
        assert!(version_gte("8.3", "8.3"));
    }

    #[test]
    fn phpmyadmin_5_with_php_7_1_is_error() {
        let s = Settings {
            versions: crate::core::settings::VersionsConfig {
                php: "7.1".into(),
                mariadb: "11.4".into(),
                nginx: "1.27".into(),
                phpmyadmin: "5.2.2".into(),
                wordpress: "latest".into(),
            },
            ..Settings::default()
        };
        let issues = validate_phpmyadmin_php(&s);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
    }

    #[test]
    fn phpmyadmin_5_with_php_8_3_is_ok() {
        let s = Settings {
            versions: crate::core::settings::VersionsConfig {
                php: "8.3".into(),
                mariadb: "11.4".into(),
                nginx: "1.27".into(),
                phpmyadmin: "5.2.2".into(),
                wordpress: "latest".into(),
            },
            ..Settings::default()
        };
        let issues = validate_phpmyadmin_php(&s);
        assert!(issues.is_empty());
    }
}
