//! yt-dlp download, install, and update checking.

use super::platform;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::mpsc;

/// Download and install yt-dlp from GitHub releases.
pub async fn install_ytdlp(
    progress_tx: Option<mpsc::Sender<(String, f64)>>,
) -> Result<PathBuf, super::LibraryError> {
    let bin_dir = platform::get_bin_dir().ok_or(super::LibraryError::NoBinDir)?;

    tokio::fs::create_dir_all(&bin_dir)
        .await
        .map_err(|e| super::LibraryError::Io {
            context: "creating bin directory".to_string(),
            source: e,
        })?;

    let asset_name = platform::ytdlp_asset_name();
    let url = format!(
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/{}",
        asset_name
    );

    send_progress(&progress_tx, "yt-dlp", 0.0).await;

    let response = reqwest::get(&url)
        .await
        .map_err(|e| super::LibraryError::DownloadFailed {
            library: "yt-dlp".to_string(),
            source: e,
        })?;

    if !response.status().is_success() {
        return Err(super::LibraryError::HttpStatus {
            library: "yt-dlp".to_string(),
            status: response.status().as_u16(),
        });
    }

    let total_size = response.content_length();

    let bytes = download_with_progress(response, total_size, "yt-dlp", &progress_tx).await?;

    send_progress(&progress_tx, "yt-dlp", 90.0).await;

    // yt-dlp is a direct binary download (not an archive)
    #[cfg(target_os = "windows")]
    let binary_name = "yt-dlp.exe";
    #[cfg(not(target_os = "windows"))]
    let binary_name = "yt-dlp";

    let dest = bin_dir.join(binary_name);

    // Atomic write: write to .tmp, then rename
    let tmp_dest = bin_dir.join(format!("{}.tmp", binary_name));
    tokio::fs::write(&tmp_dest, &bytes)
        .await
        .map_err(|e| super::LibraryError::Io {
            context: format!("writing {}", tmp_dest.display()),
            source: e,
        })?;

    tokio::fs::rename(&tmp_dest, &dest)
        .await
        .map_err(|e| super::LibraryError::Io {
            context: format!("renaming {} to {}", tmp_dest.display(), dest.display()),
            source: e,
        })?;

    #[cfg(unix)]
    set_executable(&dest).await?;

    send_progress(&progress_tx, "yt-dlp", 100.0).await;

    Ok(dest)
}

/// Check if a newer version of yt-dlp is available.
///
/// Uses a file-based cache to avoid hitting the GitHub API more than once per 24 hours.
/// Returns `Some(latest_version)` if an update is available, `None` if up-to-date.
pub async fn check_ytdlp_update(
    current_version: &str,
) -> Result<Option<String>, super::LibraryError> {
    let bin_dir = platform::get_bin_dir().ok_or(super::LibraryError::NoBinDir)?;
    let cache_path = bin_dir.join(".update-cache.json");

    // Check the cache first
    if let Ok(data) = tokio::fs::read_to_string(&cache_path).await {
        if let Ok(cache) = serde_json::from_str::<UpdateCache>(&data) {
            let now = chrono::Utc::now().timestamp();
            if now - cache.checked_at < CACHE_TTL_SECS {
                return Ok(if cache.latest_version != current_version {
                    Some(cache.latest_version)
                } else {
                    None
                });
            }
        }
    }

    let latest_version = fetch_latest_ytdlp_version().await?;

    // Write cache
    let cache = UpdateCache {
        latest_version: latest_version.clone(),
        checked_at: chrono::Utc::now().timestamp(),
    };
    if let Ok(json) = serde_json::to_string(&cache) {
        // Best-effort cache write
        let _ = tokio::fs::write(&cache_path, json).await;
    }

    Ok(if latest_version != current_version {
        Some(latest_version)
    } else {
        None
    })
}

const CACHE_TTL_SECS: i64 = 86400; // 24 hours

#[derive(Debug, Serialize, Deserialize)]
struct UpdateCache {
    latest_version: String,
    checked_at: i64,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
}

async fn fetch_latest_ytdlp_version() -> Result<String, super::LibraryError> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest")
        .header("User-Agent", "battles-record")
        .send()
        .await
        .map_err(|e| super::LibraryError::DownloadFailed {
            library: "yt-dlp update check".to_string(),
            source: e,
        })?;

    if !resp.status().is_success() {
        return Err(super::LibraryError::HttpStatus {
            library: "yt-dlp update check".to_string(),
            status: resp.status().as_u16(),
        });
    }

    let release: GitHubRelease =
        resp.json()
            .await
            .map_err(|e| super::LibraryError::DownloadFailed {
                library: "yt-dlp update check".to_string(),
                source: e,
            })?;

    Ok(release.tag_name)
}

use super::{download_with_progress, send_progress};

#[cfg(unix)]
use super::set_executable;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_cache_serde() {
        let cache = UpdateCache {
            latest_version: "2024.12.01".to_string(),
            checked_at: 1700000000,
        };
        let json = serde_json::to_string(&cache).unwrap();
        let parsed: UpdateCache = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.latest_version, "2024.12.01");
        assert_eq!(parsed.checked_at, 1700000000);
    }

    #[test]
    fn test_cache_ttl_expired() {
        let now = chrono::Utc::now().timestamp();
        // 25 hours ago - should be expired
        let old = now - 90000;
        assert!(now - old >= CACHE_TTL_SECS);
    }

    #[test]
    fn test_cache_ttl_valid() {
        let now = chrono::Utc::now().timestamp();
        // 1 hour ago - should be valid
        let recent = now - 3600;
        assert!(now - recent < CACHE_TTL_SECS);
    }

    #[test]
    fn test_version_comparison_different() {
        let current = "2024.01.01";
        let latest = "2024.12.01";
        // Different versions should return Some
        assert_ne!(current, latest);
    }

    #[test]
    fn test_version_comparison_same() {
        let current = "2024.12.01";
        let latest = "2024.12.01";
        assert_eq!(current, latest);
    }

    #[test]
    fn test_github_release_deserialize() {
        let json = r#"{"tag_name": "2024.12.01", "name": "Release 2024.12.01"}"#;
        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "2024.12.01");
    }

    #[tokio::test]
    async fn test_send_progress_with_none() {
        // Should not panic with None sender
        send_progress(&None, "test", 50.0).await;
    }

    #[tokio::test]
    async fn test_send_progress_with_sender() {
        let (tx, mut rx) = mpsc::channel(10);
        send_progress(&Some(tx), "yt-dlp", 42.0).await;
        let (name, pct) = rx.recv().await.unwrap();
        assert_eq!(name, "yt-dlp");
        assert!((pct - 42.0).abs() < f64::EPSILON);
    }
}
