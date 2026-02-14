pub mod connection;
pub mod handler;
pub mod pairing;
pub mod protocol;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::config::ExtensionConfig;
use crate::downloads::DownloadManager;
use crate::libraries::LibraryManager;
use protocol::DaemonMessage;
use crate::manager::ManagerEvent;

use connection::{handle_connection, ConnectionInfo, SharedState};
use pairing::PairingManager;

pub struct ExtensionServer {
    #[allow(dead_code)]
    config: ExtensionConfig,
    pairing: Arc<RwLock<PairingManager>>,
    connections: Arc<RwLock<HashMap<Uuid, ConnectionInfo>>>,
    message_senders: Arc<RwLock<HashMap<Uuid, mpsc::Sender<DaemonMessage>>>>,
    library_manager: Arc<tokio::sync::Mutex<LibraryManager>>,
    download_manager: Arc<DownloadManager>,
    channel_manager: Arc<crate::manager::ChannelManager>,
    event_tx: broadcast::Sender<ManagerEvent>,
    app_config: Arc<parking_lot::RwLock<crate::config::Config>>,
    config_path: std::path::PathBuf,
}

impl ExtensionServer {
    /// Start the extension WebSocket server.
    ///
    /// Tries binding to `config.port`, then each `config.fallback_ports` in order.
    /// Returns the actual bound port, a JoinHandle for the server task, and a shutdown sender.
    pub async fn start(
        config: ExtensionConfig,
        pairing: Arc<RwLock<PairingManager>>,
        connections: Arc<RwLock<HashMap<Uuid, ConnectionInfo>>>,
        message_senders: Arc<RwLock<HashMap<Uuid, mpsc::Sender<DaemonMessage>>>>,
        library_manager: Arc<tokio::sync::Mutex<LibraryManager>>,
        download_manager: Arc<DownloadManager>,
        channel_manager: Arc<crate::manager::ChannelManager>,
        event_tx: broadcast::Sender<ManagerEvent>,
        app_config: Arc<parking_lot::RwLock<crate::config::Config>>,
        config_path: std::path::PathBuf,
    ) -> anyhow::Result<(u16, JoinHandle<()>, mpsc::Sender<()>)> {
        let (listener, port) = bind_listener(&config).await?;

        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);

        let server = ExtensionServer {
            config,
            pairing,
            connections: connections.clone(),
            message_senders,
            library_manager,
            download_manager,
            channel_manager,
            event_tx,
            app_config,
            config_path,
        };

        let handle = tokio::spawn(server.run(listener, shutdown_rx));

        info!(port = port, "Extension WebSocket server started");

        Ok((port, handle, shutdown_tx))
    }

    async fn run(self, listener: TcpListener, mut shutdown_rx: mpsc::Receiver<()>) {
        let shared = Arc::new(SharedState {
            pairing: self.pairing,
            connections: self.connections,
            library_manager: self.library_manager,
            download_manager: self.download_manager,
            channel_manager: self.channel_manager,
            event_tx: self.event_tx,
            message_senders: self.message_senders.clone(),
            config: self.app_config,
            config_path: self.config_path,
        });

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            let state = shared.clone();
                            info!(addr = %addr, "Extension connection accepted");
                            tokio::spawn(async move {
                                handle_connection(stream, state).await;
                            });
                        }
                        Err(e) => {
                            error!(error = %e, "Failed to accept extension connection");
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Extension server shutting down");
                    // Send disconnect to all connected clients
                    let senders = shared.message_senders.read().await;
                    if !senders.is_empty() {
                        info!(count = senders.len(), "Disconnecting extension clients");
                        let disconnect_msg = crate::extension::protocol::DaemonMessage::Disconnected {
                            reason: "Server shutting down".into(),
                        };
                        for (_, sender) in senders.iter() {
                            let _ = sender.try_send(disconnect_msg.clone());
                        }
                    }
                    drop(senders);
                    // Give clients a moment to receive the disconnect message
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    break;
                }
            }
        }
    }

    /// Get a reference to the connections map (for REST API status endpoints).
    pub fn connections_ref(
        connections: &Arc<RwLock<HashMap<Uuid, ConnectionInfo>>>,
    ) -> Arc<RwLock<HashMap<Uuid, ConnectionInfo>>> {
        connections.clone()
    }
}

/// Try binding to the primary port, then each fallback port in order.
async fn bind_listener(config: &ExtensionConfig) -> anyhow::Result<(TcpListener, u16)> {
    // Try primary port first
    if let Some(listener) = try_bind(config.port).await {
        return Ok((listener, config.port));
    }
    warn!(
        port = config.port,
        "Failed to bind extension server to primary port"
    );

    // Try fallback ports
    for &port in &config.fallback_ports {
        if let Some(listener) = try_bind(port).await {
            info!(port = port, "Extension server bound to fallback port");
            return Ok((listener, port));
        }
        warn!(
            port = port,
            "Failed to bind extension server to fallback port"
        );
    }

    anyhow::bail!(
        "Failed to bind extension server to any port (tried {} and {:?})",
        config.port,
        config.fallback_ports
    )
}

async fn try_bind(port: u16) -> Option<TcpListener> {
    TcpListener::bind(format!("127.0.0.1:{}", port)).await.ok()
}
