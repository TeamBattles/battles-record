use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Cached version information, updated periodically by the background checker.
#[derive(Debug, Clone, Serialize)]
pub struct VersionInfo {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub release_notes: Option<String>,
    pub last_check: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
}

pub struct VersionChecker {
    client: reqwest::Client,
    info: Arc<RwLock<VersionInfo>>,
    enabled: bool,
}

const GITHUB_API_URL: &str =
    "https://api.github.com/repos/TeamBattles/battles-record/releases/latest";
const CHECK_INTERVAL_SECS: u64 = 6 * 60 * 60; // 6 hours

impl VersionChecker {
    pub fn new(current_version: String, enabled: bool) -> Self {
        let info = VersionInfo {
            current_version,
            latest_version: None,
            update_available: false,
            release_url: None,
            release_notes: None,
            last_check: None,
        };

        let client = reqwest::Client::builder()
            .user_agent(format!("battles-record/{}", info.current_version))
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_default();

        Self {
            client,
            info: Arc::new(RwLock::new(info)),
            enabled,
        }
    }

    pub fn get_info(&self) -> VersionInfo {
        self.info.read().clone()
    }

    pub async fn check_now(&self) {
        if !self.enabled {
            return;
        }

        let response = match self.client.get(GITHUB_API_URL).send().await {
            Ok(resp) => resp,
            Err(e) => {
                tracing::debug!(error = %e, "Failed to check for updates");
                return;
            }
        };

        if !response.status().is_success() {
            tracing::debug!(
                status = %response.status(),
                "GitHub API returned non-success status"
            );
            return;
        }

        let release: GitHubRelease = match response.json().await {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!(error = %e, "Failed to parse GitHub release response");
                return;
            }
        };

        let latest = release.tag_name.trim_start_matches('v').to_string();
        let current = &self.info.read().current_version;
        let update_available = is_newer_version(current, &latest);

        let mut info = self.info.write();
        info.update_available = update_available;
        info.latest_version = Some(latest);
        info.release_url = Some(release.html_url);
        info.release_notes = release.body;
        info.last_check = Some(Utc::now());

        if update_available {
            tracing::info!(
                current = %info.current_version,
                latest = info.latest_version.as_deref().unwrap_or("unknown"),
                "New version available"
            );
        }
    }

    pub fn spawn_background_task(checker: Arc<VersionChecker>) {
        tokio::spawn(async move {
            // Initial check after a short delay
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            checker.check_now().await;

            let mut interval =
                tokio::time::interval(std::time::Duration::from_secs(CHECK_INTERVAL_SECS));
            loop {
                interval.tick().await;
                checker.check_now().await;
            }
        });
    }
}

/// Returns true if `latest` is a newer semver than `current`.
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    let parse = |s: &str| -> (u64, u64, u64) {
        let s = s.trim_start_matches('v');
        let mut parts = s.split('.');
        let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    };

    let c = parse(current);
    let l = parse(latest);
    l > c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("1.0.0", "1.0.1"));
        assert!(is_newer_version("1.0.0", "1.1.0"));
        assert!(is_newer_version("1.0.0", "2.0.0"));
        assert!(is_newer_version("0.1.0", "1.0.0"));
    }

    #[test]
    fn test_same_version_not_newer() {
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("2.5.3", "2.5.3"));
    }

    #[test]
    fn test_older_version_not_newer() {
        assert!(!is_newer_version("1.0.1", "1.0.0"));
        assert!(!is_newer_version("2.0.0", "1.9.9"));
    }

    #[test]
    fn test_version_with_v_prefix() {
        assert!(is_newer_version("v1.0.0", "v1.0.1"));
        assert!(is_newer_version("1.0.0", "v2.0.0"));
        assert!(!is_newer_version("v1.0.1", "1.0.0"));
    }

    #[test]
    fn test_partial_versions() {
        assert!(is_newer_version("1", "2"));
        assert!(is_newer_version("1.0", "1.1"));
        assert!(!is_newer_version("2", "1"));
    }

    #[test]
    fn test_version_info_default() {
        let checker = VersionChecker::new("1.0.0".to_string(), false);
        let info = checker.get_info();
        assert_eq!(info.current_version, "1.0.0");
        assert!(info.latest_version.is_none());
        assert!(!info.update_available);
        assert!(info.release_url.is_none());
        assert!(info.last_check.is_none());
    }

    #[test]
    fn test_github_release_deserialization() {
        let json = r#"{
            "tag_name": "v1.2.0",
            "html_url": "https://github.com/TeamBattles/battles-record/releases/tag/v1.2.0",
            "body": "Release notes here"
        }"#;
        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "v1.2.0");
        assert_eq!(
            release.html_url,
            "https://github.com/TeamBattles/battles-record/releases/tag/v1.2.0"
        );
        assert_eq!(release.body, Some("Release notes here".to_string()));
    }
}
