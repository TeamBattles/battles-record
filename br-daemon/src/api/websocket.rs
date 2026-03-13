use crate::api::auth::verify_token;
use crate::api::events::{ActiveRecordingInfo, WsEvent};
use crate::api::AppState;
use crate::manager::ManagerEvent;
use crate::types::ChannelStatus;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, info, warn};

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

/** WebSocket upgrade handler. */
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<Arc<AppState>>,
) -> Response {
    // In local-only mode, allow connections without a token
    if state.local_only {
        info!("WebSocket connection in local-only mode");
        return ws.on_upgrade(move |socket| handle_socket(socket, state, "local".to_string()));
    }

    // Otherwise, validate token before upgrading
    let token = match &query.token {
        Some(t) => t,
        None => {
            return Response::builder()
                .status(401)
                .body("Missing token".into())
                .unwrap_or_else(|_| Response::new("Missing token".into()));
        }
    };

    match verify_token(token, &state.jwt_secret) {
        Ok(claims) => {
            info!("WebSocket connection from user: {}", claims.sub);
            ws.on_upgrade(move |socket| handle_socket(socket, state, claims.sub))
        }
        Err(_) => {
            // Return 401 by not upgrading
            Response::builder()
                .status(401)
                .body("Invalid or expired token".into())
                .unwrap_or_else(|_| Response::new("Invalid or expired token".into()))
        }
    }
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>, username: String) {
    let (mut sender, mut receiver) = socket.split();

    // Send initial state
    let initial_state = build_initial_state(&state);
    if let Ok(json) = serde_json::to_string(&initial_state) {
        if sender.send(Message::Text(json)).await.is_err() {
            return;
        }
    }

    // Trigger background refresh of all channels to ensure fresh status
    // Status updates will flow to this client via the WebSocket event stream
    let channel_manager = state.channel_manager.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        channel_manager.poll_all_channels_debounced().await;
    });

    // Subscribe to events
    let mut event_rx = state.event_tx.subscribe();

    // Spawn task to forward events to WebSocket
    let send_task = tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    if let Some(ws_event) = convert_manager_event(event) {
                        match serde_json::to_string(&ws_event) {
                            Ok(json) => {
                                if sender.send(Message::Text(json)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!("Failed to serialize event: {}", e);
                            }
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!("WebSocket client lagged, skipped {} events", n);
                }
            }
        }
    });

    // Handle incoming messages (ping/pong, close)
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Ping(data)) => {
                debug!("Received ping from {}", username);
                let _ = data;
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket closed by client: {}", username);
                break;
            }
            Err(e) => {
                warn!("WebSocket error for {}: {}", username, e);
                break;
            }
            _ => {}
        }
    }

    send_task.abort();
    info!("WebSocket connection ended for {}", username);
}

fn build_initial_state(state: &AppState) -> WsEvent {
    let channels = state.channel_manager.get_channels();

    let active_recordings: Vec<ActiveRecordingInfo> = channels
        .iter()
        .filter(|c| c.status == ChannelStatus::Recording)
        .map(|c| ActiveRecordingInfo {
            recording_id: c.id,
            channel_id: c.id,
            channel_name: c.name.clone(),
            platform: c.platform,
            duration_secs: 0,
            size_bytes: 0,
            segments: 0,
        })
        .collect();

    WsEvent::Connected {
        channels,
        active_recordings,
    }
}

fn convert_manager_event(event: ManagerEvent) -> Option<WsEvent> {
    match event {
        ManagerEvent::StatusChanged {
            channel_id,
            channel_name,
            platform,
            old_status: _,
            new_status,
        } => Some(WsEvent::ChannelStatus {
            channel_id,
            name: channel_name,
            platform,
            status: format!("{:?}", new_status).to_lowercase(),
            stream: None,
        }),
        ManagerEvent::RecordingStarted {
            channel_id,
            channel_name,
            platform,
            recording_id,
            output_dir: _,
        } => Some(WsEvent::RecordingStarted {
            recording_id,
            channel_id,
            channel_name,
            platform,
            quality: "source".to_string(),
        }),
        ManagerEvent::RecordingProgress {
            channel_id: _,
            recording_id,
            segments_downloaded,
            bytes_downloaded,
        } => Some(WsEvent::SegmentDownloaded {
            recording_id,
            sequence: segments_downloaded,
            size_bytes: 0,
            total_segments: segments_downloaded,
            total_bytes: bytes_downloaded,
        }),
        ManagerEvent::RecordingEnded {
            channel_id: _,
            channel_name: _,
            recording_id,
            total_segments,
            total_bytes,
        } => Some(WsEvent::RecordingEnded {
            recording_id,
            duration_secs: 0,
            size_bytes: total_bytes,
            segment_count: total_segments,
            reason: "stream_ended".to_string(),
        }),
        ManagerEvent::Error {
            channel_id,
            channel_name,
            message,
        } => channel_id.map(|id| WsEvent::ChannelError {
            channel_id: id,
            name: channel_name.unwrap_or_default(),
            error: message,
        }),
        ManagerEvent::ProcessingStarted { recording_id } => {
            Some(WsEvent::ProcessingStarted { recording_id })
        }
        ManagerEvent::ProcessingProgress {
            recording_id,
            percent,
        } => Some(WsEvent::ProcessingProgress {
            recording_id,
            percent,
        }),
        ManagerEvent::ProcessingComplete {
            recording_id,
            output_file,
            size_bytes,
        } => Some(WsEvent::ProcessingComplete {
            recording_id,
            output_file,
            size_bytes,
        }),
        ManagerEvent::ProcessingFailed {
            recording_id,
            error,
        } => Some(WsEvent::ProcessingFailed {
            recording_id,
            error,
        }),
        ManagerEvent::ScheduleSkip {
            channel_id,
            channel_name,
            platform,
        } => Some(WsEvent::ScheduleSkip {
            channel_id,
            channel_name,
            platform,
        }),
        ManagerEvent::FilterSkip {
            channel_id,
            channel_name,
            platform,
            reason,
        } => Some(WsEvent::FilterSkip {
            channel_id,
            channel_name,
            platform,
            reason: serde_json::to_value(&reason).unwrap_or(serde_json::Value::Null),
        }),
        ManagerEvent::QuotaSkip {
            channel_id,
            channel_name,
            platform,
            quota_used_bytes,
            quota_limit_bytes,
        } => Some(WsEvent::QuotaSkip {
            channel_id,
            channel_name,
            platform,
            quota_used_bytes,
            quota_limit_bytes,
        }),
        ManagerEvent::QuotaStatusChanged {
            channel_id,
            channel_name,
            quota_status,
            quota_used_bytes,
            quota_percent,
        } => Some(WsEvent::QuotaStatusChanged {
            channel_id,
            channel_name,
            quota_status,
            quota_used_bytes,
            quota_percent,
        }),
        ManagerEvent::PlatformAuthUpdated {
            platform,
            status,
            username,
            expires_at,
        } => Some(WsEvent::PlatformAuthUpdated {
            platform,
            status,
            username,
            expires_at,
        }),
        ManagerEvent::PlatformAuthExpired { platform, reason } => {
            Some(WsEvent::PlatformAuthExpired { platform, reason })
        }
    }
}
