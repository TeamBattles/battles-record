// br-daemon/tests/common/fixtures.rs
//! Test data factories for creating consistent test fixtures.
//!
//! This module provides factory functions for creating test data
//! with sensible defaults that can be customized as needed.

use br_daemon::config::{
    ChannelConfig, FiltersConfig, ScheduleConfig, ScheduleRule,
};
use br_daemon::types::{
    Channel, ChannelStatus, Platform, Quality, QuotaStatus, Recording, RecordingDetail,
    RecordingStatus, StreamInfo,
};
use chrono::{Duration, Utc};
use uuid::Uuid;

/**
 * Channel Fixtures
 */

/// Builder for creating ChannelConfig instances
pub struct ChannelConfigBuilder {
    config: ChannelConfig,
}

impl ChannelConfigBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            config: ChannelConfig {
                name: name.to_string(),
                platform: Platform::Twitch,
                enabled: true,
                quality: "best".to_string(),
                schedule: None,
                filters: None,
                post_processing: None,
                quota_gb: None,
                retention_days: None,
                custom_profile_image: None,
                custom_banner_image: None,
                platform_profile_url: None,
                platform_banner_url: None,
            },
        }
    }

    pub fn platform(mut self, platform: Platform) -> Self {
        self.config.platform = platform;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    pub fn quality(mut self, quality: &str) -> Self {
        self.config.quality = quality.to_string();
        self
    }

    pub fn quota_gb(mut self, quota: u32) -> Self {
        self.config.quota_gb = Some(quota);
        self
    }

    pub fn retention_days(mut self, days: u32) -> Self {
        self.config.retention_days = Some(days);
        self
    }

    pub fn with_schedule(mut self, enabled: bool, timezone: &str) -> Self {
        self.config.schedule = Some(ScheduleConfig {
            enabled,
            timezone: Some(timezone.to_string()),
            rules: vec![],
        });
        self
    }

    pub fn with_schedule_rule(mut self, days: Vec<String>, start: &str, end: &str) -> Self {
        let schedule = self.config.schedule.get_or_insert_with(|| ScheduleConfig {
            enabled: true,
            timezone: Some("UTC".to_string()),
            rules: vec![],
        });
        schedule.rules.push(ScheduleRule {
            days,
            start_time: Some(start.to_string()),
            end_time: Some(end.to_string()),
        });
        self
    }

    pub fn with_filters(mut self, filters: FiltersConfig) -> Self {
        self.config.filters = Some(filters);
        self
    }

    pub fn build(self) -> ChannelConfig {
        self.config
    }
}

/// Create a minimal channel config
pub fn channel_config(name: &str) -> ChannelConfig {
    ChannelConfigBuilder::new(name).build()
}

/// Create a channel config with all fields populated
pub fn full_channel_config(name: &str) -> ChannelConfig {
    ChannelConfigBuilder::new(name)
        .platform(Platform::Twitch)
        .quality("1080p60")
        .quota_gb(50)
        .retention_days(30)
        .with_schedule(true, "America/Los_Angeles")
        .with_schedule_rule(
            vec!["monday".to_string(), "wednesday".to_string(), "friday".to_string()],
            "18:00",
            "23:00",
        )
        .with_filters(FiltersConfig {
            title_contains: vec!["gaming".to_string()],
            title_excludes: vec!["sponsor".to_string()],
            game_contains: vec!["Valorant".to_string()],
            game_excludes: vec![],
            min_viewers: Some(100),
        })
        .build()
}

/**
 * Channel Response Fixtures
 */

/// Builder for creating Channel (API response) instances
pub struct ChannelBuilder {
    channel: Channel,
}

impl ChannelBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            channel: Channel {
                id: Uuid::new_v4(),
                name: name.to_string(),
                platform: Platform::Twitch,
                enabled: true,
                quality: "best".to_string(),
                status: ChannelStatus::Offline,
                current_stream: None,
                quota_gb: None,
                retention_days: None,
                quota_status: QuotaStatus::Unlimited,
                quota_used_bytes: 0,
                quota_percent: 0,
                schedule_enabled: false,
                timezone: None,
                schedule_rules: vec![],
                filters: None,
                profile_image_url: None,
                banner_image_url: None,
            },
        }
    }

    pub fn id(mut self, id: Uuid) -> Self {
        self.channel.id = id;
        self
    }

    pub fn platform(mut self, platform: Platform) -> Self {
        self.channel.platform = platform;
        self
    }

    pub fn status(mut self, status: ChannelStatus) -> Self {
        self.channel.status = status;
        self
    }

    pub fn live(mut self) -> Self {
        self.channel.status = ChannelStatus::Live;
        self.channel.current_stream = Some(StreamInfo {
            title: "Test Stream".to_string(),
            game: Some("Just Chatting".to_string()),
            viewer_count: 1000,
            started_at: Utc::now(),
            thumbnail_url: Some("https://example.com/thumb.jpg".to_string()),
        });
        self
    }

    pub fn recording(mut self) -> Self {
        self.channel.status = ChannelStatus::Recording;
        if self.channel.current_stream.is_none() {
            self.channel.current_stream = Some(StreamInfo {
                title: "Recording Stream".to_string(),
                game: Some("Gaming".to_string()),
                viewer_count: 500,
                started_at: Utc::now(),
                thumbnail_url: None,
            });
        }
        self
    }

    pub fn quota(mut self, quota_gb: u32, used_bytes: u64) -> Self {
        self.channel.quota_gb = Some(quota_gb);
        self.channel.quota_used_bytes = used_bytes;
        let quota_bytes = (quota_gb as u64) * 1024 * 1024 * 1024;
        if quota_bytes > 0 {
            self.channel.quota_percent = ((used_bytes as f64 / quota_bytes as f64) * 100.0) as u8;
            self.channel.quota_status = if self.channel.quota_percent >= 100 {
                QuotaStatus::Exceeded
            } else if self.channel.quota_percent >= 80 {
                QuotaStatus::Warning
            } else {
                QuotaStatus::Ok
            };
        }
        self
    }

    pub fn build(self) -> Channel {
        self.channel
    }
}

/// Create a minimal offline channel
pub fn offline_channel(name: &str) -> Channel {
    ChannelBuilder::new(name).build()
}

/// Create a live channel with stream info
pub fn live_channel(name: &str) -> Channel {
    ChannelBuilder::new(name).live().build()
}

/// Create a recording channel
pub fn recording_channel(name: &str) -> Channel {
    ChannelBuilder::new(name).recording().build()
}

/**
 * Stream Info Fixtures
 */

/// Create standard stream info
pub fn stream_info(title: &str) -> StreamInfo {
    StreamInfo {
        title: title.to_string(),
        game: Some("Just Chatting".to_string()),
        viewer_count: 1000,
        started_at: Utc::now(),
        thumbnail_url: Some("https://example.com/thumbnail.jpg".to_string()),
    }
}

/// Create stream info with specified game
pub fn stream_info_with_game(title: &str, game: &str) -> StreamInfo {
    StreamInfo {
        title: title.to_string(),
        game: Some(game.to_string()),
        viewer_count: 1000,
        started_at: Utc::now(),
        thumbnail_url: None,
    }
}

/**
 * Quality Fixtures
 */

/// Create common quality options
pub fn quality_options() -> Vec<Quality> {
    vec![
        Quality {
            name: "source".to_string(),
            resolution: Some("1920x1080".to_string()),
            bandwidth: Some(6000000),
        },
        Quality {
            name: "1080p60".to_string(),
            resolution: Some("1920x1080".to_string()),
            bandwidth: Some(6000000),
        },
        Quality {
            name: "720p60".to_string(),
            resolution: Some("1280x720".to_string()),
            bandwidth: Some(3000000),
        },
        Quality {
            name: "480p".to_string(),
            resolution: Some("854x480".to_string()),
            bandwidth: Some(1500000),
        },
    ]
}

/// Create a source quality
pub fn source_quality() -> Quality {
    Quality::source()
}

/**
 * Recording Fixtures
 */

/// Builder for creating Recording instances
pub struct RecordingBuilder {
    recording: Recording,
}

impl RecordingBuilder {
    pub fn new(channel_name: &str) -> Self {
        Self {
            recording: Recording {
                id: Uuid::new_v4(),
                channel_id: Uuid::new_v4(),
                channel_name: channel_name.to_string(),
                platform: Platform::Twitch,
                started_at: Utc::now(),
                ended_at: None,
                status: RecordingStatus::Recording,
                segments_downloaded: 0,
                size_bytes: 0,
                output_path: format!("/recordings/{}", channel_name),
            },
        }
    }

    pub fn id(mut self, id: Uuid) -> Self {
        self.recording.id = id;
        self
    }

    pub fn channel_id(mut self, id: Uuid) -> Self {
        self.recording.channel_id = id;
        self
    }

    pub fn platform(mut self, platform: Platform) -> Self {
        self.recording.platform = platform;
        self
    }

    pub fn status(mut self, status: RecordingStatus) -> Self {
        self.recording.status = status;
        self
    }

    pub fn completed(mut self) -> Self {
        self.recording.status = RecordingStatus::Completed;
        self.recording.ended_at = Some(Utc::now());
        self
    }

    pub fn processed(mut self) -> Self {
        self.recording.status = RecordingStatus::Processed;
        self.recording.ended_at = Some(Utc::now());
        self
    }

    pub fn failed(mut self) -> Self {
        self.recording.status = RecordingStatus::Failed;
        self.recording.ended_at = Some(Utc::now());
        self
    }

    pub fn segments(mut self, count: u32) -> Self {
        self.recording.segments_downloaded = count;
        self
    }

    pub fn size_bytes(mut self, bytes: u64) -> Self {
        self.recording.size_bytes = bytes;
        self
    }

    pub fn duration_hours(mut self, hours: i64) -> Self {
        let started = Utc::now() - Duration::hours(hours);
        self.recording.started_at = started;
        self
    }

    pub fn build(self) -> Recording {
        self.recording
    }
}

/// Create an active recording
pub fn active_recording(channel_name: &str) -> Recording {
    RecordingBuilder::new(channel_name)
        .segments(100)
        .size_bytes(500 * 1024 * 1024) // 500MB
        .build()
}

/// Create a completed recording
pub fn completed_recording(channel_name: &str) -> Recording {
    RecordingBuilder::new(channel_name)
        .completed()
        .segments(500)
        .size_bytes(2 * 1024 * 1024 * 1024) // 2GB
        .duration_hours(2)
        .build()
}

/**
 * Recording Detail Fixtures
 */

/// Create a detailed recording response
pub fn recording_detail(channel_name: &str) -> RecordingDetail {
    let recording = completed_recording(channel_name);
    RecordingDetail {
        id: recording.id,
        channel_id: recording.channel_id,
        channel_name: recording.channel_name,
        platform: recording.platform,
        status: recording.status,
        title: Some("Stream Title".to_string()),
        game: Some("Gaming".to_string()),
        started_at: recording.started_at,
        ended_at: recording.ended_at,
        duration_secs: 7200,
        size_bytes: recording.size_bytes,
        segment_count: recording.segments_downloaded,
        output_path: recording.output_path,
        processed_file: Some("/library/channel_name/stream.mp4".to_string()),
    }
}

/**
 * Filter Fixtures
 */

/// Create filters that match everything
pub fn permissive_filters() -> FiltersConfig {
    FiltersConfig {
        title_contains: vec![],
        title_excludes: vec![],
        game_contains: vec![],
        game_excludes: vec![],
        min_viewers: None,
    }
}

/// Create filters with title matching
pub fn title_filters(includes: Vec<&str>, excludes: Vec<&str>) -> FiltersConfig {
    FiltersConfig {
        title_contains: includes.into_iter().map(String::from).collect(),
        title_excludes: excludes.into_iter().map(String::from).collect(),
        game_contains: vec![],
        game_excludes: vec![],
        min_viewers: None,
    }
}

/// Create filters with game matching
pub fn game_filters(includes: Vec<&str>, excludes: Vec<&str>) -> FiltersConfig {
    FiltersConfig {
        title_contains: vec![],
        title_excludes: vec![],
        game_contains: includes.into_iter().map(String::from).collect(),
        game_excludes: excludes.into_iter().map(String::from).collect(),
        min_viewers: None,
    }
}

/// Create filters with viewer requirement
pub fn viewer_filter(min_viewers: u32) -> FiltersConfig {
    FiltersConfig {
        title_contains: vec![],
        title_excludes: vec![],
        game_contains: vec![],
        game_excludes: vec![],
        min_viewers: Some(min_viewers),
    }
}

/**
 * Config Fixtures (re-export from common/mod.rs for convenience)
 */

// Re-export create_test_config from parent module for convenience
// (it's defined in common/mod.rs)

/**
 * JSON Payload Fixtures
 */

/// Create a valid add channel request JSON
pub fn add_channel_json(name: &str, platform: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "platform": platform,
        "enabled": true,
        "quality": "best"
    })
}

/// Create an update channel request JSON
pub fn update_channel_json() -> serde_json::Value {
    serde_json::json!({
        "enabled": true,
        "quality": "1080p60"
    })
}

/// Create a login request JSON
pub fn login_json(username: &str, password: &str) -> serde_json::Value {
    serde_json::json!({
        "username": username,
        "password": password
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_config_builder() {
        let config = ChannelConfigBuilder::new("test")
            .platform(Platform::YouTube)
            .quality("720p")
            .quota_gb(10)
            .build();

        assert_eq!(config.name, "test");
        assert_eq!(config.platform, Platform::YouTube);
        assert_eq!(config.quality, "720p");
        assert_eq!(config.quota_gb, Some(10));
    }

    #[test]
    fn test_channel_builder_live() {
        let channel = ChannelBuilder::new("streamer").live().build();

        assert_eq!(channel.status, ChannelStatus::Live);
        assert!(channel.current_stream.is_some());
    }

    #[test]
    fn test_recording_builder() {
        let recording = RecordingBuilder::new("test")
            .completed()
            .segments(100)
            .size_bytes(1024)
            .build();

        assert_eq!(recording.status, RecordingStatus::Completed);
        assert!(recording.ended_at.is_some());
        assert_eq!(recording.segments_downloaded, 100);
    }

    #[test]
    fn test_quota_calculation() {
        let channel = ChannelBuilder::new("test")
            .quota(10, 9 * 1024 * 1024 * 1024) // 9GB of 10GB
            .build();

        assert_eq!(channel.quota_status, QuotaStatus::Warning);
        assert!(channel.quota_percent >= 80);
    }
}
