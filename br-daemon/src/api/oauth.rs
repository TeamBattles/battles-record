//! OAuth flow management for platform authentication.
//! Handles state token generation, validation, and token exchange.
//!
//! Supports PKCE (Proof Key for Code Exchange) for public clients:
//! - Twitch: PKCE without client_secret (public client)
//! - YouTube: PKCE without client_secret (public client)
//! - Kick: PKCE with client_secret via teambattles.gg proxy

use crate::api::auth::AdminUser;
use crate::api::response::{ApiError, ApiResponse};
use crate::api::AppState;
use crate::types::Platform;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Duration, Utc};
use parking_lot::RwLock;
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use serde_json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;

/** Bundled Twitch OAuth client ID (public client, PKCE flow). */
pub const BUNDLED_TWITCH_CLIENT_ID: &str = "cjve9dao5o8oo9kaqf1qdwg2mgjumi";

/** Bundled YouTube OAuth client ID (public client, PKCE flow). */
pub const BUNDLED_YOUTUBE_CLIENT_ID: &str =
    "67527108478-u1bptuaouup3nqcoct59c3e8kboqas09.apps.googleusercontent.com";

/** Bundled Kick OAuth client ID (requires proxy for token exchange). */
pub const BUNDLED_KICK_CLIENT_ID: &str = "01KGJSCBH77HJS8HZ3VJ22DC1F";

/** Twitch OAuth token proxy endpoint (holds client_secret server-side). */
pub const TWITCH_TOKEN_PROXY_URL: &str = "https://teambattles.gg/api/v1/twitch/token";

/** Twitch OAuth refresh proxy endpoint. */
pub const TWITCH_REFRESH_PROXY_URL: &str = "https://teambattles.gg/api/v1/twitch/refresh";

/** Kick OAuth token proxy endpoint (holds client_secret server-side). */
pub const KICK_TOKEN_PROXY_URL: &str = "https://teambattles.gg/api/v1/kick/token";

/** Kick OAuth refresh proxy endpoint. */
pub const KICK_REFRESH_PROXY_URL: &str = "https://teambattles.gg/api/v1/kick/refresh";

/**
 * YouTube OAuth token proxy endpoint (holds client_secret server-side).
 * Google requires client_secret even with PKCE, so we use a proxy.
 */
pub const YOUTUBE_TOKEN_PROXY_URL: &str = "https://teambattles.gg/api/v1/youtube/token";

/** YouTube OAuth refresh proxy endpoint. */
pub const YOUTUBE_REFRESH_PROXY_URL: &str = "https://teambattles.gg/api/v1/youtube/refresh";

/**
 * In-memory storage for OAuth state tokens.
 * Key: state token, Value: OAuthStateEntry.
 */
pub type OAuthStateStore = Arc<RwLock<HashMap<String, OAuthStateEntry>>>;

#[derive(Debug, Clone)]
pub struct OAuthStateEntry {
    pub platform: Platform,
    pub created_at: DateTime<Utc>,
    pub redirect_uri: String,
    /** PKCE code verifier (used in token exchange). */
    pub code_verifier: String,
    /** Client ID used for this flow (bundled or custom). */
    pub client_id: String,
    /** Optional client secret (only for custom credentials flow). */
    pub client_secret: Option<String>,
}

/** Create a new OAuth state store. */
pub fn create_state_store() -> OAuthStateStore {
    Arc::new(RwLock::new(HashMap::new()))
}

/** Clean up expired state tokens (older than 10 minutes). */
pub fn cleanup_expired_states(store: &OAuthStateStore) {
    let mut states = store.write();
    let cutoff = Utc::now() - Duration::minutes(10);
    states.retain(|_, entry| entry.created_at > cutoff);
}

/** Generate a cryptographically secure state token. */
fn generate_state_token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

/** Generate a PKCE code verifier (43-128 characters, URL-safe). */
fn generate_code_verifier() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect()
}

/** Generate a PKCE code challenge from a code verifier using S256 method. */
fn generate_code_challenge(code_verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    URL_SAFE_NO_PAD.encode(hash)
}

/** Get the bundled client ID for a platform. */
fn get_bundled_client_id(platform: Platform) -> &'static str {
    match platform {
        Platform::Twitch => BUNDLED_TWITCH_CLIENT_ID,
        Platform::YouTube => BUNDLED_YOUTUBE_CLIENT_ID,
        Platform::Kick => BUNDLED_KICK_CLIENT_ID,
    }
}

/** Request to start OAuth flow. */
#[derive(Debug, Deserialize)]
pub struct StartOAuthRequest {
    /** Override redirect URI (optional, uses default if not provided). */
    pub redirect_uri: Option<String>,
    /** Custom client ID (optional, uses bundled if not provided). */
    pub client_id: Option<String>,
    /** Custom client secret (optional, only needed for custom credentials). */
    pub client_secret: Option<String>,
}

/** Response for starting OAuth flow. */
#[derive(Debug, Serialize)]
pub struct StartOAuthResponse {
    pub auth_url: String,
    pub state: String,
}

/** Request to complete OAuth flow. */
#[derive(Debug, Deserialize)]
pub struct OAuthCallbackRequest {
    pub code: String,
    pub state: String,
}

/** Response for completing OAuth flow. */
#[derive(Debug, Serialize)]
pub struct OAuthCallbackResponse {
    pub success: bool,
    pub platform: Platform,
    pub status: String,
    pub username: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

/** Response for OAuth availability check. */
#[derive(Debug, Serialize)]
pub struct OAuthAvailabilityResponse {
    pub twitch: bool,
    pub youtube: bool,
    pub kick: bool,
}

fn parse_platform(platform_str: &str) -> Option<Platform> {
    match platform_str.to_lowercase().as_str() {
        "twitch" => Some(Platform::Twitch),
        "youtube" => Some(Platform::YouTube),
        "kick" => Some(Platform::Kick),
        _ => None,
    }
}

/**
 * GET /api/auth/platforms/oauth/availability - Check which platforms have OAuth configured.
 * With bundled credentials, OAuth is always available for all platforms.
 */
pub async fn get_oauth_availability(
    _admin_user: AdminUser,
    State(_state): State<Arc<AppState>>,
) -> Json<ApiResponse<OAuthAvailabilityResponse>> {
    // OAuth is always available with bundled credentials
    Json(ApiResponse::new(OAuthAvailabilityResponse {
        twitch: true,
        youtube: true,
        kick: true,
    }))
}

/** POST /api/auth/platforms/:platform/oauth/start - Start OAuth flow. */
pub async fn start_oauth(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(platform_str): Path<String>,
    Json(request): Json<StartOAuthRequest>,
) -> Result<Json<ApiResponse<StartOAuthResponse>>, (StatusCode, ApiError)> {
    let platform = parse_platform(&platform_str)
        .ok_or_else(|| ApiError::bad_request(format!("Invalid platform: {}", platform_str)))?;

    // Determine client ID: use custom if provided, otherwise bundled
    let client_id = request
        .client_id
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| get_bundled_client_id(platform).to_string());

    // Client secret is only used for custom credentials flow
    let client_secret = request.client_secret.filter(|s| !s.is_empty());

    // Use provided redirect_uri or default
    let redirect_uri = request
        .redirect_uri
        .filter(|uri| !uri.is_empty())
        .unwrap_or_else(|| "battles-record://oauth/callback".to_string());

    // Generate PKCE code verifier and challenge
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);

    // Log PKCE values for debugging and verify they match at generation time
    tracing::info!(
        "PKCE generated: verifier_len={}, challenge={}, verifier_first8={}, verifier_last8={}",
        code_verifier.len(),
        code_challenge,
        &code_verifier[..8],
        &code_verifier[code_verifier.len() - 8..],
    );

    // Verify at generation time that challenge matches verifier
    let verify_challenge = generate_code_challenge(&code_verifier);
    if verify_challenge != code_challenge {
        tracing::error!(
            "PKCE MISMATCH at generation: {} != {}",
            verify_challenge,
            code_challenge
        );
    }

    // Generate state token for CSRF protection
    let state_token = generate_state_token();

    // Store state with PKCE verifier
    {
        let mut states = state.oauth_states.write();
        states.insert(
            state_token.clone(),
            OAuthStateEntry {
                platform,
                created_at: Utc::now(),
                redirect_uri: redirect_uri.clone(),
                code_verifier,
                client_id: client_id.clone(),
                client_secret,
            },
        );
    }

    // Build authorization URL with PKCE code challenge
    let auth_url = build_auth_url(
        platform,
        &client_id,
        &redirect_uri,
        &state_token,
        &code_challenge,
    )?;

    Ok(Json(ApiResponse::new(StartOAuthResponse {
        auth_url,
        state: state_token,
    })))
}

fn build_auth_url(
    platform: Platform,
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
) -> Result<String, (StatusCode, ApiError)> {
    match platform {
        Platform::Twitch => {
            // Twitch OAuth with PKCE
            let params = [
                ("client_id", client_id),
                ("redirect_uri", redirect_uri),
                ("response_type", "code"),
                ("scope", "user:read:subscriptions"),
                ("state", state),
                ("code_challenge", code_challenge),
                ("code_challenge_method", "S256"),
            ];
            let query = serde_urlencoded::to_string(&params)
                .map_err(|e| ApiError::internal(format!("Failed to build URL: {}", e)))?;
            Ok(format!("https://id.twitch.tv/oauth2/authorize?{}", query))
        }
        Platform::YouTube => {
            // Google OAuth with PKCE
            let params = [
                ("client_id", client_id),
                ("redirect_uri", redirect_uri),
                ("response_type", "code"),
                ("scope", "https://www.googleapis.com/auth/youtube.readonly https://www.googleapis.com/auth/userinfo.profile"),
                ("state", state),
                ("access_type", "offline"),
                ("prompt", "consent"),
                ("code_challenge", code_challenge),
                ("code_challenge_method", "S256"),
            ];
            let query = serde_urlencoded::to_string(&params)
                .map_err(|e| ApiError::internal(format!("Failed to build URL: {}", e)))?;
            Ok(format!(
                "https://accounts.google.com/o/oauth2/v2/auth?{}",
                query
            ))
        }
        Platform::Kick => {
            // Kick OAuth 2.1 with PKCE (mandatory)
            let params = [
                ("client_id", client_id),
                ("redirect_uri", redirect_uri),
                ("response_type", "code"),
                ("scope", "user:read channel:read"),
                ("state", state),
                ("code_challenge", code_challenge),
                ("code_challenge_method", "S256"),
            ];
            let query = serde_urlencoded::to_string(&params)
                .map_err(|e| ApiError::internal(format!("Failed to build URL: {}", e)))?;
            Ok(format!("https://id.kick.com/oauth/authorize?{}", query))
        }
    }
}

/** POST /api/auth/platforms/:platform/oauth/callback - Complete OAuth flow. */
pub async fn oauth_callback(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(platform_str): Path<String>,
    Json(request): Json<OAuthCallbackRequest>,
) -> Result<Json<ApiResponse<OAuthCallbackResponse>>, (StatusCode, ApiError)> {
    let platform = parse_platform(&platform_str)
        .ok_or_else(|| ApiError::bad_request(format!("Invalid platform: {}", platform_str)))?;

    // Validate and consume state token
    let state_entry = {
        let mut states = state.oauth_states.write();
        states.remove(&request.state)
    }
    .ok_or_else(|| ApiError::bad_request("Invalid or expired state token"))?;

    // Verify platform matches
    if state_entry.platform != platform {
        return Err(ApiError::bad_request("State token platform mismatch"));
    }

    // Check state isn't expired (10 minute limit)
    if Utc::now() - state_entry.created_at > Duration::minutes(10) {
        return Err(ApiError::bad_request("State token expired"));
    }

    // Log the retrieved PKCE values before token exchange for debugging
    tracing::info!(
        "Token exchange: platform={:?}, verifier_first8={}, verifier_last8={}, verifier_len={}, redirect_uri={}",
        platform,
        &state_entry.code_verifier[..8.min(state_entry.code_verifier.len())],
        &state_entry.code_verifier[state_entry.code_verifier.len().saturating_sub(8)..],
        state_entry.code_verifier.len(),
        &state_entry.redirect_uri
    );

    // Exchange code for tokens using PKCE code_verifier
    let token_response = exchange_code_for_tokens(
        platform,
        &request.code,
        &state_entry.client_id,
        state_entry.client_secret.as_deref(),
        &state_entry.redirect_uri,
        &state_entry.code_verifier,
    )
    .await?;

    // Fetch user info
    let user_info = fetch_user_info(
        platform,
        &token_response.access_token,
        &state_entry.client_id,
    )
    .await?;

    // Calculate expiry
    let expires_at = token_response
        .expires_in
        .map(|secs| Utc::now() + Duration::seconds(secs as i64));

    // Store credentials
    let credentials = crate::config::PlatformCredentials {
        access_token: token_response.access_token,
        refresh_token: token_response.refresh_token,
        expires_at,
        username: user_info.username.clone(),
        last_validated: Some(Utc::now()),
    };

    {
        let mut config = state.config.write();
        match platform {
            Platform::Twitch => config.platform_auth.twitch = Some(credentials),
            Platform::YouTube => config.platform_auth.youtube = Some(credentials),
            Platform::Kick => config.platform_auth.kick = Some(credentials),
        }
    }

    // Persist to disk
    let config = state.config.read();
    if let Err(e) = config.save(&state.config_path) {
        tracing::error!("Failed to save config after OAuth: {}", e);
        return Err(ApiError::internal(format!(
            "Failed to save credentials: {}",
            e
        )));
    }
    drop(config); // Release read lock

    // Emit WebSocket event to notify all connected clients
    let _ = state
        .event_tx
        .send(crate::manager::ManagerEvent::PlatformAuthUpdated {
            platform,
            status: "connected".to_string(),
            username: user_info.username.clone(),
            expires_at,
        });

    Ok(Json(ApiResponse::new(OAuthCallbackResponse {
        success: true,
        platform,
        status: "connected".to_string(),
        username: user_info.username,
        expires_at,
    })))
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    #[allow(dead_code)]
    token_type: Option<String>,
}

struct UserInfo {
    username: Option<String>,
}

async fn exchange_code_for_tokens(
    platform: Platform,
    code: &str,
    _client_id: &str,
    _client_secret: Option<&str>,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<TokenResponse, (StatusCode, ApiError)> {
    // Note: client_id and client_secret are unused because all platforms now use
    // the teambattles.gg proxy which holds the secrets server-side.
    // These parameters are kept for potential future custom credentials support.

    match platform {
        Platform::Twitch => {
            // Twitch requires client_secret even with PKCE, so we use a proxy
            // that holds the secret server-side
            exchange_twitch_tokens_via_proxy(code, code_verifier, redirect_uri).await
        }
        Platform::YouTube => {
            // Google requires client_secret even with PKCE, so we use a proxy
            // that holds the secret server-side
            exchange_youtube_tokens_via_proxy(code, code_verifier, redirect_uri).await
        }
        Platform::Kick => {
            // Kick requires client_secret even with PKCE, so we use a proxy
            // that holds the secret server-side
            exchange_kick_tokens_via_proxy(code, code_verifier, redirect_uri).await
        }
    }
}

/**
 * Exchange Twitch authorization code via teambattles.gg proxy.
 * Twitch requires client_secret even with PKCE, so we use a proxy that holds
 * the secret server-side for security.
 */
async fn exchange_twitch_tokens_via_proxy(
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, (StatusCode, ApiError)> {
    let client = reqwest::Client::new();

    #[derive(Serialize)]
    struct ProxyRequest<'a> {
        code: &'a str,
        code_verifier: &'a str,
        redirect_uri: &'a str,
    }

    let request_body = ProxyRequest {
        code,
        code_verifier,
        redirect_uri,
    };

    tracing::info!(
        "Twitch token exchange via proxy: redirect_uri={}, code_verifier_len={}",
        redirect_uri,
        code_verifier.len()
    );

    let response = client
        .post(TWITCH_TOKEN_PROXY_URL)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("Twitch token proxy request failed: {}", e)))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        tracing::error!("Twitch token exchange via proxy failed: {}", error_text);
        return Err(ApiError::bad_request(format!(
            "Token exchange failed: {}",
            error_text
        )));
    }

    response
        .json::<TokenResponse>()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to parse token response: {}", e)))
}

/**
 * Exchange Kick authorization code via teambattles.gg proxy.
 * The proxy holds the client_secret server-side for security.
 */
async fn exchange_kick_tokens_via_proxy(
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, (StatusCode, ApiError)> {
    let client = reqwest::Client::new();

    #[derive(Serialize)]
    struct ProxyRequest<'a> {
        code: &'a str,
        code_verifier: &'a str,
        redirect_uri: &'a str,
    }

    let request_body = ProxyRequest {
        code,
        code_verifier,
        redirect_uri,
    };

    let response = client
        .post(KICK_TOKEN_PROXY_URL)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("Kick token proxy request failed: {}", e)))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        tracing::error!("Kick token exchange via proxy failed: {}", error_text);
        return Err(ApiError::bad_request(format!(
            "Token exchange failed: {}",
            error_text
        )));
    }

    response
        .json::<TokenResponse>()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to parse token response: {}", e)))
}

/**
 * Exchange YouTube authorization code via teambattles.gg proxy.
 * Google requires client_secret even with PKCE, so we use a proxy that holds
 * the secret server-side for security.
 */
async fn exchange_youtube_tokens_via_proxy(
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, (StatusCode, ApiError)> {
    let client = reqwest::Client::new();

    #[derive(Serialize)]
    struct ProxyRequest<'a> {
        code: &'a str,
        code_verifier: &'a str,
        redirect_uri: &'a str,
    }

    let request_body = ProxyRequest {
        code,
        code_verifier,
        redirect_uri,
    };

    tracing::info!(
        "YouTube token exchange via proxy: redirect_uri={}, code_verifier_len={}",
        redirect_uri,
        code_verifier.len()
    );

    let response = client
        .post(YOUTUBE_TOKEN_PROXY_URL)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("YouTube token proxy request failed: {}", e)))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        tracing::error!("YouTube token exchange via proxy failed: {}", error_text);
        return Err(ApiError::bad_request(format!(
            "Token exchange failed: {}",
            error_text
        )));
    }

    response
        .json::<TokenResponse>()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to parse token response: {}", e)))
}

async fn fetch_user_info(
    platform: Platform,
    access_token: &str,
    client_id: &str,
) -> Result<UserInfo, (StatusCode, ApiError)> {
    let client = reqwest::Client::new();

    match platform {
        Platform::Twitch => {
            let response = client
                .get("https://api.twitch.tv/helix/users")
                .header("Authorization", format!("Bearer {}", access_token))
                .header("Client-Id", client_id)
                .send()
                .await
                .map_err(|e| ApiError::internal(format!("Failed to fetch user info: {}", e)))?;

            if !response.status().is_success() {
                // If we can't get user info, that's okay - we still have the token
                tracing::warn!("Failed to fetch Twitch user info");
                return Ok(UserInfo { username: None });
            }

            #[derive(Deserialize)]
            struct TwitchUsersResponse {
                data: Vec<TwitchUser>,
            }
            #[derive(Deserialize)]
            struct TwitchUser {
                login: String,
            }

            let users: TwitchUsersResponse = response
                .json()
                .await
                .map_err(|e| ApiError::internal(format!("Failed to parse user info: {}", e)))?;

            Ok(UserInfo {
                username: users.data.first().map(|u| u.login.clone()),
            })
        }
        Platform::YouTube => {
            let response = client
                .get("https://www.googleapis.com/oauth2/v2/userinfo")
                .header("Authorization", format!("Bearer {}", access_token))
                .send()
                .await
                .map_err(|e| ApiError::internal(format!("Failed to fetch user info: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                tracing::warn!("Failed to fetch YouTube user info: {} - {}", status, body);
                return Ok(UserInfo { username: None });
            }

            #[derive(Deserialize)]
            struct GoogleUserInfo {
                email: Option<String>,
                name: Option<String>,
            }

            let user: GoogleUserInfo = response
                .json()
                .await
                .map_err(|e| ApiError::internal(format!("Failed to parse user info: {}", e)))?;

            Ok(UserInfo {
                username: user.name.or(user.email),
            })
        }
        Platform::Kick => {
            // Kick Public API v1 - get authenticated user
            let response = client
                .get("https://api.kick.com/public/v1/users")
                .header("Authorization", format!("Bearer {}", access_token))
                .send()
                .await
                .map_err(|e| ApiError::internal(format!("Failed to fetch user info: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                tracing::warn!("Failed to fetch Kick user info: {} - {}", status, body);
                return Ok(UserInfo { username: None });
            }

            // Log raw response for debugging
            let body = response.text().await.unwrap_or_default();
            tracing::debug!("Kick user info response: {}", body);

            // Try to parse the response - Kick may return different structures
            #[derive(Deserialize)]
            struct KickUserResponse {
                data: Option<Vec<KickUser>>,
                // Direct fields if not wrapped
                #[serde(default)]
                username: Option<String>,
                #[serde(default)]
                name: Option<String>,
                #[serde(default)]
                _user_id: Option<u64>,
            }
            #[derive(Deserialize)]
            struct KickUser {
                username: Option<String>,
                name: Option<String>,
            }

            let parsed: Result<KickUserResponse, _> = serde_json::from_str(&body);
            match parsed {
                Ok(resp) => {
                    // Check if data is wrapped in array
                    if let Some(users) = resp.data {
                        if let Some(user) = users.first() {
                            return Ok(UserInfo {
                                username: user.username.clone().or(user.name.clone()),
                            });
                        }
                    }
                    // Otherwise try direct fields
                    Ok(UserInfo {
                        username: resp.username.or(resp.name),
                    })
                }
                Err(e) => {
                    tracing::warn!("Failed to parse Kick user info: {} - body: {}", e, body);
                    Ok(UserInfo { username: None })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_state_token() {
        let token1 = generate_state_token();
        let token2 = generate_state_token();

        // Tokens should be 32 characters
        assert_eq!(token1.len(), 32);
        assert_eq!(token2.len(), 32);

        // Tokens should be different
        assert_ne!(token1, token2);

        // Tokens should be alphanumeric
        assert!(token1.chars().all(|c| c.is_alphanumeric()));
        assert!(token2.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_generate_code_verifier() {
        let verifier1 = generate_code_verifier();
        let verifier2 = generate_code_verifier();

        // Verifiers should be 64 characters
        assert_eq!(verifier1.len(), 64);
        assert_eq!(verifier2.len(), 64);

        // Verifiers should be different
        assert_ne!(verifier1, verifier2);

        // Verifiers should be alphanumeric (URL-safe)
        assert!(verifier1.chars().all(|c| c.is_alphanumeric()));
        assert!(verifier2.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_generate_code_challenge() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = generate_code_challenge(verifier);

        // Challenge should be base64url-encoded SHA256 hash
        // The length of base64url-encoded 32-byte hash is 43 characters (no padding)
        assert_eq!(challenge.len(), 43);

        // Same verifier should produce same challenge
        let challenge2 = generate_code_challenge(verifier);
        assert_eq!(challenge, challenge2);

        // Different verifier should produce different challenge
        let challenge3 = generate_code_challenge("different_verifier");
        assert_ne!(challenge, challenge3);
    }

    /**
     * Test PKCE implementation against RFC 7636 Appendix B test vector.
     * This validates that our SHA256 + base64url encoding is correct.
     */
    #[test]
    fn test_pkce_rfc7636_test_vector() {
        // RFC 7636 Appendix B test vector:
        // code_verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
        // Expected code_challenge (S256) = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let expected_challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

        let challenge = generate_code_challenge(verifier);

        assert_eq!(
            challenge, expected_challenge,
            "PKCE challenge mismatch with RFC 7636 test vector!\nGot: {}\nExpected: {}\nThis indicates a bug in the SHA256 or base64url encoding.",
            challenge, expected_challenge
        );
    }

    /** Test that our generated verifiers produce valid challenges. */
    #[test]
    fn test_pkce_roundtrip() {
        // Generate a verifier and challenge
        let verifier = generate_code_verifier();
        let challenge1 = generate_code_challenge(&verifier);

        // Regenerating should produce the same challenge
        let challenge2 = generate_code_challenge(&verifier);
        assert_eq!(
            challenge1, challenge2,
            "Same verifier should always produce same challenge"
        );

        // Challenge should be valid base64url (43 chars, no padding)
        assert_eq!(challenge1.len(), 43);
        assert!(
            challenge1
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_'),
            "Challenge should only contain base64url characters"
        );
    }

    #[test]
    fn test_parse_platform() {
        assert_eq!(parse_platform("twitch"), Some(Platform::Twitch));
        assert_eq!(parse_platform("TWITCH"), Some(Platform::Twitch));
        assert_eq!(parse_platform("Twitch"), Some(Platform::Twitch));
        assert_eq!(parse_platform("youtube"), Some(Platform::YouTube));
        assert_eq!(parse_platform("kick"), Some(Platform::Kick));
        assert_eq!(parse_platform("invalid"), None);
        assert_eq!(parse_platform(""), None);
    }

    #[test]
    fn test_cleanup_expired_states() {
        let store = create_state_store();

        // Add a fresh state
        {
            let mut states = store.write();
            states.insert(
                "fresh".to_string(),
                OAuthStateEntry {
                    platform: Platform::Twitch,
                    created_at: Utc::now(),
                    redirect_uri: "test://callback".to_string(),
                    code_verifier: "test_verifier_fresh".to_string(),
                    client_id: BUNDLED_TWITCH_CLIENT_ID.to_string(),
                    client_secret: None,
                },
            );

            // Add an expired state (15 minutes ago)
            states.insert(
                "expired".to_string(),
                OAuthStateEntry {
                    platform: Platform::YouTube,
                    created_at: Utc::now() - Duration::minutes(15),
                    redirect_uri: "test://callback".to_string(),
                    code_verifier: "test_verifier_expired".to_string(),
                    client_id: BUNDLED_YOUTUBE_CLIENT_ID.to_string(),
                    client_secret: None,
                },
            );
        }

        // Run cleanup
        cleanup_expired_states(&store);

        // Check results
        let states = store.read();
        assert!(states.contains_key("fresh"));
        assert!(!states.contains_key("expired"));
    }

    #[test]
    fn test_build_auth_url_twitch() {
        let code_challenge = generate_code_challenge("test_verifier");
        let result = build_auth_url(
            Platform::Twitch,
            "test_client_id",
            "https://example.com/callback",
            "test_state",
            &code_challenge,
        );

        assert!(result.is_ok());
        let url = result.unwrap();
        assert!(url.starts_with("https://id.twitch.tv/oauth2/authorize?"));
        assert!(url.contains("client_id=test_client_id"));
        assert!(url.contains("state=test_state"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=user%3Aread%3Asubscriptions"));
        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn test_build_auth_url_youtube() {
        let code_challenge = generate_code_challenge("test_verifier");
        let result = build_auth_url(
            Platform::YouTube,
            "google_client_id",
            "https://example.com/callback",
            "test_state",
            &code_challenge,
        );

        assert!(result.is_ok());
        let url = result.unwrap();
        assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
        assert!(url.contains("client_id=google_client_id"));
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("prompt=consent"));
        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn test_build_auth_url_kick() {
        let code_challenge = generate_code_challenge("test_verifier");
        let result = build_auth_url(
            Platform::Kick,
            "kick_client_id",
            "https://example.com/callback",
            "test_state",
            &code_challenge,
        );

        // Kick OAuth should now succeed with PKCE
        assert!(result.is_ok());
        let url = result.unwrap();
        assert!(url.starts_with("https://id.kick.com/oauth/authorize?"));
        assert!(url.contains("client_id=kick_client_id"));
        assert!(url.contains("scope=user%3Aread+channel%3Aread"));
        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn test_bundled_client_ids() {
        // Verify bundled client IDs are set
        assert!(!BUNDLED_TWITCH_CLIENT_ID.is_empty());
        assert!(!BUNDLED_YOUTUBE_CLIENT_ID.is_empty());
        assert!(!BUNDLED_KICK_CLIENT_ID.is_empty());

        // Verify get_bundled_client_id returns correct values
        assert_eq!(
            get_bundled_client_id(Platform::Twitch),
            BUNDLED_TWITCH_CLIENT_ID
        );
        assert_eq!(
            get_bundled_client_id(Platform::YouTube),
            BUNDLED_YOUTUBE_CLIENT_ID
        );
        assert_eq!(
            get_bundled_client_id(Platform::Kick),
            BUNDLED_KICK_CLIENT_ID
        );
    }
}
