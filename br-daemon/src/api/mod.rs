// br-daemon/src/api/mod.rs
pub mod auth;
pub mod config_api;
pub mod events;
pub mod images;
pub mod oauth;
pub mod platform_auth;
pub mod recordings;
pub mod response;
pub mod routes;
pub mod status;
pub mod users;
pub mod websocket;

use crate::config::Config;
use crate::manager::{ChannelManager, ManagerEvent};
use crate::processing::ProcessingManager;
use crate::storage::StorageManager;
use crate::version_check::VersionChecker;
use auth::JwtSecret;
use axum::{
    body::Body,
    http::Request,
    middleware::{self, Next},
    response::Response,
    Router,
};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, mpsc};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use users::SessionStore;

pub use oauth::OAuthStateStore;

pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    pub config_path: std::path::PathBuf,
    pub jwt_secret: String,
    pub local_only: bool,
    pub channel_manager: Arc<ChannelManager>,
    pub processing_manager: Arc<ProcessingManager>,
    pub storage_manager: Arc<StorageManager>,
    pub event_tx: broadcast::Sender<ManagerEvent>,
    pub started_at: Instant,
    pub session_store: Arc<SessionStore>,
    pub version_checker: Arc<VersionChecker>,
    /** Channel to signal graceful shutdown from API. */
    pub shutdown_tx: mpsc::Sender<()>,
    /** In-memory storage for OAuth state tokens (CSRF protection). */
    pub oauth_states: OAuthStateStore,
}

/** Marker for local-only mode (skips auth). */
#[derive(Clone)]
pub struct LocalOnlyMode(pub bool);

async fn inject_auth_context(
    mut req: Request<Body>,
    next: Next,
    secret: JwtSecret,
    local_only: LocalOnlyMode,
) -> Response {
    req.extensions_mut().insert(secret);
    req.extensions_mut().insert(local_only);
    next.run(req).await
}

pub fn create_router(state: Arc<AppState>) -> Router {
    let jwt_secret = JwtSecret(state.jwt_secret.clone());
    let local_only = LocalOnlyMode(state.local_only);

    // CORS layer - allow requests from Tauri webview
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    routes::create_routes(state)
        .layer(middleware::from_fn(move |req, next| {
            inject_auth_context(req, next, jwt_secret.clone(), local_only.clone())
        }))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}
