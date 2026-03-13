// br-daemon/src/platforms/mock.rs
//! Mock implementation of StreamPlatform for testing.
//!
//! This module provides a configurable mock platform that can simulate
//! various scenarios like live streams, offline channels, errors, and delays.

use crate::platforms::traits::{
    ChannelProfile, PlatformError, PlatformResult, StreamPlatform, StreamUrl,
};
use crate::types::{Platform, Quality, StreamInfo};
use async_trait::async_trait;
use chrono::Utc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/** Error type to return for a mocked channel (must be Clone-able). */
#[derive(Debug, Clone)]
pub enum MockError {
    /** Network error with message. */
    Network(String),
    /** API error with message. */
    Api(String),
    /** Authentication required. */
    AuthRequired,
    /** Channel not found. */
    ChannelNotFound(String),
    /** Stream is offline. */
    StreamOffline,
    /** Parse error. */
    Parse(String),
}

impl MockError {
    /** Convert to PlatformError. */
    pub fn to_platform_error(&self) -> PlatformError {
        match self {
            MockError::Network(msg) => PlatformError::Api(format!("Network: {}", msg)),
            MockError::Api(msg) => PlatformError::Api(msg.clone()),
            MockError::AuthRequired => PlatformError::AuthRequired,
            MockError::ChannelNotFound(ch) => PlatformError::ChannelNotFound(ch.clone()),
            MockError::StreamOffline => PlatformError::StreamOffline,
            MockError::Parse(msg) => PlatformError::Parse(msg.clone()),
        }
    }
}

/** Configuration for a mocked channel. */
#[derive(Debug, Clone)]
pub struct MockChannelConfig {
    /** Whether the channel is currently live. */
    pub is_live: bool,
    /** Stream info if live. */
    pub stream_info: Option<StreamInfo>,
    /** Available qualities. */
    pub qualities: Vec<Quality>,
    /** HLS playlist URL. */
    pub stream_url: Option<String>,
    /** Simulated latency for API calls. */
    pub latency: Option<Duration>,
    /** Error to return instead of success. */
    pub error: Option<MockError>,
    /** Channel profile information. */
    pub profile: Option<ChannelProfile>,
}

impl Default for MockChannelConfig {
    fn default() -> Self {
        Self {
            is_live: false,
            stream_info: None,
            qualities: vec![Quality::source()],
            stream_url: Some("https://mock.stream/playlist.m3u8".to_string()),
            latency: None,
            error: None,
            profile: Some(ChannelProfile {
                display_name: "Mock Channel".to_string(),
                description: Some("A mock channel for testing".to_string()),
                profile_image_url: Some("https://mock.cdn/avatar.jpg".to_string()),
                banner_image_url: None,
            }),
        }
    }
}

impl MockChannelConfig {
    /** Create a live channel configuration. */
    pub fn live(title: &str) -> Self {
        Self {
            is_live: true,
            stream_info: Some(StreamInfo {
                title: title.to_string(),
                game: Some("Just Chatting".to_string()),
                viewer_count: 1000,
                started_at: Utc::now(),
                thumbnail_url: Some("https://mock.cdn/thumbnail.jpg".to_string()),
            }),
            qualities: vec![
                Quality {
                    name: "source".to_string(),
                    resolution: Some("1920x1080".to_string()),
                    bandwidth: Some(6000000),
                },
                Quality {
                    name: "720p".to_string(),
                    resolution: Some("1280x720".to_string()),
                    bandwidth: Some(3000000),
                },
            ],
            ..Default::default()
        }
    }

    /** Create an offline channel configuration. */
    pub fn offline() -> Self {
        Self {
            is_live: false,
            stream_info: None,
            ..Default::default()
        }
    }

    /** Create a configuration that returns an error. */
    pub fn with_error(error: MockError) -> Self {
        Self {
            error: Some(error),
            ..Default::default()
        }
    }

    /** Add simulated latency. */
    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = Some(latency);
        self
    }

    /** Set the stream URL. */
    pub fn with_url(mut self, url: &str) -> Self {
        self.stream_url = Some(url.to_string());
        self
    }

    /** Set the stream info. */
    pub fn with_stream_info(mut self, info: StreamInfo) -> Self {
        self.stream_info = Some(info);
        self.is_live = true;
        self
    }

    /** Set available qualities. */
    pub fn with_qualities(mut self, qualities: Vec<Quality>) -> Self {
        self.qualities = qualities;
        self
    }
}

/** Mock implementation of StreamPlatform for testing. */
pub struct MockPlatform {
    /** Platform type to return. */
    platform: Platform,
    /** Channel configurations (keyed by channel name). */
    channels: Arc<RwLock<HashMap<String, MockChannelConfig>>>,
    /** Default configuration for unknown channels. */
    default_config: MockChannelConfig,
    /** Number of times check_live was called. */
    check_live_count: Arc<RwLock<u32>>,
    /** Number of times get_qualities was called. */
    get_qualities_count: Arc<RwLock<u32>>,
    /** Number of times get_stream_url was called. */
    get_stream_url_count: Arc<RwLock<u32>>,
}

impl MockPlatform {
    /** Create a new mock platform. */
    pub fn new(platform: Platform) -> Self {
        Self {
            platform,
            channels: Arc::new(RwLock::new(HashMap::new())),
            default_config: MockChannelConfig::offline(),
            check_live_count: Arc::new(RwLock::new(0)),
            get_qualities_count: Arc::new(RwLock::new(0)),
            get_stream_url_count: Arc::new(RwLock::new(0)),
        }
    }

    /** Create a mock Twitch platform. */
    pub fn twitch() -> Self {
        Self::new(Platform::Twitch)
    }

    /** Create a mock YouTube platform. */
    pub fn youtube() -> Self {
        Self::new(Platform::YouTube)
    }

    /** Create a mock Kick platform. */
    pub fn kick() -> Self {
        Self::new(Platform::Kick)
    }

    /** Set the configuration for a specific channel. */
    pub fn set_channel(&self, channel: &str, config: MockChannelConfig) {
        let mut channels = self.channels.write();
        channels.insert(channel.to_string(), config);
    }

    /** Set a channel to be live. */
    pub fn set_live(&self, channel: &str, title: &str) {
        self.set_channel(channel, MockChannelConfig::live(title));
    }

    /** Set a channel to be offline. */
    pub fn set_offline(&self, channel: &str) {
        self.set_channel(channel, MockChannelConfig::offline());
    }

    /** Set a channel to return an error. */
    pub fn set_error(&self, channel: &str, error: MockError) {
        self.set_channel(channel, MockChannelConfig::with_error(error));
    }

    /** Set the default configuration for unknown channels. */
    pub fn set_default(&mut self, config: MockChannelConfig) {
        self.default_config = config;
    }

    /** Get the number of times check_live was called. */
    pub fn check_live_call_count(&self) -> u32 {
        *self.check_live_count.read()
    }

    /** Get the number of times get_qualities was called. */
    pub fn get_qualities_call_count(&self) -> u32 {
        *self.get_qualities_count.read()
    }

    /** Get the number of times get_stream_url was called. */
    pub fn get_stream_url_call_count(&self) -> u32 {
        *self.get_stream_url_count.read()
    }

    /** Reset all call counts. */
    pub fn reset_counts(&self) {
        *self.check_live_count.write() = 0;
        *self.get_qualities_count.write() = 0;
        *self.get_stream_url_count.write() = 0;
    }

    /** Get the configuration for a channel. */
    fn get_config(&self, channel: &str) -> MockChannelConfig {
        let channels = self.channels.read();
        channels
            .get(channel)
            .cloned()
            .unwrap_or_else(|| self.default_config.clone())
    }

    /** Apply latency if configured. */
    async fn apply_latency(&self, config: &MockChannelConfig) {
        if let Some(latency) = config.latency {
            tokio::time::sleep(latency).await;
        }
    }
}

#[async_trait]
impl StreamPlatform for MockPlatform {
    fn platform(&self) -> Platform {
        self.platform
    }

    async fn check_live(&self, channel: &str) -> PlatformResult<Option<StreamInfo>> {
        *self.check_live_count.write() += 1;

        let config = self.get_config(channel);
        self.apply_latency(&config).await;

        // Return error if configured
        if let Some(error) = config.error {
            return Err(error.to_platform_error());
        }

        // Return stream info if live
        if config.is_live {
            Ok(config.stream_info)
        } else {
            Ok(None)
        }
    }

    async fn get_qualities(&self, channel: &str) -> PlatformResult<Vec<Quality>> {
        *self.get_qualities_count.write() += 1;

        let config = self.get_config(channel);
        self.apply_latency(&config).await;

        // Return error if configured
        if let Some(error) = config.error {
            return Err(error.to_platform_error());
        }

        // Return error if offline
        if !config.is_live {
            return Err(PlatformError::StreamOffline);
        }

        Ok(config.qualities)
    }

    async fn get_stream_url(&self, channel: &str, quality: &Quality) -> PlatformResult<StreamUrl> {
        *self.get_stream_url_count.write() += 1;

        let config = self.get_config(channel);
        self.apply_latency(&config).await;

        // Return error if configured
        if let Some(error) = config.error {
            return Err(error.to_platform_error());
        }

        // Return error if offline
        if !config.is_live {
            return Err(PlatformError::StreamOffline);
        }

        // Return the stream URL
        let url = config
            .stream_url
            .ok_or_else(|| PlatformError::Api("No stream URL configured for mock".to_string()))?;

        Ok(StreamUrl {
            url,
            quality: quality.clone(),
        })
    }

    async fn refresh_auth(&mut self) -> PlatformResult<()> {
        // Mock always succeeds
        Ok(())
    }

    async fn get_channel_profile(&self, channel: &str) -> PlatformResult<ChannelProfile> {
        let config = self.get_config(channel);
        self.apply_latency(&config).await;

        // Return error if configured
        if let Some(error) = config.error {
            return Err(error.to_platform_error());
        }

        config
            .profile
            .ok_or_else(|| PlatformError::ChannelNotFound(channel.to_string()))
    }
}

/**
 * Builder Pattern for Complex Test Scenarios
 */

/** Builder for creating complex mock platform configurations. */
pub struct MockPlatformBuilder {
    platform: MockPlatform,
}

impl MockPlatformBuilder {
    /** Create a new builder for the given platform. */
    pub fn new(platform: Platform) -> Self {
        Self {
            platform: MockPlatform::new(platform),
        }
    }

    /** Add a live channel. */
    pub fn with_live_channel(self, channel: &str, title: &str) -> Self {
        self.platform.set_live(channel, title);
        self
    }

    /** Add an offline channel. */
    pub fn with_offline_channel(self, channel: &str) -> Self {
        self.platform.set_offline(channel);
        self
    }

    /** Add a channel with custom configuration. */
    pub fn with_channel(self, channel: &str, config: MockChannelConfig) -> Self {
        self.platform.set_channel(channel, config);
        self
    }

    /** Set the default behavior for unknown channels. */
    pub fn with_default(mut self, config: MockChannelConfig) -> Self {
        self.platform.set_default(config);
        self
    }

    /** Build the mock platform. */
    pub fn build(self) -> MockPlatform {
        self.platform
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_platform_offline_channel() {
        let mock = MockPlatform::twitch();
        mock.set_offline("test_channel");

        let result = mock.check_live("test_channel").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_mock_platform_live_channel() {
        let mock = MockPlatform::twitch();
        mock.set_live("test_channel", "Test Stream");

        let result = mock.check_live("test_channel").await;
        assert!(result.is_ok());
        let stream_info = result.unwrap();
        assert!(stream_info.is_some());
        assert_eq!(stream_info.unwrap().title, "Test Stream");
    }

    #[tokio::test]
    async fn test_mock_platform_error() {
        let mock = MockPlatform::twitch();
        mock.set_error(
            "bad_channel",
            MockError::ChannelNotFound("bad_channel".to_string()),
        );

        let result = mock.check_live("bad_channel").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_platform_call_counts() {
        let mock = MockPlatform::twitch();
        mock.set_live("channel1", "Stream 1");
        mock.set_live("channel2", "Stream 2");

        assert_eq!(mock.check_live_call_count(), 0);

        let _ = mock.check_live("channel1").await;
        let _ = mock.check_live("channel2").await;
        let _ = mock.check_live("channel1").await;

        assert_eq!(mock.check_live_call_count(), 3);
    }

    #[tokio::test]
    async fn test_mock_platform_builder() {
        let mock = MockPlatformBuilder::new(Platform::YouTube)
            .with_live_channel("streamer1", "Gaming Stream")
            .with_offline_channel("streamer2")
            .build();

        let live = mock.check_live("streamer1").await.unwrap();
        assert!(live.is_some());

        let offline = mock.check_live("streamer2").await.unwrap();
        assert!(offline.is_none());
    }

    #[tokio::test]
    async fn test_mock_platform_qualities() {
        let mock = MockPlatform::twitch();
        mock.set_channel(
            "test",
            MockChannelConfig::live("Stream").with_qualities(vec![
                Quality {
                    name: "1080p".to_string(),
                    resolution: Some("1920x1080".to_string()),
                    bandwidth: Some(6000000),
                },
                Quality {
                    name: "720p".to_string(),
                    resolution: Some("1280x720".to_string()),
                    bandwidth: Some(3000000),
                },
            ]),
        );

        let qualities = mock.get_qualities("test").await.unwrap();
        assert_eq!(qualities.len(), 2);
        assert_eq!(qualities[0].name, "1080p");
    }

    #[tokio::test]
    async fn test_mock_platform_stream_url() {
        let mock = MockPlatform::twitch();
        mock.set_channel(
            "test",
            MockChannelConfig::live("Stream").with_url("https://custom.url/stream.m3u8"),
        );

        let quality = Quality::source();
        let stream_url = mock.get_stream_url("test", &quality).await.unwrap();
        assert_eq!(stream_url.url, "https://custom.url/stream.m3u8");
    }
}
