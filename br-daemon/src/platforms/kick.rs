//! Kick platform implementation
//!
//! Implements the StreamPlatform trait for Kick.com streams.
//! Supports both public and subscriber-only streams via Bearer token authentication.
//!
//! Uses curl subprocess for HTTP requests to bypass Cloudflare's TLS fingerprinting
//! (which blocks standard reqwest requests). Curl uses system TLS which passes
//! Cloudflare's bot detection.

use super::{ChannelProfile, PlatformError, PlatformResult, StreamPlatform, StreamUrl};
use crate::recording::{find_variant_for_quality, parse_master_playlist};
use crate::types::{Platform, Quality, StreamInfo};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use std::process::Stdio;
use tokio::process::Command;

const KICK_API_BASE: &str = "https://kick.com/api";
const CURL_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

/** Kick channel API response. */
#[derive(Debug, Deserialize)]
pub struct KickChannelResponse {
    pub id: u64,
    pub slug: String,
    pub user: KickUser,
    pub livestream: Option<KickLivestream>,
    pub playback_url: Option<String>,
    pub banner_image: Option<KickImage>,
}

/** User info from Kick API. */
#[derive(Debug, Deserialize)]
pub struct KickUser {
    pub username: String,
    pub profile_pic: Option<String>,
    pub bio: Option<String>,
}

/** Livestream info from Kick API. */
#[derive(Debug, Deserialize)]
pub struct KickLivestream {
    pub id: u64,
    pub is_live: bool,
    pub session_title: String,
    pub viewer_count: u32,
    pub created_at: String,
    pub thumbnail: Option<KickImage>,
    #[serde(default)]
    pub categories: Vec<KickCategory>,
}

/** Category/game info. */
#[derive(Debug, Deserialize)]
pub struct KickCategory {
    pub name: String,
}

/** Image URL wrapper. */
#[derive(Debug, Deserialize)]
pub struct KickImage {
    pub url: String,
}

/**
 * Kick platform implementation.
 *
 * Uses curl subprocess for API requests to bypass Cloudflare TLS fingerprinting.
 * reqwest is kept for fetching m3u8 playlists (which don't have Cloudflare protection).
 */
pub struct KickPlatform {
    client: Client,
    auth_token: Option<String>,
}

impl KickPlatform {
    /** Create the HTTP client for playlist fetching (doesn't need curl). */
    fn create_client() -> Client {
        Client::builder()
            .user_agent(CURL_USER_AGENT)
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new())
    }

    /** Create a new KickPlatform instance without authentication. */
    pub fn new() -> Self {
        Self {
            client: Self::create_client(),
            auth_token: None,
        }
    }

    /**
     * Create a KickPlatform with authentication token.
     * Token should be a Bearer token extracted from browser dev tools.
     */
    pub fn with_auth(auth_token: String) -> Self {
        Self {
            client: Self::create_client(),
            auth_token: Some(auth_token),
        }
    }

    /** Fetch JSON from Kick API using curl to bypass Cloudflare TLS fingerprinting. */
    async fn curl_get(&self, url: &str) -> PlatformResult<String> {
        let mut cmd = Command::new("curl");
        cmd.arg("-s") // Silent mode
            .arg("-A").arg(CURL_USER_AGENT)
            .arg("-H").arg("Accept: application/json, text/plain, */*")
            .arg("-H").arg("Accept-Language: en-US,en;q=0.9")
            .arg("-H").arg("Referer: https://kick.com/")
            .arg("-H").arg("Origin: https://kick.com")
            .arg("-w").arg("\n%{http_code}") // Append HTTP status code
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Add auth header if present
        if let Some(ref token) = self.auth_token {
            cmd.arg("-H").arg(format!("Authorization: Bearer {}", token));
        }

        cmd.arg(url);

        let output = cmd.output().await.map_err(|e| {
            PlatformError::Api(format!("Failed to execute curl: {}", e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stdout_str = stdout.trim();

        // The output format is: {json}\n{http_code}
        // Find the last newline to split body from status code
        let (body, status_str) = match stdout_str.rfind('\n') {
            Some(pos) => (&stdout_str[..pos], &stdout_str[pos + 1..]),
            None => {
                return Err(PlatformError::Api(format!(
                    "Unexpected curl output: {}",
                    stdout
                )));
            }
        };

        let status_code: u16 = status_str.parse().unwrap_or(0);

        if status_code == 404 {
            // Extract channel name from URL for error message
            let channel = url.rsplit('/').next().unwrap_or("unknown");
            return Err(PlatformError::ChannelNotFound(channel.to_string()));
        }

        if status_code < 200 || status_code >= 300 {
            return Err(PlatformError::Api(format!(
                "Kick API returned HTTP {}",
                status_code
            )));
        }

        Ok(body.to_string())
    }

    /** Fetch channel data from Kick API. */
    async fn fetch_channel(&self, channel: &str) -> PlatformResult<KickChannelResponse> {
        let url = format!("{}/v2/channels/{}", KICK_API_BASE, channel.to_lowercase());
        let body = self.curl_get(&url).await?;
        serde_json::from_str(&body).map_err(|e| {
            PlatformError::Api(format!("Failed to parse Kick response: {}", e))
        })
    }
}

impl Default for KickPlatform {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StreamPlatform for KickPlatform {
    fn platform(&self) -> Platform {
        Platform::Kick
    }

    async fn check_live(&self, channel: &str) -> PlatformResult<Option<StreamInfo>> {
        let data = self.fetch_channel(channel).await?;

        // livestream is null when offline
        let Some(livestream) = data.livestream else {
            return Ok(None);
        };

        if !livestream.is_live {
            return Ok(None);
        }

        let started_at = DateTime::parse_from_rfc3339(&livestream.created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(Some(StreamInfo {
            title: livestream.session_title,
            game: livestream.categories.first().map(|c| c.name.clone()),
            viewer_count: livestream.viewer_count,
            started_at,
            thumbnail_url: livestream.thumbnail.map(|t| t.url),
        }))
    }

    async fn get_qualities(&self, channel: &str) -> PlatformResult<Vec<Quality>> {
        let data = self.fetch_channel(channel).await?;
        let playback_url = data.playback_url.ok_or(PlatformError::StreamOffline)?;

        // Fetch playlist using reqwest (m3u8 URLs don't have Cloudflare)
        let playlist_text = self
            .client
            .get(&playback_url)
            .send()
            .await?
            .text()
            .await?;

        let mut qualities = vec![Quality::source()];

        for line in playlist_text.lines() {
            if line.starts_with("#EXT-X-STREAM-INF:") {
                let mut name = "unknown".to_string();
                let mut resolution = None;
                let mut bandwidth = None;

                for part in line.split(',') {
                    if let Some(res) = part.strip_prefix("RESOLUTION=") {
                        resolution = Some(res.to_string());
                        if let Some(height) = res.split('x').nth(1) {
                            name = format!("{}p", height);
                        }
                    }
                    if let Some(bw) = part.strip_prefix("BANDWIDTH=") {
                        bandwidth = bw.parse().ok();
                    }
                }

                if name != "unknown" {
                    qualities.push(Quality {
                        name,
                        resolution,
                        bandwidth,
                    });
                }
            }
        }

        Ok(qualities)
    }

    async fn get_stream_url(&self, channel: &str, quality: &Quality) -> PlatformResult<StreamUrl> {
        let data = self.fetch_channel(channel).await?;

        let playback_url = data.playback_url.ok_or(PlatformError::StreamOffline)?;

        // Fetch master playlist to find quality variant (m3u8 URLs don't have Cloudflare)
        let master_content = self
            .client
            .get(&playback_url)
            .send()
            .await?
            .text()
            .await?;

        let variants = parse_master_playlist(&master_content, &playback_url)
            .map_err(|e| PlatformError::Api(format!("Failed to parse playlist: {}", e)))?;

        let variant = find_variant_for_quality(&variants, &quality.name)
            .ok_or_else(|| PlatformError::Api("No suitable quality found".to_string()))?;

        Ok(StreamUrl {
            url: variant.uri.clone(),
            quality: quality.clone(),
        })
    }

    async fn get_channel_profile(&self, channel: &str) -> PlatformResult<ChannelProfile> {
        let data = self.fetch_channel(channel).await?;

        Ok(ChannelProfile {
            display_name: data.user.username,
            description: data.user.bio,
            profile_image_url: data.user.profile_pic,
            banner_image_url: data.banner_image.map(|b| b.url),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_channel_response_live() {
        let json = r#"{
            "id": 123,
            "slug": "testchannel",
            "user": {
                "username": "TestChannel",
                "profile_pic": "https://example.com/pic.jpg",
                "bio": "Test bio"
            },
            "livestream": {
                "id": 456,
                "is_live": true,
                "session_title": "Test Stream",
                "viewer_count": 100,
                "created_at": "2024-01-01T00:00:00.000000Z",
                "thumbnail": { "url": "https://example.com/thumb.jpg" },
                "categories": [{ "name": "Just Chatting" }]
            },
            "playback_url": "https://example.com/playlist.m3u8",
            "banner_image": { "url": "https://example.com/banner.jpg" }
        }"#;

        let data: KickChannelResponse = serde_json::from_str(json).unwrap();

        assert_eq!(data.id, 123);
        assert_eq!(data.slug, "testchannel");
        assert_eq!(data.user.username, "TestChannel");
        assert!(data.livestream.is_some());

        let livestream = data.livestream.unwrap();
        assert!(livestream.is_live);
        assert_eq!(livestream.session_title, "Test Stream");
        assert_eq!(livestream.viewer_count, 100);
        assert_eq!(livestream.categories.len(), 1);
        assert_eq!(livestream.categories[0].name, "Just Chatting");
    }

    #[test]
    fn test_parse_channel_response_offline() {
        let json = r#"{
            "id": 123,
            "slug": "testchannel",
            "user": {
                "username": "TestChannel",
                "profile_pic": null,
                "bio": null
            },
            "livestream": null,
            "playback_url": null,
            "banner_image": null
        }"#;

        let data: KickChannelResponse = serde_json::from_str(json).unwrap();

        assert!(data.livestream.is_none());
        assert!(data.playback_url.is_none());
        assert!(data.user.profile_pic.is_none());
    }

    #[test]
    fn test_parse_channel_response_live_no_categories() {
        let json = r#"{
            "id": 123,
            "slug": "testchannel",
            "user": {
                "username": "TestChannel",
                "profile_pic": null,
                "bio": null
            },
            "livestream": {
                "id": 456,
                "is_live": true,
                "session_title": "Test Stream",
                "viewer_count": 50,
                "created_at": "2024-01-01T00:00:00.000000Z",
                "thumbnail": null
            },
            "playback_url": "https://example.com/playlist.m3u8",
            "banner_image": null
        }"#;

        let data: KickChannelResponse = serde_json::from_str(json).unwrap();

        let livestream = data.livestream.unwrap();
        assert!(livestream.categories.is_empty());
        assert!(livestream.thumbnail.is_none());
    }

    #[test]
    fn test_kick_platform_default() {
        let platform = KickPlatform::default();
        assert_eq!(platform.platform(), Platform::Kick);
    }

    #[test]
    fn test_kick_platform_new() {
        let platform = KickPlatform::new();
        assert_eq!(platform.platform(), Platform::Kick);
        assert!(platform.auth_token.is_none());
    }

    #[test]
    fn test_kick_platform_with_auth() {
        let platform = KickPlatform::with_auth("test_token".to_string());
        assert_eq!(platform.platform(), Platform::Kick);
        assert!(platform.auth_token.is_some());
        assert_eq!(platform.auth_token.as_ref().unwrap(), "test_token");
    }

    #[test]
    fn test_kick_platform_default_no_auth() {
        let platform = KickPlatform::default();
        assert!(platform.auth_token.is_none());
    }
}
