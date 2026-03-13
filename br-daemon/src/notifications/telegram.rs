use crate::config::TelegramConfig;
use crate::notifications::types::{NotificationPayload, NotificationType};
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Serialize;

/** Telegram sendMessage request payload. */
#[derive(Serialize)]
struct TelegramMessage {
    chat_id: String,
    text: String,
    parse_mode: String,
}

/** Get emoji prefix for notification type. */
fn get_emoji(event_type: NotificationType) -> &'static str {
    match event_type {
        NotificationType::StreamLive => "\u{1F534}", // 🔴 Red circle
        NotificationType::RecordingStarted => "\u{23FA}\u{FE0F}", // ⏺️ Record button
        NotificationType::RecordingEnded => "\u{23F9}\u{FE0F}", // ⏹️ Stop button
        NotificationType::ProcessingComplete => "\u{2705}", // ✅ Check mark
        NotificationType::Error => "\u{274C}",       // ❌ Cross mark
    }
}

/** Escape special characters for Telegram Markdown. */
fn escape_markdown(text: &str) -> String {
    text.replace('_', "\\_")
        .replace('*', "\\*")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('`', "\\`")
}

/** Build the notification title with emoji and bold formatting. */
fn build_title(payload: &NotificationPayload) -> String {
    let emoji = get_emoji(payload.event_type);
    let channel_display = payload
        .channel_name
        .as_deref()
        .map(escape_markdown)
        .unwrap_or_else(|| "Unknown channel".to_string());

    match payload.event_type {
        NotificationType::StreamLive => {
            format!("{} *{} is now live!*", emoji, channel_display)
        }
        NotificationType::RecordingStarted => {
            format!("{} *Recording started for {}*", emoji, channel_display)
        }
        NotificationType::RecordingEnded => {
            format!("{} *Recording ended for {}*", emoji, channel_display)
        }
        NotificationType::ProcessingComplete => {
            format!("{} *Processing complete*", emoji)
        }
        NotificationType::Error => {
            if payload.channel_name.is_some() {
                format!("{} *Error for {}*", emoji, channel_display)
            } else {
                format!("{} *Error occurred*", emoji)
            }
        }
    }
}

/** Build the complete message text for Telegram. */
fn build_message_text(payload: &NotificationPayload) -> String {
    let mut text = build_title(payload);

    // Add message as description (escaped)
    if !payload.message.is_empty() {
        text.push_str("\n\n");
        text.push_str(&escape_markdown(&payload.message));
    }

    // Add platform field if present
    if let Some(ref platform) = payload.platform {
        text.push_str("\nPlatform: ");
        text.push_str(&escape_markdown(platform));
    }

    // Extract fields from data if present
    if let Some(ref data) = payload.data {
        // Segments field
        if let Some(segments) = data.get("segments").and_then(|v| v.as_u64()) {
            text.push_str("\nSegments: ");
            text.push_str(&segments.to_string());
        }

        // Size field (format as MB)
        if let Some(bytes) = data.get("bytes").and_then(|v| v.as_u64()) {
            let size_mb = bytes as f64 / (1024.0 * 1024.0);
            text.push_str(&format!("\nSize: {:.2} MB", size_mb));
        }

        // Output file field
        if let Some(output_file) = data.get("output_file").and_then(|v| v.as_str()) {
            text.push_str("\nOutput: ");
            text.push_str(&escape_markdown(output_file));
        }
    }

    text
}

/** Send a notification to Telegram via bot API. */
pub async fn send(
    client: &Client,
    config: &TelegramConfig,
    payload: &NotificationPayload,
) -> Result<()> {
    let text = build_message_text(payload);

    let message = TelegramMessage {
        chat_id: config.chat_id.clone(),
        text,
        parse_mode: "Markdown".to_string(),
    };

    let url = format!(
        "https://api.telegram.org/bot{}/sendMessage",
        config.bot_token
    );

    let response = client
        .post(&url)
        .json(&message)
        .send()
        .await
        .context("Failed to send Telegram bot API request")?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read response body".to_string());
        anyhow::bail!(
            "Telegram bot API returned non-success status {}: {}",
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
        assert_eq!(
            get_emoji(NotificationType::RecordingStarted),
            "\u{23FA}\u{FE0F}"
        );
        assert_eq!(
            get_emoji(NotificationType::RecordingEnded),
            "\u{23F9}\u{FE0F}"
        );
        assert_eq!(get_emoji(NotificationType::ProcessingComplete), "\u{2705}");
        assert_eq!(get_emoji(NotificationType::Error), "\u{274C}");
    }

    #[test]
    fn test_escape_markdown() {
        assert_eq!(escape_markdown("hello_world"), "hello\\_world");
        assert_eq!(escape_markdown("*bold*"), "\\*bold\\*");
        assert_eq!(escape_markdown("[link]"), "\\[link\\]");
        assert_eq!(escape_markdown("`code`"), "\\`code\\`");
        assert_eq!(escape_markdown("a_b*c[d]e`f"), "a\\_b\\*c\\[d\\]e\\`f");
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
        assert!(title.starts_with("\u{1F534}"));
        assert!(title.contains("*")); // Bold markers
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
    fn test_build_title_escapes_special_chars() {
        let payload = NotificationPayload {
            event_type: NotificationType::StreamLive,
            timestamp: Utc::now(),
            channel_name: Some("user_name".to_string()),
            platform: None,
            title: None,
            message: "Test".to_string(),
            data: None,
        };

        let title = build_title(&payload);
        assert!(title.contains("user\\_name"));
    }

    #[test]
    fn test_build_message_text_with_platform() {
        let payload = NotificationPayload {
            event_type: NotificationType::StreamLive,
            timestamp: Utc::now(),
            channel_name: Some("streamer".to_string()),
            platform: Some("Twitch".to_string()),
            title: None,
            message: "Live now!".to_string(),
            data: None,
        };

        let text = build_message_text(&payload);
        assert!(text.contains("Platform: Twitch"));
        assert!(text.contains("Live now!"));
    }

    #[test]
    fn test_build_message_text_with_recording_data() {
        let payload = NotificationPayload::recording_ended("streamer", 100, 1024 * 1024 * 500);

        let text = build_message_text(&payload);
        assert!(text.contains("Segments: 100"));
        assert!(text.contains("Size: 500.00 MB"));
    }

    #[test]
    fn test_build_message_text_with_output_file() {
        let payload =
            NotificationPayload::processing_complete("/output/video.mp4", 1024 * 1024 * 100);

        let text = build_message_text(&payload);
        assert!(text.contains("Size: 100.00 MB"));
        assert!(text.contains("Output: /output/video.mp4"));
    }

    #[test]
    fn test_build_message_text_escapes_output_file() {
        let payload =
            NotificationPayload::processing_complete("/output/video_file[1].mp4", 1024 * 1024);

        let text = build_message_text(&payload);
        assert!(text.contains("Output: /output/video\\_file\\[1\\].mp4"));
    }
}
