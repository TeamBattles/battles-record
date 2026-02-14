// br-daemon/tests/common/test_server.rs
//! Test server utilities for API integration testing.
//!
//! This module provides helpers for creating test instances of the API server
//! with properly configured dependencies.

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
    response::Response,
    Router,
};
use br_daemon::{
    api::{create_router, AppState},
    config::{Config, SegmentHandling},
    downloads::DownloadManager,
    manager::ChannelManager,
    processing::ProcessingManager,
    storage::StorageManager,
    types::UserRole,
};
use parking_lot::RwLock;
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Instant};
use tempfile::TempDir;
use tokio::sync::{broadcast, mpsc};
use tower::ServiceExt;

use super::create_test_config;

/// A test server instance with all dependencies configured
pub struct TestServer {
    pub router: Router,
    pub state: Arc<AppState>,
    pub temp_dir: TempDir,
    pub jwt_secret: String,
    _shutdown_rx: mpsc::Receiver<()>,
}

impl TestServer {
    /// Create a new test server with default configuration
    pub async fn new() -> anyhow::Result<Self> {
        Self::with_options(TestServerOptions::default()).await
    }

    /// Create a test server with custom options
    pub async fn with_options(options: TestServerOptions) -> anyhow::Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let config = create_test_config(&temp_dir);

        Self::from_config(config, temp_dir, options).await
    }

    /// Create a test server from an existing config
    pub async fn from_config(
        mut config: Config,
        temp_dir: TempDir,
        options: TestServerOptions,
    ) -> anyhow::Result<Self> {
        // Add test users if not in local-only mode
        if !options.local_only {
            config.users.push(br_daemon::config::UserConfig {
                username: "admin".to_string(),
                password_hash: br_daemon::api::auth::hash_password("admin123")?,
                role: UserRole::Admin,
            });
            config.users.push(br_daemon::config::UserConfig {
                username: "viewer".to_string(),
                password_hash: br_daemon::api::auth::hash_password("viewer123")?,
                role: UserRole::Viewer,
            });
        }

        let config = Arc::new(RwLock::new(config));
        let jwt_secret = options.jwt_secret.clone();

        // Create storage manager
        let storage_config = {
            let cfg = config.read();
            cfg.storage.clone()
        };
        let storage_manager = Arc::new(StorageManager::new(storage_config).await?);

        // Create channel manager
        let recordings_dir = {
            let cfg = config.read();
            cfg.storage.recordings_dir.clone()
        };
        let (channel_manager, _event_rx) =
            ChannelManager::new(recordings_dir, 60, storage_manager.clone(), config.clone());
        let channel_manager = Arc::new(channel_manager);

        // Create processing manager
        let (processing_manager, _processing_rx) =
            ProcessingManager::new(None, SegmentHandling::Keep, 1);
        let processing_manager = Arc::new(processing_manager);

        // Create event channel
        let (event_tx, _) = broadcast::channel(256);

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        // Create library manager
        let library_manager = {
            let cfg = config.read();
            Arc::new(tokio::sync::Mutex::new(
                br_daemon::libraries::LibraryManager::new(
                    cfg.libraries.clone(),
                    cfg.post_processing.ffmpeg_path.clone(),
                ),
            ))
        };

        // Create download manager
        let downloads_dir = temp_dir.path().join("downloads");
        let download_manager = Arc::new(
            DownloadManager::new(
                config.read().downloads.clone(),
                downloads_dir,
                library_manager.clone(),
            )
            .await?,
        );

        let pairing_manager = Arc::new(tokio::sync::RwLock::new(
            br_daemon::extension::pairing::PairingManager::new(temp_dir.path())
                .expect("Failed to create test pairing manager"),
        ));

        let state = Arc::new(AppState {
            config: config.clone(),
            config_path: temp_dir.path().join("config.toml"),
            jwt_secret: jwt_secret.clone(),
            local_only: options.local_only,
            channel_manager,
            processing_manager,
            storage_manager,
            download_manager,
            version_checker: Arc::new(br_daemon::version_check::VersionChecker::new(
                "0.1.0".to_string(),
                false,
            )),
            library_manager,
            event_tx,
            started_at: Instant::now(),
            session_store: Arc::new(br_daemon::api::users::SessionStore::new()),
            shutdown_tx,
            oauth_states: br_daemon::api::oauth::create_state_store(),
            pairing_manager,
            extension_connections: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            extension_message_senders: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            extension_shutdown_tx: None,
            extension_port: None,
        });

        let router = create_router(state.clone());

        Ok(Self {
            router,
            state,
            temp_dir,
            jwt_secret,
            _shutdown_rx: shutdown_rx,
        })
    }

    /// Make a request to the test server
    pub async fn request(&self, request: Request<Body>) -> Response {
        self.router
            .clone()
            .oneshot(request)
            .await
            .expect("Failed to execute request")
    }

    /// Make a GET request
    pub async fn get(&self, uri: &str) -> Response {
        self.request(
            Request::builder()
                .method(Method::GET)
                .uri(uri)
                .body(Body::empty())
                .expect("Failed to build request"),
        )
        .await
    }

    /// Make a GET request with authentication
    pub async fn get_auth(&self, uri: &str, token: &str) -> Response {
        self.request(
            Request::builder()
                .method(Method::GET)
                .uri(uri)
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .expect("Failed to build request"),
        )
        .await
    }

    /// Make a POST request with JSON body
    pub async fn post_json(&self, uri: &str, json: serde_json::Value) -> Response {
        self.request(
            Request::builder()
                .method(Method::POST)
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json.to_string()))
                .expect("Failed to build request"),
        )
        .await
    }

    /// Make a POST request with JSON body and authentication
    pub async fn post_json_auth(
        &self,
        uri: &str,
        json: serde_json::Value,
        token: &str,
    ) -> Response {
        self.request(
            Request::builder()
                .method(Method::POST)
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::from(json.to_string()))
                .expect("Failed to build request"),
        )
        .await
    }

    /// Make a PUT request with JSON body and authentication
    pub async fn put_json_auth(&self, uri: &str, json: serde_json::Value, token: &str) -> Response {
        self.request(
            Request::builder()
                .method(Method::PUT)
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::from(json.to_string()))
                .expect("Failed to build request"),
        )
        .await
    }

    /// Make a DELETE request with authentication
    pub async fn delete_auth(&self, uri: &str, token: &str) -> Response {
        self.request(
            Request::builder()
                .method(Method::DELETE)
                .uri(uri)
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .expect("Failed to build request"),
        )
        .await
    }

    /// Create a JWT token for testing
    pub fn create_token(&self, username: &str, role: UserRole) -> String {
        let (token, _) = br_daemon::api::auth::create_token(username, role, &self.jwt_secret, 24)
            .expect("Failed to create token");
        token
    }

    /// Create an admin token
    pub fn admin_token(&self) -> String {
        self.create_token("admin", UserRole::Admin)
    }

    /// Create a viewer token
    pub fn viewer_token(&self) -> String {
        self.create_token("viewer", UserRole::Viewer)
    }

    /// Get the recordings directory path
    pub fn recordings_dir(&self) -> PathBuf {
        self.temp_dir.path().join("recordings")
    }

    /// Get the library directory path
    pub fn library_dir(&self) -> PathBuf {
        self.temp_dir.path().join("library")
    }
}

/// Options for configuring the test server
#[derive(Clone)]
pub struct TestServerOptions {
    /// JWT secret for token generation
    pub jwt_secret: String,
    /// Whether to run in local-only mode (skips auth)
    pub local_only: bool,
}

impl Default for TestServerOptions {
    fn default() -> Self {
        Self {
            jwt_secret: "test-secret-key-for-testing".to_string(),
            local_only: false,
        }
    }
}

impl TestServerOptions {
    /// Create options for local-only mode (no auth required)
    pub fn local_only() -> Self {
        Self {
            jwt_secret: "test-secret-key-for-testing".to_string(),
            local_only: true,
        }
    }

    /// Set a custom JWT secret
    pub fn with_jwt_secret(mut self, secret: &str) -> Self {
        self.jwt_secret = secret.to_string();
        self
    }
}

/**
 * Response Assertion Helpers
 */

/// Extension trait for response assertions
pub trait ResponseExt {
    /// Assert the response status code
    fn assert_status(&self, expected: StatusCode);

    /// Assert the response is successful (2xx)
    fn assert_success(&self);

    /// Assert the response is a client error (4xx)
    fn assert_client_error(&self);

    /// Assert the response is unauthorized (401)
    fn assert_unauthorized(&self);

    /// Assert the response is forbidden (403)
    fn assert_forbidden(&self);

    /// Assert the response is not found (404)
    fn assert_not_found(&self);
}

impl ResponseExt for Response {
    fn assert_status(&self, expected: StatusCode) {
        assert_eq!(
            self.status(),
            expected,
            "Expected status {} but got {}",
            expected,
            self.status()
        );
    }

    fn assert_success(&self) {
        assert!(
            self.status().is_success(),
            "Expected success status but got {}",
            self.status()
        );
    }

    fn assert_client_error(&self) {
        assert!(
            self.status().is_client_error(),
            "Expected client error status but got {}",
            self.status()
        );
    }

    fn assert_unauthorized(&self) {
        self.assert_status(StatusCode::UNAUTHORIZED);
    }

    fn assert_forbidden(&self) {
        self.assert_status(StatusCode::FORBIDDEN);
    }

    fn assert_not_found(&self) {
        self.assert_status(StatusCode::NOT_FOUND);
    }
}

/// Parse response body as JSON
pub async fn json_body<T: serde::de::DeserializeOwned>(response: Response) -> T {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");
    serde_json::from_slice(&body).expect("Failed to parse JSON body")
}

/// Parse response body as raw string
pub async fn text_body(response: Response) -> String {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");
    String::from_utf8(body.to_vec()).expect("Response body is not valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_health_endpoint() {
        let server = TestServer::new().await.expect("Failed to create server");

        let response = server.get("/health").await;
        response.assert_success();

        let body: serde_json::Value = json_body(response).await;
        assert_eq!(body["status"], "ok");
    }

    #[tokio::test]
    async fn test_server_local_only_mode() {
        let server = TestServer::with_options(TestServerOptions::local_only())
            .await
            .expect("Failed to create server");

        // In local-only mode, we can access authenticated endpoints without a token
        // (the endpoint will create a local session)
        assert!(server.state.local_only);
    }

    #[tokio::test]
    async fn test_server_token_generation() {
        let server = TestServer::new().await.expect("Failed to create server");

        let admin_token = server.admin_token();
        let viewer_token = server.viewer_token();

        // Tokens should be different
        assert_ne!(admin_token, viewer_token);

        // Tokens should be valid JWTs
        assert!(admin_token.contains('.'));
        assert!(viewer_token.contains('.'));
    }
}
