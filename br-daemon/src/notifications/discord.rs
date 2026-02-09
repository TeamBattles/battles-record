use crate::config::DiscordConfig;
use crate::notifications::types::{NotificationPayload, NotificationType};
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Serialize;

/** Discord embed colors (decimal values). */
mod colors {
    /** Light green - for StreamLive, RecordingStarted. */
    pub const SUCCESS: u32 = 0x90EE90;
    /** Blue - for RecordingEnded, ProcessingComplete. */
    pub const INFO: u32 = 0x0099FF;
    /** Red - for Error. */
    pub const ERROR: u32 = 0xFF0000;
}

/** Discord webhook payload structure. */
#[derive(Serialize)]
struct DiscordWebhook {
    embeds: Vec<DiscordEmbed>,
}

/** Discord embed structure. */
#[derive(Serialize)]
struct DiscordEmbed {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    color: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fields: Vec<DiscordField>,
    timestamp: String,
}

/** Discord embed field. */
#[derive(Serialize)]
struct DiscordField {
    name: String,
    value: String,
    inline: bool,
}

/** Get emoji prefix for notification type. */
fn get_emoji(event_type: NotificationType) -> &'static str {
    match event_type {
        NotificationType::StreamLive => "\u{1F534}", // Red circle
        NotificationType::RecordingStarted => "\u{23FA}\u{FE0F}", // Record button
        NotificationType::RecordingEnded => "\u{23F9}\u{FE0F}", // Stop button
        NotificationType::ProcessingComplete => "\u{2705}", // Check mark
        NotificationType::Error => "\u{274C}", // Cross mark
    }
}

/** Get embed color for notification type. */
fn get_color(event_type: NotificationType) -> u32 {
    match event_type {
        NotificationType::StreamLive | NotificationType::RecordingStarted => colors::SUCCESS,
        NotificationType::RecordingEnded | NotificationType::ProcessingComplete => colors::INFO,
        NotificationType::Error => colors::ERROR,
    }
}

/** Build title with emoji and event-specific text. */
fn build_title(payload: &NotificationPayload) -> String {
    let emoji = get_emoji(payload.event_type);
    let channel_display = payload
        .channel_name
        .as_deref()
        .unwrap_or("Unknown channel");

    match payload.event_type {
        NotificationType::StreamLive => {
            format!("{} {} is now live!", emoji, channel_display)
        }
        NotificationType::RecordingStarted => {
            format!("{} Recording started for {}", emoji, channel_display)
        }
        NotificationType::RecordingEnded => {
            format!("{} Recording ended for {}", emoji, channel_display)
        }
        NotificationType::ProcessingComplete => {
            format!("{} Processing complete", emoji)
        }
        NotificationType::Error => {
            if payload.channel_name.is_some() {
                format!("{} Error for {}", emoji, channel_display)
            } else {
                format!("{} Error occurred", emoji)
            }
        }
    }
}

/** Build fields from payload data. */
fn build_fields(payload: &NotificationPayload) -> Vec<DiscordField> {
    let mut fields = Vec::new();

    // Add platform field if present
    if let Some(ref platform) = payload.platform {
        fields.push(DiscordField {
            name: "Platform".to_string(),
            value: platform.clone(),
            inline: true,
        });
    }

    // Extract fields from data if present
    if let Some(ref data) = payload.data {
        // Segments field
        if let Some(segments) = data.get("segments").and_then(|v| v.as_u64()) {
            fields.push(DiscordField {
                name: "Segments".to_string(),
                value: segments.to_string(),
                inline: true,
            });
        }

        // Size field (format as MB)
        if let Some(bytes) = data.get("bytes").and_then(|v| v.as_u64()) {
            let size_mb = bytes as f64 / (1024.0 * 1024.0);
            fields.push(DiscordField {
                name: "Size".to_string(),
                value: format!("{:.2} MB", size_mb),
                inline: true,
            });
        }

        // Output file field
        if let Some(output_file) = data.get("output_file").and_then(|v| v.as_str()) {
            fields.push(DiscordField {
                name: "Output".to_string(),
                value: output_file.to_string(),
                inline: false,
            });
        }
    }

    fields
}

/** Send a notification to Discord via webhook. */
pub async fn send(
    client: &Client,
    config: &DiscordConfig,
    payload: &NotificationPayload,
) -> Result<()> {
    let title = build_title(payload);
    let color = get_color(payload.event_type);
    let fields = build_fields(payload);

    // Build description from message, but only if it adds value beyond the title
    let description = if payload.message.is_empty() {
        None
    } else {
        Some(payload.message.clone())
    };

    let embed = DiscordEmbed {
        title,
        description,
        color,
        fields,
        timestamp: payload.timestamp.to_rfc3339(),
    };

    let webhook = DiscordWebhook {
        embeds: vec![embed],
    };

    let response = client
        .post(&config.webhook_url)
        .json(&webhook)
        .send()
        .await
        .context("Failed to send Discord webhook request")?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read response body".to_string());
        anyhow::bail!(
            "Discord webhook returned non-success status {}: {}",
            status,
            body
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_get_emoji() {
        assert_eq!(get_emoji(NotificationType::StreamLive), "\u{1F534}");
        assert_eq!(get_emoji(NotificationType::RecordingStarted), "\u{23FA}\u{FE0F}");
        assert_eq!(get_emoji(NotificationType::RecordingEnded), "\u{23F9}\u{FE0F}");
        assert_eq!(get_emoji(NotificationType::ProcessingComplete), "\u{2705}");
        assert_eq!(get_emoji(NotificationType::Error), "\u{274C}");
    }

    #[test]
    fn test_get_color() {
        assert_eq!(get_color(NotificationType::StreamLive), colors::SUCCESS);
        assert_eq!(get_color(NotificationType::RecordingStarted), colors::SUCCESS);
        assert_eq!(get_color(NotificationType::RecordingEnded), colors::INFO);
        assert_eq!(get_color(NotificationType::ProcessingComplete), colors::INFO);
        assert_eq!(get_color(NotificationType::Error), colors::ERROR);
    }

    #[test]
    fn test_build_title_stream_live() {
        let payload = NotificationPayload {
            event_type: NotificationType::StreamLive,
            timestamp: Utc::now(),
            channel_name: Some("xqc".to_string()),
            platform: Some("Twitch".to_string()),
            title: None,
            message: "xqc is now live on Twitch".to_string(),
            data: None,
        };

        let title = build_title(&payload);
        assert!(title.contains("xqc"));
        assert!(title.contains("is now live"));
    }

    #[test]
    fn test_build_title_error_no_channel() {
        let payload = NotificationPayload {
            event_type: NotificationType::Error,
            timestamp: Utc::now(),
            channel_name: None,
            platform: None,
            title: None,
            message: "Something went wrong".to_string(),
            data: None,
        };

        let title = build_title(&payload);
        assert!(title.contains("Error occurred"));
    }

    #[test]
    fn test_build_fields_with_platform() {
        let payload = NotificationPayload {
            event_type: NotificationType::StreamLive,
            timestamp: Utc::now(),
            channel_name: Some("streamer".to_string()),
            platform: Some("Twitch".to_string()),
            title: None,
            message: "Test".to_string(),
            data: None,
        };

        let fields = build_fields(&payload);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "Platform");
        assert_eq!(fields[0].value, "Twitch");
    }

    #[test]
    fn test_build_fields_with_recording_data() {
        let payload = NotificationPayload::recording_ended("streamer", 100, 1024 * 1024 * 500);

        let fields = build_fields(&payload);
        // Should have Segments and Size fields
        assert!(fields.iter().any(|f| f.name == "Segments" && f.value == "100"));
        assert!(fields.iter().any(|f| f.name == "Size" && f.value.contains("500.00 MB")));
    }

    #[test]
    fn test_build_fields_with_output_file() {
        let payload = NotificationPayload::processing_complete("/output/video.mp4", 1024 * 1024 * 100);

        let fields = build_fields(&payload);
        // Should have Size and Output fields
        assert!(fields.iter().any(|f| f.name == "Size"));
        assert!(fields.iter().any(|f| f.name == "Output" && f.value == "/output/video.mp4"));
    }
}
