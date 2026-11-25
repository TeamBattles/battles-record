use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::*;

type Client = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Default)]
struct FakeHost {
    service: Mutex<Option<ServicePorts>>,
    prompts: Mutex<Vec<PromptEvent>>,
    code_calls: AtomicUsize,
    changes: AtomicUsize,
}

impl FakeHost {
    fn set_service(&self, ports: Option<ServicePorts>) {
        *self.service.lock().unwrap() = ports;
    }

    fn prompts(&self) -> Vec<PromptEvent> {
        self.prompts.lock().unwrap().clone()
    }
}

impl GatewayHost for FakeHost {
    fn service(&self) -> Option<ServicePorts> {
        *self.service.lock().unwrap()
    }

    fn show_prompt(&self, prompt: &PromptEvent) {
        self.prompts.lock().unwrap().push(prompt.clone());
    }

    fn generate_pair_code(&self, _api_port: u16) -> impl Future<Output = Result<String, String>> + Send {
        self.code_calls.fetch_add(1, Ordering::SeqCst);
        async { Ok("ABC123".to_string()) }
    }

    fn notify_changed(&self) {
        self.changes.fetch_add(1, Ordering::SeqCst);
    }
}

/// Speaks just enough of the daemon's extension protocol: hello, pair with code ABC123, echo.
/// A client message of type `close_me` makes it drop the connection.
async fn spawn_fake_daemon() -> (u16, Arc<Mutex<Vec<Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_outer = seen.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let seen = seen.clone();
            tokio::spawn(async move {
                let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
                while let Some(Ok(msg)) = ws.next().await {
                    let Message::Text(text) = msg else { continue };
                    let value: Value = serde_json::from_str(&text).unwrap();
                    seen.lock().unwrap().push(value.clone());
                    let reply = match value["type"].as_str().unwrap_or_default() {
                        "hello" => json!({
                            "type": "hello", "version": "test",
                            "requires_pairing": value.get("token").map_or(true, Value::is_null),
                            "libraries": {}, "libraries_installed": true
                        }),
                        "pair" if value["code"] == "ABC123" => {
                            json!({"type": "paired", "token": "tok", "identifier": value["identifier"]})
                        }
                        "pair" => json!({"type": "pair_failed", "code": "invalid_code"}),
                        "close_me" => {
                            let _ = ws.close(None).await;
                            return;
                        }
                        other => json!({"type": "echo", "of": other}),
                    };
                    if ws.send(Message::text(reply.to_string())).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    (port, seen_outer)
}

fn daemon_seen(seen: &Arc<Mutex<Vec<Value>>>, kind: &str) -> usize {
    seen.lock().unwrap().iter().filter(|m| m["type"] == kind).count()
}

async fn start_gateway(host: Arc<FakeHost>, timeout: Duration) -> (Arc<Gateway<FakeHost>>, u16) {
    start_gateway_on(host, timeout, vec![0]).await
}

async fn start_gateway_on(
    host: Arc<FakeHost>,
    timeout: Duration,
    ports: Vec<u16>,
) -> (Arc<Gateway<FakeHost>>, u16) {
    let gateway = Gateway::new(host, timeout);
    tokio::spawn(gateway.clone().run(ports));
    for _ in 0..200 {
        if let Some(port) = gateway.port() {
            return (gateway, port);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("gateway never bound a port");
}

async fn connect(port: u16, origin: Option<&'static str>) -> Result<Client, tokio_tungstenite::tungstenite::Error> {
    let mut request = format!("ws://127.0.0.1:{port}").into_client_request()?;
    if let Some(origin) = origin {
        request.headers_mut().insert("Origin", HeaderValue::from_static(origin));
    }
    tokio_tungstenite::connect_async(request).await.map(|(ws, _)| ws)
}

async fn send(ws: &mut Client, value: Value) {
    ws.send(Message::text(value.to_string())).await.unwrap();
}

async fn recv(ws: &mut Client) -> Value {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("timed out waiting for a message")
            .expect("socket ended")
            .expect("socket error");
        if let Message::Text(text) = msg {
            return serde_json::from_str(&text).unwrap();
        }
        if let Message::Close(_) = msg {
            panic!("socket closed while waiting for a message");
        }
    }
}

/// True when the gateway closes the socket (close frame, end of stream or reset).
async fn closes(ws: &mut Client) -> bool {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
            Err(_) => return false,
            Ok(None) | Ok(Some(Err(_))) | Ok(Some(Ok(Message::Close(_)))) => return true,
            Ok(Some(Ok(_))) => continue,
        }
    }
}

async fn wait_for_prompts(host: &FakeHost, count: usize) -> Vec<PromptEvent> {
    for _ in 0..500 {
        let prompts = host.prompts();
        if prompts.len() >= count {
            return prompts;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("expected {count} prompt(s), saw {:?}", host.prompts());
}

fn hello(token: Option<&str>, interactive: bool) -> Value {
    let mut msg = json!({"type": "hello", "id": "h", "extension_version": "3.4.0", "identifier": "Chrome on Linux"});
    if let Some(token) = token {
        msg["token"] = json!(token);
    }
    if interactive {
        msg["interactive"] = json!(true);
    }
    msg
}

async fn running_host() -> (Arc<FakeHost>, Arc<Mutex<Vec<Value>>>) {
    let (daemon_port, seen) = spawn_fake_daemon().await;
    let host = Arc::new(FakeHost::default());
    host.set_service(Some(ServicePorts { api: 1, extension: daemon_port }));
    (host, seen)
}

#[test]
fn origin_rule_allows_extensions_and_missing_origin_only() {
    assert!(is_allowed_origin(None));
    assert!(is_allowed_origin(Some("chrome-extension://abcdefghijklmnop")));
    assert!(is_allowed_origin(Some("moz-extension://0b1c2d3e-uuid")));
    assert!(!is_allowed_origin(Some("https://evil.example")));
    assert!(!is_allowed_origin(Some("http://127.0.0.1:5173")));
    assert!(!is_allowed_origin(Some("null")));
    assert!(!is_allowed_origin(Some("")));
}

#[tokio::test]
async fn rejects_web_origin_without_prompt() {
    let (host, _) = running_host().await;
    let (_gw, port) = start_gateway(host.clone(), PROMPT_TIMEOUT).await;
    assert!(connect(port, Some("https://evil.example")).await.is_err());
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(host.prompts().is_empty());
}

#[tokio::test]
async fn allows_extension_and_missing_origins() {
    let (host, _) = running_host().await;
    let (_gw, port) = start_gateway(host, PROMPT_TIMEOUT).await;
    for origin in [None, Some("chrome-extension://abc"), Some("moz-extension://uuid")] {
        let mut ws = connect(port, origin).await.expect("handshake should succeed");
        send(&mut ws, hello(Some("tok"), false)).await;
        assert_eq!(recv(&mut ws).await["type"], "hello");
    }
}

#[tokio::test]
async fn token_hello_is_relayed_when_service_running() {
    let (host, _) = running_host().await;
    let (_gw, port) = start_gateway(host.clone(), PROMPT_TIMEOUT).await;
    let mut ws = connect(port, None).await.unwrap();
    send(&mut ws, hello(Some("tok"), false)).await;
    let reply = recv(&mut ws).await;
    assert_eq!(reply["type"], "hello");
    assert_eq!(reply["requires_pairing"], false);
    send(&mut ws, json!({"type": "get_queue", "id": "1"})).await;
    assert_eq!(recv(&mut ws).await, json!({"type": "echo", "of": "get_queue"}));
    assert!(host.prompts().is_empty());
}

#[tokio::test]
async fn request_pairing_allow_pairs_once() {
    let (host, seen) = running_host().await;
    let (gw, port) = start_gateway(host.clone(), PROMPT_TIMEOUT).await;
    let mut ws = connect(port, Some("chrome-extension://abc")).await.unwrap();
    send(&mut ws, hello(None, true)).await;
    assert_eq!(recv(&mut ws).await["requires_pairing"], true);
    send(&mut ws, json!({"type": "request_pairing", "id": "r", "identifier": "Chrome on Linux"})).await;
    assert_eq!(recv(&mut ws).await, json!({"type": "connection_status", "state": "awaiting_approval"}));
    let prompts = wait_for_prompts(&host, 1).await;
    assert_eq!(prompts[0].kind, PromptKind::Allow);
    assert_eq!(prompts[0].identifier.as_deref(), Some("Chrome on Linux"));
    assert_eq!(prompts[0].origin.as_deref(), Some("chrome-extension://abc"));
    assert!(gw.resolve_prompt(&prompts[0].request_id, Decision::Allow));
    let paired = recv(&mut ws).await;
    assert_eq!(paired["type"], "paired");
    assert_eq!(paired["token"], "tok");
    assert_eq!(daemon_seen(&seen, "pair"), 1);
    assert_eq!(daemon_seen(&seen, "request_pairing"), 0);
    assert_eq!(host.code_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn request_pairing_deny_never_reaches_daemon() {
    let (host, seen) = running_host().await;
    let (gw, port) = start_gateway(host.clone(), PROMPT_TIMEOUT).await;
    let mut ws = connect(port, None).await.unwrap();
    send(&mut ws, hello(None, true)).await;
    recv(&mut ws).await;
    send(&mut ws, json!({"type": "request_pairing", "id": "r", "identifier": "Chrome on Linux"})).await;
    recv(&mut ws).await;
    let prompts = wait_for_prompts(&host, 1).await;
    gw.resolve_prompt(&prompts[0].request_id, Decision::Deny);
    assert_eq!(recv(&mut ws).await, json!({"type": "connection_denied", "code": "pairing_declined"}));
    assert!(closes(&mut ws).await);
    assert_eq!(daemon_seen(&seen, "pair"), 0);
    assert_eq!(host.code_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn concurrent_approvals_both_pair() {
    let (host, seen) = running_host().await;
    let (gw, port) = start_gateway(host.clone(), PROMPT_TIMEOUT).await;
    let mut a = connect(port, None).await.unwrap();
    let mut b = connect(port, None).await.unwrap();
    for (ws, name) in [(&mut a, "Chrome on Linux"), (&mut b, "Firefox on Linux")] {
        send(ws, hello(None, true)).await;
        recv(ws).await;
        send(ws, json!({"type": "request_pairing", "id": "r", "identifier": name})).await;
        recv(ws).await;
    }
    let first = wait_for_prompts(&host, 1).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(host.prompts().len(), 1, "second prompt must wait for the first");
    gw.resolve_prompt(&first[0].request_id, Decision::Allow);
    let second = wait_for_prompts(&host, 2).await;
    gw.resolve_prompt(&second[1].request_id, Decision::Allow);
    assert_eq!(recv(&mut a).await["type"], "paired");
    assert_eq!(recv(&mut b).await["type"], "paired");
    assert_eq!(daemon_seen(&seen, "pair"), 2);
}

#[tokio::test]
async fn stopped_service_start_prompt_then_relay() {
    let (daemon_port, _) = spawn_fake_daemon().await;
    let host = Arc::new(FakeHost::default());
    let (gw, port) = start_gateway(host.clone(), PROMPT_TIMEOUT).await;
    let mut ws = connect(port, None).await.unwrap();
    send(&mut ws, hello(Some("tok"), true)).await;
    assert_eq!(recv(&mut ws).await, json!({"type": "connection_status", "state": "awaiting_start"}));
    let prompts = wait_for_prompts(&host, 1).await;
    assert_eq!(prompts[0].kind, PromptKind::Start);
    host.set_service(Some(ServicePorts { api: 1, extension: daemon_port }));
    gw.resolve_prompt(&prompts[0].request_id, Decision::Allow);
    assert_eq!(recv(&mut ws).await, json!({"type": "connection_status", "state": "starting"}));
    let reply = recv(&mut ws).await;
    assert_eq!(reply["type"], "hello");
    assert_eq!(reply["requires_pairing"], false);
}

#[tokio::test]
async fn start_and_allow_preapproves_pairing() {
    let (daemon_port, seen) = spawn_fake_daemon().await;
    let host = Arc::new(FakeHost::default());
    let (gw, port) = start_gateway(host.clone(), PROMPT_TIMEOUT).await;
    let mut ws = connect(port, None).await.unwrap();
    send(&mut ws, hello(None, true)).await;
    recv(&mut ws).await;
    let prompts = wait_for_prompts(&host, 1).await;
    assert_eq!(prompts[0].kind, PromptKind::StartAndAllow);
    host.set_service(Some(ServicePorts { api: 1, extension: daemon_port }));
    gw.resolve_prompt(&prompts[0].request_id, Decision::Allow);
    recv(&mut ws).await;
    assert_eq!(recv(&mut ws).await["requires_pairing"], true);
    send(&mut ws, json!({"type": "request_pairing", "id": "r", "identifier": "Chrome on Linux"})).await;
    assert_eq!(recv(&mut ws).await["type"], "paired");
    assert_eq!(host.prompts().len(), 1);
    assert_eq!(daemon_seen(&seen, "pair"), 1);
}

#[tokio::test]
async fn non_interactive_after_decline_is_denied_without_prompt() {
    let host = Arc::new(FakeHost::default());
    let (gw, port) = start_gateway(host.clone(), PROMPT_TIMEOUT).await;
    gw.mark_declined();
    let mut ws = connect(port, None).await.unwrap();
    send(&mut ws, hello(Some("tok"), false)).await;
    assert_eq!(recv(&mut ws).await, json!({"type": "connection_denied", "code": "service_stopped"}));
    assert!(closes(&mut ws).await);
    assert!(host.prompts().is_empty());
}

#[tokio::test]
async fn interactive_after_decline_still_prompts() {
    let host = Arc::new(FakeHost::default());
    let (gw, port) = start_gateway(host.clone(), PROMPT_TIMEOUT).await;
    gw.mark_declined();
    let mut ws = connect(port, None).await.unwrap();
    send(&mut ws, hello(Some("tok"), true)).await;
    assert_eq!(recv(&mut ws).await["state"], "awaiting_start");
    assert_eq!(wait_for_prompts(&host, 1).await[0].kind, PromptKind::Start);
}

#[tokio::test]
async fn declining_the_start_prompt_remembers_it() {
    let host = Arc::new(FakeHost::default());
    let (gw, port) = start_gateway(host.clone(), PROMPT_TIMEOUT).await;
    let mut ws = connect(port, None).await.unwrap();
    send(&mut ws, hello(Some("tok"), true)).await;
    recv(&mut ws).await;
    let prompts = wait_for_prompts(&host, 1).await;
    gw.resolve_prompt(&prompts[0].request_id, Decision::Deny);
    assert_eq!(recv(&mut ws).await, json!({"type": "connection_denied", "code": "service_declined"}));
    let mut again = connect(port, None).await.unwrap();
    send(&mut again, hello(Some("tok"), false)).await;
    assert_eq!(recv(&mut again).await["code"], "service_stopped");
    assert_eq!(host.prompts().len(), 1);
}

#[tokio::test]
async fn failed_start_is_reported() {
    let host = Arc::new(FakeHost::default());
    let (gw, port) = start_gateway(host.clone(), PROMPT_TIMEOUT).await;
    let mut ws = connect(port, None).await.unwrap();
    send(&mut ws, hello(Some("tok"), true)).await;
    recv(&mut ws).await;
    let prompts = wait_for_prompts(&host, 1).await;
    gw.resolve_prompt(&prompts[0].request_id, Decision::Failed("disk full".into()));
    assert_eq!(
        recv(&mut ws).await,
        json!({"type": "connection_denied", "code": "service_start_failed", "detail": "disk full"})
    );
}

#[tokio::test]
async fn prompt_timeout_denies() {
    let host = Arc::new(FakeHost::default());
    let (_gw, port) = start_gateway(host.clone(), Duration::from_millis(200)).await;
    let mut ws = connect(port, None).await.unwrap();
    send(&mut ws, hello(Some("tok"), true)).await;
    assert_eq!(recv(&mut ws).await["state"], "awaiting_start");
    assert_eq!(recv(&mut ws).await, json!({"type": "connection_denied", "code": "approval_timeout"}));
}

#[tokio::test]
async fn second_connection_joins_open_start_prompt() {
    let (daemon_port, _) = spawn_fake_daemon().await;
    let host = Arc::new(FakeHost::default());
    let (gw, port) = start_gateway(host.clone(), PROMPT_TIMEOUT).await;
    let mut a = connect(port, None).await.unwrap();
    let mut b = connect(port, None).await.unwrap();
    for ws in [&mut a, &mut b] {
        send(ws, hello(Some("tok"), true)).await;
        assert_eq!(recv(ws).await["state"], "awaiting_start");
    }
    let prompts = wait_for_prompts(&host, 1).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(host.prompts().len(), 1);
    host.set_service(Some(ServicePorts { api: 1, extension: daemon_port }));
    gw.resolve_prompt(&prompts[0].request_id, Decision::Allow);
    for ws in [&mut a, &mut b] {
        assert_eq!(recv(ws).await["state"], "starting");
        assert_eq!(recv(ws).await["type"], "hello");
    }
}

#[tokio::test]
async fn legacy_pair_message_is_refused_as_extension_outdated() {
    let (host, seen) = running_host().await;
    let (_gw, port) = start_gateway(host.clone(), PROMPT_TIMEOUT).await;
    let mut ws = connect(port, None).await.unwrap();
    send(&mut ws, hello(None, false)).await;
    assert_eq!(recv(&mut ws).await["requires_pairing"], true);
    send(&mut ws, json!({"type": "pair", "code": "ZZZ999", "identifier": "x"})).await;
    assert_eq!(recv(&mut ws).await, json!({"type": "pair_failed", "code": "extension_outdated"}));
    assert_eq!(daemon_seen(&seen, "pair"), 0);
    assert!(host.prompts().is_empty());
}

#[tokio::test]
async fn daemon_close_sends_disconnected_to_client() {
    let (host, _) = running_host().await;
    let (_gw, port) = start_gateway(host.clone(), PROMPT_TIMEOUT).await;
    let mut ws = connect(port, None).await.unwrap();
    send(&mut ws, hello(Some("tok"), false)).await;
    recv(&mut ws).await;
    send(&mut ws, json!({"type": "close_me"})).await;
    assert_eq!(recv(&mut ws).await, json!({"type": "disconnected", "code": "service_stopped"}));
    assert!(closes(&mut ws).await);
}

#[tokio::test]
async fn falls_back_when_primary_port_is_busy() {
    let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let busy_port = busy.local_addr().unwrap().port();
    let host = Arc::new(FakeHost::default());
    let (_gw, port) = start_gateway_on(host, PROMPT_TIMEOUT, vec![busy_port, 0]).await;
    assert_ne!(port, busy_port);
}

#[tokio::test]
async fn pending_prompt_is_exposed_until_answered() {
    let (host, _) = running_host().await;
    let (gw, port) = start_gateway(host.clone(), PROMPT_TIMEOUT).await;
    assert!(gw.pending_prompt().is_none());
    let mut ws = connect(port, None).await.unwrap();
    send(&mut ws, hello(None, true)).await;
    recv(&mut ws).await;
    send(&mut ws, json!({"type": "request_pairing", "id": "r", "identifier": "Chrome on Linux"})).await;
    recv(&mut ws).await;
    let shown = wait_for_prompts(&host, 1).await;
    let pending = gw.pending_prompt().expect("the open prompt should be exposed");
    assert_eq!(pending.request_id, shown[0].request_id);
    assert_eq!(pending.kind, PromptKind::Allow);
    gw.resolve_prompt(&pending.request_id, Decision::Allow);
    assert!(gw.pending_prompt().is_none());
}
