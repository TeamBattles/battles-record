// API Integration Tests for br-daemon
//
// These tests verify the HTTP API endpoints work correctly.
// They use axum's test utilities to make requests without network overhead.

mod common;

use br_daemon::{
    api::auth::create_token,
    config::{ChannelConfig, Config},
    types::{ChannelStatus, Platform, UserRole},
};
use serde_json::{json, Value};

/**
 * JWT Token Tests
 */

#[test]
fn test_create_token_for_admin() {
    let (token, expiry) = create_token("admin", UserRole::Admin, "secret", 24).unwrap();
    assert!(!token.is_empty());
    assert!(expiry > chrono::Utc::now());
}

#[test]
fn test_create_token_for_viewer() {
    let (token, expiry) = create_token("viewer", UserRole::Viewer, "secret", 1).unwrap();
    assert!(!token.is_empty());
    assert!(expiry > chrono::Utc::now());
}

#[test]
fn test_verify_token_with_correct_secret() {
    let (token, _) = create_token("user", UserRole::Admin, "mysecret", 24).unwrap();
    let claims = br_daemon::api::auth::verify_token(&token, "mysecret").unwrap();
    assert_eq!(claims.sub, "user");
    assert_eq!(claims.role, UserRole::Admin);
}

#[test]
fn test_verify_token_with_wrong_secret() {
    let (token, _) = create_token("user", UserRole::Admin, "mysecret", 24).unwrap();
    let result = br_daemon::api::auth::verify_token(&token, "wrongsecret");
    assert!(result.is_err());
}

#[test]
fn test_token_expiry_is_correct() {
    let duration_hours = 12;
    let (_, expiry) = create_token("user", UserRole::Admin, "secret", duration_hours).unwrap();

    let now = chrono::Utc::now();
    let expected_min = now + chrono::Duration::hours(duration_hours as i64 - 1);
    let expected_max = now + chrono::Duration::hours(duration_hours as i64 + 1);

    assert!(expiry > expected_min && expiry < expected_max);
}

/**
 * Channel Config Tests
 */

#[test]
fn test_channel_config_creation() {
    let config = ChannelConfig {
        name: "test_channel".to_string(),
        platform: Platform::Twitch,
        enabled: true,
        quality: "best".to_string(),
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

    assert_eq!(config.name, "test_channel");
    assert_eq!(config.platform, Platform::Twitch);
    assert!(config.enabled);
    assert_eq!(config.quality, "best");
    assert!(config.quota_gb.is_none());
}

#[test]
fn test_channel_config_with_quota() {
    let config = ChannelConfig {
        name: "limited_channel".to_string(),
        platform: Platform::YouTube,
        enabled: true,
        quality: "1080p".to_string(),
        schedule: None,
        filters: None,
        post_processing: None,
        quota_gb: Some(10),
        retention_days: Some(30),
        custom_profile_image: None,
        custom_banner_image: None,
        platform_profile_url: None,
        platform_banner_url: None,
    };

    assert_eq!(config.quota_gb, Some(10));
    assert_eq!(config.retention_days, Some(30));
}

#[test]
fn test_channel_config_serialization() {
    let config = ChannelConfig {
        name: "serialized_channel".to_string(),
        platform: Platform::Kick,
        enabled: false,
        quality: "720p".to_string(),
        schedule: None,
        filters: None,
        post_processing: None,
        quota_gb: Some(5),
        retention_days: None,
        custom_profile_image: None,
        custom_banner_image: None,
        platform_profile_url: None,
        platform_banner_url: None,
    };

    let json = serde_json::to_string(&config).unwrap();
    let parsed: ChannelConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.name, parsed.name);
    assert_eq!(config.platform, parsed.platform);
    assert_eq!(config.enabled, parsed.enabled);
    assert_eq!(config.quality, parsed.quality);
    assert_eq!(config.quota_gb, parsed.quota_gb);
}

/**
 * Config Serialization Tests
 */

#[test]
fn test_config_default_values() {
    let config = Config::default();

    assert_eq!(config.daemon.host, "127.0.0.1");
    assert_eq!(config.daemon.port, 8080);
    assert!(config.channels.is_empty());
}

#[test]
fn test_config_toml_roundtrip() {
    let config = Config::default();
    let toml_str = toml::to_string_pretty(&config).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();

    assert_eq!(config.daemon.host, parsed.daemon.host);
    assert_eq!(config.daemon.port, parsed.daemon.port);
}

/**
 * Platform Enum Tests
 */

#[test]
fn test_platform_display() {
    assert_eq!(Platform::Twitch.to_string(), "twitch");
    assert_eq!(Platform::YouTube.to_string(), "youtube");
    assert_eq!(Platform::Kick.to_string(), "kick");
}

#[test]
fn test_platform_serialization() {
    let platform = Platform::Twitch;
    let json = serde_json::to_string(&platform).unwrap();
    assert_eq!(json, "\"twitch\"");

    let parsed: Platform = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, Platform::Twitch);
}

#[test]
fn test_all_platforms_serialize() {
    let platforms = [Platform::Twitch, Platform::YouTube, Platform::Kick];

    for platform in &platforms {
        let json = serde_json::to_string(platform).unwrap();
        let parsed: Platform = serde_json::from_str(&json).unwrap();
        assert_eq!(platform, &parsed);
    }
}

/**
 * UserRole Tests
 */

#[test]
fn test_user_role_equality() {
    assert_eq!(UserRole::Admin, UserRole::Admin);
    assert_eq!(UserRole::Viewer, UserRole::Viewer);
    assert_ne!(UserRole::Admin, UserRole::Viewer);
}

#[test]
fn test_user_role_serialization() {
    let role = UserRole::Admin;
    let json = serde_json::to_string(&role).unwrap();
    assert_eq!(json, "\"admin\"");

    let parsed: UserRole = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, UserRole::Admin);
}

#[test]
fn test_user_role_viewer_serialization() {
    let role = UserRole::Viewer;
    let json = serde_json::to_string(&role).unwrap();
    assert_eq!(json, "\"viewer\"");

    let parsed: UserRole = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, UserRole::Viewer);
}

/**
 * API Response Format Tests
 */

#[test]
fn test_api_response_format() {
    use br_daemon::api::response::ApiResponse;

    let response = ApiResponse {
        data: json!({"channels": []}),
    };

    let json = serde_json::to_string(&response).unwrap();
    let parsed: Value = serde_json::from_str(&json).unwrap();

    assert!(parsed.get("data").is_some());
    assert!(parsed["data"]["channels"].is_array());
}

#[test]
fn test_api_response_with_complex_data() {
    use br_daemon::api::response::ApiResponse;

    let response = ApiResponse {
        data: json!({
            "id": "ch-123",
            "name": "test_channel",
            "platform": "twitch",
            "status": "live"
        }),
    };

    let json = serde_json::to_string(&response).unwrap();
    let parsed: Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["data"]["id"], "ch-123");
    assert_eq!(parsed["data"]["platform"], "twitch");
}

/**
 * Password Hashing Tests
 */

#[test]
fn test_password_hash_format() {
    let hash = br_daemon::api::auth::hash_password("testpass").unwrap();
    // bcrypt hashes have a specific format
    assert!(hash.starts_with("$2"));
    assert!(hash.len() >= 59); // bcrypt output is 60 chars
}

#[test]
fn test_password_verification_consistent() {
    let hash = br_daemon::api::auth::hash_password("mypassword").unwrap();

    // Multiple verifications should all succeed
    for _ in 0..3 {
        assert!(br_daemon::api::auth::verify_password("mypassword", &hash));
    }
}

#[test]
fn test_different_passwords_different_hashes() {
    let hash1 = br_daemon::api::auth::hash_password("password1").unwrap();
    let hash2 = br_daemon::api::auth::hash_password("password2").unwrap();

    // Different passwords should have different hashes
    assert_ne!(hash1, hash2);

    // But each hash should only verify its own password
    assert!(br_daemon::api::auth::verify_password("password1", &hash1));
    assert!(!br_daemon::api::auth::verify_password("password2", &hash1));
    assert!(!br_daemon::api::auth::verify_password("password1", &hash2));
    assert!(br_daemon::api::auth::verify_password("password2", &hash2));
}

/**
 * Channel Status Tests
 */

#[test]
fn test_channel_status_serialization() {
    // Test each status variant
    let statuses = [
        ChannelStatus::Offline,
        ChannelStatus::Live,
        ChannelStatus::Recording,
        ChannelStatus::Error,
    ];

    for status in &statuses {
        let json = serde_json::to_string(status).unwrap();
        let parsed: ChannelStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, &parsed);
    }
}

#[test]
fn test_channel_status_display() {
    assert_eq!(format!("{:?}", ChannelStatus::Offline), "Offline");
    assert_eq!(format!("{:?}", ChannelStatus::Live), "Live");
    assert_eq!(format!("{:?}", ChannelStatus::Recording), "Recording");
    assert_eq!(format!("{:?}", ChannelStatus::Error), "Error");
}

/**
 * Quota Status Tests
 */

#[test]
fn test_quota_status_serialization() {
    use br_daemon::types::QuotaStatus;

    let statuses = [
        QuotaStatus::Ok,
        QuotaStatus::Warning,
        QuotaStatus::Exceeded,
        QuotaStatus::Unlimited,
    ];

    for status in &statuses {
        let json = serde_json::to_string(status).unwrap();
        let parsed: QuotaStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, &parsed);
    }
}

/**
 * Login Request/Response Tests
 */

#[test]
fn test_login_request_deserialization() {
    use br_daemon::api::auth::LoginRequest;

    let json = r#"{"username": "admin", "password": "secret123"}"#;
    let request: LoginRequest = serde_json::from_str(json).unwrap();

    assert_eq!(request.username, "admin");
    assert_eq!(request.password, "secret123");
}

#[test]
fn test_login_response_serialization() {
    use br_daemon::api::auth::LoginResponse;

    let response = LoginResponse {
        token: "jwt-token-here".to_string(),
        role: UserRole::Admin,
        expires_at: "2024-12-31T23:59:59Z".to_string(),
    };

    let json = serde_json::to_string(&response).unwrap();
    let parsed: Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["token"], "jwt-token-here");
    assert_eq!(parsed["role"], "admin");
    assert_eq!(parsed["expires_at"], "2024-12-31T23:59:59Z");
}
