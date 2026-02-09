// Common test utilities for br-daemon integration tests

pub mod fixtures;
pub mod test_server;

// Re-export commonly used items from submodules
pub use test_server::{TestServer, TestServerOptions, ResponseExt, json_body, text_body};

use br_daemon::config::{Config, DaemonConfig, AuthConfig, StorageConfig, PollingConfig, PostProcessingConfig, QuotaConfig, RetentionConfig, OAuthConfig};
use tempfile::TempDir;

/// Create a minimal test configuration with a temporary directory
pub fn create_test_config(temp_dir: &TempDir) -> Config {
    let recordings_dir = temp_dir.path().join("recordings");
    let library_dir = temp_dir.path().join("library");
    let images_dir = temp_dir.path().join("images");

    // Create directories
    std::fs::create_dir_all(&recordings_dir).expect("Failed to create recordings dir");
    std::fs::create_dir_all(&library_dir).expect("Failed to create library dir");
    std::fs::create_dir_all(&images_dir).expect("Failed to create images dir");

    Config {
        daemon: DaemonConfig {
            host: "127.0.0.1".to_string(),
            port: 0, // Use any available port
            log_level: "warn".to_string(),
            log_file: None,
            channels_file: None,
        },
        auth: AuthConfig {
            jwt_secret: Some("test-secret-key".to_string()),
            session_duration: 86400,
            refresh_grace_period: 3600, // 1 hour
        },
        users: vec![],
        storage: StorageConfig {
            recordings_dir,
            library_dir,
            images_dir,
            disk_warning_threshold: 90,
            quotas: QuotaConfig {
                global_max_gb: None,
                per_channel_max_gb: None,
                warn_at_percent: 80,
            },
            retention: RetentionConfig {
                max_age_days: None,
                keep_minimum: 1,
                cleanup_interval_hours: 24,
            },
        },
        polling: PollingConfig::default(),
        post_processing: PostProcessingConfig::default(),
        jellyfin: Default::default(),
        notifications: Default::default(),
        channels: vec![],
        platform_auth: Default::default(),
        oauth: OAuthConfig::default(),
    }
}

/// Create a test JWT token for authenticated requests
pub fn create_test_token(username: &str, role: br_daemon::types::UserRole) -> String {
    let (token, _) =
        br_daemon::api::auth::create_token(username, role, "test-secret-key", 24).unwrap();
    token
}
