use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/** Types of notifications that can be sent. */
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType {
    StreamLive,
    RecordingStarted,
    RecordingEnded,
    ProcessingComplete,
    Error,
}

/** Payload for a notification. */
#[derive(Debug, Clone, Serialize)]
pub struct NotificationPayload {
    pub event_type: NotificationType,
    pub timestamp: DateTime<Utc>,
    pub channel_name: Option<String>,
    pub platform: Option<String>,
    pub title: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl NotificationPayload {
    /** Create a notification for a stream going live. */
    pub fn stream_live(channel: &str, platform: &str, title: Option<&str>) -> Self {
        Self {
            event_type: NotificationType::StreamLive,
            timestamp: Utc::now(),
            channel_name: Some(channel.to_string()),
            platform: Some(platform.to_string()),
            title: title.map(|t| t.to_string()),
            message: format!(
                "{} is now live on {}{}",
                channel,
                platform,
                title.map(|t| format!(": {}", t)).unwrap_or_default()
            ),
            data: None,
        }
    }

    /** Create a notification for recording started. */
    pub fn recording_started(channel: &str) -> Self {
        Self {
            event_type: NotificationType::RecordingStarted,
            timestamp: Utc::now(),
            channel_name: Some(channel.to_string()),
            platform: None,
            title: None,
            message: format!("Recording started for {}", channel),
            data: None,
        }
    }

    /** Create a notification for recording ended. */
    pub fn recording_ended(channel: &str, segments: u32, bytes: u64) -> Self {
        let size_mb = bytes as f64 / (1024.0 * 1024.0);
        Self {
            event_type: NotificationType::RecordingEnded,
            timestamp: Utc::now(),
            channel_name: Some(channel.to_string()),
            platform: None,
            title: None,
            message: format!(
                "Recording ended for {} - {} segments, {:.2} MB",
                channel, segments, size_mb
            ),
            data: Some(serde_json::json!({
                "segments": segments,
                "bytes": bytes,
                "size_mb": size_mb
            })),
        }
    }

    /** Create a notification for processing complete. */
    pub fn processing_complete(output_file: &str, size_bytes: u64) -> Self {
        let size_mb = size_bytes as f64 / (1024.0 * 1024.0);
        Self {
            event_type: NotificationType::ProcessingComplete,
            timestamp: Utc::now(),
            channel_name: None,
            platform: None,
            title: None,
            message: format!("Processing complete: {} ({:.2} MB)", output_file, size_mb),
            data: Some(serde_json::json!({
                "output_file": output_file,
                "bytes": size_bytes,
                "size_mb": size_mb
            })),
        }
    }

    /** Create an error notification. */
    pub fn error(message: &str) -> Self {
        Self {
            event_type: NotificationType::Error,
            timestamp: Utc::now(),
            channel_name: None,
            platform: None,
            title: None,
            message: message.to_string(),
            data: None,
        }
    }

    /** Create an error notification with channel context. */
    pub fn error_with_channel(channel: &str, message: &str) -> Self {
        Self {
            event_type: NotificationType::Error,
            timestamp: Utc::now(),
            channel_name: Some(channel.to_string()),
            platform: None,
            title: None,
            message: format!("[{}] {}", channel, message),
            data: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_live_payload() {
        let payload = NotificationPayload::stream_live("streamer", "twitch", Some("Playing games"));
        assert_eq!(payload.event_type, NotificationType::StreamLive);
        assert_eq!(payload.channel_name.as_deref(), Some("streamer"));
        assert_eq!(payload.platform.as_deref(), Some("twitch"));
        assert!(payload.message.contains("streamer"));
        assert!(payload.message.contains("twitch"));
    }

    #[test]
    fn test_recording_ended_payload() {
        let payload = NotificationPayload::recording_ended("streamer", 100, 1024 * 1024 * 500);
        assert_eq!(payload.event_type, NotificationType::RecordingEnded);
        assert!(payload.data.is_some());
        let data = payload.data.unwrap();
        assert_eq!(data["segments"], 100);
    }

    #[test]
    fn test_error_with_channel_payload() {
        let payload = NotificationPayload::error_with_channel("streamer", "Connection failed");
        assert_eq!(payload.event_type, NotificationType::Error);
        assert!(payload.message.contains("streamer"));
        assert!(payload.message.contains("Connection failed"));
    }
}
