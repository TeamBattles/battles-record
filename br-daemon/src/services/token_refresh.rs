//! Background service to proactively refresh platform OAuth tokens.
//!
//! All platforms use the teambattles.gg proxy for token refresh:
//! - Twitch: Uses proxy (client_secret stays server-side)
//! - YouTube: Uses proxy (client_secret stays server-side)
//! - Kick: Uses proxy (client_secret stays server-side)

use crate::api::oauth::{
    KICK_REFRESH_PROXY_URL, TWITCH_REFRESH_PROXY_URL, YOUTUBE_REFRESH_PROXY_URL,
};
use crate::api::AppState;
use crate::config::PlatformCredentials;
use crate::manager::ManagerEvent;
use crate::types::Platform;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::time;
use tracing::{debug, error, info, warn};

/**
 * Start the token refresh background service.
 * Checks every 5 minutes for tokens expiring within 10 minutes.
 */
pub fn start_token_refresh_service(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = time::interval(time::Duration::from_secs(5 * 60)); // 5 minutes

        loop {
            interval.tick().await;

            // Also clean up expired OAuth states
            crate::api::oauth::cleanup_expired_states(&state.oauth_states);

            // Check and refresh tokens
            check_and_refresh_tokens(&state).await;
        }
    });
}

async fn check_and_refresh_tokens(state: &Arc<AppState>) {
    let platforms = [Platform::Twitch, Platform::YouTube, Platform::Kick];

    for platform in platforms {
        if let Err(e) = check_and_refresh_platform(state, platform).await {
            warn!("Token refresh check failed for {}: {}", platform, e);
        }
    }
}

async fn check_and_refresh_platform(
    state: &Arc<AppState>,
    platform: Platform,
) -> Result<(), String> {
    // Get current credentials
    let credentials = {
        let config = state.config.read();
        match platform {
            Platform::Twitch => config.platform_auth.twitch.clone(),
            Platform::YouTube => config.platform_auth.youtube.clone(),
            Platform::Kick => config.platform_auth.kick.clone(),
        }
    };

    let credentials = match credentials {
        Some(c) => c,
        None => return Ok(()), // No credentials configured
    };

    // Check if token needs refresh (expires within 10 minutes)
    let needs_refresh = match credentials.expires_at {
        Some(expiry) => {
            let threshold = Utc::now() + Duration::minutes(10);
            expiry < threshold
        }
        None => false, // No expiry = doesn't need refresh
    };

    if !needs_refresh {
        return Ok(());
    }

    // Need refresh token to refresh
    let refresh_token = match &credentials.refresh_token {
        Some(t) => t.clone(),
        None => {
            // Token is expiring but no refresh token - emit expired event
            let _ = state.event_tx.send(ManagerEvent::PlatformAuthExpired {
                platform,
                reason: "Token expiring and no refresh token available".to_string(),
            });
            return Ok(());
        }
    };

    info!("Refreshing token for {}", platform);

    // Attempt refresh - all platforms use proxies (client_secret is server-side)
    let result = match platform {
        Platform::Twitch => {
            // Twitch uses the proxy (client_secret is server-side)
            refresh_twitch_token_via_proxy(&refresh_token).await
        }
        Platform::YouTube => {
            // YouTube uses the proxy (client_secret is server-side)
            refresh_youtube_token_via_proxy(&refresh_token).await
        }
        Platform::Kick => {
            // Kick uses the proxy (client_secret is server-side)
            refresh_kick_token_via_proxy(&refresh_token).await
        }
    };

    match result {
        Ok(new_creds) => {
            // Preserve existing username if new_creds doesn't have one
            let username = new_creds.username.or(credentials.username);
            let final_creds = PlatformCredentials {
                username: username.clone(),
                ..new_creds
            };

            // Update credentials
            {
                let mut config = state.config.write();
                match platform {
                    Platform::Twitch => config.platform_auth.twitch = Some(final_creds.clone()),
                    Platform::YouTube => config.platform_auth.youtube = Some(final_creds.clone()),
                    Platform::Kick => config.platform_auth.kick = Some(final_creds.clone()),
                }
            }

            // Persist to disk
            let config = state.config.read();
            if let Err(e) = config.save(&state.config_path) {
                error!("Failed to save config after token refresh: {}", e);
            }

            // Emit success event
            let _ = state.event_tx.send(ManagerEvent::PlatformAuthUpdated {
                platform,
                status: "connected".to_string(),
                username,
                expires_at: final_creds.expires_at,
            });

            debug!("Successfully refreshed token for {}", platform);
            Ok(())
        }
        Err(e) => {
            error!("Failed to refresh token for {}: {}", platform, e);

            // Emit expired event
            let _ = state.event_tx.send(ManagerEvent::PlatformAuthExpired {
                platform,
                reason: e.clone(),
            });

            Err(e)
        }
    }
}

/** Standard token response structure. */
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    #[allow(dead_code)]
    token_type: Option<String>,
    #[allow(dead_code)]
    scope: Option<String>,
}

/**
 * Refresh Twitch token via teambattles.gg proxy.
 * The proxy holds the client_secret server-side for security.
 */
async fn refresh_twitch_token_via_proxy(refresh_token: &str) -> Result<PlatformCredentials, String> {
    let client = reqwest::Client::new();

    #[derive(Serialize)]
    struct RefreshRequest<'a> {
        refresh_token: &'a str,
    }

    let request_body = RefreshRequest { refresh_token };

    let response = client
        .post(TWITCH_REFRESH_PROXY_URL)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Twitch refresh proxy request failed: {}", e))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Twitch token refresh via proxy failed: {}", error_text));
    }

    let token: TokenResponse = response.json().await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let expires_at = token.expires_in.map(|secs| {
        Utc::now() + Duration::seconds(secs as i64)
    });

    Ok(PlatformCredentials {
        access_token: token.access_token,
        // Twitch rotates refresh tokens - always use the new one
        refresh_token: token.refresh_token.or_else(|| Some(refresh_token.to_string())),
        expires_at,
        username: None,
        last_validated: Some(Utc::now()),
    })
}

/**
 * Refresh YouTube token via teambattles.gg proxy.
 * The proxy holds the client_secret server-side for security.
 */
async fn refresh_youtube_token_via_proxy(refresh_token: &str) -> Result<PlatformCredentials, String> {
    let client = reqwest::Client::new();

    #[derive(Serialize)]
    struct RefreshRequest<'a> {
        refresh_token: &'a str,
    }

    let request_body = RefreshRequest { refresh_token };

    let response = client
        .post(YOUTUBE_REFRESH_PROXY_URL)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("YouTube refresh proxy request failed: {}", e))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("YouTube token refresh via proxy failed: {}", error_text));
    }

    let token: TokenResponse = response.json().await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let expires_at = token.expires_in.map(|secs| {
        Utc::now() + Duration::seconds(secs as i64)
    });

    // Google doesn't return new refresh token on refresh
    Ok(PlatformCredentials {
        access_token: token.access_token,
        refresh_token: Some(refresh_token.to_string()),
        expires_at,
        username: None,
        last_validated: Some(Utc::now()),
    })
}

/**
 * Refresh Kick token via teambattles.gg proxy.
 * The proxy holds the client_secret server-side for security.
 */
async fn refresh_kick_token_via_proxy(refresh_token: &str) -> Result<PlatformCredentials, String> {
    let client = reqwest::Client::new();

    #[derive(Serialize)]
    struct RefreshRequest<'a> {
        refresh_token: &'a str,
    }

    let request_body = RefreshRequest { refresh_token };

    let response = client
        .post(KICK_REFRESH_PROXY_URL)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Kick refresh proxy request failed: {}", e))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Kick token refresh via proxy failed: {}", error_text));
    }

    let token: TokenResponse = response.json().await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let expires_at = token.expires_in.map(|secs| {
        Utc::now() + Duration::seconds(secs as i64)
    });

    Ok(PlatformCredentials {
        access_token: token.access_token,
        // Kick refresh tokens are reusable (as of Nov 2025), but may return a new one
        refresh_token: token.refresh_token.or_else(|| Some(refresh_token.to_string())),
        expires_at,
        username: None,
        last_validated: Some(Utc::now()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_display() {
        // Basic test to ensure platforms can be displayed
        assert_eq!(format!("{}", Platform::Twitch), "twitch");
        assert_eq!(format!("{}", Platform::YouTube), "youtube");
        assert_eq!(format!("{}", Platform::Kick), "kick");
    }
}
