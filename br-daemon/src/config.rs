// br-daemon/src/config.rs
use crate::types::{Platform, UserRole};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_channels_file() -> Option<PathBuf> {
    None // By default, channels are stored in the main config file
}

fn default_session_duration() -> u64 {
    86400 // 24 hours in seconds
}

fn default_refresh_grace_period() -> u64 {
    3600 // 1 hour in seconds - tokens can be refreshed for this long after expiry
}

fn default_recordings_dir() -> PathBuf {
    PathBuf::from("./recordings")
}

fn default_library_dir() -> PathBuf {
    PathBuf::from("./library")
}

fn default_images_dir() -> PathBuf {
    PathBuf::from("./images")
}

fn default_disk_warning_threshold() -> u8 {
    90
}

fn default_warn_at_percent() -> u8 {
    80
}

fn default_keep_minimum() -> u32 {
    5
}

fn default_cleanup_interval() -> u32 {
    6
}

fn default_polling_interval() -> u64 {
    60 // seconds
}

fn default_playlist_interval() -> u64 {
    2 // seconds
}

fn default_stale_timeout_secs() -> u64 {
    300 // 5 minutes
}

fn default_enabled() -> bool {
    true
}

fn default_quality() -> String {
    "best".to_string()
}

fn default_max_concurrent_remux() -> u8 {
    2
}

pub fn default_true() -> bool {
    true
}

fn default_check_interval_minutes() -> u32 {
    15
}

fn default_crf() -> u8 {
    20
}

fn default_preset() -> String {
    "medium".to_string()
}

fn default_video_codec() -> String {
    "libx264".to_string()
}

fn default_audio_codec() -> String {
    "aac".to_string()
}

fn default_audio_bitrate() -> String {
    "128k".to_string()
}

fn default_output_format() -> String {
    "mp4_reencode".to_string()
}

fn default_extension_port() -> u16 {
    9555
}

fn default_fallback_ports() -> Vec<u16> {
    vec![9556, 9557]
}

fn default_downloads_dir() -> PathBuf {
    PathBuf::from("./downloads")
}

fn default_max_concurrent_downloads() -> u8 {
    3
}

fn default_download_format() -> String {
    "bestvideo[height<=1080]+bestaudio/best".to_string()
}

fn default_output_template() -> String {
    "%(title)s [%(height)sp].%(ext)s".to_string()
}

/** Main configuration structure for br-daemon. */
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub daemon: DaemonConfig,
    pub auth: AuthConfig,
    #[serde(default)]
    pub users: Vec<UserConfig>,
    pub storage: StorageConfig,
    pub polling: PollingConfig,
    pub post_processing: PostProcessingConfig,
    #[serde(default)]
    pub jellyfin: JellyfinConfig,
    #[serde(default)]
    pub notifications: NotificationsConfig,
    #[serde(default)]
    pub channels: Vec<ChannelConfig>,
    #[serde(default)]
    pub platform_auth: PlatformAuthConfig,
    #[serde(default)]
    pub oauth: OAuthConfig,
    #[serde(default)]
    pub extension: ExtensionConfig,
    #[serde(default)]
    pub downloads: DownloadsConfig,
    #[serde(default)]
    pub libraries: LibrariesConfig,
}

impl Config {
    /** Load configuration from a file path. */
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /** Load configuration from a file path, or return default if file doesn't exist. */
    pub fn load_or_default(path: &Path) -> Self {
        match Self::load(path) {
            Ok(config) => config,
            Err(e) => {
                tracing::warn!(
                    "Failed to load config from {:?}: {}. Using defaults.",
                    path,
                    e
                );
                Self::default()
            }
        }
    }

    /** Save configuration to a file path. */
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

/** Wrapper struct for channels TOML file. */
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChannelsFile {
    #[serde(default)]
    channels: Vec<ChannelConfig>,
}

/** Load channels from a separate TOML file. */
pub fn load_channels_file(path: &Path) -> Vec<ChannelConfig> {
    match std::fs::read_to_string(path) {
        Ok(content) => match toml::from_str::<ChannelsFile>(&content) {
            Ok(file) => {
                tracing::info!("Loaded {} channels from {:?}", file.channels.len(), path);
                file.channels
            }
            Err(e) => {
                tracing::warn!("Failed to parse channels from {:?}: {}", path, e);
                Vec::new()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::info!(
                "Channels file {:?} not found, starting with empty channels",
                path
            );
            Vec::new()
        }
        Err(e) => {
            tracing::warn!("Failed to read channels file {:?}: {}", path, e);
            Vec::new()
        }
    }
}

/** Save channels to a separate TOML file. */
pub fn save_channels_file(path: &Path, channels: &[ChannelConfig]) -> anyhow::Result<()> {
    let file = ChannelsFile {
        channels: channels.to_vec(),
    };
    let content = toml::to_string_pretty(&file)?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(path, content)?;
    tracing::debug!("Saved {} channels to {:?}", channels.len(), path);
    Ok(())
}

/** Configuration for the daemon server. */
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    pub log_file: Option<PathBuf>,
    /**
     * Path to a separate channels JSON file. When set, channels are persisted
     * to this file instead of the main config file. This is useful in Docker
     * where the main config is regenerated on each container start.
     */
    #[serde(default = "default_channels_file")]
    pub channels_file: Option<PathBuf>,
    #[serde(default = "default_true")]
    pub check_for_updates: bool,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            log_level: default_log_level(),
            log_file: None,
            channels_file: None,
            check_for_updates: true,
        }
    }
}

/** Configuration for authentication. */
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    pub jwt_secret: Option<String>,
    #[serde(default = "default_session_duration")]
    pub session_duration: u64,
    /** Grace period in seconds after token expiry during which refresh is still allowed. */
    #[serde(default = "default_refresh_grace_period")]
    pub refresh_grace_period: u64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: None,
            session_duration: default_session_duration(),
            refresh_grace_period: default_refresh_grace_period(),
        }
    }
}

/** Configuration for a user account. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    pub username: String,
    pub password_hash: String,
    #[serde(default)]
    pub role: UserRole,
}

/**
 * Storage directory configuration.
 *
 * Defines paths for recordings (raw segments), library (processed files),
 * and images (channel profile/banner uploads).
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    #[serde(default = "default_recordings_dir")]
    pub recordings_dir: PathBuf,
    #[serde(default = "default_library_dir")]
    pub library_dir: PathBuf,
    #[serde(default = "default_images_dir")]
    pub images_dir: PathBuf,
    #[serde(default = "default_disk_warning_threshold")]
    pub disk_warning_threshold: u8,
    #[serde(default)]
    pub quotas: QuotaConfig,
    #[serde(default)]
    pub retention: RetentionConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            recordings_dir: default_recordings_dir(),
            library_dir: default_library_dir(),
            images_dir: default_images_dir(),
            disk_warning_threshold: default_disk_warning_threshold(),
            quotas: QuotaConfig::default(),
            retention: RetentionConfig::default(),
        }
    }
}

/**
 * Storage quota configuration.
 *
 * Controls global and per-channel storage limits with warning thresholds.
 */
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct QuotaConfig {
    #[serde(default)]
    pub global_max_gb: Option<u64>,
    #[serde(default)]
    pub per_channel_max_gb: Option<u64>,
    #[serde(default = "default_warn_at_percent")]
    pub warn_at_percent: u8,
}

/**
 * Recording retention policy configuration.
 *
 * Controls automatic cleanup of old recordings based on age,
 * with a minimum keep count to prevent accidental deletion.
 */
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RetentionConfig {
    #[serde(default)]
    pub max_age_days: Option<u32>,
    #[serde(default = "default_keep_minimum")]
    pub keep_minimum: u32,
    #[serde(default = "default_cleanup_interval")]
    pub cleanup_interval_hours: u32,
}

/** Configuration for polling intervals. */
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PollingConfig {
    #[serde(default = "default_polling_interval")]
    pub default_interval: u64,
    #[serde(default = "default_playlist_interval")]
    pub playlist_interval: u64,
    /** Seconds with no new segments before assuming stream ended. */
    #[serde(default = "default_stale_timeout_secs")]
    pub stale_timeout_secs: u64,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            default_interval: default_polling_interval(),
            playlist_interval: default_playlist_interval(),
            stale_timeout_secs: default_stale_timeout_secs(),
        }
    }
}

/** What to do with original .ts segment files after post-processing. */
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SegmentHandling {
    /** Delete all .ts segment files after processing. */
    #[default]
    Delete,
    /** Concatenate all .ts segments into a single .ts file, then delete originals. */
    Concatenate,
    /** Keep all .ts segment files as-is. */
    Keep,
}

impl std::fmt::Display for SegmentHandling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SegmentHandling::Delete => write!(f, "delete"),
            SegmentHandling::Concatenate => write!(f, "concatenate"),
            SegmentHandling::Keep => write!(f, "keep"),
        }
    }
}

impl std::str::FromStr for SegmentHandling {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "delete" => Ok(SegmentHandling::Delete),
            "concatenate" => Ok(SegmentHandling::Concatenate),
            "keep" => Ok(SegmentHandling::Keep),
            _ => Err(format!("Invalid segment handling: {}", s)),
        }
    }
}

/** Configuration for post-processing of recordings. */
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PostProcessingConfig {
    /** Enable automatic post-processing. */
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /** Check interval in minutes for background reconciliation. */
    #[serde(default = "default_check_interval_minutes")]
    pub check_interval_minutes: u32,
    /** Output format: "mp4_reencode", "mp4_copy", "ts_concat". */
    #[serde(default = "default_output_format")]
    pub output_format: String,
    /** What to do with original .ts segment files after processing. */
    #[serde(default)]
    pub segment_handling: SegmentHandling,
    /** Encoding settings (used when output_format = "mp4_reencode"). */
    #[serde(default)]
    pub encoding: EncodingConfig,
    /** Path to FFmpeg binary (None = use PATH). */
    pub ffmpeg_path: Option<PathBuf>,
    /** Maximum concurrent processing jobs. */
    #[serde(default = "default_max_concurrent_remux")]
    pub max_concurrent: u8,

    // Deprecated field for backwards compatibility with old configs
    #[serde(default, skip_serializing)]
    keep_original_segments: Option<bool>,
}

impl Default for PostProcessingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_interval_minutes: default_check_interval_minutes(),
            output_format: default_output_format(),
            segment_handling: SegmentHandling::default(),
            encoding: EncodingConfig::default(),
            ffmpeg_path: None,
            max_concurrent: default_max_concurrent_remux(),
            keep_original_segments: None,
        }
    }
}

impl PostProcessingConfig {
    /** Get segment handling, migrating from old keep_original_segments if present. */
    pub fn get_segment_handling(&self) -> SegmentHandling {
        // If old field was set, migrate it
        if let Some(keep) = self.keep_original_segments {
            if keep {
                SegmentHandling::Keep
            } else {
                SegmentHandling::Delete
            }
        } else {
            self.segment_handling
        }
    }
}

/** Encoding settings for re-encoding mode. */
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EncodingConfig {
    /** CRF value (0-51, lower = better quality). */
    #[serde(default = "default_crf")]
    pub crf: u8,
    /** FFmpeg preset (ultrafast to veryslow). */
    #[serde(default = "default_preset")]
    pub preset: String,
    /** Video codec (libx264, libx265, h264_nvenc, h264_qsv). */
    #[serde(default = "default_video_codec")]
    pub video_codec: String,
    /** Audio codec (aac, copy). */
    #[serde(default = "default_audio_codec")]
    pub audio_codec: String,
    /** Audio bitrate (e.g., "128k", "192k"). */
    #[serde(default = "default_audio_bitrate")]
    pub audio_bitrate: String,
}

impl Default for EncodingConfig {
    fn default() -> Self {
        Self {
            crf: default_crf(),
            preset: default_preset(),
            video_codec: default_video_codec(),
            audio_codec: default_audio_codec(),
            audio_bitrate: default_audio_bitrate(),
        }
    }
}

/** Configuration for Jellyfin-compatible media library export. */
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct JellyfinConfig {
    /** Enable Jellyfin library export after processing. */
    #[serde(default)]
    pub enabled: bool,
    /** Download profile images from platform APIs. */
    #[serde(default = "default_true")]
    pub fetch_profile_images: bool,
    /** Extract thumbnails from video files. */
    #[serde(default = "default_true")]
    pub generate_thumbnails: bool,
}

impl Default for JellyfinConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            fetch_profile_images: true,
            generate_thumbnails: true,
        }
    }
}

/** Configuration for notifications. */
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationsConfig {
    pub discord: Option<DiscordConfig>,
    pub telegram: Option<TelegramConfig>,
    pub webhook: Option<WebhookConfig>,
}

/** Discord notification configuration. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub webhook_url: String,
    #[serde(default = "default_true")]
    pub on_stream_start: bool,
    #[serde(default = "default_true")]
    pub on_stream_end: bool,
    #[serde(default)]
    pub on_error: bool,
    #[serde(default)]
    pub on_download_complete: bool,
    #[serde(default = "default_true")]
    pub on_download_failed: bool,
}

/** Telegram notification configuration. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
    #[serde(default = "default_true")]
    pub on_stream_start: bool,
    #[serde(default = "default_true")]
    pub on_stream_end: bool,
    #[serde(default)]
    pub on_error: bool,
    #[serde(default)]
    pub on_download_complete: bool,
    #[serde(default = "default_true")]
    pub on_download_failed: bool,
}

/** Webhook notification configuration. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub on_stream_start: bool,
    #[serde(default = "default_true")]
    pub on_stream_end: bool,
    #[serde(default)]
    pub on_error: bool,
    #[serde(default)]
    pub on_download_complete: bool,
    #[serde(default = "default_true")]
    pub on_download_failed: bool,
}

/** Configuration for a channel to monitor. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub name: String,
    pub platform: Platform,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_quality")]
    pub quality: String,
    #[serde(default)]
    pub schedule: Option<ScheduleConfig>,
    #[serde(default)]
    pub filters: Option<FiltersConfig>,
    #[serde(default)]
    pub post_processing: Option<ChannelPostProcessing>,
    /** Maximum storage quota in GB (None = unlimited). */
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota_gb: Option<u32>,
    /** Retention period in days (None = unlimited). */
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u32>,
    /** Custom profile image path (relative to images_dir). */
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_profile_image: Option<String>,
    /** Custom banner image path (relative to images_dir). */
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_banner_image: Option<String>,
    /** Cached platform profile image URL (fetched from Twitch/YouTube/Kick API). */
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_profile_url: Option<String>,
    /** Cached platform banner image URL (fetched from Twitch/YouTube/Kick API). */
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_banner_url: Option<String>,
}

/** Schedule configuration for a channel. */
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScheduleConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /** Timezone for schedule rules (e.g., "America/Los_Angeles", "UTC"). */
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub rules: Vec<ScheduleRule>,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timezone: None,
            rules: Vec::new(),
        }
    }
}

/** A schedule rule defining when to record. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleRule {
    #[serde(default)]
    pub days: Vec<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

/** Filter configuration for a channel. */
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FiltersConfig {
    #[serde(default)]
    pub title_contains: Vec<String>,
    #[serde(default)]
    pub title_excludes: Vec<String>,
    #[serde(default)]
    pub game_contains: Vec<String>,
    #[serde(default)]
    pub game_excludes: Vec<String>,
    pub min_viewers: Option<u32>,
}

/** Per-channel post-processing overrides. */
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelPostProcessing {
    pub remux: Option<bool>,
    pub remux_format: Option<String>,
    /** Per-channel override for segment handling: "delete", "concatenate", "keep". */
    pub segment_handling: Option<SegmentHandling>,
    pub filename_template: Option<String>,
}

/** Configuration for platform authentication (for subscriber-only content). */
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PlatformAuthConfig {
    pub twitch: Option<PlatformCredentials>,
    pub youtube: Option<PlatformCredentials>,
    pub kick: Option<PlatformCredentials>,
}

/** Credentials for a streaming platform. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCredentials {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub last_validated: Option<DateTime<Utc>>,
}

/** OAuth client configuration for each platform. */
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OAuthConfig {
    pub twitch: Option<OAuthClientConfig>,
    pub youtube: Option<OAuthClientConfig>,
    pub kick: Option<OAuthClientConfig>,
}

/** OAuth client credentials for a single platform. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthClientConfig {
    pub client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    /** Redirect URI for OAuth callback (e.g., "battles-record://oauth/callback"). */
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,
}

/** Configuration for the browser extension WebSocket bridge. */
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtensionConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_extension_port")]
    pub port: u16,
    #[serde(default = "default_fallback_ports")]
    pub fallback_ports: Vec<u16>,
}

impl Default for ExtensionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: default_extension_port(),
            fallback_ports: default_fallback_ports(),
        }
    }
}

/** Retention policy for yt-dlp downloads. */
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DownloadRetentionConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_age_days: Option<u32>,
    #[serde(default = "default_cleanup_interval")]
    pub cleanup_interval_hours: u32,
}

impl Default for DownloadRetentionConfig {
    fn default() -> Self {
        Self {
            max_age_days: None,
            cleanup_interval_hours: default_cleanup_interval(),
        }
    }
}

/** Configuration for yt-dlp download management. */
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DownloadsConfig {
    #[serde(default = "default_downloads_dir")]
    pub directory: PathBuf,
    #[serde(default = "default_max_concurrent_downloads")]
    pub max_concurrent: u8,
    #[serde(default = "default_download_format")]
    pub default_format: String,
    #[serde(default = "default_true")]
    pub embed_thumbnail: bool,
    #[serde(default = "default_true")]
    pub embed_metadata: bool,
    #[serde(default = "default_output_template")]
    pub output_template: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_total_gb: Option<u64>,
    #[serde(default)]
    pub retention: DownloadRetentionConfig,
}

impl Default for DownloadsConfig {
    fn default() -> Self {
        Self {
            directory: default_downloads_dir(),
            max_concurrent: default_max_concurrent_downloads(),
            default_format: default_download_format(),
            embed_thumbnail: true,
            embed_metadata: true,
            output_template: default_output_template(),
            max_total_gb: None,
            retention: DownloadRetentionConfig::default(),
        }
    }
}

/** Configuration for managed library binaries (yt-dlp, FFmpeg). */
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LibrariesConfig {
    #[serde(default)]
    pub auto_update: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ytdlp_path: Option<PathBuf>,
}

impl Default for LibrariesConfig {
    fn default() -> Self {
        Self {
            auto_update: false,
            ytdlp_path: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.daemon.host, "127.0.0.1");
        assert_eq!(config.daemon.port, 8080);
        assert_eq!(config.polling.default_interval, 60);
    }

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
            [daemon]
            port = 9000
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.daemon.port, 9000);
        assert_eq!(config.daemon.host, "127.0.0.1"); // default
    }

    #[test]
    fn test_parse_channel_config() {
        let toml = r#"
            [[channels]]
            name = "teststreamer"
            platform = "twitch"
            quality = "1080p60"
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.channels.len(), 1);
        assert_eq!(config.channels[0].name, "teststreamer");
        assert_eq!(config.channels[0].platform, Platform::Twitch);
    }

    #[test]
    fn test_parse_platform_auth_config() {
        let toml = r#"
            [platform_auth.twitch]
            access_token = "test_token"
            username = "test_user"
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert!(config.platform_auth.twitch.is_some());
        let twitch = config.platform_auth.twitch.unwrap();
        assert_eq!(twitch.access_token, "test_token");
        assert_eq!(twitch.username, Some("test_user".to_string()));
    }

    #[test]
    fn test_parse_oauth_config() {
        let toml = r#"
            [oauth.twitch]
            client_id = "twitch_client_123"
            client_secret = "twitch_secret_456"
            redirect_uri = "battles-record://oauth/callback"

            [oauth.youtube]
            client_id = "google_client_789"
        "#;
        let config: Config = toml::from_str(toml).unwrap();

        // Verify Twitch OAuth config
        assert!(config.oauth.twitch.is_some());
        let twitch = config.oauth.twitch.unwrap();
        assert_eq!(twitch.client_id, "twitch_client_123");
        assert_eq!(twitch.client_secret, Some("twitch_secret_456".to_string()));
        assert_eq!(
            twitch.redirect_uri,
            Some("battles-record://oauth/callback".to_string())
        );

        // Verify YouTube OAuth config (minimal)
        assert!(config.oauth.youtube.is_some());
        let youtube = config.oauth.youtube.unwrap();
        assert_eq!(youtube.client_id, "google_client_789");
        assert!(youtube.client_secret.is_none());
        assert!(youtube.redirect_uri.is_none());

        // Verify Kick OAuth config is not set
        assert!(config.oauth.kick.is_none());
    }

    #[test]
    fn test_oauth_config_defaults_when_missing() {
        let toml = r#"
            [daemon]
            port = 8080
        "#;
        let config: Config = toml::from_str(toml).unwrap();

        // OAuth should default to None for all platforms
        assert!(config.oauth.twitch.is_none());
        assert!(config.oauth.youtube.is_none());
        assert!(config.oauth.kick.is_none());
    }
}
