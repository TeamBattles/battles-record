use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::downloads::DownloadManager;
use crate::extension::handler;
use crate::extension::pairing::PairingManager;
use crate::extension::protocol::{DaemonMessage, ExtensionMessage};
use crate::libraries::LibraryManager;
use crate::manager::ManagerEvent;

const MAX_MESSAGE_LOG: usize = 50;
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub client_id: Uuid,
    pub identifier: String,
    pub connected_at: DateTime<Utc>,
    pub message_log: VecDeque<MessageLogEntry>,
}

#[derive(Debug, Clone)]
pub struct MessageLogEntry {
    pub timestamp: DateTime<Utc>,
    pub direction: MessageDirection,
    pub message_type: String,
    pub payload: Option<String>,
}

#[derive(Debug, Clone)]
pub enum MessageDirection {
    Sent,
    Received,
}

/// Shared state passed to each connection handler.
pub struct SharedState {
    pub pairing: Arc<RwLock<PairingManager>>,
    pub connections: Arc<RwLock<HashMap<Uuid, ConnectionInfo>>>,
    pub library_manager: Arc<tokio::sync::Mutex<LibraryManager>>,
    pub download_manager: Arc<DownloadManager>,
    pub channel_manager: Arc<crate::manager::ChannelManager>,
    pub event_tx: broadcast::Sender<ManagerEvent>,
    pub message_senders: Arc<RwLock<HashMap<Uuid, mpsc::Sender<DaemonMessage>>>>,
    pub config: Arc<parking_lot::RwLock<crate::config::Config>>,
    pub config_path: std::path::PathBuf,
}

type WsSink = SplitSink<WebSocketStream<TcpStream>, Message>;

/// Handle a single TCP connection: upgrade to WebSocket, authenticate, then run message loop.
pub async fn handle_connection(stream: TcpStream, state: Arc<SharedState>) {
    let addr = stream
        .peer_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let ws = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            warn!(addr = %addr, error = %e, "WebSocket handshake failed");
            return;
        }
    };

    debug!(addr = %addr, "WebSocket connection established");

    let (mut sink, mut stream_rx) = ws.split();
    let mut log = VecDeque::with_capacity(MAX_MESSAGE_LOG);

    // Wait for Hello message with timeout
    let hello_msg = tokio::time::timeout(HELLO_TIMEOUT, async {
        while let Some(msg) = stream_rx.next().await {
            match msg {
                Ok(Message::Text(text)) => return Some(text),
                Ok(Message::Close(_)) => return None,
                Err(e) => {
                    warn!(addr = %addr, error = %e, "Error reading Hello");
                    return None;
                }
                _ => continue,
            }
        }
        None
    })
    .await;

    let hello_text = match hello_msg {
        Ok(Some(text)) => text,
        Ok(None) => {
            debug!(addr = %addr, "Connection closed before Hello");
            return;
        }
        Err(_) => {
            warn!(addr = %addr, "Timed out waiting for Hello");
            let _ = send_message(
                &mut sink,
                &DaemonMessage::Error {
                    id: None,
                    code: "timeout".to_string(),
                    message: "Timed out waiting for Hello message".to_string(),
                },
                &mut log,
            )
            .await;
            return;
        }
    };

    log_received(&mut log, "hello", Some(hello_text.clone()));

    let ext_msg: ExtensionMessage = match serde_json::from_str(&hello_text) {
        Ok(msg) => msg,
        Err(e) => {
            warn!(addr = %addr, error = %e, "Invalid Hello message");
            let _ = send_message(
                &mut sink,
                &DaemonMessage::Error {
                    id: None,
                    code: "invalid_message".to_string(),
                    message: format!("Invalid Hello message: {e}"),
                },
                &mut log,
            )
            .await;
            return;
        }
    };

    let (_ext_version, token) = match ext_msg {
        ExtensionMessage::Hello {
            extension_version,
            token,
        } => (extension_version, token),
        _ => {
            warn!(addr = %addr, "First message was not Hello");
            let _ = send_message(
                &mut sink,
                &DaemonMessage::Error {
                    id: None,
                    code: "protocol_error".to_string(),
                    message: "First message must be Hello".to_string(),
                },
                &mut log,
            )
            .await;
            return;
        }
    };

    let libraries = {
        let lib_mgr = state.library_manager.lock().await;
        lib_mgr.check_status().await
    };
    let libraries_installed = libraries.ytdlp.installed && libraries.ffmpeg.installed;
    let daemon_version = env!("CARGO_PKG_VERSION").to_string();

    // Authentication
    let (client_id, identifier) = if let Some(ref tok) = token {
        // Token-based reconnection
        let mut pairing = state.pairing.write().await;
        match pairing.verify_token(tok) {
            Some(client) => {
                let id = client.id;
                let ident = client.identifier.clone();
                info!(addr = %addr, client_id = %id, identifier = %ident, "Extension reconnected");

                if send_message(
                    &mut sink,
                    &DaemonMessage::Hello {
                        version: daemon_version,
                        requires_pairing: false,
                        identifier: Some(ident.clone()),
                        libraries,
                        libraries_installed,
                    },
                    &mut log,
                )
                .await
                .is_err()
                {
                    return;
                }

                (id, ident)
            }
            None => {
                warn!(addr = %addr, "Invalid token");
                let _ = send_message(
                    &mut sink,
                    &DaemonMessage::Error {
                        id: None,
                        code: "invalid_token".to_string(),
                        message: "Token is invalid or revoked".to_string(),
                    },
                    &mut log,
                )
                .await;
                return;
            }
        }
    } else {
        // New client - needs pairing (code generated separately via REST API)
        {
            let pairing = state.pairing.read().await;
            debug!(
                addr = %addr,
                has_active_code = pairing.has_active_code(),
                "New extension connection, requires pairing"
            );
        }

        if send_message(
            &mut sink,
            &DaemonMessage::Hello {
                version: daemon_version,
                requires_pairing: true,
                identifier: None,
                libraries,
                libraries_installed,
            },
            &mut log,
        )
        .await
        .is_err()
        {
            return;
        }

        // Wait for Pair message
        match wait_for_pair(&mut stream_rx, &mut sink, &mut log, &state, &addr).await {
            Some((id, ident)) => (id, ident),
            None => return,
        }
    };

    // Send initial queue state from download manager
    let queue_summaries = state.download_manager.get_queue().await;
    let proto_downloads: Vec<crate::extension::protocol::DownloadJobSummary> = queue_summaries
        .iter()
        .map(job_summary_to_proto)
        .collect();
    if send_message(
        &mut sink,
        &DaemonMessage::QueueState { downloads: proto_downloads },
        &mut log,
    )
    .await
    .is_err()
    {
        return;
    }

    // Send current channels list
    let channels = state.channel_manager.get_channels();
    let channel_summaries: Vec<crate::extension::protocol::ChannelSummary> = channels
        .iter()
        .map(|ch| crate::extension::protocol::ChannelSummary {
            channel_id: ch.id.to_string(),
            name: ch.name.clone(),
            platform: ch.platform.to_string().to_lowercase(),
            enabled: ch.enabled,
            status: format!("{:?}", ch.status).to_lowercase(),
            profile_image_url: ch.profile_image_url.clone(),
        })
        .collect();
    if send_message(
        &mut sink,
        &DaemonMessage::ChannelsState { channels: channel_summaries },
        &mut log,
    )
    .await
    .is_err()
    {
        return;
    }

    // Register connection
    {
        let mut connections = state.connections.write().await;
        connections.insert(
            client_id,
            ConnectionInfo {
                client_id,
                identifier: identifier.clone(),
                connected_at: Utc::now(),
                message_log: log.clone(),
            },
        );
    }

    // Notify Tauri UI about the new connection
    let _ = state.event_tx.send(ManagerEvent::ExtensionConnected {
        client_id,
        identifier: identifier.clone(),
    });

    info!(client_id = %client_id, identifier = %identifier, "Extension authenticated, entering message loop");

    // Register message sender for graceful shutdown
    let (msg_tx, mut msg_rx) = mpsc::channel::<DaemonMessage>(16);
    {
        let mut senders = state.message_senders.write().await;
        senders.insert(client_id, msg_tx);
    }

    // Subscribe to broadcast events for forwarding
    let mut event_rx = state.event_tx.subscribe();

    loop {
        tokio::select! {
            msg = stream_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ExtensionMessage>(&text) {
                            Ok(ext_msg) => {
                                let msg_type = message_type_name(&ext_msg);
                                log_received(&mut log, msg_type, Some(text.clone()));
                                update_connection_log(&state.connections, client_id, &log).await;

                                // Unpair requires closing the connection
                                if matches!(ext_msg, ExtensionMessage::Unpair) {
                                    info!(client_id = %client_id, "Extension requested unpair");
                                    let _ = send_message(&mut sink, &DaemonMessage::Unpaired, &mut log).await;
                                    break;
                                }

                                let response = handler::handle_message(&ext_msg, &state, client_id).await;
                                if let Some(resp) = response {
                                    if send_message(&mut sink, &resp, &mut log).await.is_err() {
                                        break;
                                    }
                                    update_connection_log(&state.connections, client_id, &log).await;
                                }
                            }
                            Err(e) => {
                                warn!(client_id = %client_id, error = %e, "Invalid message from extension");
                                if send_message(
                                    &mut sink,
                                    &DaemonMessage::Error {
                                        id: None,
                                        code: "invalid_message".to_string(),
                                        message: format!("Failed to parse message: {e}"),
                                    },
                                    &mut log,
                                ).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!(client_id = %client_id, "Extension disconnected (close frame)");
                        break;
                    }
                    Some(Err(e)) => {
                        warn!(client_id = %client_id, error = %e, "WebSocket error");
                        break;
                    }
                    None => {
                        info!(client_id = %client_id, "Extension connection closed");
                        break;
                    }
                    _ => {}
                }
            }
            event = event_rx.recv() => {
                match event {
                    Ok(manager_event) => {
                        if let Some(daemon_msg) = convert_event_to_daemon_message(&manager_event) {
                            if send_message(&mut sink, &daemon_msg, &mut log).await.is_err() {
                                break;
                            }
                            update_connection_log(&state.connections, client_id, &log).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(client_id = %client_id, skipped = n, "Extension client lagged on events");
                    }
                }
            }
            Some(daemon_msg) = msg_rx.recv() => {
                let is_disconnect = matches!(daemon_msg, DaemonMessage::Disconnected { .. });
                if send_message(&mut sink, &daemon_msg, &mut log).await.is_err() {
                    break;
                }
                update_connection_log(&state.connections, client_id, &log).await;
                if is_disconnect {
                    info!(client_id = %client_id, "Closing connection after Disconnected message");
                    break;
                }
            }
        }
    }

    // Notify Tauri UI about the disconnection
    let _ = state.event_tx.send(ManagerEvent::ExtensionDisconnected {
        client_id,
        identifier: identifier.clone(),
    });

    // Cleanup
    {
        let mut senders = state.message_senders.write().await;
        senders.remove(&client_id);
    }
    {
        let mut connections = state.connections.write().await;
        connections.remove(&client_id);
    }
    info!(client_id = %client_id, identifier = %identifier, "Extension connection ended");
}

/// Wait for a Pair message, verify it, send Paired response, and return (client_id, identifier).
async fn wait_for_pair<S>(
    stream_rx: &mut S,
    sink: &mut WsSink,
    log: &mut VecDeque<MessageLogEntry>,
    state: &SharedState,
    addr: &str,
) -> Option<(Uuid, String)>
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    while let Some(msg) = stream_rx.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let ext_msg: ExtensionMessage = match serde_json::from_str(&text) {
                    Ok(m) => m,
                    Err(e) => {
                        warn!(addr = %addr, error = %e, "Invalid message while awaiting Pair");
                        let _ = send_message(
                            sink,
                            &DaemonMessage::Error {
                                id: None,
                                code: "invalid_message".to_string(),
                                message: format!("Failed to parse message: {e}"),
                            },
                            log,
                        )
                        .await;
                        continue;
                    }
                };

                match ext_msg {
                    ExtensionMessage::Pair { code, identifier } => {
                        log_received(log, "pair", Some(text.clone()));
                        debug!(addr = %addr, identifier = %identifier, "Pair attempt received");

                        let mut pairing = state.pairing.write().await;
                        match pairing.verify_code(&code, &identifier) {
                            Ok((client_id, token)) => {
                                info!(addr = %addr, client_id = %client_id, identifier = %identifier, "Extension paired successfully");

                                let _ = send_message(
                                    sink,
                                    &DaemonMessage::Paired {
                                        token,
                                        identifier: identifier.clone(),
                                    },
                                    log,
                                )
                                .await;

                                return Some((client_id, identifier));
                            }
                            Err(e) => {
                                warn!(addr = %addr, error = %e, error_variant = ?e, identifier = %identifier, "Pair code verification failed");
                                let _ = send_message(
                                    sink,
                                    &DaemonMessage::PairFailed {
                                        reason: e.to_string(),
                                    },
                                    log,
                                )
                                .await;
                                // Allow retry for recoverable errors, close for abuse
                                if matches!(
                                    e,
                                    crate::extension::pairing::PairingError::InvalidCode
                                        | crate::extension::pairing::PairingError::NoActiveCode
                                        | crate::extension::pairing::PairingError::CodeExpired
                                ) {
                                    continue;
                                }
                                return None;
                            }
                        }
                    }
                    ExtensionMessage::Ping => {
                        log_received(log, "ping", Some(text.clone()));
                        let _ = send_message(sink, &DaemonMessage::Pong, log).await;
                        continue;
                    }
                    _ => {
                        warn!(addr = %addr, "Expected Pair message during pairing phase");
                        let _ = send_message(
                            sink,
                            &DaemonMessage::Error {
                                id: None,
                                code: "protocol_error".to_string(),
                                message: "Expected Pair message during pairing".to_string(),
                            },
                            log,
                        )
                        .await;
                        continue;
                    }
                }
            }
            Ok(Message::Close(_)) => return None,
            Err(e) => {
                warn!(addr = %addr, error = %e, "Error while waiting for Pair");
                return None;
            }
            _ => continue,
        }
    }
    None
}

/// Send a DaemonMessage as JSON over the WebSocket.
async fn send_message(
    sink: &mut WsSink,
    msg: &DaemonMessage,
    log: &mut VecDeque<MessageLogEntry>,
) -> Result<(), ()> {
    let msg_type = daemon_message_type_name(msg);
    let json = match serde_json::to_string(msg) {
        Ok(j) => j,
        Err(e) => {
            warn!(error = %e, "Failed to serialize DaemonMessage");
            return Err(());
        }
    };

    let payload = json.clone();
    if let Err(e) = sink.send(Message::Text(json)).await {
        warn!(error = %e, "Failed to send WebSocket message");
        return Err(());
    }

    log_sent(log, msg_type, Some(payload));
    Ok(())
}

fn log_received(log: &mut VecDeque<MessageLogEntry>, msg_type: &str, payload: Option<String>) {
    if log.len() >= MAX_MESSAGE_LOG {
        log.pop_front();
    }
    log.push_back(MessageLogEntry {
        timestamp: Utc::now(),
        direction: MessageDirection::Received,
        message_type: msg_type.to_string(),
        payload,
    });
}

fn log_sent(log: &mut VecDeque<MessageLogEntry>, msg_type: &str, payload: Option<String>) {
    if log.len() >= MAX_MESSAGE_LOG {
        log.pop_front();
    }
    log.push_back(MessageLogEntry {
        timestamp: Utc::now(),
        direction: MessageDirection::Sent,
        message_type: msg_type.to_string(),
        payload,
    });
}

async fn update_connection_log(
    connections: &Arc<RwLock<HashMap<Uuid, ConnectionInfo>>>,
    client_id: Uuid,
    log: &VecDeque<MessageLogEntry>,
) {
    let mut conns = connections.write().await;
    if let Some(info) = conns.get_mut(&client_id) {
        info.message_log = log.clone();
    }
}

fn message_type_name(msg: &ExtensionMessage) -> &'static str {
    match msg {
        ExtensionMessage::Hello { .. } => "hello",
        ExtensionMessage::Pair { .. } => "pair",
        ExtensionMessage::ExtractInfo { .. } => "extract_info",
        ExtensionMessage::Download { .. } => "download",
        ExtensionMessage::Pause { .. } => "pause",
        ExtensionMessage::Resume { .. } => "resume",
        ExtensionMessage::Cancel { .. } => "cancel",
        ExtensionMessage::Prioritize { .. } => "prioritize",
        ExtensionMessage::GetQueue => "get_queue",
        ExtensionMessage::GetLibraryStatus => "get_library_status",
        ExtensionMessage::InstallLibraries => "install_libraries",
        ExtensionMessage::UpdateLibrary { .. } => "update_library",
        ExtensionMessage::UninstallLibrary { .. } => "uninstall_library",
        ExtensionMessage::Unpair => "unpair",
        ExtensionMessage::Ping => "ping",
        ExtensionMessage::MergeChannels { .. } => "merge_channels",
        ExtensionMessage::AddChannel { .. } => "add_channel",
        ExtensionMessage::RemoveChannel { .. } => "remove_channel",
    }
}

/// Convert a ManagerEvent into a DaemonMessage for forwarding to the extension.
/// Returns None for events that aren't relevant to extension clients.
fn convert_event_to_daemon_message(event: &ManagerEvent) -> Option<DaemonMessage> {
    match event {
        ManagerEvent::DownloadProgress {
            download_id,
            percent,
            speed,
            eta,
            downloaded_bytes,
            total_bytes,
        } => Some(DaemonMessage::DownloadProgress {
            download_id: download_id.to_string(),
            status: "downloading".to_string(),
            percent: *percent,
            speed: speed.clone(),
            eta: *eta,
            downloaded_bytes: *downloaded_bytes,
            total_bytes: *total_bytes,
        }),
        ManagerEvent::DownloadComplete {
            download_id,
            filepath,
            filesize,
            ..
        } => Some(DaemonMessage::DownloadComplete {
            download_id: download_id.to_string(),
            filepath: filepath.to_string_lossy().to_string(),
            filesize: *filesize,
        }),
        ManagerEvent::DownloadFailed {
            download_id,
            error,
            update_available,
            ..
        } => Some(DaemonMessage::DownloadFailed {
            download_id: download_id.to_string(),
            error: error.clone(),
            update_available: *update_available,
        }),
        ManagerEvent::DownloadPaused { download_id } => Some(DaemonMessage::DownloadPaused {
            id: String::new(),
            download_id: download_id.to_string(),
        }),
        ManagerEvent::DownloadResumed { download_id } => Some(DaemonMessage::DownloadResumed {
            id: String::new(),
            download_id: download_id.to_string(),
        }),
        ManagerEvent::DownloadCancelled { download_id } => Some(DaemonMessage::DownloadCancelled {
            id: String::new(),
            download_id: download_id.to_string(),
        }),
        // Skip DownloadQueued - client that initiated already knows
        ManagerEvent::DownloadQueued { .. } => None,
        // Forward library status changes to extension
        ManagerEvent::LibraryStatusChanged {
            library,
            installed,
            version,
        } => {
            if *installed {
                Some(DaemonMessage::LibraryInstalled {
                    library: library.clone(),
                    version: version.clone().unwrap_or_default(),
                })
            } else {
                Some(DaemonMessage::LibraryUninstalled {
                    id: String::new(),
                    library: library.clone(),
                })
            }
        }
        ManagerEvent::ChannelAdded {
            channel_id,
            channel_name,
            platform,
        } => Some(DaemonMessage::ChannelAdded {
            id: String::new(),
            channel_id: channel_id.to_string(),
            name: channel_name.clone(),
            platform: platform.to_string().to_lowercase(),
        }),
        ManagerEvent::ChannelRemoved {
            channel_id,
            channel_name,
            platform,
        } => Some(DaemonMessage::ChannelRemoved {
            id: String::new(),
            channel_id: channel_id.to_string(),
            name: channel_name.clone(),
            platform: platform.to_string().to_lowercase(),
        }),
        // All other events are not relevant to extension clients
        _ => None,
    }
}

/// Convert a job summary from the downloads module into the protocol format for extensions.
fn job_summary_to_proto(
    summary: &crate::downloads::job::DownloadJobSummary,
) -> crate::extension::protocol::DownloadJobSummary {
    crate::extension::protocol::DownloadJobSummary {
        id: summary.id.to_string(),
        url: summary.url.clone(),
        title: summary.title.clone(),
        thumbnail: summary.thumbnail.clone(),
        platform_name: summary.platform_name.clone(),
        channel_name: summary.channel_name.clone(),
        source_platform: summary.source_platform.clone(),
        status: serde_json::to_value(summary.status)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "unknown".to_string()),
        percent: summary.percent,
        speed: summary.speed.clone(),
        eta: summary.eta,
        downloaded_bytes: summary.downloaded_bytes,
        total_bytes: summary.total_bytes,
        quality: summary.quality.clone().or_else(|| crate::extension::handler::format_to_quality(summary.format.as_deref())),
        format: summary.format.clone(),
        requested_by: summary.requested_by.to_string(),
        requested_by_name: summary.requested_by_name.clone(),
        created_at: summary.created_at.to_rfc3339(),
        completed_at: summary.completed_at.map(|t| t.to_rfc3339()),
        error: summary.error.clone(),
        update_available: summary.update_available,
    }
}

fn daemon_message_type_name(msg: &DaemonMessage) -> &'static str {
    match msg {
        DaemonMessage::Hello { .. } => "hello",
        DaemonMessage::Paired { .. } => "paired",
        DaemonMessage::PairFailed { .. } => "pair_failed",
        DaemonMessage::InfoResult { .. } => "info_result",
        DaemonMessage::DownloadStarted { .. } => "download_started",
        DaemonMessage::DownloadProgress { .. } => "download_progress",
        DaemonMessage::DownloadComplete { .. } => "download_complete",
        DaemonMessage::DownloadFailed { .. } => "download_failed",
        DaemonMessage::DownloadPaused { .. } => "download_paused",
        DaemonMessage::DownloadResumed { .. } => "download_resumed",
        DaemonMessage::DownloadCancelled { .. } => "download_cancelled",
        DaemonMessage::DownloadPrioritized { .. } => "download_prioritized",
        DaemonMessage::Unpaired => "unpaired",
        DaemonMessage::QuotaWarning { .. } => "quota_warning",
        DaemonMessage::QuotaExceeded { .. } => "quota_exceeded",
        DaemonMessage::QueueState { .. } => "queue_state",
        DaemonMessage::ChannelsState { .. } => "channels_state",
        DaemonMessage::LibraryDownloadProgress { .. } => "library_download_progress",
        DaemonMessage::LibraryInstalled { .. } => "library_installed",
        DaemonMessage::LibraryInstallFailed { .. } => "library_install_failed",
        DaemonMessage::LibraryUninstalled { .. } => "library_uninstalled",
        DaemonMessage::LibraryUpdateAvailable { .. } => "library_update_available",
        DaemonMessage::PortChanged { .. } => "port_changed",
        DaemonMessage::Disconnected { .. } => "disconnected",
        DaemonMessage::ChannelsMerged { .. } => "channels_merged",
        DaemonMessage::ChannelAdded { .. } => "channel_added",
        DaemonMessage::ChannelRemoved { .. } => "channel_removed",
        DaemonMessage::Pong => "pong",
        DaemonMessage::Error { .. } => "error",
    }
}
