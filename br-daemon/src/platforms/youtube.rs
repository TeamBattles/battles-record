//! YouTube platform implementation
//!
//! Implements the StreamPlatform trait for YouTube livestreams.
//! Uses yt-dlp subprocess for stream extraction, requiring Bun JS runtime.

use super::cookie_utils::get_youtube_cookie_path;
use super::{ChannelProfile, PlatformError, PlatformResult, StreamPlatform, StreamUrl};
use crate::types::{Platform, Quality, StreamInfo};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Stdio;
use thiserror::Error;
use tokio::process::Command;
use tracing::{debug, warn};

/** Errors specific to yt-dlp operations. */
#[derive(Error, Debug)]
pub enum YtdlpError {
    #[error("Bun runtime not found. Install the Bun-bundled app or install Bun separately.")]
    BunNotFound,

    #[error("yt-dlp not found. Ensure yt-dlp is installed and in PATH.")]
    YtdlpNotFound,

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Stream is not live")]
    NotLive,

    #[error("yt-dlp failed: {0}")]
    CommandFailed(String),

    #[error("Failed to parse yt-dlp output: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<YtdlpError> for PlatformError {
    fn from(err: YtdlpError) -> Self {
        match err {
            YtdlpError::ChannelNotFound(ch) => PlatformError::ChannelNotFound(ch),
            YtdlpError::NotLive => PlatformError::StreamOffline,
            _ => PlatformError::Api(err.to_string()),
        }
    }
}

/** Get potential paths for dependencies including app data bin folder. */
fn get_dependency_paths(name: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Check app data directories (platform-specific)
    // Uses Tauri's bundle identifier for the folder name
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let binary_name = format!("{}.exe", name);
            paths.push(
                PathBuf::from(appdata)
                    .join("com.battles.record")
                    .join("bin")
                    .join(&binary_name),
            );
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            paths.push(
                PathBuf::from(home)
                    .join("Library/Application Support/com.battles.record/bin")
                    .join(name),
            );
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            paths.push(
                PathBuf::from(home)
                    .join(".local/share/com.battles.record/bin")
                    .join(name),
            );
        }
    }

    paths
}

fn find_dependency(name: &str) -> Option<PathBuf> {
    // Check app bin paths first
    for path in get_dependency_paths(name) {
        if path.exists() {
            return Some(path);
        }
    }

    // Fall back to PATH
    which::which(name).ok()
}

/** Check if Bun runtime is available. */
pub fn is_bun_available() -> bool {
    if let Some(path) = find_dependency("bun") {
        std::process::Command::new(&path)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    } else {
        false
    }
}

/** Check if yt-dlp is available. */
pub fn is_ytdlp_available() -> bool {
    if let Some(path) = find_dependency("yt-dlp") {
        std::process::Command::new(&path)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    } else {
        false
    }
}

/** YouTube platform implementation using yt-dlp subprocess. */
pub struct YoutubePlatform {
    /** Whether to use cookies for authentication (checks cookie file existence). */
    use_cookies: bool,
}

impl YoutubePlatform {
    /** Create a new YoutubePlatform without authentication. */
    pub fn new() -> Self {
        Self { use_cookies: false }
    }

    /**
     * Create a YoutubePlatform with cookie authentication enabled.
     * The actual cookie file path is determined by get_youtube_cookie_path().
     */
    pub fn with_auth(_auth_token: String) -> Self {
        // The auth_token parameter is kept for API compatibility but not used.
        // Cookie path is determined dynamically via get_youtube_cookie_path().
        Self { use_cookies: true }
    }

    /** Get the cookie file path if it exists and cookies are enabled. */
    fn get_cookie_path(&self) -> Option<PathBuf> {
        if !self.use_cookies {
            return None;
        }

        match get_youtube_cookie_path() {
            Ok(path) if path.exists() => {
                debug!("Using YouTube cookie file: {:?}", path);
                Some(path)
            }
            Ok(path) => {
                warn!("YouTube cookies enabled but file not found: {:?}", path);
                None
            }
            Err(e) => {
                warn!("Failed to get YouTube cookie path: {}", e);
                None
            }
        }
    }

    /** Build the channel URL for yt-dlp queries. */
    fn build_channel_url(&self, channel: &str) -> String {
        // If already a full URL, use as-is
        if channel.starts_with("http://") || channel.starts_with("https://") {
            return channel.to_string();
        }

        // If it's a handle (@username), build the live URL
        if channel.starts_with('@') {
            return format!("https://www.youtube.com/{}/live", channel);
        }

        // Otherwise assume it's a channel name/ID
        format!("https://www.youtube.com/@{}/live", channel)
    }

    /** Execute yt-dlp with JSON output and return parsed result. */
    async fn ytdlp_json(&self, url: &str) -> Result<YtdlpVideoInfo, YtdlpError> {
        let ytdlp_path = find_dependency("yt-dlp").ok_or(YtdlpError::YtdlpNotFound)?;
        let mut cmd = Command::new(&ytdlp_path);
        cmd.arg("-j") // JSON output
            .arg("--no-download")
            .arg("--no-warnings")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Add cookies if available
        if let Some(cookie_path) = self.get_cookie_path() {
            cmd.arg("--cookies").arg(&cookie_path);
        }

        cmd.arg(url);

        debug!("Running yt-dlp -j for {}", url);
        let output = cmd.output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stderr_lower = stderr.to_lowercase();

            // Log full stderr at debug level for troubleshooting
            debug!("yt-dlp stderr for {}: {}", url, stderr);

            // Check for common error patterns (case-insensitive)
            if stderr_lower.contains("video unavailable")
                || stderr_lower.contains("not found")
                || stderr_lower.contains("does not exist")
            {
                return Err(YtdlpError::ChannelNotFound(url.to_string()));
            }
            if stderr_lower.contains("not a live stream")
                || stderr_lower.contains("is not live")
                || stderr_lower.contains("offline")
            {
                return Err(YtdlpError::NotLive);
            }

            return Err(YtdlpError::CommandFailed(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&stdout)
            .map_err(|e| YtdlpError::ParseError(format!("Failed to parse JSON: {}", e)))
    }

    /** Execute yt-dlp to get stream URL (-g flag). */
    async fn ytdlp_get_url(&self, url: &str, format_selector: &str) -> Result<String, YtdlpError> {
        let ytdlp_path = find_dependency("yt-dlp").ok_or(YtdlpError::YtdlpNotFound)?;
        let mut cmd = Command::new(&ytdlp_path);
        cmd.arg("-g") // Get URL only
            .arg("-f")
            .arg(format_selector)
            .arg("--no-warnings")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Add cookies if available
        if let Some(cookie_path) = self.get_cookie_path() {
            cmd.arg("--cookies").arg(&cookie_path);
        }

        cmd.arg(url);

        debug!("Running yt-dlp -g -f {} for {}", format_selector, url);
        let output = cmd.output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(YtdlpError::CommandFailed(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim().to_string())
    }
}

/** Map quality preset names to yt-dlp format selectors. */
fn quality_to_format_selector(quality: &Quality) -> &'static str {
    match quality.name.as_str() {
        "source" | "best" => "bestvideo+bestaudio/best",
        "1080p" | "1080p60" => "bestvideo[height<=1080]+bestaudio/best",
        "720p" | "720p60" => "bestvideo[height<=720]+bestaudio/best",
        "480p" => "bestvideo[height<=480]+bestaudio/best",
        "360p" => "bestvideo[height<=360]+bestaudio/best",
        "audio" | "audio_only" => "bestaudio/best",
        _ => "bestvideo+bestaudio/best",
    }
}

impl Default for YoutubePlatform {
    fn default() -> Self {
        Self::new()
    }
}

/** Partial yt-dlp JSON output for video info. */
#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct YtdlpVideoInfo {
    #[serde(default)]
    id: String,
    title: Option<String>,
    description: Option<String>,
    channel: Option<String>,
    channel_id: Option<String>,
    uploader: Option<String>,
    thumbnail: Option<String>,
    is_live: Option<bool>,
    view_count: Option<u64>,
    live_status: Option<String>,
    release_timestamp: Option<i64>,
    formats: Option<Vec<YtdlpFormat>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct YtdlpFormat {
    format_id: String,
    ext: Option<String>,
    height: Option<u32>,
    width: Option<u32>,
    vcodec: Option<String>,
    acodec: Option<String>,
    tbr: Option<f64>, // bitrate in kbps
    url: Option<String>,
}

#[async_trait]
impl StreamPlatform for YoutubePlatform {
    fn platform(&self) -> Platform {
        Platform::YouTube
    }

    async fn check_live(&self, channel: &str) -> PlatformResult<Option<StreamInfo>> {
        let url = self.build_channel_url(channel);

        let info = match self.ytdlp_json(&url).await {
            Ok(info) => info,
            Err(YtdlpError::NotLive) => return Ok(None),
            Err(YtdlpError::ChannelNotFound(ch)) => return Err(PlatformError::ChannelNotFound(ch)),
            Err(e) => return Err(e.into()),
        };

        // Check if actually live
        let is_live =
            info.is_live.unwrap_or(false) || info.live_status.as_deref() == Some("is_live");

        if !is_live {
            return Ok(None);
        }

        let started_at = info
            .release_timestamp
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0))
            .unwrap_or_else(Utc::now);

        Ok(Some(StreamInfo {
            title: info.title.unwrap_or_else(|| "Untitled Stream".to_string()),
            game: None, // YouTube doesn't have game categories like Twitch
            viewer_count: info.view_count.unwrap_or(0) as u32,
            started_at,
            thumbnail_url: info.thumbnail,
        }))
    }

    async fn get_qualities(&self, channel: &str) -> PlatformResult<Vec<Quality>> {
        let url = self.build_channel_url(channel);
        let info = self.ytdlp_json(&url).await?;

        let mut qualities = vec![Quality::source()];
        let mut seen_heights: HashSet<u32> = HashSet::new();

        if let Some(formats) = info.formats {
            for format in formats {
                // Skip audio-only formats for quality list
                if format.vcodec.as_deref() == Some("none") {
                    continue;
                }

                if let Some(height) = format.height {
                    if height > 0 && !seen_heights.contains(&height) {
                        seen_heights.insert(height);
                        qualities.push(Quality {
                            name: format!("{}p", height),
                            resolution: format.width.map(|w| format!("{}x{}", w, height)),
                            bandwidth: format.tbr.map(|b| (b * 1000.0) as u64),
                        });
                    }
                }
            }
        }

        // Sort by height descending (source first, then highest to lowest)
        qualities.sort_by(|a, b| {
            let height_a = a
                .name
                .trim_end_matches('p')
                .parse::<u32>()
                .unwrap_or(u32::MAX);
            let height_b = b
                .name
                .trim_end_matches('p')
                .parse::<u32>()
                .unwrap_or(u32::MAX);
            height_b.cmp(&height_a)
        });

        Ok(qualities)
    }

    async fn get_stream_url(&self, channel: &str, quality: &Quality) -> PlatformResult<StreamUrl> {
        let url = self.build_channel_url(channel);
        let format_selector = quality_to_format_selector(quality);

        let stream_url = self.ytdlp_get_url(&url, format_selector).await?;

        // yt-dlp may return multiple URLs (video + audio), take the first line
        let first_url = stream_url
            .lines()
            .next()
            .ok_or_else(|| PlatformError::Api("No stream URL returned".to_string()))?;

        Ok(StreamUrl {
            url: first_url.to_string(),
            quality: quality.clone(),
        })
    }

    async fn get_channel_profile(&self, channel: &str) -> PlatformResult<ChannelProfile> {
        // For profile, we don't need /live - just the channel page
        let url = if channel.starts_with("http") {
            channel.to_string()
        } else if channel.starts_with('@') {
            format!("https://www.youtube.com/{}", channel)
        } else {
            format!("https://www.youtube.com/@{}", channel)
        };

        let info = match self.ytdlp_json(&url).await {
            Ok(info) => info,
            Err(YtdlpError::NotLive) => {
                // For profile, NotLive is OK - we just want channel info
                warn!(
                    "Channel {} is not live, profile info may be limited",
                    channel
                );
                YtdlpVideoInfo::default()
            }
            Err(YtdlpError::ChannelNotFound(ch)) => {
                return Err(PlatformError::ChannelNotFound(ch));
            }
            Err(e) => {
                warn!("Failed to fetch profile for {}: {}", channel, e);
                YtdlpVideoInfo::default()
            }
        };

        Ok(ChannelProfile {
            display_name: info
                .channel
                .or(info.uploader)
                .unwrap_or_else(|| channel.trim_start_matches('@').to_string()),
            description: info.description,
            profile_image_url: info.thumbnail,
            banner_image_url: None, // yt-dlp doesn't provide banner
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_youtube_platform_new() {
        let platform = YoutubePlatform::new();
        assert_eq!(platform.platform(), Platform::YouTube);
        assert!(!platform.use_cookies);
    }

    #[test]
    fn test_youtube_platform_with_auth() {
        let platform = YoutubePlatform::with_auth("test_token".to_string());
        assert_eq!(platform.platform(), Platform::YouTube);
        // with_auth enables cookie usage
        assert!(platform.use_cookies);
    }

    #[test]
    fn test_youtube_platform_default() {
        let platform = YoutubePlatform::default();
        assert_eq!(platform.platform(), Platform::YouTube);
        assert!(!platform.use_cookies);
    }

    #[test]
    fn test_get_cookie_path_disabled() {
        let platform = YoutubePlatform::new();
        // When use_cookies is false, get_cookie_path should return None
        assert!(platform.get_cookie_path().is_none());
    }

    #[test]
    fn test_build_channel_url_handle() {
        let platform = YoutubePlatform::new();
        let url = platform.build_channel_url("@MrBeast");
        assert_eq!(url, "https://www.youtube.com/@MrBeast/live");
    }

    #[test]
    fn test_build_channel_url_plain_name() {
        let platform = YoutubePlatform::new();
        let url = platform.build_channel_url("pewdiepie");
        assert_eq!(url, "https://www.youtube.com/@pewdiepie/live");
    }

    #[test]
    fn test_build_channel_url_full_url() {
        let platform = YoutubePlatform::new();
        let url = platform.build_channel_url("https://www.youtube.com/watch?v=abc123");
        assert_eq!(url, "https://www.youtube.com/watch?v=abc123");
    }

    #[test]
    fn test_quality_to_format_selector() {
        assert_eq!(
            quality_to_format_selector(&Quality::source()),
            "bestvideo+bestaudio/best"
        );
        assert_eq!(
            quality_to_format_selector(&Quality {
                name: "1080p".to_string(),
                resolution: None,
                bandwidth: None,
            }),
            "bestvideo[height<=1080]+bestaudio/best"
        );
        assert_eq!(
            quality_to_format_selector(&Quality {
                name: "720p".to_string(),
                resolution: None,
                bandwidth: None,
            }),
            "bestvideo[height<=720]+bestaudio/best"
        );
        assert_eq!(
            quality_to_format_selector(&Quality {
                name: "audio".to_string(),
                resolution: None,
                bandwidth: None,
            }),
            "bestaudio/best"
        );
    }

    #[test]
    fn test_ytdlp_error_to_platform_error() {
        let err: PlatformError = YtdlpError::ChannelNotFound("test".to_string()).into();
        assert!(matches!(err, PlatformError::ChannelNotFound(_)));

        let err: PlatformError = YtdlpError::NotLive.into();
        assert!(matches!(err, PlatformError::StreamOffline));

        let err: PlatformError = YtdlpError::BunNotFound.into();
        assert!(matches!(err, PlatformError::Api(_)));
    }

    #[test]
    fn test_parse_ytdlp_video_info() {
        let json = r#"{
            "id": "abc123",
            "title": "Test Stream",
            "channel": "TestChannel",
            "is_live": true,
            "view_count": 1000,
            "thumbnail": "https://example.com/thumb.jpg",
            "formats": [
                {"format_id": "1", "height": 1080, "width": 1920, "vcodec": "avc1", "tbr": 5000.0},
                {"format_id": "2", "height": 720, "width": 1280, "vcodec": "avc1", "tbr": 2500.0}
            ]
        }"#;

        let info: YtdlpVideoInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, "abc123");
        assert_eq!(info.title, Some("Test Stream".to_string()));
        assert_eq!(info.is_live, Some(true));
        assert_eq!(info.formats.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_parse_ytdlp_video_info_minimal() {
        let json = r#"{"id": "abc123"}"#;
        let info: YtdlpVideoInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, "abc123");
        assert!(info.title.is_none());
        assert!(info.is_live.is_none());
    }
}
