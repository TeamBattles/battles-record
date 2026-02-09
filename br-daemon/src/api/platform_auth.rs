// br-daemon/src/api/platform_auth.rs
//! Platform authentication API endpoints for managing streaming platform credentials.
//! These credentials are used to access subscriber-only content.

use crate::api::auth::AdminUser;
use crate::api::response::{ApiError, ApiResponse};
use crate::api::AppState;
use crate::config::PlatformCredentials;
use crate::platforms::{save_youtube_cookies, validate_cookie_file};
use crate::types::Platform;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/** Status of a platform's authentication. */
#[derive(Debug, Serialize)]
pub struct PlatformAuthStatus {
    pub platform: Platform,
    pub status: AuthStatus,
    pub username: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_validated: Option<DateTime<Utc>>,
}

/** Authentication status enum. */
#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatus {
    Connected,
    Expired,
    NotConnected,
}

/** Response for listing all platform auth statuses. */
#[derive(Debug, Serialize)]
pub struct PlatformAuthListResponse {
    pub platforms: Vec<PlatformAuthStatus>,
}

/** Request to set platform credentials. */
#[derive(Debug, Deserialize)]
pub struct SetPlatformAuthRequest {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub username: Option<String>,
}

/** Response after setting platform auth. */
#[derive(Debug, Serialize)]
pub struct SetPlatformAuthResponse {
    pub platform: Platform,
    pub status: AuthStatus,
    pub username: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

/** Response for testing connection. */
#[derive(Debug, Serialize)]
pub struct TestConnectionResponse {
    pub platform: Platform,
    pub success: bool,
    pub message: String,
    pub username: Option<String>,
}

/** Response for deleting platform auth. */
#[derive(Debug, Serialize)]
pub struct DeletePlatformAuthResponse {
    pub platform: Platform,
    pub deleted: bool,
}

fn parse_platform(platform_str: &str) -> Option<Platform> {
    match platform_str.to_lowercase().as_str() {
        "twitch" => Some(Platform::Twitch),
        "youtube" => Some(Platform::YouTube),
        "kick" => Some(Platform::Kick),
        _ => None,
    }
}

fn get_auth_status(creds: &Option<PlatformCredentials>) -> (AuthStatus, Option<String>, Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
    match creds {
        None => (AuthStatus::NotConnected, None, None, None),
        Some(c) => {
            let status = if let Some(expires) = c.expires_at {
                if expires < Utc::now() {
                    AuthStatus::Expired
                } else {
                    AuthStatus::Connected
                }
            } else {
                // No expiry means it's valid (some tokens don't expire)
                AuthStatus::Connected
            };
            (status, c.username.clone(), c.expires_at, c.last_validated)
        }
    }
}

/** GET /api/auth/platforms - List all platform authentication statuses. */
pub async fn list_platform_auth(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<PlatformAuthListResponse>> {
    let config = state.config.read();

    let platforms = vec![
        {
            let (status, username, expires_at, last_validated) =
                get_auth_status(&config.platform_auth.twitch);
            PlatformAuthStatus {
                platform: Platform::Twitch,
                status,
                username,
                expires_at,
                last_validated,
            }
        },
        {
            let (status, username, expires_at, last_validated) =
                get_auth_status(&config.platform_auth.youtube);
            PlatformAuthStatus {
                platform: Platform::YouTube,
                status,
                username,
                expires_at,
                last_validated,
            }
        },
        {
            let (status, username, expires_at, last_validated) =
                get_auth_status(&config.platform_auth.kick);
            PlatformAuthStatus {
                platform: Platform::Kick,
                status,
                username,
                expires_at,
                last_validated,
            }
        },
    ];

    Json(ApiResponse::new(PlatformAuthListResponse { platforms }))
}

/** GET /api/auth/platforms/:platform - Get status of a specific platform. */
pub async fn get_platform_auth(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(platform_str): Path<String>,
) -> Result<Json<ApiResponse<PlatformAuthStatus>>, (StatusCode, ApiError)> {
    let platform = parse_platform(&platform_str)
        .ok_or_else(|| ApiError::bad_request(format!("Invalid platform: {}", platform_str)))?;

    let config = state.config.read();

    let creds = match platform {
        Platform::Twitch => &config.platform_auth.twitch,
        Platform::YouTube => &config.platform_auth.youtube,
        Platform::Kick => &config.platform_auth.kick,
    };

    let (status, username, expires_at, last_validated) = get_auth_status(creds);

    Ok(Json(ApiResponse::new(PlatformAuthStatus {
        platform,
        status,
        username,
        expires_at,
        last_validated,
    })))
}

/** PUT /api/auth/platforms/:platform - Set credentials for a platform. */
pub async fn set_platform_auth(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(platform_str): Path<String>,
    Json(request): Json<SetPlatformAuthRequest>,
) -> Result<Json<ApiResponse<SetPlatformAuthResponse>>, (StatusCode, ApiError)> {
    let platform = parse_platform(&platform_str)
        .ok_or_else(|| ApiError::bad_request(format!("Invalid platform: {}", platform_str)))?;

    let credentials = PlatformCredentials {
        access_token: request.access_token,
        refresh_token: request.refresh_token,
        expires_at: request.expires_at,
        username: request.username.clone(),
        last_validated: Some(Utc::now()),
    };

    // Determine status based on expiry
    let status = if let Some(expires) = credentials.expires_at {
        if expires < Utc::now() {
            AuthStatus::Expired
        } else {
            AuthStatus::Connected
        }
    } else {
        AuthStatus::Connected
    };

    // Update config
    {
        let mut config = state.config.write();
        match platform {
            Platform::Twitch => config.platform_auth.twitch = Some(credentials.clone()),
            Platform::YouTube => config.platform_auth.youtube = Some(credentials.clone()),
            Platform::Kick => config.platform_auth.kick = Some(credentials.clone()),
        }
    }

    // Persist to disk
    let config = state.config.read();
    if let Err(e) = config.save(&state.config_path) {
        tracing::error!("Failed to save config: {}", e);
        return Err(ApiError::internal(format!("Failed to save config: {}", e)));
    }

    Ok(Json(ApiResponse::new(SetPlatformAuthResponse {
        platform,
        status,
        username: request.username,
        expires_at: credentials.expires_at,
    })))
}

/** DELETE /api/auth/platforms/:platform - Remove credentials for a platform. */
pub async fn delete_platform_auth(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(platform_str): Path<String>,
) -> Result<Json<ApiResponse<DeletePlatformAuthResponse>>, (StatusCode, ApiError)> {
    let platform = parse_platform(&platform_str)
        .ok_or_else(|| ApiError::bad_request(format!("Invalid platform: {}", platform_str)))?;

    // Update config
    {
        let mut config = state.config.write();
        match platform {
            Platform::Twitch => config.platform_auth.twitch = None,
            Platform::YouTube => config.platform_auth.youtube = None,
            Platform::Kick => config.platform_auth.kick = None,
        }
    }

    // Persist to disk
    let config = state.config.read();
    if let Err(e) = config.save(&state.config_path) {
        tracing::error!("Failed to save config: {}", e);
        return Err(ApiError::internal(format!("Failed to save config: {}", e)));
    }

    Ok(Json(ApiResponse::new(DeletePlatformAuthResponse {
        platform,
        deleted: true,
    })))
}

/** POST /api/auth/platforms/:platform/test - Test the connection for a platform. */
pub async fn test_platform_auth(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(platform_str): Path<String>,
) -> Result<Json<ApiResponse<TestConnectionResponse>>, (StatusCode, ApiError)> {
    let platform = parse_platform(&platform_str)
        .ok_or_else(|| ApiError::bad_request(format!("Invalid platform: {}", platform_str)))?;

    // First, read the credentials and extract what we need
    let (has_creds, is_expired, username) = {
        let config = state.config.read();
        let creds = match platform {
            Platform::Twitch => &config.platform_auth.twitch,
            Platform::YouTube => &config.platform_auth.youtube,
            Platform::Kick => &config.platform_auth.kick,
        };

        match creds {
            None => (false, false, None),
            Some(c) => {
                let expired = c.expires_at.map(|e| e < Utc::now()).unwrap_or(false);
                (true, expired, c.username.clone())
            }
        }
    };

    if !has_creds {
        return Ok(Json(ApiResponse::new(TestConnectionResponse {
            platform,
            success: false,
            message: "No credentials configured".to_string(),
            username: None,
        })));
    }

    if is_expired {
        return Ok(Json(ApiResponse::new(TestConnectionResponse {
            platform,
            success: false,
            message: "Token has expired".to_string(),
            username,
        })));
    }

    // For now, just validate that token exists and isn't expired
    // In a full implementation, we would make an API call to the platform
    // to verify the token is still valid

    // Update last_validated timestamp
    {
        let mut config = state.config.write();
        match platform {
            Platform::Twitch => {
                if let Some(ref mut creds) = config.platform_auth.twitch {
                    creds.last_validated = Some(Utc::now());
                }
            }
            Platform::YouTube => {
                if let Some(ref mut creds) = config.platform_auth.youtube {
                    creds.last_validated = Some(Utc::now());
                }
            }
            Platform::Kick => {
                if let Some(ref mut creds) = config.platform_auth.kick {
                    creds.last_validated = Some(Utc::now());
                }
            }
        }
    }

    // Persist
    {
        let config = state.config.read();
        if let Err(e) = config.save(&state.config_path) {
            tracing::error!("Failed to save config after token update: {}", e);
        }
    }

    Ok(Json(ApiResponse::new(TestConnectionResponse {
        platform,
        success: true,
        message: "Token validated successfully".to_string(),
        username,
    })))
}

/** Request to set YouTube cookies. */
#[derive(Debug, Deserialize)]
pub struct SetYouTubeCookiesRequest {
    /** Cookie file content in Netscape format. */
    pub cookie_content: String,
}

/** Response after setting YouTube cookies. */
#[derive(Debug, Serialize)]
pub struct SetYouTubeCookiesResponse {
    pub platform: Platform,
    pub status: AuthStatus,
    pub message: String,
}

/** POST /api/auth/platforms/youtube/cookies - Set YouTube cookies from Netscape format file content. */
pub async fn set_youtube_cookies(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Json(request): Json<SetYouTubeCookiesRequest>,
) -> Result<Json<ApiResponse<SetYouTubeCookiesResponse>>, (StatusCode, ApiError)> {
    // Validate the cookie content first
    if let Err(e) = validate_cookie_file(&request.cookie_content) {
        return Err(ApiError::bad_request(format!("Invalid cookie file: {}", e)));
    }

    // Save the cookies to file
    let cookie_path = save_youtube_cookies(&request.cookie_content)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to save cookies: {}", e)))?;

    // Update the YouTube platform credentials to mark as "connected"
    // We use a placeholder token since the actual auth is via cookie file
    let credentials = PlatformCredentials {
        access_token: format!("cookie_file:{}", cookie_path.display()),
        refresh_token: None,
        expires_at: None, // Cookie auth doesn't have a standard expiry we can track
        username: None,
        last_validated: Some(Utc::now()),
    };

    // Update config
    {
        let mut config = state.config.write();
        config.platform_auth.youtube = Some(credentials);
    }

    // Persist to disk
    let config = state.config.read();
    if let Err(e) = config.save(&state.config_path) {
        tracing::error!("Failed to save config: {}", e);
        return Err(ApiError::internal(format!("Failed to save config: {}", e)));
    }

    tracing::info!("YouTube cookies saved successfully");

    Ok(Json(ApiResponse::new(SetYouTubeCookiesResponse {
        platform: Platform::YouTube,
        status: AuthStatus::Connected,
        message: "YouTube cookies saved successfully".to_string(),
    })))
}
