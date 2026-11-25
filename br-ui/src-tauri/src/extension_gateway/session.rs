//! One extension connection: origin check, hello, optional start prompt, daemon hello, optional
//! Allow prompt and pairing, then the relay.

use std::sync::Arc;
use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::broker::PromptOutcome;
use super::relay::{self, json_frame, service_stopped_frame};
use super::{is_allowed_origin, Gateway, GatewayHost, PromptKind, ServicePorts, HELLO_TIMEOUT};

type ClientWs = WebSocketStream<TcpStream>;
type UpstreamWs = WebSocketStream<MaybeTlsStream<TcpStream>>;
type UpTx = SplitSink<UpstreamWs, Message>;
type UpRx = SplitStream<UpstreamWs>;

const UPSTREAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_IDENTIFIER_CHARS: usize = 100;

#[derive(Clone, Copy)]
enum DenyCode {
    ServiceStopped,
    ServiceDeclined,
    ServiceStartFailed,
    PairingDeclined,
    ApprovalTimeout,
}

impl DenyCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::ServiceStopped => "service_stopped",
            Self::ServiceDeclined => "service_declined",
            Self::ServiceStartFailed => "service_start_failed",
            Self::PairingDeclined => "pairing_declined",
            Self::ApprovalTimeout => "approval_timeout",
        }
    }
}

pub(super) async fn handle<H: GatewayHost>(gateway: Arc<Gateway<H>>, stream: TcpStream) {
    let Some((ws, origin)) = accept(stream).await else { return };
    let (client_tx, mut client_rx) = ws.split();

    let Some(hello_text) = read_text(&mut client_rx, HELLO_TIMEOUT).await else { return };
    let hello: Value = serde_json::from_str(&hello_text).unwrap_or(Value::Null);

    let mut session = Session {
        gateway,
        origin,
        identifier: hello.get("identifier").and_then(Value::as_str).map(clip_identifier),
        client_tx,
        client_rx,
        preapproved: false,
    };

    if hello["type"] != "hello" {
        session
            .send(json!({"type": "error", "code": "protocol_error", "detail": "First message must be Hello"}))
            .await;
        session.close().await;
        return;
    }
    let has_token = hello.get("token").and_then(Value::as_str).is_some();
    let interactive = hello.get("interactive").and_then(Value::as_bool).unwrap_or(false);

    let Some(service) = session.ensure_service(has_token, interactive).await else { return };
    let Some((mut up_tx, mut up_rx)) = connect_upstream(service.extension).await else {
        session.deny(DenyCode::ServiceStartFailed, None).await;
        return;
    };

    if up_tx.send(Message::text(hello_text)).await.is_err() {
        session.deny(DenyCode::ServiceStartFailed, None).await;
        return;
    }
    let Some(reply_text) = read_text(&mut up_rx, HELLO_TIMEOUT).await else {
        session.send_frame(service_stopped_frame()).await;
        session.close().await;
        return;
    };
    let reply: Value = serde_json::from_str(&reply_text).unwrap_or(Value::Null);
    if !session.send_frame(Message::text(reply_text)).await {
        return;
    }

    if reply["type"] != "hello" {
        // The daemon refused the hello (an invalid token); it closes its side and so do we.
        session.close().await;
        return;
    }
    if reply["requires_pairing"] == true
        && !session.pairing_phase(service, &mut up_tx, &mut up_rx).await
    {
        return;
    }

    session.gateway.host.notify_changed();
    let cancel = session.gateway.cancel.clone();
    relay::run(&mut session.client_tx, &mut session.client_rx, &mut up_tx, &mut up_rx, &cancel).await;
    session.gateway.host.notify_changed();
}

struct Session<H: GatewayHost> {
    gateway: Arc<Gateway<H>>,
    origin: Option<String>,
    identifier: Option<String>,
    client_tx: SplitSink<ClientWs, Message>,
    client_rx: SplitStream<ClientWs>,
    /// Accepting a combined start-and-connect prompt already counts as Allow.
    preapproved: bool,
}

impl<H: GatewayHost> Session<H> {
    async fn ensure_service(&mut self, has_token: bool, interactive: bool) -> Option<ServicePorts> {
        if let Some(service) = self.gateway.host.service() {
            return Some(service);
        }
        if !interactive && self.gateway.is_declined() {
            self.deny(DenyCode::ServiceStopped, None).await;
            return None;
        }
        if !self.status("awaiting_start").await {
            return None;
        }
        let kind = if has_token { PromptKind::Start } else { PromptKind::StartAndAllow };
        match self.ask(kind, self.identifier.clone()).await? {
            PromptOutcome::Allowed => {}
            PromptOutcome::Denied => {
                self.gateway.mark_declined();
                self.deny(DenyCode::ServiceDeclined, None).await;
                return None;
            }
            PromptOutcome::Failed(reason) => {
                self.deny(DenyCode::ServiceStartFailed, Some(&reason)).await;
                return None;
            }
            PromptOutcome::TimedOut => {
                self.deny(DenyCode::ApprovalTimeout, None).await;
                return None;
            }
        }
        if !self.status("starting").await {
            return None;
        }
        match self.gateway.host.service() {
            Some(service) => {
                self.preapproved = kind == PromptKind::StartAndAllow;
                Some(service)
            }
            None => {
                self.deny(DenyCode::ServiceStartFailed, None).await;
                None
            }
        }
    }

    /// Before the daemon has authenticated this browser: intercept `request_pairing`, refuse
    /// legacy `pair`, pass everything else through. Returns true once the daemon sent `paired`.
    async fn pairing_phase(&mut self, service: ServicePorts, up_tx: &mut UpTx, up_rx: &mut UpRx) -> bool {
        loop {
            tokio::select! {
                msg = self.client_rx.next() => {
                    let text = match msg {
                        Some(Ok(Message::Text(text))) => text,
                        Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return false,
                        Some(Ok(_)) => continue,
                    };
                    let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                    match value["type"].as_str() {
                        Some("request_pairing") => {
                            let identifier = value
                                .get("identifier")
                                .and_then(Value::as_str)
                                .map(clip_identifier)
                                .or_else(|| self.identifier.clone());
                            return self.approve_and_pair(service, identifier, up_tx, up_rx).await;
                        }
                        Some("pair") => {
                            self.send(json!({"type": "pair_failed", "code": "extension_outdated"})).await;
                        }
                        _ => {
                            if up_tx.send(Message::Text(text)).await.is_err() {
                                return self.upstream_gone().await;
                            }
                        }
                    }
                }
                msg = up_rx.next() => match msg {
                    Some(Ok(Message::Text(text))) => {
                        if !self.send_frame(Message::Text(text)).await {
                            return false;
                        }
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return self.upstream_gone().await,
                    Some(Ok(_)) => {}
                },
            }
        }
    }

    async fn approve_and_pair(
        &mut self,
        service: ServicePorts,
        identifier: Option<String>,
        up_tx: &mut UpTx,
        up_rx: &mut UpRx,
    ) -> bool {
        if !self.preapproved {
            if !self.status("awaiting_approval").await {
                return false;
            }
            let Some(outcome) = self.ask(PromptKind::Allow, identifier.clone()).await else {
                return false;
            };
            match outcome {
                PromptOutcome::Allowed => {}
                PromptOutcome::Denied => {
                    self.deny(DenyCode::PairingDeclined, None).await;
                    return false;
                }
                PromptOutcome::Failed(reason) => {
                    self.deny(DenyCode::PairingDeclined, Some(&reason)).await;
                    return false;
                }
                PromptOutcome::TimedOut => {
                    self.deny(DenyCode::ApprovalTimeout, None).await;
                    return false;
                }
            }
        }

        let gateway = self.gateway.clone();
        let _pairing = gateway.pairing.lock().await;
        let code = match gateway.host.generate_pair_code(service.api).await {
            Ok(code) => code,
            Err(e) => {
                self.send(json!({"type": "pair_failed", "code": "pairing_unavailable", "detail": e}))
                    .await;
                self.close().await;
                return false;
            }
        };
        let pair = json!({
            "type": "pair",
            "code": code,
            "identifier": identifier.unwrap_or_else(|| "Browser".to_string()),
        });
        if up_tx.send(json_frame(pair)).await.is_err() {
            return self.upstream_gone().await;
        }

        loop {
            match up_rx.next().await {
                Some(Ok(Message::Text(text))) => {
                    let kind = serde_json::from_str::<Value>(&text)
                        .ok()
                        .and_then(|v| v["type"].as_str().map(str::to_string));
                    if !self.send_frame(Message::Text(text)).await {
                        return false;
                    }
                    match kind.as_deref() {
                        Some("paired") => {
                            gateway.host.notify_changed();
                            return true;
                        }
                        Some("pair_failed") => {
                            self.close().await;
                            return false;
                        }
                        _ => {}
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return self.upstream_gone().await,
                Some(Ok(_)) => {}
            }
        }
    }

    /// Waits for the user's answer, giving up early if the extension disconnects meanwhile so a
    /// late Allow cannot pair a browser that is no longer listening.
    async fn ask(&mut self, kind: PromptKind, identifier: Option<String>) -> Option<PromptOutcome> {
        let gateway = self.gateway.clone();
        let ask = gateway.broker.ask(
            &*gateway.host,
            kind,
            identifier,
            self.origin.clone(),
            gateway.prompt_timeout,
        );
        tokio::pin!(ask);
        loop {
            tokio::select! {
                outcome = &mut ask => return Some(outcome),
                msg = self.client_rx.next() => match msg {
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return None,
                    Some(Ok(_)) => {}
                },
            }
        }
    }

    async fn status(&mut self, state: &str) -> bool {
        self.send(json!({"type": "connection_status", "state": state})).await
    }

    /// `detail` only when there is diagnostic text beyond the code (why a start failed).
    async fn deny(&mut self, code: DenyCode, detail: Option<&str>) {
        let mut frame = json!({"type": "connection_denied", "code": code.as_str()});
        if let Some(detail) = detail {
            frame["detail"] = json!(detail);
        }
        self.send(frame).await;
        self.close().await;
    }

    async fn upstream_gone(&mut self) -> bool {
        self.send_frame(service_stopped_frame()).await;
        self.close().await;
        false
    }

    async fn send(&mut self, value: Value) -> bool {
        self.send_frame(json_frame(value)).await
    }

    async fn send_frame(&mut self, frame: Message) -> bool {
        self.client_tx.send(frame).await.is_ok()
    }

    async fn close(&mut self) {
        let _ = self.client_tx.send(Message::Close(None)).await;
    }
}

// The handshake callback's error type is fixed by tungstenite.
#[allow(clippy::result_large_err)]
async fn accept(stream: TcpStream) -> Option<(ClientWs, Option<String>)> {
    let mut origin = None;
    let check_origin = |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
        let value = match request.headers().get("origin") {
            None => None,
            Some(header) => match header.to_str() {
                Ok(value) => Some(value.to_string()),
                Err(_) => return Err(forbidden()),
            },
        };
        if !is_allowed_origin(value.as_deref()) {
            return Err(forbidden());
        }
        origin = value;
        Ok(response)
    };
    match tokio_tungstenite::accept_hdr_async(stream, check_origin).await {
        Ok(ws) => Some((ws, origin)),
        Err(e) => {
            log::debug!("Extension gateway handshake refused: {}", e);
            None
        }
    }
}

fn forbidden() -> ErrorResponse {
    let mut response = ErrorResponse::new(Some("Only browser extensions may connect".to_string()));
    *response.status_mut() = StatusCode::FORBIDDEN;
    response
}

async fn connect_upstream(port: u16) -> Option<(UpTx, UpRx)> {
    let url = format!("ws://127.0.0.1:{port}");
    match tokio::time::timeout(UPSTREAM_CONNECT_TIMEOUT, tokio_tungstenite::connect_async(url)).await {
        Ok(Ok((ws, _))) => Some(ws.split()),
        Ok(Err(e)) => {
            log::warn!("Extension gateway could not reach the local service: {}", e);
            None
        }
        Err(_) => {
            log::warn!("Extension gateway timed out reaching the local service");
            None
        }
    }
}

async fn read_text<S>(rx: &mut S, timeout: Duration) -> Option<String>
where
    S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let deadline = Instant::now() + timeout;
    loop {
        match tokio::time::timeout_at(deadline, rx.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => return Some(text.to_string()),
            Ok(Some(Ok(Message::Close(_)))) | Ok(Some(Err(_))) | Ok(None) | Err(_) => return None,
            Ok(Some(Ok(_))) => continue,
        }
    }
}

fn clip_identifier(identifier: &str) -> String {
    identifier.chars().take(MAX_IDENTIFIER_CHARS).collect()
}
