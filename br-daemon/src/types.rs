use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::ValidationError;

/** Supported streaming platforms. */
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Twitch,
    YouTube,
    Kick,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Twitch => write!(f, "twitch"),
            Platform::YouTube => write!(f, "youtube"),
            Platform::Kick => write!(f, "kick"),
        }
    }
}

/** Maximum allowed length for channel names. */
pub const CHANNEL_NAME_MAX_LENGTH: usize = 64;

/**
 * A validated channel name.
 *
 * Channel names must:
 * - Be non-empty
 * - Be at most 64 characters
 * - Contain only alphanumeric characters, underscores, or hyphens
 *
 * # Example
 * ```ignore
 * use br_daemon::types::ChannelName;
 *
 * let name = ChannelName::parse("my_channel").unwrap();
 * assert_eq!(name.as_str(), "my_channel");
 *
 * // Invalid names are rejected
 * assert!(ChannelName::parse("").is_err());
 * assert!(ChannelName::parse("invalid name!").is_err());
 * ```
 */
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ChannelName(String);

impl ChannelName {
    /**
     * Parse and validate a channel name.
     *
     * Returns an error if the name is empty, too long, or contains invalid characters.
     */
    pub fn parse(s: impl Into<String>) -> Result<Self, ValidationError> {
        let s = s.into();

        if s.is_empty() {
            return Err(ValidationError::EmptyChannelName);
        }

        if s.len() > CHANNEL_NAME_MAX_LENGTH {
            return Err(ValidationError::ChannelNameTooLong {
                name: s,
                max: CHANNEL_NAME_MAX_LENGTH,
            });
        }

        // Allow alphanumeric, underscores, and hyphens (common in Twitch/YouTube/Kick usernames)
        if !s
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(ValidationError::InvalidChannelNameChars { name: s });
        }

        Ok(ChannelName(s))
    }

    /**
     * Parse and validate a channel name with platform-specific rules.
     *
     * YouTube allows additional characters: `@`, `/`, `.`, `:`
     * Twitch/Kick only allow alphanumeric, underscores, and hyphens.
     */
    pub fn parse_for_platform(
        s: impl Into<String>,
        platform: Platform,
    ) -> Result<Self, ValidationError> {
        let s = s.into();

        if s.is_empty() {
            return Err(ValidationError::EmptyChannelName);
        }

        if s.len() > CHANNEL_NAME_MAX_LENGTH {
            return Err(ValidationError::ChannelNameTooLong {
                name: s,
                max: CHANNEL_NAME_MAX_LENGTH,
            });
        }

        let valid = match platform {
            Platform::YouTube => s
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '@' | '/' | '.' | ':')),
            Platform::Twitch | Platform::Kick => s
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-'),
        };

        if !valid {
            return Err(ValidationError::InvalidChannelNameCharsForPlatform {
                name: s,
                platform: platform.to_string(),
            });
        }

        Ok(ChannelName(s))
    }

    /** Get the channel name as a string slice. */
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /** Consume the ChannelName and return the inner String. */
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Display for ChannelName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for ChannelName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ChannelName {
    type Error = ValidationError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl From<ChannelName> for String {
    fn from(name: ChannelName) -> Self {
        name.0
    }
}

/** Information about a live stream. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub title: String,
    pub game: Option<String>,
    pub viewer_count: u32,
    pub started_at: DateTime<Utc>,
    pub thumbnail_url: Option<String>,
}

/** Quality option for a stream. */
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quality {
    pub name: String, // e.g., "1080p60", "720p", "source"
    pub resolution: Option<String>,
    pub bandwidth: Option<u64>,
}

impl Quality {
    pub fn source() -> Self {
        Self {
            name: "source".to_string(),
            resolution: None,
            bandwidth: None,
        }
    }
}

/** Status of a channel. */
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelStatus {
    Offline,
    Live,
    Recording,
    Error,
}

/** Quota status for a channel. */
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaStatus {
    #[default]
    Ok, // Under 90%
    Warning,   // 90-99% used
    Exceeded,  // At or over limit
    Unlimited, // No quota set
}

/** Status of a recording. */
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingStatus {
    Recording,
    Completed,
    Processing,
    Processed,
    Failed,
}

/** Schedule rule for API responses (uses numeric days 0-6). */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleRuleResponse {
    pub days: Vec<u8>,      // 0-6, Sunday=0
    pub start_time: String, // "HH:MM"
    pub end_time: String,   // "HH:MM"
}

/** Filter configuration for API responses. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiltersResponse {
    #[serde(default)]
    pub title_includes: Vec<String>,
    #[serde(default)]
    pub title_excludes: Vec<String>,
    #[serde(default)]
    pub game_includes: Vec<String>,
    #[serde(default)]
    pub game_excludes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_viewers: Option<u32>,
}

/** A channel being monitored. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: Uuid,
    pub name: String,
    pub platform: Platform,
    pub enabled: bool,
    pub quality: String,
    pub status: ChannelStatus,
    pub current_stream: Option<StreamInfo>,
    /** Maximum storage quota in GB (None = unlimited). */
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota_gb: Option<u32>,
    /** Retention period in days (None = unlimited). */
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u32>,
    /** Current quota status. */
    #[serde(default)]
    pub quota_status: QuotaStatus,
    /** Current storage used by this channel in bytes. */
    #[serde(default)]
    pub quota_used_bytes: u64,
    /** Usage as percentage of quota (0 if unlimited). */
    #[serde(default)]
    pub quota_percent: u8,
    /** Whether schedule-based recording is enabled. */
    #[serde(default)]
    pub schedule_enabled: bool,
    /** Timezone for schedule rules (e.g., "America/Los_Angeles"). */
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /** Schedule rules defining when to record. */
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schedule_rules: Vec<ScheduleRuleResponse>,
    /** Content filters for recording decisions. */
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<FiltersResponse>,
    /** Profile image URL (custom or platform). */
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_image_url: Option<String>,
    /** Banner image URL (custom or platform). */
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner_image_url: Option<String>,
}

/** A recording session. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub channel_name: String,
    pub platform: Platform,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub status: RecordingStatus,
    pub segments_downloaded: u32,
    pub size_bytes: u64,
    pub output_path: String,
}

/** User role for authorization. */
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    #[default]
    Viewer,
}

/** Extended recording info for API responses. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingDetail {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub channel_name: String,
    pub platform: Platform,
    pub status: RecordingStatus,
    pub title: Option<String>,
    pub game: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_secs: u64,
    pub size_bytes: u64,
    pub segment_count: u32,
    pub output_path: String,
    pub processed_file: Option<String>,
}

/** Image URLs for a channel (both platform-fetched and custom-uploaded). */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelImages {
    /** Profile image URL from the platform (Twitch/YouTube/Kick). */
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_profile_url: Option<String>,
    /** Banner image URL from the platform. */
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_banner_url: Option<String>,
    /** Custom-uploaded profile image URL (local daemon URL). */
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_profile_url: Option<String>,
    /** Custom-uploaded banner image URL (local daemon URL). */
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_banner_url: Option<String>,
}

/** Channel profile with images for API responses. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelProfile {
    pub channel_id: Uuid,
    pub display_name: String,
    pub platform: Platform,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /** Profile image URL from the platform. */
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_profile_url: Option<String>,
    /** Banner image URL from the platform. */
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_banner_url: Option<String>,
    /** Custom-uploaded profile image URL (local daemon URL). */
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_profile_url: Option<String>,
    /** Custom-uploaded banner image URL (local daemon URL). */
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_banner_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_name_valid() {
        assert!(ChannelName::parse("valid_name").is_ok());
        assert!(ChannelName::parse("ValidName123").is_ok());
        assert!(ChannelName::parse("name-with-hyphens").is_ok());
        assert!(ChannelName::parse("a").is_ok()); // Single char
        assert!(ChannelName::parse("_underscore").is_ok());
    }

    #[test]
    fn test_channel_name_empty() {
        let result = ChannelName::parse("");
        assert!(matches!(result, Err(ValidationError::EmptyChannelName)));
    }

    #[test]
    fn test_channel_name_too_long() {
        let long_name = "a".repeat(65);
        let result = ChannelName::parse(long_name);
        assert!(matches!(
            result,
            Err(ValidationError::ChannelNameTooLong { max: 64, .. })
        ));

        // Exactly 64 chars should be OK
        let max_name = "a".repeat(64);
        assert!(ChannelName::parse(max_name).is_ok());
    }

    #[test]
    fn test_channel_name_invalid_chars() {
        assert!(matches!(
            ChannelName::parse("has spaces"),
            Err(ValidationError::InvalidChannelNameChars { .. })
        ));
        assert!(matches!(
            ChannelName::parse("has@symbol"),
            Err(ValidationError::InvalidChannelNameChars { .. })
        ));
        assert!(matches!(
            ChannelName::parse("has/slash"),
            Err(ValidationError::InvalidChannelNameChars { .. })
        ));
        assert!(matches!(
            ChannelName::parse("emoji🎮"),
            Err(ValidationError::InvalidChannelNameChars { .. })
        ));
    }

    #[test]
    fn test_channel_name_display() {
        let name = ChannelName::parse("test_channel").unwrap();
        assert_eq!(format!("{}", name), "test_channel");
        assert_eq!(name.as_str(), "test_channel");
    }

    #[test]
    fn test_channel_name_serde() {
        let name = ChannelName::parse("test_channel").unwrap();
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, "\"test_channel\"");

        let parsed: ChannelName = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, name);
    }

    #[test]
    fn test_channel_name_serde_invalid() {
        // Invalid names should fail deserialization
        let result: Result<ChannelName, _> = serde_json::from_str("\"invalid name!\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_channel_name_youtube_handles() {
        // YouTube allows @ for handles
        assert!(ChannelName::parse_for_platform("@MrBeast", Platform::YouTube).is_ok());
        assert!(ChannelName::parse_for_platform("@pewdiepie", Platform::YouTube).is_ok());

        // YouTube allows channel URLs with slashes
        assert!(ChannelName::parse_for_platform("youtube.com/@MrBeast", Platform::YouTube).is_ok());

        // But Twitch/Kick should reject @
        assert!(ChannelName::parse_for_platform("@MrBeast", Platform::Twitch).is_err());
        assert!(ChannelName::parse_for_platform("@MrBeast", Platform::Kick).is_err());
    }

    #[test]
    fn test_channel_name_twitch_kick_unchanged() {
        // Twitch/Kick validation should work same as before
        assert!(ChannelName::parse_for_platform("valid_name", Platform::Twitch).is_ok());
        assert!(ChannelName::parse_for_platform("name-with-hyphens", Platform::Kick).is_ok());
        assert!(ChannelName::parse_for_platform("has spaces", Platform::Twitch).is_err());
    }
}
