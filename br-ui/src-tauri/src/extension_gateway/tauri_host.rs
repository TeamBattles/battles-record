//! Tauri side of the extension gateway: reads the sidecar's ports from `DaemonState`, surfaces
//! prompts to the frontend, and exposes the local service's browser list to the UI. Every call
//! here targets the local sidecar, never the server the UI happens to be viewing.

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use super::{Decision, Gateway, GatewayHost, PromptEvent, ServicePorts};
use crate::daemon::{DaemonStateMutex, DaemonStatus};

pub type AppGateway = Gateway<TauriGatewayHost>;

const REQUEST_EVENT: &str = "extension-request";
const CHANGED_EVENT: &str = "extension-gateway-changed";
const LOCAL_API_TIMEOUT: Duration = Duration::from_secs(5);

pub struct TauriGatewayHost {
    app: AppHandle,
}

impl TauriGatewayHost {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl GatewayHost for TauriGatewayHost {
    fn service(&self) -> Option<ServicePorts> {
        let state = self.app.try_state::<DaemonStateMutex>()?;
        let daemon = state.lock().ok()?;
        if !matches!(daemon.status, DaemonStatus::Running) {
            return None;
        }
        Some(ServicePorts {
            api: daemon.port?,
            extension: daemon.extension_port?,
        })
    }

    fn show_prompt(&self, prompt: &PromptEvent) {
        // The window may be hidden in the tray; a prompt nobody can see would just time out.
        if let Some(window) = self.app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
        if let Err(e) = self.app.emit(REQUEST_EVENT, prompt) {
            log::warn!("Failed to emit extension request: {}", e);
        }
    }

    async fn generate_pair_code(&self, api_port: u16) -> Result<String, String> {
        let body = local_request(reqwest::Method::POST, api_port, "/api/extensions/pair-code").await?;
        body["data"]["code"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "The local service returned no pair code".to_string())
    }

    fn notify_changed(&self) {
        notify_changed(&self.app);
    }
}

fn notify_changed(app: &AppHandle) {
    if let Err(e) = app.emit(CHANGED_EVENT, ()) {
        log::warn!("Failed to emit extension gateway change: {}", e);
    }
}

/// A start from Settings, the startup path or a prompt all count as the user wanting the service.
pub fn service_started(app: &AppHandle) {
    if let Some(gateway) = app.try_state::<Arc<AppGateway>>() {
        gateway.clear_declined();
    }
    notify_changed(app);
}

/// A deliberate stop must not be undone by the extension's automatic reconnects.
pub fn service_stopped(app: &AppHandle) {
    if let Some(gateway) = app.try_state::<Arc<AppGateway>>() {
        gateway.mark_declined();
    }
    notify_changed(app);
}

fn running_api_port(app: &AppHandle) -> Option<u16> {
    let state = app.try_state::<DaemonStateMutex>()?;
    let daemon = state.lock().ok()?;
    match daemon.status {
        DaemonStatus::Running => daemon.port,
        _ => None,
    }
}

async fn local_request(method: reqwest::Method, api_port: u16, path: &str) -> Result<Value, String> {
    let response = reqwest::Client::new()
        .request(method, format!("http://127.0.0.1:{api_port}{path}"))
        .timeout(LOCAL_API_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("Could not reach the local service: {e}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| format!("Could not read the local service's reply: {e}"))?;
    if !status.is_success() {
        return Err(format!("The local service answered {status}"));
    }
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).map_err(|e| format!("The local service sent invalid JSON: {e}"))
}

fn parse_client_id(client_id: &str) -> Result<uuid::Uuid, String> {
    uuid::Uuid::parse_str(client_id).map_err(|_| "Invalid browser id".to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayBrowser {
    client_id: String,
    identifier: String,
    connected: bool,
    last_connected: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayStatus {
    listening: bool,
    port: Option<u16>,
    service_running: bool,
    browsers: Vec<GatewayBrowser>,
}

fn browsers_from(body: &Value) -> Vec<GatewayBrowser> {
    let Some(items) = body["data"].as_array() else { return Vec::new() };
    items
        .iter()
        .filter_map(|item| {
            Some(GatewayBrowser {
                client_id: item["client_id"].as_str()?.to_string(),
                identifier: item["identifier"].as_str().unwrap_or("Browser").to_string(),
                connected: item["connected"].as_bool().unwrap_or(false),
                last_connected: item["last_connected"].as_str().unwrap_or_default().to_string(),
            })
        })
        .collect()
}

#[tauri::command]
pub async fn get_extension_gateway_status(
    app: AppHandle,
    gateway: tauri::State<'_, Arc<AppGateway>>,
) -> Result<GatewayStatus, String> {
    let port = gateway.port();
    let api_port = running_api_port(&app);
    let browsers = match api_port {
        Some(api) => match local_request(reqwest::Method::GET, api, "/api/extensions/connections").await {
            Ok(body) => browsers_from(&body),
            Err(e) => {
                log::warn!("Could not list extension browsers: {}", e);
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    Ok(GatewayStatus {
        listening: port.is_some(),
        port,
        service_running: api_port.is_some(),
        browsers,
    })
}

/// A prompt raised before the webview was listening (e.g. at app launch) would otherwise be lost.
#[tauri::command]
pub fn get_pending_extension_request(gateway: tauri::State<'_, Arc<AppGateway>>) -> Option<PromptEvent> {
    gateway.pending_prompt()
}

#[tauri::command]
pub fn respond_extension_request(
    gateway: tauri::State<'_, Arc<AppGateway>>,
    request_id: String,
    decision: String,
    reason: Option<String>,
) -> bool {
    let decision = match decision.as_str() {
        "allow" => Decision::Allow,
        "failed" => Decision::Failed(reason.unwrap_or_else(|| "The local service did not start".to_string())),
        _ => Decision::Deny,
    };
    gateway.resolve_prompt(&request_id, decision)
}

#[tauri::command]
pub async fn remove_extension_browser(app: AppHandle, client_id: String) -> Result<(), String> {
    let id = parse_client_id(&client_id)?;
    let api = running_api_port(&app).ok_or("The local service is not running")?;
    local_request(reqwest::Method::DELETE, api, &format!("/api/extensions/connections/{id}")).await?;
    notify_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn get_extension_message_log(app: AppHandle, client_id: String) -> Result<Vec<Value>, String> {
    let id = parse_client_id(&client_id)?;
    let api = running_api_port(&app).ok_or("The local service is not running")?;
    let body = local_request(reqwest::Method::GET, api, &format!("/api/extensions/connections/{id}/logs")).await?;
    Ok(body["data"].as_array().cloned().unwrap_or_default())
}
