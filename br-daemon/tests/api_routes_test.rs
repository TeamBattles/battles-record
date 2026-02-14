// br-daemon/tests/api_routes_test.rs
//
// Comprehensive tests for the HTTP API endpoints.
// These tests verify authentication, channel CRUD, recordings, and storage endpoints.

mod common;

use axum::http::StatusCode;
use common::{
    fixtures::{add_channel_json, login_json, update_channel_json, ChannelConfigBuilder},
    test_server::{json_body, ResponseExt, TestServer, TestServerOptions},
};
use serde_json::{json, Value};

/**
 * Health Endpoint Tests
 */

#[tokio::test]
async fn test_health_endpoint_returns_ok() {
    let server = TestServer::new().await.expect("Failed to create server");

    let response = server.get("/health").await;
    response.assert_success();

    let body: Value = json_body(response).await;
    assert_eq!(body["status"], "ok");
    assert!(body["version"].is_string());
}

/**
 * Authentication Tests
 */

#[tokio::test]
async fn test_login_with_valid_credentials() {
    let server = TestServer::new().await.expect("Failed to create server");

    let response = server
        .post_json("/api/auth/login", login_json("admin", "admin123"))
        .await;
    response.assert_success();

    let body: Value = json_body(response).await;
    assert!(body["data"]["token"].is_string());
    assert_eq!(body["data"]["role"], "admin");
    assert!(body["data"]["expires_at"].is_string());
}

#[tokio::test]
async fn test_login_with_invalid_password() {
    let server = TestServer::new().await.expect("Failed to create server");

    let response = server
        .post_json("/api/auth/login", login_json("admin", "wrongpassword"))
        .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_login_with_invalid_username() {
    let server = TestServer::new().await.expect("Failed to create server");

    let response = server
        .post_json("/api/auth/login", login_json("nonexistent", "password"))
        .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_login_in_local_only_mode_bypasses_auth() {
    let server = TestServer::with_options(TestServerOptions::local_only())
        .await
        .expect("Failed to create server");

    // Any credentials should work in local-only mode
    let response = server
        .post_json("/api/auth/login", login_json("anyone", "anything"))
        .await;
    response.assert_success();

    let body: Value = json_body(response).await;
    assert!(body["data"]["token"].is_string());
    // In local mode, user is granted admin role
    assert_eq!(body["data"]["role"], "admin");
}

#[tokio::test]
async fn test_viewer_login_returns_viewer_role() {
    let server = TestServer::new().await.expect("Failed to create server");

    let response = server
        .post_json("/api/auth/login", login_json("viewer", "viewer123"))
        .await;
    response.assert_success();

    let body: Value = json_body(response).await;
    assert_eq!(body["data"]["role"], "viewer");
}

/**
 * Channel List Tests
 */

#[tokio::test]
async fn test_list_channels_requires_auth() {
    let server = TestServer::new().await.expect("Failed to create server");

    let response = server.get("/api/channels").await;
    response.assert_unauthorized();
}

#[tokio::test]
async fn test_list_channels_returns_empty_initially() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    let response = server.get_auth("/api/channels", &token).await;
    response.assert_success();

    let body: Value = json_body(response).await;
    assert!(body["data"]["channels"].is_array());
    assert_eq!(body["data"]["channels"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_list_channels_viewer_can_access() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.viewer_token();

    let response = server.get_auth("/api/channels", &token).await;
    response.assert_success();
}

/**
 * Channel Create Tests
 */

#[tokio::test]
async fn test_create_channel_success() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    let response = server
        .post_json_auth(
            "/api/channels",
            add_channel_json("test_streamer", "twitch"),
            &token,
        )
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);

    let body: Value = json_body(response).await;
    assert!(body["data"]["id"].is_string());
    assert_eq!(body["data"]["channel"]["name"], "test_streamer");
    assert_eq!(body["data"]["channel"]["platform"], "twitch");
    assert_eq!(body["data"]["channel"]["enabled"], true);
    assert_eq!(body["data"]["channel"]["quality"], "best");
}

#[tokio::test]
async fn test_create_channel_requires_admin() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.viewer_token();

    let response = server
        .post_json_auth(
            "/api/channels",
            add_channel_json("test_streamer", "twitch"),
            &token,
        )
        .await;

    // AdminUser extractor returns 401 with "Admin access required" message
    response.assert_unauthorized();
}

#[tokio::test]
async fn test_create_channel_duplicate_fails() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    // Create first channel
    let response = server
        .post_json_auth(
            "/api/channels",
            add_channel_json("duplicate_test", "twitch"),
            &token,
        )
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    // Try to create duplicate
    let response = server
        .post_json_auth(
            "/api/channels",
            add_channel_json("duplicate_test", "twitch"),
            &token,
        )
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_channel_same_name_different_platform_succeeds() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    // Create Twitch channel
    let response = server
        .post_json_auth(
            "/api/channels",
            add_channel_json("multi_platform", "twitch"),
            &token,
        )
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    // Create Kick channel with same name - should succeed (different platform)
    let response = server
        .post_json_auth(
            "/api/channels",
            add_channel_json("multi_platform", "kick"),
            &token,
        )
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_create_channel_with_custom_quality() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    let payload = json!({
        "name": "quality_test",
        "platform": "twitch",
        "enabled": true,
        "quality": "1080p60"
    });

    let response = server
        .post_json_auth("/api/channels", payload, &token)
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let body: Value = json_body(response).await;
    assert_eq!(body["data"]["channel"]["quality"], "1080p60");
}

#[tokio::test]
async fn test_create_channel_disabled() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    let payload = json!({
        "name": "disabled_channel",
        "platform": "kick",
        "enabled": false,
        "quality": "best"
    });

    let response = server
        .post_json_auth("/api/channels", payload, &token)
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let body: Value = json_body(response).await;
    assert_eq!(body["data"]["channel"]["enabled"], false);
}

/**
 * Channel Get Tests
 */

#[tokio::test]
async fn test_get_channel_success() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    // Create a channel first
    let create_response = server
        .post_json_auth(
            "/api/channels",
            add_channel_json("get_test", "twitch"),
            &token,
        )
        .await;
    let create_body: Value = json_body(create_response).await;
    let channel_id = create_body["data"]["id"].as_str().unwrap();

    // Get the channel
    let response = server
        .get_auth(&format!("/api/channels/{}", channel_id), &token)
        .await;
    response.assert_success();

    let body: Value = json_body(response).await;
    assert_eq!(body["data"]["name"], "get_test");
    assert_eq!(body["data"]["platform"], "twitch");
}

#[tokio::test]
async fn test_get_channel_not_found() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    let response = server
        .get_auth("/api/channels/00000000-0000-0000-0000-000000000000", &token)
        .await;

    response.assert_not_found();
}

/**
 * Channel Update Tests
 */

#[tokio::test]
async fn test_update_channel_success() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    // Create a channel
    let create_response = server
        .post_json_auth(
            "/api/channels",
            add_channel_json("update_test", "twitch"),
            &token,
        )
        .await;
    let create_body: Value = json_body(create_response).await;
    let channel_id = create_body["data"]["id"].as_str().unwrap();

    // Update the channel
    let update_payload = json!({
        "quality": "720p",
        "enabled": false
    });

    let response = server
        .put_json_auth(
            &format!("/api/channels/{}", channel_id),
            update_payload,
            &token,
        )
        .await;
    response.assert_success();

    let body: Value = json_body(response).await;
    assert_eq!(body["data"]["quality"], "720p");
    assert_eq!(body["data"]["enabled"], false);
}

#[tokio::test]
async fn test_update_channel_requires_admin() {
    let server = TestServer::new().await.expect("Failed to create server");
    let admin_token = server.admin_token();
    let viewer_token = server.viewer_token();

    // Create a channel as admin
    let create_response = server
        .post_json_auth(
            "/api/channels",
            add_channel_json("viewer_update_test", "twitch"),
            &admin_token,
        )
        .await;
    let create_body: Value = json_body(create_response).await;
    let channel_id = create_body["data"]["id"].as_str().unwrap();

    // Try to update as viewer
    let response = server
        .put_json_auth(
            &format!("/api/channels/{}", channel_id),
            json!({"enabled": false}),
            &viewer_token,
        )
        .await;

    // AdminUser extractor returns 401 with "Admin access required" message
    response.assert_unauthorized();
}

#[tokio::test]
async fn test_update_channel_quota_settings() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    // Create a channel
    let create_response = server
        .post_json_auth(
            "/api/channels",
            add_channel_json("quota_test", "twitch"),
            &token,
        )
        .await;
    let create_body: Value = json_body(create_response).await;
    let channel_id = create_body["data"]["id"].as_str().unwrap();

    // Update with quota settings
    let update_payload = json!({
        "quota_gb": 50,
        "retention_days": 30
    });

    let response = server
        .put_json_auth(
            &format!("/api/channels/{}", channel_id),
            update_payload,
            &token,
        )
        .await;
    response.assert_success();

    let body: Value = json_body(response).await;
    assert_eq!(body["data"]["quota_gb"], 50);
    assert_eq!(body["data"]["retention_days"], 30);
}

#[tokio::test]
async fn test_update_channel_clear_quota() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    // Create a channel with quota
    let create_response = server
        .post_json_auth(
            "/api/channels",
            add_channel_json("clear_quota_test", "twitch"),
            &token,
        )
        .await;
    let create_body: Value = json_body(create_response).await;
    let channel_id = create_body["data"]["id"].as_str().unwrap();

    // Set quota first
    let _ = server
        .put_json_auth(
            &format!("/api/channels/{}", channel_id),
            json!({"quota_gb": 100}),
            &token,
        )
        .await;

    // Clear quota by setting to null
    let response = server
        .put_json_auth(
            &format!("/api/channels/{}", channel_id),
            json!({"quota_gb": null}),
            &token,
        )
        .await;
    response.assert_success();

    let body: Value = json_body(response).await;
    assert!(body["data"]["quota_gb"].is_null());
}

/**
 * Channel Delete Tests
 */

#[tokio::test]
async fn test_delete_channel_success() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    // Create a channel
    let create_response = server
        .post_json_auth(
            "/api/channels",
            add_channel_json("delete_test", "twitch"),
            &token,
        )
        .await;
    let create_body: Value = json_body(create_response).await;
    let channel_id = create_body["data"]["id"].as_str().unwrap();

    // Delete the channel
    let response = server
        .delete_auth(&format!("/api/channels/{}", channel_id), &token)
        .await;
    response.assert_success();

    let body: Value = json_body(response).await;
    assert_eq!(body["data"]["deleted"], true);

    // Verify it's gone
    let get_response = server
        .get_auth(&format!("/api/channels/{}", channel_id), &token)
        .await;
    get_response.assert_not_found();
}

#[tokio::test]
async fn test_delete_channel_requires_admin() {
    let server = TestServer::new().await.expect("Failed to create server");
    let admin_token = server.admin_token();
    let viewer_token = server.viewer_token();

    // Create a channel as admin
    let create_response = server
        .post_json_auth(
            "/api/channels",
            add_channel_json("viewer_delete_test", "twitch"),
            &admin_token,
        )
        .await;
    let create_body: Value = json_body(create_response).await;
    let channel_id = create_body["data"]["id"].as_str().unwrap();

    // Try to delete as viewer
    let response = server
        .delete_auth(&format!("/api/channels/{}", channel_id), &viewer_token)
        .await;

    // AdminUser extractor returns 401 with "Admin access required" message
    response.assert_unauthorized();
}

#[tokio::test]
async fn test_delete_channel_not_found() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    let response = server
        .delete_auth("/api/channels/00000000-0000-0000-0000-000000000000", &token)
        .await;

    response.assert_not_found();
}

/**
 * Channel Check Tests
 */

#[tokio::test]
async fn test_check_channel_success() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    // Create a channel
    let create_response = server
        .post_json_auth(
            "/api/channels",
            add_channel_json("check_test", "twitch"),
            &token,
        )
        .await;
    let create_body: Value = json_body(create_response).await;
    let channel_id = create_body["data"]["id"].as_str().unwrap();

    // Check the channel (will likely return offline since it's a test)
    let response = server
        .post_json_auth(
            &format!("/api/channels/{}/check", channel_id),
            json!({}),
            &token,
        )
        .await;
    response.assert_success();

    let body: Value = json_body(response).await;
    assert!(body["data"]["channel"].is_object());
    assert!(body["data"]["message"].is_string());
}

#[tokio::test]
async fn test_check_channel_not_found() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    let response = server
        .post_json_auth(
            "/api/channels/00000000-0000-0000-0000-000000000000/check",
            json!({}),
            &token,
        )
        .await;

    response.assert_not_found();
}

/**
 * Storage Stats Tests
 */

#[tokio::test]
async fn test_get_storage_stats() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    let response = server.get_auth("/api/storage/stats", &token).await;
    response.assert_success();

    let body: Value = json_body(response).await;
    assert!(body["data"]["total_recordings"].is_number());
    assert!(body["data"]["total_size_bytes"].is_number());
    assert!(body["data"]["disk_free_bytes"].is_number());
    assert!(body["data"]["disk_total_bytes"].is_number());
    assert!(body["data"]["per_channel"].is_array());
}

#[tokio::test]
async fn test_get_storage_stats_requires_auth() {
    let server = TestServer::new().await.expect("Failed to create server");

    let response = server.get("/api/storage/stats").await;
    response.assert_unauthorized();
}

/**
 * Recordings List Tests
 */

#[tokio::test]
async fn test_list_recordings_requires_auth() {
    let server = TestServer::new().await.expect("Failed to create server");

    let response = server.get("/api/recordings").await;
    response.assert_unauthorized();
}

#[tokio::test]
async fn test_list_recordings_returns_empty_initially() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    let response = server.get_auth("/api/recordings", &token).await;
    response.assert_success();

    let body: Value = json_body(response).await;
    // Recordings are nested inside data.recordings
    assert!(body["data"]["recordings"].is_array());
    assert_eq!(body["data"]["total"], 0);
}

/**
 * Shutdown Tests
 */

#[tokio::test]
async fn test_shutdown_only_in_local_mode() {
    let server = TestServer::new().await.expect("Failed to create server");

    // In non-local mode, shutdown should be forbidden
    let response = server.post_json("/api/shutdown", json!({})).await;
    response.assert_forbidden();
}

#[tokio::test]
async fn test_shutdown_succeeds_in_local_mode() {
    let server = TestServer::with_options(TestServerOptions::local_only())
        .await
        .expect("Failed to create server");

    let response = server.post_json("/api/shutdown", json!({})).await;
    response.assert_success();
}

/**
 * Status Endpoint Tests
 */

#[tokio::test]
async fn test_status_endpoint() {
    let server = TestServer::new().await.expect("Failed to create server");
    let token = server.admin_token();

    let response = server.get_auth("/api/status", &token).await;
    response.assert_success();

    let body: Value = json_body(response).await;
    // Field is uptime_secs not uptime_seconds
    assert!(body["data"]["uptime_secs"].is_number());
    assert!(body["data"]["version"].is_string());
    assert!(body["data"]["disk"].is_object());
    assert!(body["data"]["channels"].is_object());
}

#[tokio::test]
async fn test_status_requires_auth() {
    let server = TestServer::new().await.expect("Failed to create server");

    let response = server.get("/api/status").await;
    response.assert_unauthorized();
}
