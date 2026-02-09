use crate::config::WebhookConfig;
use crate::notifications::types::{NotificationPayload, NotificationType};
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Serialize;

/** Webhook payload structure sent to external endpoints. */
#[derive(Serialize)]
struct WebhookPayload {
    event: &'static str,
    timestamp: String,
    channel: Option<String>,
    platform: Option<String>,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

/** Convert a NotificationType to its snake_case string representation. */
fn event_to_string(event_type: NotificationType) -> &'static str {
    match event_type {
        NotificationType::StreamLive => "stream_live",
        NotificationType::RecordingStarted => "recording_started",
        NotificationType::RecordingEnded => "recording_ended",
        NotificationType::ProcessingComplete => "processing_complete",
        NotificationType::Error => "error",
    }
}

/**
 * Send a notification to a generic webhook.
 *
 * Sends a POST request with JSON body containing the notification payload.
 * Custom headers from the config are included in the request.
 */
pub async fn send(
    client: &Client,
    config: &WebhookConfig,
    payload: &NotificationPayload,
) -> Result<()> {
    // Build the webhook payload
    let webhook_payload = WebhookPayload {
        event: event_to_string(payload.event_type),
        timestamp: payload.timestamp.to_rfc3339(),
        channel: payload.channel_name.clone(),
        platform: payload.platform.clone(),
        message: payload.message.clone(),
        data: payload.data.clone(),
    };

    // Create the request
    let mut request = client
        .post(&config.url)
        .header("Content-Type", "application/json")
        .json(&webhook_payload);

    // Add custom headers from config
    for (key, value) in &config.headers {
        request = request.header(key, value);
    }

    // Send the request
    let response = request
        .send()
        .await
        .context("Failed to send webhook request")?;

    // Check response status
    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read response body".to_string());
        anyhow::bail!(
            "Webhook request failed with status {}: {}",
            status.as_u16(),
            body
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_event_to_string() {
        assert_eq!(event_to_string(NotificationType::StreamLive), "stream_live");
        assert_eq!(
            event_to_string(NotificationType::RecordingStarted),
            "recording_started"
        );
        assert_eq!(
            event_to_string(NotificationType::RecordingEnded),
            "recording_ended"
        );
        assert_eq!(
            event_to_string(NotificationType::ProcessingComplete),
            "processing_complete"
        );
        assert_eq!(event_to_string(NotificationType::Error), "error");
    }

    #[test]
    fn test_webhook_payload_serialization() {
        let payload = WebhookPayload {
            event: "stream_live",
            timestamp: "2026-01-24T15:30:00+00:00".to_string(),
            channel: Some("xqc".to_string()),
            platform: Some("twitch".to_string()),
            message: "xqc is now live!".to_string(),
            data: Some(serde_json::json!({
                "title": "GAMING TIME"
            })),
        };

        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["event"], "stream_live");
        assert_eq!(json["channel"], "xqc");
        assert_eq!(json["platform"], "twitch");
        assert_eq!(json["data"]["title"], "GAMING TIME");
    }

    #[test]
    fn test_webhook_payload_without_data() {
        let payload = WebhookPayload {
            event: "recording_started",
            timestamp: "2026-01-24T15:30:00+00:00".to_string(),
            channel: Some("streamer".to_string()),
            platform: None,
            message: "Recording started".to_string(),
            data: None,
        };

        let json = serde_json::to_value(&payload).unwrap();
        // data field should be omitted when None
        assert!(!json.as_object().unwrap().contains_key("data"));
    }

    #[test]
    fn test_webhook_config_headers() {
        let mut headers = HashMap::new();
        headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());
        headers.insert("Authorization".to_string(), "Bearer token123".to_string());

        let config = WebhookConfig {
            url: "https://example.com/webhook".to_string(),
            headers,
            on_stream_start: true,
            on_stream_end: true,
            on_error: false,
        };

        assert_eq!(config.headers.len(), 2);
        assert_eq!(
            config.headers.get("X-Custom-Header"),
            Some(&"custom-value".to_string())
        );
    }
}
