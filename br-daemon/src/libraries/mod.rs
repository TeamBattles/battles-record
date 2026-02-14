//! Managed library resolution, installation, and update checking.
//!
//! Provides a unified interface for locating, installing, and updating
//! external binaries (yt-dlp, FFmpeg, Bun) used by the daemon.

pub mod ffmpeg;
pub mod platform;
pub mod ytdlp;

use crate::config::LibrariesConfig;
use serde::Serialize;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::sync::mpsc;

/// Errors from library management operations.
#[derive(thiserror::Error, Debug)]
pub enum LibraryError {
    #[error("Failed to download {library}: {source}")]
    DownloadFailed {
        library: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("HTTP {status} downloading {library}")]
    HttpStatus { library: String, status: u16 },

    #[error("Failed to extract {library}: {detail}")]
    Extraction { library: String, detail: String },

    #[error("I/O error ({context}): {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Cannot determine binary directory")]
    NoBinDir,

    #[error("Unsupported platform for {library}: {detail}")]
    Unsupported { library: String, detail: String },

    #[error("Library not found in managed directory: {library}")]
    NotFound { library: String },
}

// -- Shared helpers used by ytdlp.rs and ffmpeg.rs --

/// Download response body with progress reporting.
pub(crate) async fn download_with_progress(
    response: reqwest::Response,
    total_size: Option<u64>,
    library: &str,
    progress_tx: &Option<mpsc::Sender<(String, f64)>>,
) -> Result<bytes::Bytes, LibraryError> {
    use futures::StreamExt;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut buf = Vec::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| LibraryError::DownloadFailed {
            library: library.to_string(),
            source: e,
        })?;
        downloaded += chunk.len() as u64;
        buf.extend_from_slice(&chunk);

        if let Some(total) = total_size {
            let pct = (downloaded as f64 / total as f64) * 80.0;
            send_progress(progress_tx, library, pct).await;
        }
    }

    Ok(bytes::Bytes::from(buf))
}

pub(crate) async fn send_progress(
    tx: &Option<mpsc::Sender<(String, f64)>>,
    library: &str,
    progress: f64,
) {
    if let Some(tx) = tx {
        let _ = tx.send((library.to_string(), progress)).await;
    }
}

#[cfg(unix)]
pub(crate) async fn set_executable(path: &std::path::Path) -> Result<(), LibraryError> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o755);
    tokio::fs::set_permissions(path, perms)
        .await
        .map_err(|e| LibraryError::Io {
            context: format!("setting executable permissions on {}", path.display()),
            source: e,
        })
}

/// Status of all managed libraries.
#[derive(Debug, Clone, Serialize)]
pub struct LibraryStatus {
    pub ytdlp: LibraryInfo,
    pub ffmpeg: LibraryInfo,
    pub bun: LibraryInfo,
}

/// Information about a single managed library binary.
#[derive(Debug, Clone, Serialize)]
pub struct LibraryInfo {
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<PathBuf>,
    pub update_available: Option<String>,
}

/// Manages resolution, installation, and updates for external library binaries.
pub struct LibraryManager {
    config: LibrariesConfig,
    ffmpeg_path: Option<PathBuf>,
    cached_status: Option<LibraryStatus>,
}

impl LibraryManager {
    pub fn new(config: LibrariesConfig, ffmpeg_path: Option<PathBuf>) -> Self {
        Self {
            config,
            ffmpeg_path,
            cached_status: None,
        }
    }

    /// Check status of all managed libraries.
    pub async fn check_status(&self) -> LibraryStatus {
        LibraryStatus {
            ytdlp: self.check_binary("yt-dlp", self.resolve_ytdlp()).await,
            ffmpeg: self.check_binary("ffmpeg", self.resolve_ffmpeg()).await,
            bun: self.check_binary("bun", self.resolve_bun()).await,
        }
    }

    /// Resolve the yt-dlp binary path using config override, bin dir, then PATH.
    pub fn resolve_ytdlp(&self) -> Option<PathBuf> {
        platform::resolve_binary("yt-dlp", self.config.ytdlp_path.as_ref())
    }

    /// Resolve the FFmpeg binary path using config override, bin dir, then PATH.
    pub fn resolve_ffmpeg(&self) -> Option<PathBuf> {
        platform::resolve_binary("ffmpeg", self.ffmpeg_path.as_ref())
    }

    /// Resolve the Bun runtime binary path using bin dir, then PATH.
    pub fn resolve_bun(&self) -> Option<PathBuf> {
        platform::resolve_binary("bun", None)
    }

    /// Install both yt-dlp and FFmpeg if they aren't already available.
    pub async fn install_all(
        &mut self,
        progress_tx: Option<mpsc::Sender<(String, f64)>>,
    ) -> Result<(), LibraryError> {
        if self.resolve_ytdlp().is_none() {
            let path = ytdlp::install_ytdlp(progress_tx.clone()).await?;
            tracing::info!(path = %path.display(), "yt-dlp installed");
        }

        if self.resolve_ffmpeg().is_none() {
            let path = ffmpeg::install_ffmpeg(progress_tx).await?;
            tracing::info!(path = %path.display(), "ffmpeg installed");
            self.ffmpeg_path = Some(path);
        }

        self.cached_status = None;
        Ok(())
    }

    /// Download and install (or reinstall) yt-dlp.
    pub async fn update_ytdlp(
        &mut self,
        progress_tx: Option<mpsc::Sender<(String, f64)>>,
    ) -> Result<(), LibraryError> {
        let path = ytdlp::install_ytdlp(progress_tx).await?;
        tracing::info!(path = %path.display(), "yt-dlp updated");
        self.cached_status = None;
        Ok(())
    }

    /// Download and install (or reinstall) FFmpeg.
    pub async fn update_ffmpeg(
        &mut self,
        progress_tx: Option<mpsc::Sender<(String, f64)>>,
    ) -> Result<(), LibraryError> {
        let path = ffmpeg::install_ffmpeg(progress_tx).await?;
        tracing::info!(path = %path.display(), "ffmpeg updated");
        self.ffmpeg_path = Some(path);
        self.cached_status = None;
        Ok(())
    }

    /// Remove a managed library binary from the app's bin directory.
    /// Only removes binaries in the managed dir, not system PATH installs.
    pub async fn uninstall_library(&mut self, name: &str) -> Result<(), LibraryError> {
        let bin_dir = platform::get_bin_dir().ok_or(LibraryError::NoBinDir)?;
        let binary_name = platform::platform_binary_name(name);
        let binary_path = bin_dir.join(&binary_name);

        if !binary_path.exists() {
            return Err(LibraryError::NotFound {
                library: name.to_string(),
            });
        }

        tokio::fs::remove_file(&binary_path)
            .await
            .map_err(|e| LibraryError::Io {
                context: format!("removing {}", binary_path.display()),
                source: e,
            })?;

        tracing::info!(library = name, path = %binary_path.display(), "Library uninstalled");

        // Clean up cache files
        if name == "yt-dlp" {
            let _ = tokio::fs::remove_file(bin_dir.join(".update-cache.json")).await;
        } else if name == "ffmpeg" {
            self.ffmpeg_path = None;
            let _ = tokio::fs::remove_file(bin_dir.join(".ffmpeg-update-cache.json")).await;
        }

        self.cached_status = None;
        Ok(())
    }

    /// Check for yt-dlp updates (24h cache). Updates `LibraryStatus.update_available` fields.
    pub async fn check_updates(&mut self) -> Result<(), LibraryError> {
        let status = self.check_status().await;

        let ytdlp_update = if let Some(ref version) = status.ytdlp.version {
            ytdlp::check_ytdlp_update(version).await?
        } else {
            None
        };

        // Store the update info in cached status
        let mut updated = status;
        updated.ytdlp.update_available = ytdlp_update;
        let ffmpeg_update = if let Some(ref version) = updated.ffmpeg.version {
            match ffmpeg::check_ffmpeg_update(version).await {
                Ok(update) => update,
                Err(e) => {
                    tracing::debug!(error = %e, "Failed to check FFmpeg updates");
                    None
                }
            }
        } else {
            None
        };
        updated.ffmpeg.update_available = ffmpeg_update;
        self.cached_status = Some(updated);

        Ok(())
    }

    /// Get the most recently cached status (including update info), or compute fresh.
    pub async fn status_with_updates(&self) -> LibraryStatus {
        match self.cached_status.clone() {
            Some(status) => status,
            None => self.check_status().await,
        }
    }

    /// Check a single binary: try to run its version flag to get version info.
    async fn check_binary(&self, name: &str, resolved_path: Option<PathBuf>) -> LibraryInfo {
        let Some(path) = resolved_path else {
            return LibraryInfo {
                installed: false,
                version: None,
                path: None,
                update_available: None,
            };
        };

        // FFmpeg uses single-dash `-version`, everything else uses `--version`
        let version_arg = match name {
            "ffmpeg" => "-version",
            _ => "--version",
        };

        let version = tokio::process::Command::new(&path)
            .arg(version_arg)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .await
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    Some(extract_version_string(name, &raw))
                } else {
                    None
                }
            });

        LibraryInfo {
            installed: true,
            version,
            path: Some(path),
            update_available: None,
        }
    }
}

/// Extract a clean version string from command output.
fn extract_version_string(name: &str, raw: &str) -> String {
    match name {
        "ffmpeg" => parse_ffmpeg_version(raw)
            .unwrap_or_else(|| raw.lines().next().unwrap_or(raw).to_string()),
        _ => {
            // yt-dlp and bun just output the version string directly
            raw.lines().next().unwrap_or(raw).to_string()
        }
    }
}

/// Parse the version number from ffmpeg -version output.
/// Input like "ffmpeg version 7.1-full_build-www.gyan.dev Copyright ..."
/// returns "7.1". Handles formats like "7.1", "7.1.2", "n8.0", "N-xxxxx-g...".
fn parse_ffmpeg_version(output: &str) -> Option<String> {
    let first_line = output.lines().next()?;
    let after_prefix = first_line.strip_prefix("ffmpeg version ")?;
    let token = after_prefix.split_whitespace().next()?;
    // Strip build metadata after version (e.g. "7.1-full_build..." -> "7.1")
    // Only split on hyphen if the part before it looks like a version number
    let version_part = token.split('-').next().unwrap_or(token);
    // Strip 'n' prefix from BtbN stable builds (e.g. "n8.0" -> "8.0")
    let version_part = version_part
        .strip_prefix('n')
        .unwrap_or(version_part);
    if version_part.chars().next()?.is_ascii_digit() {
        Some(version_part.to_string())
    } else {
        // Fallback: return the whole token if it doesn't start with a digit
        Some(token.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::platform::resolve_binary;
    use super::*;

    #[test]
    fn test_resolve_nonexistent_binary() {
        let result = resolve_binary("nonexistent_binary_xyz_12345", None);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_library_manager_new() {
        let config = LibrariesConfig::default();
        let mgr = LibraryManager::new(config, None);
        // Should not panic
        let _status = mgr.check_status().await;
    }

    #[test]
    fn test_extract_version_string_ffmpeg() {
        let raw = "ffmpeg version 6.1.1 Copyright (c) 2000-2024";
        assert_eq!(extract_version_string("ffmpeg", raw), "6.1.1");
    }

    #[test]
    fn test_parse_ffmpeg_version_with_build_suffix() {
        let raw =
            "ffmpeg version 7.1-full_build-www.gyan.dev Copyright (c) 2000-2024 the FFmpeg developers";
        assert_eq!(parse_ffmpeg_version(raw), Some("7.1".to_string()));
    }

    #[test]
    fn test_parse_ffmpeg_version_semver() {
        let raw = "ffmpeg version 6.1.1 Copyright (c) 2000-2024";
        assert_eq!(parse_ffmpeg_version(raw), Some("6.1.1".to_string()));
    }

    #[test]
    fn test_parse_ffmpeg_version_n_prefix() {
        // Some builds use N-xxxxx format; not a clean version so return whole token
        let raw = "ffmpeg version N-12345-gabcdef Copyright (c) 2000-2024";
        assert_eq!(
            parse_ffmpeg_version(raw),
            Some("N-12345-gabcdef".to_string())
        );
    }

    #[test]
    fn test_parse_ffmpeg_version_btbn_stable() {
        // BtbN stable builds use lowercase "n" prefix (e.g. "n8.0")
        let raw = "ffmpeg version n8.0 Copyright (c) 2000-2025 the FFmpeg developers";
        assert_eq!(parse_ffmpeg_version(raw), Some("8.0".to_string()));
    }

    #[test]
    fn test_parse_ffmpeg_version_btbn_stable_with_commits() {
        let raw = "ffmpeg version n8.0-43-gabcdef Copyright (c) 2000-2025";
        assert_eq!(parse_ffmpeg_version(raw), Some("8.0".to_string()));
    }

    #[test]
    fn test_parse_ffmpeg_version_no_prefix() {
        let raw = "not ffmpeg output";
        assert_eq!(parse_ffmpeg_version(raw), None);
    }

    #[test]
    fn test_extract_version_string_ytdlp() {
        let raw = "2024.01.01";
        assert_eq!(extract_version_string("yt-dlp", raw), "2024.01.01");
    }

    #[test]
    fn test_extract_version_string_bun() {
        let raw = "1.0.25";
        assert_eq!(extract_version_string("bun", raw), "1.0.25");
    }

    #[test]
    fn test_library_error_display() {
        let err = LibraryError::NoBinDir;
        assert_eq!(err.to_string(), "Cannot determine binary directory");

        let err = LibraryError::HttpStatus {
            library: "yt-dlp".to_string(),
            status: 404,
        };
        assert_eq!(err.to_string(), "HTTP 404 downloading yt-dlp");

        let err = LibraryError::Extraction {
            library: "ffmpeg".to_string(),
            detail: "corrupt archive".to_string(),
        };
        assert_eq!(err.to_string(), "Failed to extract ffmpeg: corrupt archive");
    }

    #[test]
    fn test_library_status_defaults() {
        let info = LibraryInfo {
            installed: false,
            version: None,
            path: None,
            update_available: None,
        };
        assert!(!info.installed);
        assert!(info.version.is_none());
        assert!(info.path.is_none());
        assert!(info.update_available.is_none());
    }

    #[tokio::test]
    async fn test_status_with_updates_without_cache() {
        let config = LibrariesConfig::default();
        let mgr = LibraryManager::new(config, None);
        // Without calling check_updates, should still return valid status
        let status = mgr.status_with_updates().await;
        // All should be not installed (no real binaries in test env)
        // Just verify it doesn't panic
        let _ = status.ytdlp.installed;
    }
}
