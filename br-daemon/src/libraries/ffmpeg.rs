//! FFmpeg download, install, and update checking.
//!
//! Windows and Linux use stable releases from BtbN/FFmpeg-Builds on GitHub.
//! macOS uses evermeet.cx (BtbN doesn't build macOS binaries).

use super::platform;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::mpsc;

/// Download and install the latest stable FFmpeg.
pub async fn install_ffmpeg(
    progress_tx: Option<mpsc::Sender<(String, f64)>>,
) -> Result<PathBuf, super::LibraryError> {
    let bin_dir = platform::get_bin_dir().ok_or(super::LibraryError::NoBinDir)?;

    tokio::fs::create_dir_all(&bin_dir)
        .await
        .map_err(|e| super::LibraryError::Io {
            context: "creating bin directory".to_string(),
            source: e,
        })?;

    // On Windows/Linux: fetch latest stable URL from BtbN GitHub API
    // On macOS: use the hardcoded evermeet.cx URL
    let url = if platform::ffmpeg_btbn_asset_suffix().is_some() {
        let (_version, url) = fetch_latest_ffmpeg_info().await?;
        url
    } else {
        platform::ffmpeg_download_url()
            .ok_or_else(|| super::LibraryError::Unsupported {
                library: "ffmpeg".to_string(),
                detail: "no download URL for this platform".to_string(),
            })?
            .to_string()
    };

    send_progress(&progress_tx, "ffmpeg", 0.0).await;

    let response = reqwest::get(&url)
        .await
        .map_err(|e| super::LibraryError::DownloadFailed {
            library: "ffmpeg".to_string(),
            source: e,
        })?;

    if !response.status().is_success() {
        return Err(super::LibraryError::HttpStatus {
            library: "ffmpeg".to_string(),
            status: response.status().as_u16(),
        });
    }

    let total_size = response.content_length();
    let bytes = download_with_progress(response, total_size, "ffmpeg", &progress_tx).await?;

    send_progress(&progress_tx, "ffmpeg", 80.0).await;

    let dest = extract_ffmpeg(&bin_dir, &bytes)?;

    #[cfg(unix)]
    set_executable(&dest).await?;

    send_progress(&progress_tx, "ffmpeg", 100.0).await;

    Ok(dest)
}

/// Check if a newer stable FFmpeg is available.
/// Uses a file-based cache to avoid hitting the GitHub API more than once per 24 hours.
pub async fn check_ffmpeg_update(
    current_version: &str,
) -> Result<Option<String>, super::LibraryError> {
    // Only BtbN platforms (Windows/Linux) support update checking
    if platform::ffmpeg_btbn_asset_suffix().is_none() {
        return Ok(None);
    }

    let bin_dir = platform::get_bin_dir().ok_or(super::LibraryError::NoBinDir)?;
    let cache_path = bin_dir.join(".ffmpeg-update-cache.json");

    // Check file cache first
    if let Ok(data) = tokio::fs::read_to_string(&cache_path).await {
        if let Ok(cache) = serde_json::from_str::<FfmpegUpdateCache>(&data) {
            let now = chrono::Utc::now().timestamp();
            if now - cache.checked_at < CACHE_TTL_SECS {
                return Ok(
                    if crate::version_check::is_newer_version(current_version, &cache.latest_version)
                    {
                        Some(cache.latest_version)
                    } else {
                        None
                    },
                );
            }
        }
    }

    let (latest_version, _url) = fetch_latest_ffmpeg_info().await?;

    // Write cache
    let cache = FfmpegUpdateCache {
        latest_version: latest_version.clone(),
        checked_at: chrono::Utc::now().timestamp(),
    };
    if let Ok(json) = serde_json::to_string(&cache) {
        let _ = tokio::fs::write(&cache_path, json).await;
    }

    Ok(
        if crate::version_check::is_newer_version(current_version, &latest_version) {
            Some(latest_version)
        } else {
            None
        },
    )
}

const CACHE_TTL_SECS: i64 = 86400; // 24 hours

const BTBN_API_URL: &str =
    "https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/tags/latest";

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct FfmpegUpdateCache {
    latest_version: String,
    checked_at: i64,
}

/// Query BtbN GitHub releases for the latest stable FFmpeg version and download URL.
/// Returns (version, download_url) for the highest stable version available.
async fn fetch_latest_ffmpeg_info() -> Result<(String, String), super::LibraryError> {
    let suffix = platform::ffmpeg_btbn_asset_suffix().ok_or_else(|| {
        super::LibraryError::Unsupported {
            library: "ffmpeg".to_string(),
            detail: "BtbN builds not available for this platform".to_string(),
        }
    })?;

    let client = reqwest::Client::new();
    let resp = client
        .get(BTBN_API_URL)
        .header("User-Agent", "battles-record")
        .send()
        .await
        .map_err(|e| super::LibraryError::DownloadFailed {
            library: "ffmpeg update check".to_string(),
            source: e,
        })?;

    if !resp.status().is_success() {
        return Err(super::LibraryError::HttpStatus {
            library: "ffmpeg update check".to_string(),
            status: resp.status().as_u16(),
        });
    }

    let release: GitHubRelease =
        resp.json()
            .await
            .map_err(|e| super::LibraryError::DownloadFailed {
                library: "ffmpeg update check".to_string(),
                source: e,
            })?;

    // Find stable assets: starts with "ffmpeg-n", contains our platform suffix,
    // excludes master/shared/lgpl builds
    let mut best: Option<(String, String)> = None;
    for asset in &release.assets {
        if !asset.name.starts_with("ffmpeg-n") {
            continue;
        }
        if asset.name.contains("master") || asset.name.contains("shared") || asset.name.contains("lgpl") {
            continue;
        }
        if !asset.name.contains(suffix) {
            continue;
        }

        // Extract version from asset name suffix, e.g. "ffmpeg-n8.0-latest-win64-gpl-8.0.zip" -> "8.0"
        if let Some(version) = extract_version_from_asset(&asset.name) {
            let dominated = best.as_ref().is_some_and(|(best_ver, _)| {
                !crate::version_check::is_newer_version(best_ver, &version)
            });
            if !dominated {
                best = Some((version, asset.browser_download_url.clone()));
            }
        }
    }

    best.ok_or_else(|| super::LibraryError::Unsupported {
        library: "ffmpeg".to_string(),
        detail: "no stable FFmpeg asset found in BtbN release".to_string(),
    })
}

/// Extract version from BtbN asset name.
/// e.g. "ffmpeg-n8.0-latest-win64-gpl-8.0.zip" -> Some("8.0")
/// e.g. "ffmpeg-n7.1-latest-linux64-gpl-7.1.tar.xz" -> Some("7.1")
fn extract_version_from_asset(name: &str) -> Option<String> {
    // The version appears as the last segment before the extension: "...-X.Y.zip" or "...-X.Y.tar.xz"
    let stem = name.strip_suffix(".tar.xz").or_else(|| name.strip_suffix(".zip"))?;
    let last_dash = stem.rfind('-')?;
    let version = &stem[last_dash + 1..];
    // Validate it looks like a version number
    if version.chars().next()?.is_ascii_digit() {
        Some(version.to_string())
    } else {
        None
    }
}

/// Extract the ffmpeg binary from the downloaded archive.
fn extract_ffmpeg(
    bin_dir: &std::path::Path,
    archive_bytes: &[u8],
) -> Result<PathBuf, super::LibraryError> {
    #[cfg(target_os = "windows")]
    {
        extract_from_zip(bin_dir, archive_bytes, "ffmpeg.exe")
    }

    #[cfg(target_os = "macos")]
    {
        extract_from_zip(bin_dir, archive_bytes, "ffmpeg")
    }

    #[cfg(target_os = "linux")]
    {
        extract_from_tar_xz(bin_dir, archive_bytes, "ffmpeg")
    }
}

/// Extract a named binary from a ZIP archive.
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn extract_from_zip(
    bin_dir: &std::path::Path,
    archive_bytes: &[u8],
    binary_name: &str,
) -> Result<PathBuf, super::LibraryError> {
    let cursor = std::io::Cursor::new(archive_bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| super::LibraryError::Extraction {
            library: "ffmpeg".to_string(),
            detail: format!("failed to open zip: {}", e),
        })?;

    let dest = bin_dir.join(binary_name);
    let tmp_dest = bin_dir.join(format!("{}.tmp", binary_name));

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| super::LibraryError::Extraction {
                library: "ffmpeg".to_string(),
                detail: format!("failed to read zip entry {}: {}", i, e),
            })?;

        let name = file.name().to_string();

        // Match the binary by filename, handling nested directories
        if name.ends_with(binary_name) && !name.contains("ffprobe") && !name.contains("ffplay") {
            let mut outfile =
                std::fs::File::create(&tmp_dest).map_err(|e| super::LibraryError::Io {
                    context: format!("creating {}", tmp_dest.display()),
                    source: e,
                })?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| super::LibraryError::Io {
                context: format!("extracting ffmpeg to {}", tmp_dest.display()),
                source: e,
            })?;

            std::fs::rename(&tmp_dest, &dest).map_err(|e| super::LibraryError::Io {
                context: format!("renaming {} to {}", tmp_dest.display(), dest.display()),
                source: e,
            })?;

            return Ok(dest);
        }
    }

    Err(super::LibraryError::Extraction {
        library: "ffmpeg".to_string(),
        detail: format!("{} not found in archive", binary_name),
    })
}

/// Extract a named binary from a tar.xz archive.
#[cfg(target_os = "linux")]
fn extract_from_tar_xz(
    bin_dir: &std::path::Path,
    archive_bytes: &[u8],
    binary_name: &str,
) -> Result<PathBuf, super::LibraryError> {
    use std::io::Read;

    let cursor = std::io::Cursor::new(archive_bytes);
    let xz_decoder = xz2::read::XzDecoder::new(cursor);
    let mut archive = tar::Archive::new(xz_decoder);

    let dest = bin_dir.join(binary_name);
    let tmp_dest = bin_dir.join(format!("{}.tmp", binary_name));

    let entries = archive
        .entries()
        .map_err(|e| super::LibraryError::Extraction {
            library: "ffmpeg".to_string(),
            detail: format!("failed to read tar entries: {}", e),
        })?;

    for entry_result in entries {
        let mut entry = entry_result.map_err(|e| super::LibraryError::Extraction {
            library: "ffmpeg".to_string(),
            detail: format!("failed to read tar entry: {}", e),
        })?;

        let path = entry
            .path()
            .map_err(|e| super::LibraryError::Extraction {
                library: "ffmpeg".to_string(),
                detail: format!("failed to read entry path: {}", e),
            })?
            .to_path_buf();

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if file_name == binary_name {
            let mut outfile =
                std::fs::File::create(&tmp_dest).map_err(|e| super::LibraryError::Io {
                    context: format!("creating {}", tmp_dest.display()),
                    source: e,
                })?;

            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| super::LibraryError::Io {
                    context: "reading ffmpeg from tar".to_string(),
                    source: e,
                })?;

            use std::io::Write;
            outfile
                .write_all(&buf)
                .map_err(|e| super::LibraryError::Io {
                    context: format!("writing ffmpeg to {}", tmp_dest.display()),
                    source: e,
                })?;

            std::fs::rename(&tmp_dest, &dest).map_err(|e| super::LibraryError::Io {
                context: format!("renaming {} to {}", tmp_dest.display(), dest.display()),
                source: e,
            })?;

            return Ok(dest);
        }
    }

    Err(super::LibraryError::Extraction {
        library: "ffmpeg".to_string(),
        detail: format!("{} not found in archive", binary_name),
    })
}

use super::{download_with_progress, send_progress};

#[cfg(unix)]
use super::set_executable;

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_send_progress_none() {
        // Should not panic
        send_progress(&None, "ffmpeg", 50.0).await;
    }

    #[tokio::test]
    async fn test_send_progress_with_sender() {
        let (tx, mut rx) = mpsc::channel(10);
        send_progress(&Some(tx), "ffmpeg", 75.0).await;
        let (name, pct) = rx.recv().await.unwrap();
        assert_eq!(name, "ffmpeg");
        assert!((pct - 75.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_extract_version_from_asset_win() {
        assert_eq!(
            extract_version_from_asset("ffmpeg-n8.0-latest-win64-gpl-8.0.zip"),
            Some("8.0".to_string())
        );
    }

    #[test]
    fn test_extract_version_from_asset_linux() {
        assert_eq!(
            extract_version_from_asset("ffmpeg-n7.1-latest-linux64-gpl-7.1.tar.xz"),
            Some("7.1".to_string())
        );
    }

    #[test]
    fn test_extract_version_from_asset_master_returns_none() {
        assert_eq!(
            extract_version_from_asset("ffmpeg-master-latest-win64-gpl.zip"),
            None
        );
    }

    #[test]
    fn test_extract_version_from_asset_three_component() {
        assert_eq!(
            extract_version_from_asset("ffmpeg-n7.1.3-latest-win64-gpl-7.1.3.zip"),
            Some("7.1.3".to_string())
        );
    }

    #[test]
    fn test_ffmpeg_update_cache_serde() {
        let cache = FfmpegUpdateCache {
            latest_version: "8.0".to_string(),
            checked_at: 1700000000,
        };
        let json = serde_json::to_string(&cache).unwrap();
        let parsed: FfmpegUpdateCache = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.latest_version, "8.0");
        assert_eq!(parsed.checked_at, 1700000000);
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    #[test]
    fn test_extract_from_empty_zip_fails() {
        let bin_dir = std::env::temp_dir();
        // An empty byte slice is not a valid zip
        let result = extract_from_zip(&bin_dir, &[], "ffmpeg.exe");
        assert!(result.is_err());
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    #[test]
    fn test_extract_from_zip_missing_binary() {
        // Create a valid ZIP with a different file inside
        let buf = Vec::new();
        let cursor = std::io::Cursor::new(buf);
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("readme.txt", options).unwrap();
        std::io::Write::write_all(&mut writer, b"hello").unwrap();
        let cursor = writer.finish().unwrap();

        let bin_dir = std::env::temp_dir();
        let result = extract_from_zip(&bin_dir, cursor.get_ref(), "ffmpeg.exe");
        assert!(result.is_err());
        if let Err(super::super::LibraryError::Extraction { detail, .. }) = &result {
            assert!(detail.contains("not found in archive"));
        }
    }
}
