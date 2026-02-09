use crate::api::auth::{
    create_token, verify_password, verify_token_detailed,
    AdminUser, AuthError, AuthErrorCode, AuthUser, Claims, LoginRequest, LoginResponse, TokenError,
};
use crate::api::config_api::{
    get_config, get_post_processing_config, update_config, update_post_processing_config,
};
use crate::api::images::{
    delete_banner_image, delete_profile_image, get_banner_image, get_channel_profile,
    get_profile_image, upload_banner_image, upload_profile_image,
};
use crate::api::platform_auth::{
    delete_platform_auth, get_platform_auth, list_platform_auth, set_platform_auth,
    set_youtube_cookies, test_platform_auth,
};
use crate::api::recordings::{
    cleanup_storage, delete_recording, get_recording, get_storage_stats, list_recordings,
    process_recording, reprocess_recording,
};
use crate::api::response::{ApiError, ApiResponse};
use crate::api::status::get_status;
use crate::api::users::{
    create_user, delete_user, get_user_sessions, list_users, revoke_all_user_sessions,
    revoke_user_session, update_user,
};
use crate::api::websocket::ws_handler;
use crate::api::AppState;
use crate::config::{ChannelConfig, FiltersConfig, ScheduleRule};
use crate::manager::ChannelUpdate;
use crate::platforms::traits::StreamPlatform;
use crate::platforms::twitch::TwitchPlatform;
use crate::platforms::{is_bun_available, is_ytdlp_available};
use crate::types::{Channel, Platform};

/**
 * Normalize channel name for a platform to ensure consistent duplicate detection.
 * For YouTube, ensures the name starts with '@' (unless it's a full URL).
 */
fn normalize_channel_name(name: &str, platform: Platform) -> String {
    match platform {
        Platform::YouTube => {
            // Don't modify full URLs
            if name.starts_with("http://") || name.starts_with("https://") {
                return name.to_string();
            }
            // Ensure YouTube handles start with @
            if name.starts_with('@') {
                name.to_string()
            } else {
                format!("@{}", name)
            }
        }
        // Other platforms: return as-is
        _ => name.to_string(),
    }
}
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Serialize)]
struct DependencyInfo {
    available: bool,
    version: Option<String>,
}

#[derive(Serialize)]
struct DependenciesResponse {
    bun: DependencyInfo,
    ytdlp: DependencyInfo,
}

async fn get_system_dependencies() -> Json<ApiResponse<DependenciesResponse>> {
    // Run both checks concurrently using spawn_blocking to avoid blocking the async runtime
    let bun_handle = tokio::task::spawn_blocking(|| get_dependency_version("bun", &["--version"]));
    let ytdlp_handle =
        tokio::task::spawn_blocking(|| get_dependency_version("yt-dlp", &["--version"]));

    let bun_version = bun_handle.await.ok().flatten();
    let ytdlp_version = ytdlp_handle.await.ok().flatten();

    Json(ApiResponse::new(DependenciesResponse {
        bun: DependencyInfo {
            available: bun_version.is_some(),
            version: bun_version,
        },
        ytdlp: DependencyInfo {
            available: ytdlp_version.is_some(),
            version: ytdlp_version,
        },
    }))
}

fn get_dependency_version(cmd: &str, args: &[&str]) -> Option<String> {
    std::process::Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

pub fn create_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/status", get(get_status))
        .route("/api/auth/login", post(login))
        .route("/api/auth/refresh", post(refresh_token))
        // WebSocket endpoint for real-time events
        .route("/api/events", get(ws_handler))
        // Channel CRUD endpoints
        .route("/api/channels", get(list_channels).post(add_channel))
        .route(
            "/api/channels/:id",
            get(get_channel).put(update_channel).delete(delete_channel),
        )
        .route("/api/channels/:id/check", post(check_channel))
        .route("/api/channels/:id/stop-recording", post(stop_recording))
        // Channel image endpoints
        .route("/api/channels/:id/profile", get(get_channel_profile))
        .route(
            "/api/channels/:id/images/profile",
            get(get_profile_image)
                .post(upload_profile_image)
                .delete(delete_profile_image),
        )
        .route(
            "/api/channels/:id/images/banner",
            get(get_banner_image)
                .post(upload_banner_image)
                .delete(delete_banner_image),
        )
        .route(
            "/api/channels/:id/images/fetch-platform",
            post(fetch_platform_images),
        )
        // Recording endpoints
        .route("/api/recordings", get(list_recordings))
        .route(
            "/api/recordings/:id",
            get(get_recording).delete(delete_recording),
        )
        .route("/api/recordings/:id/process", post(process_recording))
        .route("/api/recordings/:id/reprocess", post(reprocess_recording))
        // Storage endpoints
        .route("/api/storage/stats", get(get_storage_stats))
        .route("/api/storage/cleanup", post(cleanup_storage))
        // System endpoints
        .route("/api/system/dependencies", get(get_system_dependencies))
        // Config endpoints
        .route("/api/config", get(get_config).put(update_config))
        .route(
            "/api/config/post-processing",
            get(get_post_processing_config).put(update_post_processing_config),
        )
        // User endpoints (admin only)
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/:id", put(update_user).delete(delete_user))
        .route(
            "/api/users/:id/sessions",
            get(get_user_sessions).delete(revoke_all_user_sessions),
        )
        .route("/api/users/:user_id/sessions/:session_id", delete(revoke_user_session))
        // Platform auth endpoints (admin only)
        .route("/api/auth/platforms", get(list_platform_auth))
        .route(
            "/api/auth/platforms/:platform",
            get(get_platform_auth)
                .put(set_platform_auth)
                .delete(delete_platform_auth),
        )
        .route("/api/auth/platforms/:platform/test", post(test_platform_auth))
        // YouTube-specific cookie auth endpoint
        .route("/api/auth/platforms/youtube/cookies", post(set_youtube_cookies))
        // OAuth endpoints
        .route("/api/auth/platforms/oauth/availability", get(crate::api::oauth::get_oauth_availability))
        .route("/api/auth/platforms/:platform/oauth/start", post(crate::api::oauth::start_oauth))
        .route("/api/auth/platforms/:platform/oauth/callback", post(crate::api::oauth::oauth_callback))
        // Shutdown endpoint (local-only mode)
        .route("/api/shutdown", post(shutdown))
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/**
 * Trigger graceful shutdown of the daemon.
 * Only available in local-only mode.
 */
async fn shutdown(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<String>>, (StatusCode, ApiError)> {
    // Only allow shutdown in local-only mode for security
    if !state.local_only {
        return Err(ApiError::forbidden("Shutdown only available in local-only mode"));
    }

    tracing::info!("Shutdown requested via API");

    // Send shutdown signal
    if let Err(e) = state.shutdown_tx.send(()).await {
        tracing::error!("Failed to send shutdown signal: {}", e);
        return Err(ApiError::internal("Failed to initiate shutdown"));
    }

    Ok(Json(ApiResponse::new("Shutdown initiated".to_string())))
}

/** Sync channels from manager to config and save to disk. */
fn save_channels_to_config(state: &Arc<AppState>) {
    // Get channel configs directly from the manager
    let channel_configs = state.channel_manager.get_channel_configs();

    // Check if we have a separate channels file configured
    let channels_file = {
        let config = state.config.read();
        config.daemon.channels_file.clone()
    };

    if let Some(channels_path) = channels_file {
        // Save to separate channels file (doesn't get overwritten by docker-entrypoint.sh)
        if let Err(e) = crate::config::save_channels_file(&channels_path, &channel_configs) {
            tracing::error!("Failed to save channels to {:?}: {}", channels_path, e);
        } else {
            tracing::debug!("Saved {} channels to {:?}", channel_configs.len(), channels_path);
        }
    } else {
        // Legacy behavior: save to main config file
        {
            let mut config = state.config.write();
            config.channels = channel_configs;
        }

        let config = state.config.read();
        if let Err(e) = config.save(&state.config_path) {
            tracing::error!("Failed to save config: {}", e);
        } else {
            tracing::debug!("Saved config with {} channels", config.channels.len());
        }
    }
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, AuthError> {
    let config = state.config.read();

    // In local-only mode, skip password verification
    if state.local_only {
        let duration_hours = config.auth.session_duration / 3600;
        let duration_hours = if duration_hours == 0 { 24 } else { duration_hours };

        let (token, expiry) =
            create_token(&request.username, crate::types::UserRole::Admin, &state.jwt_secret, duration_hours)
                .map_err(|_| AuthError::new("Failed to create token"))?;

        return Ok(Json(ApiResponse::new(LoginResponse {
            token,
            role: crate::types::UserRole::Admin,
            expires_at: expiry.to_rfc3339(),
        })));
    }

    // Find user in config (also get index for session tracking)
    let (user_id, user) = config
        .users
        .iter()
        .enumerate()
        .find(|(_, u)| u.username == request.username)
        .ok_or_else(|| AuthError::with_code(
            "Invalid username or password",
            AuthErrorCode::Unauthorized,
        ))?;

    // Verify password
    if !verify_password(&request.password, &user.password_hash) {
        return Err(AuthError::with_code(
            "Invalid username or password",
            AuthErrorCode::Unauthorized,
        ));
    }

    // Calculate session duration in hours (config is in seconds)
    let duration_hours = config.auth.session_duration / 3600;
    let duration_hours = if duration_hours == 0 {
        24
    } else {
        duration_hours
    };

    // Create token
    let (token, expiry) =
        create_token(&user.username, user.role, &state.jwt_secret, duration_hours)
            .map_err(|_| AuthError::new("Failed to create token"))?;

    // Create session to track login (IP/user agent would come from request headers in real impl)
    state.session_store.create_session(user_id, None, None);

    Ok(Json(ApiResponse::new(LoginResponse {
        token,
        role: user.role,
        expires_at: expiry.to_rfc3339(),
    })))
}

/** Extract bearer token from Authorization header. */
fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
}

/** Issue a refreshed token for existing claims. */
fn issue_refreshed_token(
    state: &Arc<AppState>,
    duration_hours: u64,
    claims: &Claims,
) -> Result<Json<ApiResponse<LoginResponse>>, AuthError> {
    let (token, expiry) = create_token(&claims.sub, claims.role, &state.jwt_secret, duration_hours)
        .map_err(|_| AuthError::new("Failed to create token"))?;

    Ok(Json(ApiResponse::new(LoginResponse {
        token,
        role: claims.role,
        expires_at: expiry.to_rfc3339(),
    })))
}

/**
 * Refresh an existing JWT token.
 *
 * If the token is still valid, issues a new token with a fresh expiry.
 * If the token is expired but within the grace period, also issues a new token.
 * Otherwise returns an error.
 */
async fn refresh_token(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<LoginResponse>>, AuthError> {
    // In local-only mode, skip token refresh (tokens are infinite)
    if state.local_only {
        let (token, expiry) = create_token(
            "local",
            crate::types::UserRole::Admin,
            &state.jwt_secret,
            24 * 365, // 1 year for local mode
        )
        .map_err(|_| AuthError::new("Failed to create token"))?;

        return Ok(Json(ApiResponse::new(LoginResponse {
            token,
            role: crate::types::UserRole::Admin,
            expires_at: expiry.to_rfc3339(),
        })));
    }

    let token = extract_bearer_token(&headers)
        .ok_or_else(AuthError::token_missing)?;

    let config = state.config.read();
    let duration_hours = config.auth.session_duration / 3600;
    let duration_hours = if duration_hours == 0 { 24 } else { duration_hours };
    let grace_period_secs = config.auth.refresh_grace_period;
    drop(config);

    match verify_token_detailed(token, &state.jwt_secret) {
        Ok(claims) => {
            // Token still valid - issue new one
            issue_refreshed_token(&state, duration_hours, &claims)
        }
        Err(TokenError::Expired { claims }) => {
            // Check if within grace period
            let now = chrono::Utc::now().timestamp() as usize;
            let grace_end = claims.exp + grace_period_secs as usize;

            if now <= grace_end {
                // Within grace period - issue new token
                tracing::debug!(
                    "Refreshing expired token for {} (within grace period)",
                    claims.sub
                );
                issue_refreshed_token(&state, duration_hours, &claims)
            } else {
                // Beyond grace period
                tracing::debug!(
                    "Token refresh rejected for {} (beyond grace period)",
                    claims.sub
                );
                Err(AuthError::with_code(
                    "Token expired beyond refresh window",
                    AuthErrorCode::TokenExpired,
                ))
            }
        }
        Err(TokenError::Invalid(_)) | Err(TokenError::Malformed) => {
            Err(AuthError::token_invalid())
        }
    }
}

/** Request to add a new channel. */
#[derive(Debug, Deserialize)]
pub struct AddChannelRequest {
    pub name: String,
    pub platform: Platform,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_quality")]
    pub quality: String,
}

fn default_enabled() -> bool {
    true
}

fn default_quality() -> String {
    "best".to_string()
}

/** Response for channel list. */
#[derive(Serialize)]
pub struct ChannelsResponse {
    pub channels: Vec<Channel>,
}

/** Response for adding a channel. */
#[derive(Serialize)]
pub struct AddChannelResponse {
    pub id: Uuid,
    pub channel: Channel,
}

/** Response for checking a channel. */
#[derive(Serialize)]
pub struct CheckChannelResponse {
    pub channel: Channel,
    pub message: String,
}

/** Response for deleting a channel. */
#[derive(Serialize)]
pub struct DeleteResponse {
    pub deleted: bool,
}

/**
 * Helper module for deserializing Option<Option<T>> where:
 * - Missing field -> None (don't change)
 * - null -> Some(None) (clear the value)
 * - value -> Some(Some(value)) (set the value).
 */
mod double_option {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
    where
        T: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        // Deserialize as Option<T>, then wrap in Some()
        // If the field is missing, serde won't call this at all (we use #[serde(default)])
        // If the field is null, we get None here, which becomes Some(None)
        // If the field has a value, we get Some(v), which becomes Some(Some(v))
        Ok(Some(Option::deserialize(deserializer)?))
    }
}

/** Request to update a channel. */
#[derive(Debug, Deserialize)]
pub struct UpdateChannelRequest {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub quality: Option<String>,
    /** Storage quota in GB (pass null to clear). */
    #[serde(default, deserialize_with = "double_option::deserialize")]
    pub quota_gb: Option<Option<u32>>,
    /** Retention period in days (pass null to clear). */
    #[serde(default, deserialize_with = "double_option::deserialize")]
    pub retention_days: Option<Option<u32>>,
    // Schedule fields
    pub schedule_enabled: Option<bool>,
    pub timezone: Option<String>,
    pub schedule_rules: Option<Vec<UpdateScheduleRule>>,
    // Filter fields
    pub filters: Option<UpdateFiltersRequest>,
}

/** Schedule rule in API request format. */
#[derive(Debug, Deserialize)]
pub struct UpdateScheduleRule {
    pub days: Vec<u8>,        // 0-6, Sunday=0
    pub start_time: String,   // "HH:MM"
    pub end_time: String,     // "HH:MM"
}

/** Filters in API request format. */
#[derive(Debug, Deserialize)]
pub struct UpdateFiltersRequest {
    pub title_includes: Option<Vec<String>>,
    pub title_excludes: Option<Vec<String>>,
    pub game_includes: Option<Vec<String>>,
    pub game_excludes: Option<Vec<String>>,
    pub min_viewers: Option<u32>,
}

/** List all channels. */
async fn list_channels(
    _auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<ChannelsResponse>> {
    let channels = state.channel_manager.get_channels();
    Json(ApiResponse::new(ChannelsResponse { channels }))
}

/** Add a new channel (admin only). */
async fn add_channel(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Json(request): Json<AddChannelRequest>,
) -> Result<(StatusCode, Json<ApiResponse<AddChannelResponse>>), (StatusCode, ApiError)> {
    // Normalize channel name (e.g., YouTube handles get @ prefix)
    let normalized_name = normalize_channel_name(&request.name, request.platform);

    // Check for duplicate channel (same normalized name and platform)
    let existing_channels = state.channel_manager.get_channels();
    let duplicate = existing_channels.iter().any(|ch| {
        // Normalize existing channel names for comparison
        let existing_normalized = normalize_channel_name(&ch.name, ch.platform);
        existing_normalized.eq_ignore_ascii_case(&normalized_name) && ch.platform == request.platform
    });
    if duplicate {
        return Err(ApiError::bad_request(format!(
            "Channel '{}' already exists for {}",
            normalized_name, request.platform
        )));
    }

    // Check Bun and yt-dlp availability for YouTube channels
    if request.platform == Platform::YouTube {
        if !is_bun_available() {
            return Err(ApiError::bad_request(
                "YouTube requires Bun runtime. Install the Bun-bundled app or install Bun separately.".to_string()
            ));
        }
        if !is_ytdlp_available() {
            return Err(ApiError::bad_request(
                "YouTube requires yt-dlp. Install yt-dlp (pip install yt-dlp) or use the Docker image.".to_string()
            ));
        }
    }

    let config = ChannelConfig {
        name: normalized_name.clone(),
        platform: request.platform,
        enabled: request.enabled,
        quality: request.quality,
        schedule: None,
        filters: None,
        post_processing: None,
        quota_gb: None,
        retention_days: None,
        custom_profile_image: None,
        custom_banner_image: None,
        platform_profile_url: None,
        platform_banner_url: None,
    };

    let id = state.channel_manager.add_channel(config);

    let channel = state
        .channel_manager
        .get_channel(id)
        .ok_or_else(|| ApiError::internal("Failed to retrieve created channel"))?;

    // Persist to config file
    save_channels_to_config(&state);

    // Spawn immediate check in background (don't block the response)
    let channel_manager = state.channel_manager.clone();
    let channel_name = normalized_name.clone();
    tokio::spawn(async move {
        tracing::debug!("Triggering immediate check for newly added channel: {}", channel_name);
        if let Err(e) = channel_manager.check_channel(id).await {
            tracing::warn!("Initial check for {} failed: {}", channel_name, e);
        }
    });

    // Spawn background task to fetch profile images from platform API
    let channel_manager_for_profile = state.channel_manager.clone();
    let config_for_profile = state.config.clone();
    let config_path_for_profile = state.config_path.clone();
    let channel_name_for_profile = normalized_name;
    let platform_type = request.platform;
    tokio::spawn(async move {
        tracing::debug!("Fetching profile images for newly added channel: {}", channel_name_for_profile);

        // Create platform instance based on type
        // Note: Profile data is public, no auth needed. Using auth with the public Client-ID
        // can cause issues if the user's token was issued by a different OAuth app.
        let platform: Option<Box<dyn StreamPlatform + Send>> = match platform_type {
            Platform::Twitch => Some(Box::new(TwitchPlatform::new())),
            Platform::YouTube => {
                tracing::debug!("YouTube profile fetch not yet implemented");
                None
            }
            Platform::Kick => {
                tracing::debug!("Kick profile fetch not yet implemented");
                None
            }
        };

        if let Some(platform) = platform {
            match platform.get_channel_profile(&channel_name_for_profile).await {
                Ok(profile) => {
                    // Store the fetched URLs in the channel config
                    if channel_manager_for_profile.update_platform_images(
                        id,
                        profile.profile_image_url,
                        profile.banner_image_url,
                    ).is_some() {
                        // Persist to config file
                        let channel_configs = channel_manager_for_profile.get_channel_configs();
                        let channels_file = {
                            let config = config_for_profile.read();
                            config.daemon.channels_file.clone()
                        };

                        if let Some(channels_path) = channels_file {
                            if let Err(e) = crate::config::save_channels_file(&channels_path, &channel_configs) {
                                tracing::error!("Failed to save channels after profile fetch: {}", e);
                            }
                        } else {
                            let mut config = config_for_profile.write();
                            config.channels = channel_configs;
                            if let Err(e) = config.save(&config_path_for_profile) {
                                tracing::error!("Failed to save config after profile fetch: {}", e);
                            }
                        }
                        tracing::info!("Fetched and stored profile images for {}", channel_name_for_profile);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch profile for {}: {}", channel_name_for_profile, e);
                }
            }
        }
    });

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::new(AddChannelResponse { id, channel })),
    ))
}

/** Get a specific channel. */
async fn get_channel(
    _auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Channel>>, (StatusCode, ApiError)> {
    let channel = state
        .channel_manager
        .get_channel(id)
        .ok_or_else(|| ApiError::not_found("Channel"))?;

    Ok(Json(ApiResponse::new(channel)))
}

#[derive(Serialize)]
struct FetchPlatformImagesResponse {
    success: bool,
    profile_image_url: Option<String>,
    banner_image_url: Option<String>,
}

/** Fetch platform images for an existing channel (admin only). */
async fn fetch_platform_images(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<FetchPlatformImagesResponse>>, (StatusCode, ApiError)> {
    // Get channel info
    let channel = state
        .channel_manager
        .get_channel(id)
        .ok_or_else(|| ApiError::not_found("Channel"))?;

    // Create platform instance based on type
    // Note: Profile data is public, no auth needed. Using auth with the public Client-ID
    // can cause issues if the user's token was issued by a different OAuth app.
    let platform: Option<Box<dyn StreamPlatform + Send>> = match channel.platform {
        Platform::Twitch => Some(Box::new(TwitchPlatform::new())),
        Platform::YouTube => {
            return Err(ApiError::bad_request("YouTube profile fetch not yet implemented"));
        }
        Platform::Kick => {
            return Err(ApiError::bad_request("Kick profile fetch not yet implemented"));
        }
    };

    let platform = platform.ok_or_else(|| ApiError::internal("Failed to create platform adapter"))?;

    // Fetch profile from platform API
    let profile = platform
        .get_channel_profile(&channel.name)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to fetch profile: {}", e)))?;

    // Store the fetched URLs in the channel config
    state
        .channel_manager
        .update_platform_images(id, profile.profile_image_url.clone(), profile.banner_image_url.clone())
        .ok_or_else(|| ApiError::not_found("Channel"))?;

    // Persist to config file
    save_channels_to_config(&state);

    tracing::info!("Fetched platform images for channel {}", channel.name);

    Ok(Json(ApiResponse::new(FetchPlatformImagesResponse {
        success: true,
        profile_image_url: profile.profile_image_url,
        banner_image_url: profile.banner_image_url,
    })))
}

/** Convert day number (0-6, Sunday=0) to day name. */
fn day_number_to_name(day: u8) -> &'static str {
    match day {
        0 => "sunday",
        1 => "monday",
        2 => "tuesday",
        3 => "wednesday",
        4 => "thursday",
        5 => "friday",
        6 => "saturday",
        _ => "sunday",
    }
}

/** Update a channel (admin only). */
async fn update_channel(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateChannelRequest>,
) -> Result<Json<ApiResponse<Channel>>, (StatusCode, ApiError)> {
    // Convert schedule rules from API format to config format
    let schedule_rules = request.schedule_rules.map(|rules| {
        rules
            .into_iter()
            .map(|r| ScheduleRule {
                days: r.days.into_iter().map(|d| day_number_to_name(d).to_string()).collect(),
                start_time: Some(r.start_time),
                end_time: Some(r.end_time),
            })
            .collect()
    });

    // Convert filters from API format to config format
    // If all filter fields are empty, clear filters entirely (Some(None))
    // If request.filters is missing, don't change (None)
    let filters = request.filters.map(|f| {
        let title_contains = f.title_includes.unwrap_or_default();
        let title_excludes = f.title_excludes.unwrap_or_default();
        let game_contains = f.game_includes.unwrap_or_default();
        let game_excludes = f.game_excludes.unwrap_or_default();
        let min_viewers = f.min_viewers.filter(|&v| v > 0);

        // If all fields are empty, return None to clear filters
        if title_contains.is_empty()
            && title_excludes.is_empty()
            && game_contains.is_empty()
            && game_excludes.is_empty()
            && min_viewers.is_none()
        {
            None
        } else {
            Some(FiltersConfig {
                title_contains,
                title_excludes,
                game_contains,
                game_excludes,
                min_viewers,
            })
        }
    });

    let updates = ChannelUpdate {
        name: request.name,
        enabled: request.enabled,
        quality: request.quality,
        quota_gb: request.quota_gb,
        retention_days: request.retention_days,
        schedule_enabled: request.schedule_enabled,
        timezone: request.timezone,
        schedule_rules,
        filters,
    };

    // Check if we're enabling the channel
    let enabling = request.enabled == Some(true);

    let (channel, shutdown_tx) = state
        .channel_manager
        .update_channel(id, updates)
        .ok_or_else(|| ApiError::not_found("Channel"))?;

    // Stop recording if channel was disabled
    if let Some(tx) = shutdown_tx {
        let _ = tx.send(()).await;
    }

    // Persist to config file
    save_channels_to_config(&state);

    // If channel was enabled, trigger a check to start recording if live
    if enabling {
        let channel_manager = state.channel_manager.clone();
        let channel_name = channel.name.clone();
        tokio::spawn(async move {
            tracing::debug!("Triggering check for re-enabled channel: {}", channel_name);
            if let Err(e) = channel_manager.check_channel(id).await {
                tracing::warn!("Check for re-enabled channel {} failed: {}", channel_name, e);
            }
        });
    }

    Ok(Json(ApiResponse::new(channel)))
}

/** Delete a channel (admin only). */
async fn delete_channel(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<DeleteResponse>>, (StatusCode, ApiError)> {
    let (_, shutdown_tx) = state
        .channel_manager
        .remove_channel(id)
        .ok_or_else(|| ApiError::not_found("Channel"))?;

    // Persist to config file
    save_channels_to_config(&state);

    // If there was an active recording, signal it to stop
    if let Some(tx) = shutdown_tx {
        let _ = tx.send(()).await;
    }

    Ok(Json(ApiResponse::new(DeleteResponse { deleted: true })))
}

/** Check a channel and start recording if live. */
async fn check_channel(
    _auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<CheckChannelResponse>>, (StatusCode, ApiError)> {
    // First check if channel exists
    let _channel = state
        .channel_manager
        .get_channel(id)
        .ok_or_else(|| ApiError::not_found("Channel"))?;

    // Check the channel
    let status = state
        .channel_manager
        .check_channel(id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    // Get updated channel info
    let channel = state
        .channel_manager
        .get_channel(id)
        .ok_or_else(|| ApiError::not_found("Channel"))?;

    let message = match status {
        crate::types::ChannelStatus::Offline => "Channel is offline".to_string(),
        crate::types::ChannelStatus::Live => "Channel is live".to_string(),
        crate::types::ChannelStatus::Recording => "Channel is live, recording started".to_string(),
        crate::types::ChannelStatus::Error => "Error checking channel".to_string(),
    };

    Ok(Json(ApiResponse::new(CheckChannelResponse {
        channel,
        message,
    })))
}

/** Response for stopping a recording. */
#[derive(Serialize)]
pub struct StopRecordingResponse {
    pub channel: Channel,
    pub message: String,
}

/** Stop recording for a channel and pause it (admin only). */
async fn stop_recording(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<StopRecordingResponse>>, (StatusCode, ApiError)> {
    let channel = state
        .channel_manager
        .stop_recording(id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Channel"))?;

    // Persist to config file
    save_channels_to_config(&state);

    Ok(Json(ApiResponse::new(StopRecordingResponse {
        channel,
        message: "Recording stopped, channel paused".to_string(),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_channel_name_youtube_without_at() {
        let normalized = normalize_channel_name("BaianotTV1", Platform::YouTube);
        assert_eq!(normalized, "@BaianotTV1");
    }

    #[test]
    fn test_normalize_channel_name_youtube_with_at() {
        let normalized = normalize_channel_name("@BaianotTV1", Platform::YouTube);
        assert_eq!(normalized, "@BaianotTV1");
    }

    #[test]
    fn test_normalize_channel_name_youtube_full_url_preserved() {
        let url = "https://www.youtube.com/watch?v=abc123";
        let normalized = normalize_channel_name(url, Platform::YouTube);
        assert_eq!(normalized, url);
    }

    #[test]
    fn test_normalize_channel_name_twitch_unchanged() {
        let normalized = normalize_channel_name("shroud", Platform::Twitch);
        assert_eq!(normalized, "shroud");
    }

    #[test]
    fn test_normalize_channel_name_kick_unchanged() {
        let normalized = normalize_channel_name("xqc", Platform::Kick);
        assert_eq!(normalized, "xqc");
    }

    #[test]
    fn test_youtube_duplicate_detection_with_normalization() {
        // Both should normalize to the same value
        let name1 = normalize_channel_name("BaianotTV1", Platform::YouTube);
        let name2 = normalize_channel_name("@BaianotTV1", Platform::YouTube);
        assert_eq!(name1, name2);
    }
}
