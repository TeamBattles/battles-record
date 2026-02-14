use crate::types::{Platform, Quality, StreamInfo};
use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlatformError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API error: {0}")]
    Api(String),

    #[error("Authentication required")]
    AuthRequired,

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Stream offline")]
    StreamOffline,

    #[error("Parse error: {0}")]
    Parse(String),
}

pub type PlatformResult<T> = Result<T, PlatformError>;

/** URL to an HLS stream. */
#[derive(Debug, Clone)]
pub struct StreamUrl {
    pub url: String,
    pub quality: Quality,
}

/** Channel profile information for Jellyfin metadata. */
#[derive(Debug, Clone)]
pub struct ChannelProfile {
    /** Display name of the channel. */
    pub display_name: String,
    /** Channel description/bio. */
    pub description: Option<String>,
    /** URL to profile image (avatar). */
    pub profile_image_url: Option<String>,
    /** URL to banner/offline image. */
    pub banner_image_url: Option<String>,
}

#[async_trait]
pub trait StreamPlatform: Send + Sync {
    /** Get the platform type. */
    fn platform(&self) -> Platform;

    /** Check if a channel is currently live. */
    async fn check_live(&self, channel: &str) -> PlatformResult<Option<StreamInfo>>;

    /** Get available quality options for a live stream. */
    async fn get_qualities(&self, channel: &str) -> PlatformResult<Vec<Quality>>;

    /** Get the HLS playlist URL for recording at the specified quality. */
    async fn get_stream_url(&self, channel: &str, quality: &Quality) -> PlatformResult<StreamUrl>;

    /** Refresh authentication tokens if needed. */
    async fn refresh_auth(&mut self) -> PlatformResult<()> {
        Ok(()) // Default: no-op for platforms without auth
    }

    /** Get channel profile information for Jellyfin metadata. */
    async fn get_channel_profile(&self, channel: &str) -> PlatformResult<ChannelProfile>;
}
