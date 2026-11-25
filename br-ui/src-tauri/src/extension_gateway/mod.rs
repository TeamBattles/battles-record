//! Front door for the Battles Replay browser extension.
//!
//! The extension only ever dials `127.0.0.1:9555`. Owning that port here, rather than letting the
//! daemon bind it, means the app can answer even while the local service is stopped or while the
//! UI is viewing a remote server: it offers to start the service, asks the user to allow new
//! browsers, and only then relays frames to the daemon's extension server on a private port.
//!
//! Approval reuses the daemon's own pairing: once the user clicks Allow, the gateway fetches a
//! pair code over the local REST API and sends `pair` upstream itself, so tokens are still issued
//! and verified by the daemon. Extension-sent `pair` messages are never forwarded.

mod broker;
mod origin;
mod relay;
mod session;
pub mod tauri_host;
#[cfg(test)]
mod tests;

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

pub use origin::is_allowed_origin;

pub const FRONT_DOOR_PORTS: [u16; 3] = [9555, 9556, 9557];
pub const PROMPT_TIMEOUT: Duration = Duration::from_secs(120);
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServicePorts {
    pub api: u16,
    pub extension: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptKind {
    Start,
    Allow,
    StartAndAllow,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptEvent {
    pub request_id: String,
    pub kind: PromptKind,
    pub identifier: Option<String>,
    pub origin: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny,
    Failed(String),
}

/// What the gateway needs from the app. Kept free of Tauri types so the gateway can be tested
/// against a fake daemon.
pub trait GatewayHost: Send + Sync + 'static {
    /// Ports of the running local service; None while it is stopped.
    fn service(&self) -> Option<ServicePorts>;
    /// Surface the prompt to the user. Must not block; the answer arrives via
    /// `Gateway::resolve_prompt`.
    fn show_prompt(&self, prompt: &PromptEvent);
    fn generate_pair_code(&self, api_port: u16) -> impl Future<Output = Result<String, String>> + Send;
    /// Browser list or connection state changed.
    fn notify_changed(&self);
}

pub struct Gateway<H: GatewayHost> {
    host: Arc<H>,
    broker: broker::PromptBroker,
    prompt_timeout: Duration,
    declined: AtomicBool,
    port: Mutex<Option<u16>>,
    cancel: CancellationToken,
    /// Serializes pair-code generation and its `pair` round trip; a second code would invalidate
    /// the first before it is used.
    pairing: tokio::sync::Mutex<()>,
}

impl<H: GatewayHost> Gateway<H> {
    pub fn new(host: Arc<H>, prompt_timeout: Duration) -> Arc<Self> {
        Arc::new(Self {
            host,
            broker: broker::PromptBroker::default(),
            prompt_timeout,
            declined: AtomicBool::new(false),
            port: Mutex::new(None),
            cancel: CancellationToken::new(),
            pairing: tokio::sync::Mutex::new(()),
        })
    }

    /// Binds the first free port in `ports` and serves until shutdown. Logs and returns if none bind.
    pub async fn run(self: Arc<Self>, ports: Vec<u16>) {
        let Some(listener) = bind_first(&ports).await else {
            log::warn!("Extension gateway could not bind any of {:?}", ports);
            self.host.notify_changed();
            return;
        };
        let bound = listener.local_addr().map(|addr| addr.port()).ok();
        self.set_port(bound);
        log::info!("Extension gateway listening on 127.0.0.1:{:?}", bound);
        self.host.notify_changed();

        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => break,
                accepted = listener.accept() => match accepted {
                    Ok((stream, _)) => {
                        let gateway = self.clone();
                        tokio::spawn(async move { session::handle(gateway, stream).await });
                    }
                    Err(e) => log::warn!("Extension gateway accept failed: {}", e),
                },
            }
        }
        self.set_port(None);
    }

    pub fn port(&self) -> Option<u16> {
        self.port.lock().map(|port| *port).unwrap_or(None)
    }

    pub fn resolve_prompt(&self, request_id: &str, decision: Decision) -> bool {
        self.broker.resolve(request_id, decision)
    }

    pub fn pending_prompt(&self) -> Option<PromptEvent> {
        self.broker.pending()
    }

    /// Automatic reconnects stop prompting once the user has said no or stopped the service.
    pub fn mark_declined(&self) {
        self.declined.store(true, Ordering::SeqCst);
    }

    pub fn clear_declined(&self) {
        self.declined.store(false, Ordering::SeqCst);
    }

    pub fn shutdown(&self) {
        self.cancel.cancel();
    }

    fn is_declined(&self) -> bool {
        self.declined.load(Ordering::SeqCst)
    }

    fn set_port(&self, port: Option<u16>) {
        if let Ok(mut slot) = self.port.lock() {
            *slot = port;
        }
    }
}

async fn bind_first(ports: &[u16]) -> Option<TcpListener> {
    for &port in ports {
        match TcpListener::bind(("127.0.0.1", port)).await {
            Ok(listener) => return Some(listener),
            Err(e) => log::info!("Extension gateway port {} unavailable: {}", port, e),
        }
    }
    None
}
