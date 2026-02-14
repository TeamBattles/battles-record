mod discord;
mod telegram;
mod types;
mod webhook;

pub use types::{NotificationPayload, NotificationType};

use crate::config::NotificationsConfig;
use crate::manager::ManagerEvent;
use crate::types::ChannelStatus;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/** Manages sending notifications to configured channels (Discord, Telegram, webhooks). */
pub struct NotificationManager {
    config: NotificationsConfig,
    client: Client,
}

impl NotificationManager {
    /**
     * Create a new NotificationManager with the given configuration.
     *
     * # Panics
     * This function will log an error and use a default client if TLS initialization fails,
     * which should only happen in extremely rare circumstances (e.g., missing system certificates).
     */
    pub fn new(config: NotificationsConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|e| {
                error!(
                    "Failed to create HTTP client with TLS: {}, using default client",
                    e
                );
                Client::new()
            });

        Self { config, client }
    }

    /**
     * Start the notification manager, listening for events.
     *
     * This spawns an async task that listens for ManagerEvents and dispatches
     * notifications to configured channels.
     */
    pub fn start(self: Arc<Self>, mut event_rx: broadcast::Receiver<ManagerEvent>) {
        tokio::spawn(async move {
            info!("NotificationManager started");

            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        if let Some(payload) = self.event_to_payload(&event) {
                            self.dispatch(payload);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(count)) => {
                        warn!("NotificationManager lagged, missed {} events", count);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Event channel closed, NotificationManager stopping");
                        break;
                    }
                }
            }
        });
    }

    /** Convert a ManagerEvent to a NotificationPayload if it should trigger a notification. */
    pub fn event_to_payload(&self, event: &ManagerEvent) -> Option<NotificationPayload> {
        match event {
            ManagerEvent::StatusChanged {
                channel_name,
                old_status: _,
                new_status,
                ..
            } => {
                if *new_status == ChannelStatus::Live {
                    Some(NotificationPayload::stream_live(channel_name, "", None))
                } else {
                    None
                }
            }
            ManagerEvent::RecordingStarted { channel_name, .. } => {
                Some(NotificationPayload::recording_started(channel_name))
            }
            ManagerEvent::RecordingEnded {
                channel_name,
                total_segments,
                total_bytes,
                ..
            } => Some(NotificationPayload::recording_ended(
                channel_name,
                *total_segments,
                *total_bytes,
            )),
            ManagerEvent::ProcessingComplete {
                output_file,
                size_bytes,
                ..
            } => Some(NotificationPayload::processing_complete(
                output_file,
                *size_bytes,
            )),
            ManagerEvent::Error {
                channel_name,
                message,
                ..
            } => {
                if let Some(name) = channel_name {
                    Some(NotificationPayload::error_with_channel(name, message))
                } else {
                    Some(NotificationPayload::error(message))
                }
            }
            ManagerEvent::DownloadComplete {
                channel_name,
                filepath,
                filesize,
                ..
            } => Some(NotificationPayload::download_complete(
                channel_name,
                &filepath.to_string_lossy(),
                *filesize,
            )),
            ManagerEvent::DownloadFailed {
                channel_name,
                error,
                ..
            } => Some(NotificationPayload::download_failed(channel_name, error)),
            // Events that don't generate notifications
            ManagerEvent::RecordingProgress { .. }
            | ManagerEvent::ProcessingStarted { .. }
            | ManagerEvent::ProcessingProgress { .. }
            | ManagerEvent::ProcessingFailed { .. }
            | ManagerEvent::ScheduleSkip { .. }
            | ManagerEvent::FilterSkip { .. }
            | ManagerEvent::QuotaSkip { .. }
            | ManagerEvent::QuotaStatusChanged { .. }
            | ManagerEvent::PlatformAuthUpdated { .. }
            | ManagerEvent::PlatformAuthExpired { .. }
            | ManagerEvent::DownloadQueued { .. }
            | ManagerEvent::DownloadProgress { .. }
            | ManagerEvent::DownloadPaused { .. }
            | ManagerEvent::DownloadResumed { .. }
            | ManagerEvent::DownloadCancelled { .. }
            | ManagerEvent::LibraryStatusChanged { .. }
            | ManagerEvent::ExtensionConnected { .. }
            | ManagerEvent::ExtensionDisconnected { .. }
            | ManagerEvent::ChannelAdded { .. }
            | ManagerEvent::ChannelRemoved { .. } => None,
        }
    }

    /**
     * Dispatch a notification payload to all configured notification channels.
     *
     * This method is non-blocking - it spawns async tasks for each sender.
     */
    pub fn dispatch(&self, payload: NotificationPayload) {
        debug!("Dispatching notification: {:?}", payload.event_type);

        // Check which event types should trigger notifications
        let (send_on_start, send_on_end, send_on_error, send_on_dl_complete, send_on_dl_failed) =
            match payload.event_type {
                NotificationType::StreamLive | NotificationType::RecordingStarted => {
                    (true, false, false, false, false)
                }
                NotificationType::RecordingEnded | NotificationType::ProcessingComplete => {
                    (false, true, false, false, false)
                }
                NotificationType::Error => (false, false, true, false, false),
                NotificationType::DownloadComplete => (false, false, false, true, false),
                NotificationType::DownloadFailed => (false, false, false, false, true),
            };

        // Dispatch to Discord if configured
        if let Some(ref discord_config) = self.config.discord {
            let should_send = (send_on_start && discord_config.on_stream_start)
                || (send_on_end && discord_config.on_stream_end)
                || (send_on_error && discord_config.on_error)
                || (send_on_dl_complete && discord_config.on_download_complete)
                || (send_on_dl_failed && discord_config.on_download_failed);

            if should_send {
                let client = self.client.clone();
                let config = discord_config.clone();
                let payload = payload.clone();

                tokio::spawn(async move {
                    if let Err(e) = discord::send(&client, &config, &payload).await {
                        error!("Failed to send Discord notification: {}", e);
                    }
                });
            }
        }

        // Dispatch to Telegram if configured
        if let Some(ref telegram_config) = self.config.telegram {
            let should_send = (send_on_start && telegram_config.on_stream_start)
                || (send_on_end && telegram_config.on_stream_end)
                || (send_on_error && telegram_config.on_error)
                || (send_on_dl_complete && telegram_config.on_download_complete)
                || (send_on_dl_failed && telegram_config.on_download_failed);

            if should_send {
                let client = self.client.clone();
                let config = telegram_config.clone();
                let payload = payload.clone();

                tokio::spawn(async move {
                    if let Err(e) = telegram::send(&client, &config, &payload).await {
                        error!("Failed to send Telegram notification: {}", e);
                    }
                });
            }
        }

        // Dispatch to webhook if configured
        if let Some(ref webhook_config) = self.config.webhook {
            let should_send = (send_on_start && webhook_config.on_stream_start)
                || (send_on_end && webhook_config.on_stream_end)
                || (send_on_error && webhook_config.on_error)
                || (send_on_dl_complete && webhook_config.on_download_complete)
                || (send_on_dl_failed && webhook_config.on_download_failed);

            if should_send {
                let client = self.client.clone();
                let config = webhook_config.clone();
                let payload = payload.clone();

                tokio::spawn(async move {
                    if let Err(e) = webhook::send(&client, &config, &payload).await {
                        error!("Failed to send webhook notification: {}", e);
                    }
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NotificationsConfig;

    #[test]
    fn test_notification_manager_creation() {
        let config = NotificationsConfig::default();
        let _manager = NotificationManager::new(config);
    }

    #[test]
    fn test_event_to_payload_recording_started() {
        let config = NotificationsConfig::default();
        let manager = NotificationManager::new(config);

        let event = ManagerEvent::RecordingStarted {
            channel_id: uuid::Uuid::new_v4(),
            channel_name: "testchannel".to_string(),
            platform: crate::types::Platform::Twitch,
            recording_id: uuid::Uuid::new_v4(),
            output_dir: std::path::PathBuf::from("/tmp/test"),
        };

        let payload = manager.event_to_payload(&event);
        assert!(payload.is_some());
        let payload = payload.unwrap();
        assert_eq!(payload.event_type, NotificationType::RecordingStarted);
        assert_eq!(payload.channel_name.as_deref(), Some("testchannel"));
    }

    #[test]
    fn test_event_to_payload_progress_returns_none() {
        let config = NotificationsConfig::default();
        let manager = NotificationManager::new(config);

        let event = ManagerEvent::RecordingProgress {
            channel_id: uuid::Uuid::new_v4(),
            recording_id: uuid::Uuid::new_v4(),
            segments_downloaded: 10,
            bytes_downloaded: 1024,
        };

        let payload = manager.event_to_payload(&event);
        assert!(payload.is_none());
    }

    #[test]
    fn test_event_to_payload_error() {
        let config = NotificationsConfig::default();
        let manager = NotificationManager::new(config);

        let event = ManagerEvent::Error {
            channel_id: Some(uuid::Uuid::new_v4()),
            channel_name: Some("testchannel".to_string()),
            message: "Connection failed".to_string(),
        };

        let payload = manager.event_to_payload(&event);
        assert!(payload.is_some());
        let payload = payload.unwrap();
        assert_eq!(payload.event_type, NotificationType::Error);
        assert!(payload.message.contains("testchannel"));
    }
}
