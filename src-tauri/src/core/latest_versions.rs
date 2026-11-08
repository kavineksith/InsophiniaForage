//! Dynamic version fetcher. On app startup (or on demand from the
//! Settings → Detection tab), the app queries the official source for
//! each component and returns the list of available versions.
//!
//! ## Sources
//!
//! - **PHP**: php.net releases API (`https://www.php.net/releases/index.php?json`)
//!   Returns a JSON object with version → release info. We parse out the
//!   8.x / 9.x versions.
//! - **nginx**: nginx.org download page (`https://nginx.org/en/download.html`)
//!   We parse the page for `nginx-X.Y.Z` links. No JSON API, so we scrape.
//! - **MariaDB**: mariadb.org REST API (`https://downloads.mariadb.org/rest-api/mariadb/`)
//!   Returns JSON with all releases grouped by major version.
//! - **phpMyAdmin**: phpmyadmin.net files API
//!   (`https://www.phpmyadmin.net/files/`) — we scrape the JSON-LD.
//! - **WordPress**: wordpress.org version-check API
//!   (`https://api.wordpress.org/core/version-check/1.7/`) — JSON.
//!
//! All fetches are async and time out after 10 seconds. If a fetch fails
//! (no internet, API down), we fall back to the hardcoded list in
//! `binaries::available_versions_for`.

use crate::error::{ForageError, ForageResult};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::warn;

const TIMEOUT_SECS: u64 = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableVersions {
    pub php: Vec<String>,
    pub nginx: Vec<String>,
    pub mariadb: Vec<String>,
    pub phpmyadmin: Vec<String>,
    pub wordpress: Vec<String>,
    /// True if the fetch succeeded. False = fell back to hardcoded list.
    pub fetched: bool,
    /// Error message if fetch failed.
    pub error: Option<String>,
}

/// Fetch all available versions from official sources. Best-effort —
/// on any failure, falls back to the hardcoded list.
pub async fn fetch_available_versions() -> AvailableVersions {
    let mut out = AvailableVersions {
        php: hardcoded_php(),
        nginx: hardcoded_nginx(),
        mariadb: hardcoded_mariadb(),
        phpmyadmin: hardcoded_phpmyadmin(),
        wordpress: hardcoded_wordpress(),
        fetched: false,
        error: None,
    };

    let client = match reqwest::Client::builder()
        .user_agent("InsophiniaForage/0.1")
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            out.error = Some(format!("HTTP client build failed: {}", e));
            return out;
        }
    };

    // Fetch all in parallel. Each is independent — if one fails, we keep
    // the hardcoded list for that component but still use the fetched
    // list for the others.
    let (php, nginx, mariadb, phpmyadmin, wordpress) = tokio::join!(
        fetch_php_versions(&client),
        fetch_nginx_versions(&client),
        fetch_mariadb_versions(&client),
        fetch_phpmyadmin_versions(&client),
        fetch_wordpress_versions(&client),
    );

    let mut any_succeeded = false;
    if let Ok(v) = php {
        out.php = v;
        any_succeeded = true;
    }
    if let Ok(v) = nginx {
        out.nginx = v;
        any_succeeded = true;
    }
    if let Ok(v) = mariadb {
        out.mariadb = v;
        any_succeeded = true;
    }
    if let Ok(v) = phpmyadmin {
        out.phpmyadmin = v;
        any_succeeded = true;
    }
    if let Ok(v) = wordpress {
        out.wordpress = v;
        any_succeeded = true;
    }

    out.fetched = any_succeeded;
    if !any_succeeded {
        out.error = Some("all version fetches failed — using hardcoded lists".into());
    }
    out
}

// ---- PHP -------------------------------------------------------------------

/// Fetch PHP versions from php.net's releases API.
///
/// The API returns a JSON object like:
/// ```json
/// {
///   "1":    { ... },
///   "8.4":  { "release_date": "2024-11-21", "version": "8.4.0", ... },
///   "8.3":  { ... },
///   ...
/// }
/// ```
/// We extract the top-level keys that look like version numbers and
/// return them sorted descending.
async fn fetch_php_versions(client: &reqwest::Client) -> ForageResult<Vec<String>> {
    let url = "https://www.php.net/releases/index.php?json";
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| ForageError::Download(format!("php.net API: {}", e)))?;
    if !resp.status().is_success() {
        return Err(ForageError::Download(format!(
            "php.net API returned {}",
            resp.status()
        )));
    }
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ForageError::Download(format!("php.net parse: {}", e)))?;

    let mut versions: Vec<String> = json
        .as_object()
        .map(|obj| {
            obj.keys()
                .filter(|k| {
                    // Top-level keys like "8.4", "8.3", "9.0" are major versions.
                    // Skip keys like "1", "2", "3" (legacy) and non-numeric.
                    k.contains('.') && k.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    // Sort descending. Use naive split-by-dot comparison.
    versions.sort_by(|a, b| {
        let an: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
        let bn: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
        bn.cmp(&an)
    });

    if versions.is_empty() {
        return Err(ForageError::Download(
            "php.net API returned no versions".into(),
        ));
    }
    Ok(versions)
}

// ---- nginx -----------------------------------------------------------------

/// Fetch nginx versions by scraping nginx.org/en/download.html.
/// The page contains lines like `<a href="/download/nginx-1.27.3.tar.gz">`.
/// We extract the version numbers and dedupe.
async fn fetch_nginx_versions(client: &reqwest::Client) -> ForageResult<Vec<String>> {
    let url = "https://nginx.org/en/download.html";
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| ForageError::Download(format!("nginx.org: {}", e)))?;
    if !resp.status().is_success() {
        return Err(ForageError::Download(format!(
            "nginx.org returned {}",
            resp.status()
        )));
    }
    let html = resp
        .text()
        .await
        .map_err(|e| ForageError::Download(format!("nginx.org body: {}", e)))?;

    // Parse `nginx-X.Y.Z` from the HTML.
    let mut versions: Vec<String> = Vec::new();
    for line in html.lines() {
        if let Some(idx) = line.find("nginx-") {
            let rest = &line[idx + 6..];
            // Take chars until we hit a non-version char.
            let ver: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if ver.contains('.') && !versions.contains(&ver) {
                versions.push(ver);
            }
        }
    }

    // Sort descending.
    versions.sort_by(|a, b| {
        let an: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
        let bn: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
        bn.cmp(&an)
    });

    // Keep only mainline (odd minor version) + stable (even minor). Limit
    // to last 8 to avoid an overwhelming dropdown.
    versions.truncate(8);

    if versions.is_empty() {
        return Err(ForageError::Download(
            "nginx.org returned no versions".into(),
        ));
    }
    Ok(versions)
}

// ---- MariaDB ---------------------------------------------------------------

/// Fetch MariaDB versions from downloads.mariadb.org REST API.
/// Returns JSON like:
/// ```json
/// [
///   { "release_id": "11.6.2", "release_name": "MariaDB 11.6.2", ... },
///   { "release_id": "11.4.4", ... },
///   ...
/// ]
/// ```
async fn fetch_mariadb_versions(client: &reqwest::Client) -> ForageResult<Vec<String>> {
    let url = "https://downloads.mariadb.org/rest-api/mariadb/";
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| ForageError::Download(format!("mariadb.org API: {}", e)))?;
    if !resp.status().is_success() {
        return Err(ForageError::Download(format!(
            "mariadb.org API returned {}",
            resp.status()
        )));
    }
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ForageError::Download(format!("mariadb.org parse: {}", e)))?;

    let mut versions: Vec<String> = json
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    v.get("release_id")
                        .and_then(|r| r.as_str())
                        .map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_default();

    // Group by major.minor — we want one entry per "11.6", "11.4", etc.
    // in the dropdown, not every patch version.
    let mut major_minors: Vec<String> = versions
        .iter()
        .filter_map(|v| {
            let parts: Vec<&str> = v.split('.').collect();
            if parts.len() >= 2 {
                Some(format!("{}.{}", parts[0], parts[1]))
            } else {
                None
            }
        })
        .collect();
    major_minors.sort_by(|a, b| {
        let an: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
        let bn: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
        bn.cmp(&an)
    });
    major_minors.dedup();
    major_minors.truncate(8);

    if major_minors.is_empty() {
        return Err(ForageError::Download(
            "mariadb.org API returned no versions".into(),
        ));
    }
    Ok(major_minors)
}

// ---- phpMyAdmin ------------------------------------------------------------

/// Fetch phpMyAdmin versions from the official files listing.
/// `https://www.phpmyadmin.net/files/` returns JSON-LD with all releases.
/// We extract the version numbers.
async fn fetch_phpmyadmin_versions(client: &reqwest::Client) -> ForageResult<Vec<String>> {
    // The phpmyadmin.net site uses a JSON API at this endpoint:
    let url = "https://www.phpmyadmin.net/files.json";
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| ForageError::Download(format!("phpmyadmin.net: {}", e)))?;
    if !resp.status().is_success() {
        // Fall back to scraping the files page.
        return fetch_phpmyadmin_versions_scrape(client).await;
    }
    let text = resp
        .text()
        .await
        .map_err(|e| ForageError::Download(format!("phpmyadmin.net body: {}", e)))?;

    // The files.json is JSON-LD with a "releases" array of objects
    // like { "version": "5.2.2", "date": "...", ... }.
    let mut versions: Vec<String> = Vec::new();
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
        if let Some(arr) = json
            .get("releases")
            .and_then(|r| r.as_array())
            .or_else(|| json.as_array())
        {
            for v in arr {
                if let Some(ver) = v.get("version").and_then(|v| v.as_str()) {
                    versions.push(ver.to_string());
                }
            }
        }
    }

    if versions.is_empty() {
        return fetch_phpmyadmin_versions_scrape(client).await;
    }

    // Sort descending and dedupe.
    versions.sort_by(|a, b| {
        let an: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
        let bn: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
        bn.cmp(&an)
    });
    versions.dedup();
    versions.truncate(8);
    Ok(versions)
}

/// Fallback: scrape the phpMyAdmin files page if the JSON API is unavailable.
async fn fetch_phpmyadmin_versions_scrape(client: &reqwest::Client) -> ForageResult<Vec<String>> {
    let url = "https://www.phpmyadmin.net/files/";
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| ForageError::Download(format!("phpmyadmin.net scrape: {}", e)))?;
    let html = resp
        .text()
        .await
        .map_err(|e| ForageError::Download(format!("phpmyadmin.net body: {}", e)))?;

    let mut versions: Vec<String> = Vec::new();
    // Look for hrefs like `/files/phpMyAdmin/5.2.2/`.
    for line in html.lines() {
        if let Some(idx) = line.find("phpMyAdmin/") {
            let rest = &line[idx + 11..];
            let ver: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if ver.contains('.') && !versions.contains(&ver) {
                versions.push(ver);
            }
        }
    }

    versions.sort_by(|a, b| {
        let an: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
        let bn: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
        bn.cmp(&an)
    });
    versions.truncate(8);

    if versions.is_empty() {
        return Err(ForageError::Download(
            "phpmyadmin.net scrape returned no versions".into(),
        ));
    }
    Ok(versions)
}

// ---- WordPress -------------------------------------------------------------

/// Fetch WordPress versions from the official version-check API.
/// `https://api.wordpress.org/core/version-check/1.7/` returns JSON with
/// an "offers" array — the first offer is the latest version.
async fn fetch_wordpress_versions(client: &reqwest::Client) -> ForageResult<Vec<String>> {
    let url = "https://api.wordpress.org/core/version-check/1.7/";
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| ForageError::Download(format!("wordpress.org API: {}", e)))?;
    if !resp.status().is_success() {
        return Err(ForageError::Download(format!(
            "wordpress.org API returned {}",
            resp.status()
        )));
    }
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ForageError::Download(format!("wordpress.org parse: {}", e)))?;

    let mut versions: Vec<String> = Vec::new();
    if let Some(offers) = json.get("offers").and_then(|o| o.as_array()) {
        for offer in offers {
            if let Some(v) = offer.get("version").and_then(|v| v.as_str()) {
                versions.push(v.to_string());
            }
        }
    }

    // Always include "latest" as the first option, since the wizard uses
    // it as the default for WordPress.
    versions.insert(0, "latest".to_string());

    versions.dedup();
    versions.truncate(8);
    Ok(versions)
}

// ---- Hardcoded fallbacks ---------------------------------------------------

/// These are the fallback lists if the network fetch fails. They're also
/// the lists shown in the wizard BEFORE the fetch completes, so the user
/// can start picking immediately.
pub fn hardcoded_php() -> Vec<String> {
    vec!["8.4".into(), "8.3".into(), "8.2".into(), "8.1".into()]
}
pub fn hardcoded_nginx() -> Vec<String> {
    vec!["1.27".into(), "1.26".into()]
}
pub fn hardcoded_mariadb() -> Vec<String> {
    vec!["11.6".into(), "11.4".into(), "10.11".into()]
}
pub fn hardcoded_phpmyadmin() -> Vec<String> {
    vec!["5.2.2".into()]
}
pub fn hardcoded_wordpress() -> Vec<String> {
    vec!["latest".into()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardcoded_lists_are_nonempty() {
        assert!(!hardcoded_php().is_empty());
        assert!(!hardcoded_nginx().is_empty());
        assert!(!hardcoded_mariadb().is_empty());
        assert!(!hardcoded_phpmyadmin().is_empty());
        assert!(!hardcoded_wordpress().is_empty());
    }
}
