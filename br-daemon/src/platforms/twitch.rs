use super::{ChannelProfile, PlatformError, PlatformResult, StreamPlatform, StreamUrl};
use crate::recording::{find_variant_for_quality, parse_master_playlist};
use crate::types::{Platform, Quality, StreamInfo};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/**
 * Public client ID for Twitch GQL API - works for public data without OAuth.
 * Note: The GQL API is undocumented/internal and doesn't accept third-party OAuth tokens.
 * It only works with this public client ID for accessing public stream data.
 */
const TWITCH_CLIENT_ID: &str = "kimne78kx3ncx6brgo4mv6wki5h1ko";
const TWITCH_GQL_URL: &str = "https://gql.twitch.tv/gql";

pub struct TwitchPlatform {
    client: Client,
}

impl TwitchPlatform {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    async fn gql_request<T: for<'de> Deserialize<'de>>(
        &self,
        body: &impl Serialize,
    ) -> PlatformResult<T> {
        let req = self
            .client
            .post(TWITCH_GQL_URL)
            .header("Client-ID", TWITCH_CLIENT_ID)
            .json(body);

        let resp = req.send().await?;

        if !resp.status().is_success() {
            return Err(PlatformError::Api(format!(
                "Twitch API returned {}",
                resp.status()
            )));
        }

        let data: T = resp.json().await?;
        Ok(data)
    }
}

impl Default for TwitchPlatform {
    fn default() -> Self {
        Self::new()
    }
}

// GraphQL request/response types
#[derive(Serialize)]
struct GqlRequest {
    query: String,
    variables: serde_json::Value,
}

#[derive(Deserialize)]
struct StreamMetadataResponse {
    data: StreamMetadataData,
}

#[derive(Deserialize)]
struct StreamMetadataData {
    user: Option<UserData>,
}

#[derive(Deserialize)]
struct UserData {
    stream: Option<StreamData>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StreamData {
    title: String,
    game: Option<GameData>,
    viewers_count: u32,
    created_at: String,
    preview_image_url: Option<String>,
}

#[derive(Deserialize)]
struct GameData {
    name: String,
}

#[derive(Deserialize)]
struct PlaybackAccessTokenResponse {
    data: PlaybackAccessTokenData,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaybackAccessTokenData {
    stream_playback_access_token: Option<AccessToken>,
}

#[derive(Deserialize)]
struct AccessToken {
    value: String,
    signature: String,
}

#[derive(Deserialize)]
struct ChannelProfileResponse {
    data: ChannelProfileData,
}

#[derive(Deserialize)]
struct ChannelProfileData {
    user: Option<ChannelProfileUserData>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChannelProfileUserData {
    display_name: String,
    description: Option<String>,
    #[serde(rename = "profileImageURL")]
    profile_image_url: Option<String>,
    #[serde(rename = "bannerImageURL")]
    banner_image_url: Option<String>,
}

#[async_trait]
impl StreamPlatform for TwitchPlatform {
    fn platform(&self) -> Platform {
        Platform::Twitch
    }

    async fn check_live(&self, channel: &str) -> PlatformResult<Option<StreamInfo>> {
        let query = r#"
            query StreamMetadata($login: String!) {
                user(login: $login) {
                    stream {
                        title
                        game { name }
                        viewersCount
                        createdAt
                        previewImageURL(width: 320, height: 180)
                    }
                }
            }
        "#;

        let req = GqlRequest {
            query: query.to_string(),
            variables: serde_json::json!({ "login": channel.to_lowercase() }),
        };

        let resp: StreamMetadataResponse = self.gql_request(&req).await?;

        let Some(user) = resp.data.user else {
            return Err(PlatformError::ChannelNotFound(channel.to_string()));
        };

        let Some(stream) = user.stream else {
            return Ok(None); // Channel exists but not live
        };

        let started_at = DateTime::parse_from_rfc3339(&stream.created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(Some(StreamInfo {
            title: stream.title,
            game: stream.game.map(|g| g.name),
            viewer_count: stream.viewers_count,
            started_at,
            thumbnail_url: stream.preview_image_url,
        }))
    }

    async fn get_qualities(&self, channel: &str) -> PlatformResult<Vec<Quality>> {
        // First get access token, then fetch master playlist
        let stream_url = self.get_stream_url(channel, &Quality::source()).await?;

        // Fetch master playlist
        let playlist_text = self
            .client
            .get(&stream_url.url)
            .send()
            .await?
            .text()
            .await?;

        // Parse qualities from master playlist
        let mut qualities = vec![Quality::source()];

        for line in playlist_text.lines() {
            if line.starts_with("#EXT-X-STREAM-INF:") {
                // Parse resolution and bandwidth
                let mut name = "unknown".to_string();
                let mut resolution = None;
                let mut bandwidth = None;

                for part in line.split(',') {
                    if let Some(res) = part.strip_prefix("RESOLUTION=") {
                        resolution = Some(res.to_string());
                        // Extract name from resolution (e.g., "1920x1080" -> "1080p")
                        if let Some(height) = res.split('x').nth(1) {
                            name = format!("{}p", height);
                        }
                    }
                    if let Some(bw) = part.strip_prefix("BANDWIDTH=") {
                        bandwidth = bw.parse().ok();
                    }
                    if let Some(n) = part.strip_prefix("VIDEO=\"") {
                        name = n.trim_end_matches('"').to_string();
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
        // Get playback access token
        let query = r#"
            query PlaybackAccessToken($login: String!) {
                streamPlaybackAccessToken(
                    channelName: $login,
                    params: {
                        platform: "web",
                        playerBackend: "mediaplayer",
                        playerType: "site"
                    }
                ) {
                    value
                    signature
                }
            }
        "#;

        let req = GqlRequest {
            query: query.to_string(),
            variables: serde_json::json!({ "login": channel.to_lowercase() }),
        };

        let resp: PlaybackAccessTokenResponse = self.gql_request(&req).await?;

        let token = resp
            .data
            .stream_playback_access_token
            .ok_or(PlatformError::StreamOffline)?;

        // Construct master playlist URL
        let master_url = format!(
            "https://usher.ttvnw.net/api/channel/hls/{}.m3u8?sig={}&token={}&allow_source=true",
            channel.to_lowercase(),
            token.signature,
            urlencoding::encode(&token.value)
        );

        // Fetch master playlist to resolve to media playlist
        let master_content = self.client.get(&master_url).send().await?.text().await?;

        // Parse master playlist
        let variants = parse_master_playlist(&master_content, &master_url)
            .map_err(|e| PlatformError::Api(format!("Failed to parse master playlist: {}", e)))?;

        // Find variant for requested quality
        let variant = find_variant_for_quality(&variants, &quality.name)
            .ok_or_else(|| PlatformError::Api("No suitable quality variant found".to_string()))?;

        Ok(StreamUrl {
            url: variant.uri.clone(),
            quality: quality.clone(),
        })
    }

    async fn get_channel_profile(&self, channel: &str) -> PlatformResult<ChannelProfile> {
        let query = r#"
            query ChannelProfile($login: String!) {
                user(login: $login) {
                    displayName
                    description
                    profileImageURL(width: 300)
                    bannerImageURL
                }
            }
        "#;

        let req = GqlRequest {
            query: query.to_string(),
            variables: serde_json::json!({ "login": channel.to_lowercase() }),
        };

        // Make the request and get raw response for debugging
        let http_req = self
            .client
            .post(TWITCH_GQL_URL)
            .header("Client-ID", TWITCH_CLIENT_ID)
            .json(&req);

        let http_resp = http_req.send().await?;

        if !http_resp.status().is_success() {
            return Err(PlatformError::Api(format!(
                "Twitch API returned {}",
                http_resp.status()
            )));
        }

        // Get raw text for debugging
        let raw_text = http_resp.text().await?;
        tracing::debug!("Twitch profile response for {}: {}", channel, raw_text);

        // Parse the response
        let resp: ChannelProfileResponse = serde_json::from_str(&raw_text).map_err(|e| {
            PlatformError::Api(format!(
                "Failed to parse response: {} - Raw: {}",
                e, raw_text
            ))
        })?;

        let user = resp
            .data
            .user
            .ok_or_else(|| PlatformError::ChannelNotFound(channel.to_string()))?;

        Ok(ChannelProfile {
            display_name: user.display_name,
            description: user.description,
            profile_image_url: user.profile_image_url,
            banner_image_url: user.banner_image_url,
        })
    }
}
