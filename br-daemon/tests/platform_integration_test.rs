//! Platform Integration Tests
//!
//! Tests for the StreamPlatform implementations using wiremock for HTTP mocking.
//! These tests verify that the platform implementations correctly parse API responses
//! and handle various edge cases.

use br_daemon::platforms::{
    ChannelProfile, MockChannelConfig, MockError, MockPlatform, MockPlatformBuilder, PlatformError,
    PlatformResult, StreamPlatform,
};
use br_daemon::types::{Platform, Quality, StreamInfo};
use chrono::Utc;
use std::time::Duration;

mod common;

/**
 * Mock Platform Tests (Extended)
 */

#[tokio::test]
async fn test_mock_platform_returns_correct_platform_type() {
    let twitch = MockPlatform::twitch();
    let youtube = MockPlatform::youtube();
    let kick = MockPlatform::kick();

    assert_eq!(twitch.platform(), Platform::Twitch);
    assert_eq!(youtube.platform(), Platform::YouTube);
    assert_eq!(kick.platform(), Platform::Kick);
}

#[tokio::test]
async fn test_mock_platform_default_returns_offline() {
    let mock = MockPlatform::twitch();

    // Unknown channel should return offline (None) by default
    let result: PlatformResult<Option<StreamInfo>> = mock.check_live("unknown_channel").await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_mock_platform_live_channel_returns_stream_info() {
    let mock = MockPlatform::twitch();
    mock.set_live("streamer123", "Playing Minecraft");

    let result: PlatformResult<Option<StreamInfo>> = mock.check_live("streamer123").await;
    assert!(result.is_ok());

    let stream_info = result.unwrap().expect("Should be live");
    assert_eq!(stream_info.title, "Playing Minecraft");
    assert_eq!(stream_info.game, Some("Just Chatting".to_string()));
    assert!(stream_info.viewer_count > 0);
}

#[tokio::test]
async fn test_mock_platform_channel_not_found_error() {
    let mock = MockPlatform::twitch();
    mock.set_error(
        "nonexistent",
        MockError::ChannelNotFound("nonexistent".to_string()),
    );

    let result: PlatformResult<Option<StreamInfo>> = mock.check_live("nonexistent").await;
    assert!(result.is_err());

    match result.unwrap_err() {
        PlatformError::ChannelNotFound(channel) => {
            assert_eq!(channel, "nonexistent");
        }
        other => panic!("Expected ChannelNotFound, got {:?}", other),
    }
}

#[tokio::test]
async fn test_mock_platform_network_error() {
    let mock = MockPlatform::twitch();
    mock.set_error("bad_network", MockError::Network("Connection refused".to_string()));

    let result: PlatformResult<Option<StreamInfo>> = mock.check_live("bad_network").await;
    assert!(result.is_err());

    match result.unwrap_err() {
        PlatformError::Api(msg) => {
            assert!(msg.contains("Connection refused"));
        }
        other => panic!("Expected Api error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_mock_platform_auth_required_error() {
    let mock = MockPlatform::twitch();
    mock.set_error("private_channel", MockError::AuthRequired);

    let result: PlatformResult<Option<StreamInfo>> = mock.check_live("private_channel").await;
    assert!(result.is_err());

    match result.unwrap_err() {
        PlatformError::AuthRequired => {}
        other => panic!("Expected AuthRequired, got {:?}", other),
    }
}

#[tokio::test]
async fn test_mock_platform_get_qualities_offline_returns_error() {
    let mock = MockPlatform::twitch();
    mock.set_offline("offline_channel");

    let result: PlatformResult<Vec<Quality>> = mock.get_qualities("offline_channel").await;
    assert!(result.is_err());

    match result.unwrap_err() {
        PlatformError::StreamOffline => {}
        other => panic!("Expected StreamOffline, got {:?}", other),
    }
}

#[tokio::test]
async fn test_mock_platform_get_qualities_live_returns_qualities() {
    let mock = MockPlatform::twitch();
    mock.set_channel(
        "live_channel",
        MockChannelConfig::live("Test Stream").with_qualities(vec![
            Quality {
                name: "1080p60".to_string(),
                resolution: Some("1920x1080".to_string()),
                bandwidth: Some(8_000_000),
            },
            Quality {
                name: "720p60".to_string(),
                resolution: Some("1280x720".to_string()),
                bandwidth: Some(4_500_000),
            },
            Quality {
                name: "480p".to_string(),
                resolution: Some("854x480".to_string()),
                bandwidth: Some(2_000_000),
            },
        ]),
    );

    let qualities: PlatformResult<Vec<Quality>> = mock.get_qualities("live_channel").await;
    let qualities = qualities.unwrap();
    assert_eq!(qualities.len(), 3);
    assert_eq!(qualities[0].name, "1080p60");
    assert_eq!(qualities[1].name, "720p60");
    assert_eq!(qualities[2].name, "480p");
}

#[tokio::test]
async fn test_mock_platform_get_stream_url_offline_returns_error() {
    let mock = MockPlatform::twitch();
    mock.set_offline("offline_channel");

    let quality = Quality::source();
    let result: PlatformResult<br_daemon::platforms::StreamUrl> = mock.get_stream_url("offline_channel", &quality).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        PlatformError::StreamOffline => {}
        other => panic!("Expected StreamOffline, got {:?}", other),
    }
}

#[tokio::test]
async fn test_mock_platform_get_stream_url_live_returns_url() {
    let mock = MockPlatform::twitch();
    mock.set_channel(
        "live_channel",
        MockChannelConfig::live("Test").with_url("https://cdn.example.com/stream/playlist.m3u8"),
    );

    let quality = Quality::source();
    let result: PlatformResult<br_daemon::platforms::StreamUrl> = mock.get_stream_url("live_channel", &quality).await;
    assert!(result.is_ok());

    let stream_url = result.unwrap();
    assert_eq!(stream_url.url, "https://cdn.example.com/stream/playlist.m3u8");
    assert_eq!(stream_url.quality.name, "source");
}

#[tokio::test]
async fn test_mock_platform_call_counting() {
    let mock = MockPlatform::twitch();
    mock.set_live("channel1", "Stream 1");
    mock.set_live("channel2", "Stream 2");

    // Initially zero
    assert_eq!(mock.check_live_call_count(), 0);
    assert_eq!(mock.get_qualities_call_count(), 0);
    assert_eq!(mock.get_stream_url_call_count(), 0);

    // Make some calls
    let _: PlatformResult<Option<StreamInfo>> = mock.check_live("channel1").await;
    let _: PlatformResult<Option<StreamInfo>> = mock.check_live("channel2").await;
    let _: PlatformResult<Vec<Quality>> = mock.get_qualities("channel1").await;
    let _: PlatformResult<br_daemon::platforms::StreamUrl> = mock.get_stream_url("channel1", &Quality::source()).await;

    assert_eq!(mock.check_live_call_count(), 2);
    assert_eq!(mock.get_qualities_call_count(), 1);
    assert_eq!(mock.get_stream_url_call_count(), 1);

    // Reset counts
    mock.reset_counts();
    assert_eq!(mock.check_live_call_count(), 0);
    assert_eq!(mock.get_qualities_call_count(), 0);
    assert_eq!(mock.get_stream_url_call_count(), 0);
}

#[tokio::test]
async fn test_mock_platform_latency_simulation() {
    let mock = MockPlatform::twitch();
    mock.set_channel(
        "slow_channel",
        MockChannelConfig::live("Stream").with_latency(Duration::from_millis(50)),
    );

    let start = std::time::Instant::now();
    let _: PlatformResult<Option<StreamInfo>> = mock.check_live("slow_channel").await;
    let elapsed = start.elapsed();

    // Should have taken at least 50ms
    assert!(elapsed.as_millis() >= 50, "Expected >= 50ms, got {:?}", elapsed);
}

#[tokio::test]
async fn test_mock_platform_builder_chain() {
    let mock = MockPlatformBuilder::new(Platform::Twitch)
        .with_live_channel("live1", "Stream One")
        .with_live_channel("live2", "Stream Two")
        .with_offline_channel("offline1")
        .with_channel(
            "custom",
            MockChannelConfig::live("Custom Stream")
                .with_qualities(vec![Quality {
                    name: "4k".to_string(),
                    resolution: Some("3840x2160".to_string()),
                    bandwidth: Some(25_000_000),
                }])
                .with_url("https://4k.stream/playlist.m3u8"),
        )
        .build();

    // Verify live channels
    assert!(StreamPlatform::check_live(&mock, "live1").await.unwrap().is_some());
    assert!(StreamPlatform::check_live(&mock, "live2").await.unwrap().is_some());

    // Verify offline channel
    assert!(StreamPlatform::check_live(&mock, "offline1").await.unwrap().is_none());

    // Verify custom channel
    let custom: PlatformResult<Option<StreamInfo>> = mock.check_live("custom").await;
    assert!(custom.as_ref().unwrap().is_some());
    assert_eq!(custom.unwrap().unwrap().title, "Custom Stream");

    let qualities: PlatformResult<Vec<Quality>> = mock.get_qualities("custom").await;
    let qualities = qualities.unwrap();
    assert_eq!(qualities.len(), 1);
    assert_eq!(qualities[0].name, "4k");
}

#[tokio::test]
async fn test_mock_platform_refresh_auth_succeeds() {
    let mut mock = MockPlatform::twitch();
    let result: PlatformResult<()> = mock.refresh_auth().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_platform_get_channel_profile() {
    let mock = MockPlatform::twitch();
    mock.set_live("test_channel", "Test Stream");

    let profile: PlatformResult<ChannelProfile> = mock.get_channel_profile("test_channel").await;
    let profile = profile.unwrap();
    assert_eq!(profile.display_name, "Mock Channel");
    assert!(profile.description.is_some());
    assert!(profile.profile_image_url.is_some());
}

#[tokio::test]
async fn test_mock_platform_channel_profile_not_found() {
    let mock = MockPlatform::twitch();
    mock.set_channel(
        "no_profile",
        MockChannelConfig {
            profile: None,
            ..Default::default()
        },
    );

    let result: PlatformResult<ChannelProfile> = mock.get_channel_profile("no_profile").await;
    assert!(result.is_err());

    match result.unwrap_err() {
        PlatformError::ChannelNotFound(ch) => assert_eq!(ch, "no_profile"),
        other => panic!("Expected ChannelNotFound, got {:?}", other),
    }
}

/**
 * Stream Info Tests
 */

#[tokio::test]
async fn test_stream_info_custom_values() {
    let mock = MockPlatform::twitch();
    let started_at = Utc::now() - chrono::Duration::hours(2);

    mock.set_channel(
        "custom_stream",
        MockChannelConfig::live("").with_stream_info(StreamInfo {
            title: "24 Hour Stream Marathon!".to_string(),
            game: Some("Fortnite".to_string()),
            viewer_count: 50_000,
            started_at,
            thumbnail_url: Some("https://cdn.example.com/thumb.jpg".to_string()),
        }),
    );

    let result: PlatformResult<Option<StreamInfo>> = mock.check_live("custom_stream").await;
    let info = result.unwrap().unwrap();
    assert_eq!(info.title, "24 Hour Stream Marathon!");
    assert_eq!(info.game, Some("Fortnite".to_string()));
    assert_eq!(info.viewer_count, 50_000);
    assert!(info.thumbnail_url.is_some());
}

#[tokio::test]
async fn test_stream_info_no_game() {
    let mock = MockPlatform::twitch();

    mock.set_channel(
        "no_game_stream",
        MockChannelConfig::live("").with_stream_info(StreamInfo {
            title: "Just Chatting".to_string(),
            game: None,
            viewer_count: 100,
            started_at: Utc::now(),
            thumbnail_url: None,
        }),
    );

    let result: PlatformResult<Option<StreamInfo>> = mock.check_live("no_game_stream").await;
    let info = result.unwrap().unwrap();
    assert!(info.game.is_none());
    assert!(info.thumbnail_url.is_none());
}

/**
 * Quality Tests
 */

#[test]
fn test_quality_source_factory() {
    let source = Quality::source();
    assert_eq!(source.name, "source");
    assert!(source.resolution.is_none());
    assert!(source.bandwidth.is_none());
}

#[test]
fn test_quality_clone() {
    let quality = Quality {
        name: "1080p60".to_string(),
        resolution: Some("1920x1080".to_string()),
        bandwidth: Some(8_000_000),
    };

    let cloned = quality.clone();
    assert_eq!(cloned.name, "1080p60");
    assert_eq!(cloned.resolution, Some("1920x1080".to_string()));
    assert_eq!(cloned.bandwidth, Some(8_000_000));
}

/**
 * Platform Error Tests
 */

#[test]
fn test_platform_error_display() {
    let errors = vec![
        (PlatformError::Api("Bad request".to_string()), "API error: Bad request"),
        (PlatformError::AuthRequired, "Authentication required"),
        (
            PlatformError::ChannelNotFound("test".to_string()),
            "Channel not found: test",
        ),
        (PlatformError::StreamOffline, "Stream offline"),
        (
            PlatformError::Parse("Invalid JSON".to_string()),
            "Parse error: Invalid JSON",
        ),
    ];

    for (error, expected) in errors {
        assert_eq!(error.to_string(), expected);
    }
}

/**
 * Concurrency Tests
 */

#[tokio::test]
async fn test_mock_platform_concurrent_access() {
    use std::sync::Arc;
    use tokio::task::JoinSet;

    let mock = Arc::new(MockPlatform::twitch());

    // Set up channels
    for i in 0..10 {
        mock.set_live(&format!("channel{}", i), &format!("Stream {}", i));
    }

    // Spawn concurrent tasks
    let mut join_set = JoinSet::new();

    for i in 0..50 {
        let mock_clone = mock.clone();
        let channel = format!("channel{}", i % 10);

        join_set.spawn(async move {
            let result: PlatformResult<Option<StreamInfo>> = mock_clone.check_live(&channel).await;
            result
        });
    }

    // Collect results
    let mut successes = 0;
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(Some(_))) => successes += 1,
            Ok(Ok(None)) => panic!("Expected live stream"),
            Ok(Err(e)) => panic!("Platform error: {:?}", e),
            Err(e) => panic!("Join error: {:?}", e),
        }
    }

    assert_eq!(successes, 50);
    assert_eq!(mock.check_live_call_count(), 50);
}

#[tokio::test]
async fn test_mock_platform_state_changes_during_operation() {
    let mock = MockPlatform::twitch();

    // Start offline
    mock.set_offline("dynamic_channel");
    let result1: PlatformResult<Option<StreamInfo>> = mock.check_live("dynamic_channel").await;
    assert!(result1.unwrap().is_none());

    // Go live
    mock.set_live("dynamic_channel", "Now Live!");
    let result2: PlatformResult<Option<StreamInfo>> = mock.check_live("dynamic_channel").await;
    let result2 = result2.unwrap();
    assert!(result2.is_some());
    assert_eq!(result2.unwrap().title, "Now Live!");

    // Go offline again
    mock.set_offline("dynamic_channel");
    let result3: PlatformResult<Option<StreamInfo>> = mock.check_live("dynamic_channel").await;
    assert!(result3.unwrap().is_none());
}

/**
 * Default Config Tests
 */

#[tokio::test]
async fn test_mock_platform_custom_default() {
    let mut mock = MockPlatform::twitch();

    // Set default to return ChannelNotFound for unknown channels
    mock.set_default(MockChannelConfig::with_error(MockError::ChannelNotFound(
        "unknown".to_string(),
    )));

    let result: PlatformResult<Option<StreamInfo>> = mock.check_live("random_channel").await;
    assert!(result.is_err());

    match result.unwrap_err() {
        PlatformError::ChannelNotFound(_) => {}
        other => panic!("Expected ChannelNotFound, got {:?}", other),
    }
}

#[tokio::test]
async fn test_mock_platform_builder_with_default() {
    let mock = MockPlatformBuilder::new(Platform::YouTube)
        .with_live_channel("known_channel", "Stream")
        .with_default(MockChannelConfig::with_error(MockError::Api(
            "Unknown channel".to_string(),
        )))
        .build();

    // Known channel should work
    let known: PlatformResult<Option<StreamInfo>> = mock.check_live("known_channel").await;
    assert!(known.is_ok());
    assert!(known.unwrap().is_some());

    // Unknown channel should return error
    let unknown: PlatformResult<Option<StreamInfo>> = mock.check_live("some_other_channel").await;
    assert!(unknown.is_err());
}

/**
 * MockError Tests
 */

#[test]
fn test_mock_error_conversion() {
    let test_cases: Vec<(MockError, &str)> = vec![
        (
            MockError::Network("timeout".to_string()),
            "Network: timeout",
        ),
        (MockError::Api("rate limited".to_string()), "rate limited"),
        (MockError::AuthRequired, "Authentication required"),
        (
            MockError::ChannelNotFound("test".to_string()),
            "Channel not found: test",
        ),
        (MockError::StreamOffline, "Stream offline"),
        (MockError::Parse("bad json".to_string()), "Parse error: bad json"),
    ];

    for (mock_error, expected_substring) in test_cases {
        let platform_error = MockError::to_platform_error(&mock_error);
        let error_str = platform_error.to_string();
        assert!(
            error_str.contains(expected_substring),
            "Expected '{}' to contain '{}', got: {}",
            error_str,
            expected_substring,
            error_str
        );
    }
}

#[test]
fn test_mock_error_clone() {
    let errors: Vec<MockError> = vec![
        MockError::Network("test".to_string()),
        MockError::Api("test".to_string()),
        MockError::AuthRequired,
        MockError::ChannelNotFound("test".to_string()),
        MockError::StreamOffline,
        MockError::Parse("test".to_string()),
    ];

    for error in errors {
        let cloned: MockError = Clone::clone(&error);
        // Just verify it compiles and doesn't panic
        let _ = format!("{:?}", cloned);
    }
}
