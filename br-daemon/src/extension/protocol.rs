use serde::{Deserialize, Serialize};

use crate::libraries::LibraryStatus;

/// Messages sent from the browser extension to the daemon.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExtensionMessage {
    Hello {
        extension_version: String,
        #[serde(default)]
        token: Option<String>,
    },
    Pair {
        code: String,
        identifier: String,
    },
    ExtractInfo {
        id: String,
        url: String,
        #[serde(default)]
        auto_start: Option<bool>,
        #[serde(default)]
        cookies: Option<Vec<CookieEntry>>,
    },
    Download {
        id: String,
        url: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        quality: Option<String>,
        #[serde(default)]
        channel_name: Option<String>,
        #[serde(default)]
        format: Option<String>,
        #[serde(default)]
        options: Option<DownloadOptions>,
        #[serde(default)]
        cookies: Option<Vec<CookieEntry>>,
        #[serde(default)]
        source_platform: Option<String>,
    },
    Pause {
        id: String,
        download_id: String,
    },
    Resume {
        id: String,
        download_id: String,
    },
    Cancel {
        id: String,
        download_id: String,
    },
    Prioritize {
        id: String,
        download_id: String,
    },
    GetQueue,
    GetLibraryStatus,
    InstallLibraries,
    UpdateLibrary {
        library: String,
    },
    UninstallLibrary {
        id: String,
        library: String,
    },
    Unpair,
    Ping,
    MergeChannels {
        id: String,
        platform: String,
        source: String,
        target: String,
        #[serde(default)]
        include_recordings: bool,
    },
    AddChannel {
        id: String,
        name: String,
        platform: String,
    },
    RemoveChannel {
        id: String,
        channel_id: String,
    },
}

/// Messages sent from the daemon to the browser extension.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonMessage {
    Hello {
        version: String,
        requires_pairing: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        identifier: Option<String>,
        libraries: LibraryStatus,
        libraries_installed: bool,
    },
    Paired {
        token: String,
        identifier: String,
    },
    PairFailed {
        reason: String,
    },
    InfoResult {
        id: String,
        title: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        thumbnail: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        uploader: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        platform_name: Option<String>,
        formats: Vec<FormatInfo>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        existing_downloads: Vec<DownloadJobSummary>,
    },
    DownloadStarted {
        id: String,
        download_id: String,
    },
    DownloadProgress {
        download_id: String,
        status: String,
        percent: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        speed: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        eta: Option<u64>,
        downloaded_bytes: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        total_bytes: Option<u64>,
    },
    DownloadComplete {
        download_id: String,
        filepath: String,
        filesize: u64,
    },
    DownloadFailed {
        download_id: String,
        error: String,
        #[serde(default)]
        update_available: bool,
    },
    DownloadPaused {
        id: String,
        download_id: String,
    },
    DownloadResumed {
        id: String,
        download_id: String,
    },
    DownloadCancelled {
        id: String,
        download_id: String,
    },
    DownloadPrioritized {
        id: String,
        download_id: String,
    },
    Unpaired,
    QuotaWarning {
        channel_name: String,
        quota_used_bytes: u64,
        quota_limit_bytes: u64,
        estimated_download_bytes: u64,
    },
    QuotaExceeded {
        download_id: String,
        channel_name: String,
        quota_used_bytes: u64,
        quota_limit_bytes: u64,
        estimated_download_bytes: u64,
    },
    QueueState {
        downloads: Vec<DownloadJobSummary>,
    },
    ChannelsState {
        channels: Vec<ChannelSummary>,
    },
    LibraryDownloadProgress {
        library: String,
        percent: f64,
    },
    LibraryInstalled {
        library: String,
        version: String,
    },
    LibraryInstallFailed {
        library: String,
        error: String,
    },
    LibraryUninstalled {
        id: String,
        library: String,
    },
    LibraryUpdateAvailable {
        library: String,
        current: String,
        latest: String,
    },
    PortChanged {
        new_port: u16,
    },
    Disconnected {
        reason: String,
    },
    ChannelsMerged {
        platform: String,
        source: String,
        target: String,
        merged_downloads: bool,
        merged_recordings: bool,
    },
    ChannelAdded {
        id: String,
        channel_id: String,
        name: String,
        platform: String,
    },
    ChannelRemoved {
        id: String,
        channel_id: String,
        name: String,
        platform: String,
    },
    Pong,
    Error {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadOptions {
    #[serde(default)]
    pub embed_thumbnail: bool,
    #[serde(default)]
    pub embed_metadata: bool,
}

/// Cookie entry matching chrome.cookies.getAll() JSON format (camelCase fields).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CookieEntry {
    pub domain: String,
    pub path: String,
    pub secure: bool,
    #[serde(default)]
    pub expiration_date: f64,
    #[serde(default)]
    pub http_only: bool,
    pub name: String,
    pub value: String,
}

/// Lightweight channel info sent to extensions.
#[derive(Debug, Clone, Serialize)]
pub struct ChannelSummary {
    pub channel_id: String,
    pub name: String,
    pub platform: String,
    pub enabled: bool,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatInfo {
    pub format_id: String,
    pub ext: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filesize_approx: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vcodec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acodec: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadJobSummary {
    pub id: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_name: Option<String>,
    pub channel_name: String,
    pub source_platform: String,
    pub status: String,
    pub percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta: Option<u64>,
    pub downloaded_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    pub requested_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_by_name: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub update_available: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_extension_hello_with_token() {
        let json_str = r#"{"type": "hello", "extension_version": "1.0.0", "token": "abc123"}"#;
        let msg: ExtensionMessage = serde_json::from_str(json_str).unwrap();
        match msg {
            ExtensionMessage::Hello {
                extension_version,
                token,
            } => {
                assert_eq!(extension_version, "1.0.0");
                assert_eq!(token, Some("abc123".to_string()));
            }
            _ => panic!("expected Hello variant"),
        }
    }

    #[test]
    fn test_extension_hello_without_token() {
        let json_str = r#"{"type": "hello", "extension_version": "2.0.0"}"#;
        let msg: ExtensionMessage = serde_json::from_str(json_str).unwrap();
        match msg {
            ExtensionMessage::Hello {
                extension_version,
                token,
            } => {
                assert_eq!(extension_version, "2.0.0");
                assert_eq!(token, None);
            }
            _ => panic!("expected Hello variant"),
        }
    }

    #[test]
    fn test_extension_download_all_fields() {
        let json_str = r#"{
            "type": "download",
            "id": "req-1",
            "url": "https://example.com/video",
            "channel_name": "test_channel",
            "format": "bestvideo+bestaudio",
            "options": {"embed_thumbnail": true, "embed_metadata": false},
            "cookies": [
                {
                    "domain": ".example.com",
                    "path": "/",
                    "secure": true,
                    "expirationDate": 1700000000.0,
                    "httpOnly": false,
                    "name": "session",
                    "value": "abc"
                }
            ],
            "source_platform": "youtube"
        }"#;
        let msg: ExtensionMessage = serde_json::from_str(json_str).unwrap();
        match msg {
            ExtensionMessage::Download {
                id,
                url,
                title,
                quality,
                channel_name,
                format,
                options,
                cookies,
                source_platform,
            } => {
                assert_eq!(id, "req-1");
                assert_eq!(url, "https://example.com/video");
                assert_eq!(title, None);
                assert_eq!(quality, None);
                assert_eq!(channel_name, Some("test_channel".to_string()));
                assert_eq!(format, Some("bestvideo+bestaudio".to_string()));
                let opts = options.unwrap();
                assert!(opts.embed_thumbnail);
                assert!(!opts.embed_metadata);
                let cookies = cookies.unwrap();
                assert_eq!(cookies.len(), 1);
                assert_eq!(cookies[0].name, "session");
                assert_eq!(cookies[0].domain, ".example.com");
                assert!((cookies[0].expiration_date - 1700000000.0).abs() < f64::EPSILON);
                assert!(!cookies[0].http_only);
                assert_eq!(source_platform, Some("youtube".to_string()));
            }
            _ => panic!("expected Download variant"),
        }
    }

    #[test]
    fn test_extension_download_minimal() {
        let json_str = r#"{
            "type": "download",
            "id": "req-2",
            "url": "https://example.com/vid2",
            "channel_name": "ch"
        }"#;
        let msg: ExtensionMessage = serde_json::from_str(json_str).unwrap();
        match msg {
            ExtensionMessage::Download {
                format,
                options,
                cookies,
                source_platform,
                ..
            } => {
                assert_eq!(format, None);
                assert!(options.is_none());
                assert!(cookies.is_none());
                assert_eq!(source_platform, None);
            }
            _ => panic!("expected Download variant"),
        }
    }

    #[test]
    fn test_daemon_hello_serialization() {
        use crate::libraries::{LibraryInfo, LibraryStatus};

        let msg = DaemonMessage::Hello {
            version: "0.5.0".to_string(),
            requires_pairing: true,
            identifier: Some("my-pc".to_string()),
            libraries: LibraryStatus {
                ytdlp: LibraryInfo {
                    installed: true,
                    version: Some("2024.01.01".to_string()),
                    path: None,
                    update_available: None,
                },
                ffmpeg: LibraryInfo {
                    installed: true,
                    version: Some("6.0".to_string()),
                    path: None,
                    update_available: None,
                },
                bun: LibraryInfo {
                    installed: false,
                    version: None,
                    path: None,
                    update_available: None,
                },
            },
            libraries_installed: true,
        };

        let json: Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "hello");
        assert_eq!(json["version"], "0.5.0");
        assert_eq!(json["requires_pairing"], true);
        assert_eq!(json["identifier"], "my-pc");
        assert_eq!(json["libraries"]["ytdlp"]["installed"], true);
        assert_eq!(json["libraries"]["bun"]["installed"], false);
        assert_eq!(json["libraries_installed"], true);
    }

    #[test]
    fn test_daemon_download_progress_serialization() {
        let msg = DaemonMessage::DownloadProgress {
            download_id: "dl-1".to_string(),
            status: "downloading".to_string(),
            percent: 45.5,
            speed: Some("2.5 MiB/s".to_string()),
            eta: Some(120),
            downloaded_bytes: 50_000_000,
            total_bytes: Some(110_000_000),
        };

        let json: Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "download_progress");
        assert_eq!(json["download_id"], "dl-1");
        assert_eq!(json["status"], "downloading");
        assert_eq!(json["percent"], 45.5);
        assert_eq!(json["speed"], "2.5 MiB/s");
        assert_eq!(json["eta"], 120);
        assert_eq!(json["downloaded_bytes"], 50_000_000);
        assert_eq!(json["total_bytes"], 110_000_000);
    }

    #[test]
    fn test_daemon_download_progress_omits_none() {
        let msg = DaemonMessage::DownloadProgress {
            download_id: "dl-2".to_string(),
            status: "downloading".to_string(),
            percent: 10.0,
            speed: None,
            eta: None,
            downloaded_bytes: 1000,
            total_bytes: None,
        };

        let json_str = serde_json::to_string(&msg).unwrap();
        assert!(!json_str.contains("speed"));
        assert!(!json_str.contains("eta"));
        assert!(!json_str.contains("total_bytes"));
    }

    #[test]
    fn test_daemon_error_serialization() {
        let msg = DaemonMessage::Error {
            id: Some("req-5".to_string()),
            code: "not_found".to_string(),
            message: "Download not found".to_string(),
        };

        let json: Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "error");
        assert_eq!(json["id"], "req-5");
        assert_eq!(json["code"], "not_found");
        assert_eq!(json["message"], "Download not found");
    }

    #[test]
    fn test_daemon_error_without_id() {
        let msg = DaemonMessage::Error {
            id: None,
            code: "internal".to_string(),
            message: "Something went wrong".to_string(),
        };

        let json_str = serde_json::to_string(&msg).unwrap();
        // id should be omitted when None
        let json: Value = serde_json::from_str(&json_str).unwrap();
        assert!(json.get("id").is_none());
        assert_eq!(json["code"], "internal");
    }

    #[test]
    fn test_extension_ping_and_daemon_pong() {
        let json_str = r#"{"type": "ping"}"#;
        let msg: ExtensionMessage = serde_json::from_str(json_str).unwrap();
        assert!(matches!(msg, ExtensionMessage::Ping));

        let pong = DaemonMessage::Pong;
        let json: Value = serde_json::to_value(&pong).unwrap();
        assert_eq!(json["type"], "pong");
    }

    #[test]
    fn test_extension_get_queue() {
        let json_str = r#"{"type": "get_queue"}"#;
        let msg: ExtensionMessage = serde_json::from_str(json_str).unwrap();
        assert!(matches!(msg, ExtensionMessage::GetQueue));
    }

    #[test]
    fn test_extension_merge_channels() {
        let json_str = r#"{
            "type": "merge_channels",
            "id": "req-10",
            "platform": "twitch",
            "source": "old_name",
            "target": "new_name",
            "include_recordings": true
        }"#;
        let msg: ExtensionMessage = serde_json::from_str(json_str).unwrap();
        match msg {
            ExtensionMessage::MergeChannels {
                id,
                platform,
                source,
                target,
                include_recordings,
            } => {
                assert_eq!(id, "req-10");
                assert_eq!(platform, "twitch");
                assert_eq!(source, "old_name");
                assert_eq!(target, "new_name");
                assert!(include_recordings);
            }
            _ => panic!("expected MergeChannels variant"),
        }
    }

    #[test]
    fn test_extension_remove_channel() {
        let json_str = r#"{
            "type": "remove_channel",
            "id": "req-11",
            "channel_id": "550e8400-e29b-41d4-a716-446655440000"
        }"#;
        let msg: ExtensionMessage = serde_json::from_str(json_str).unwrap();
        match msg {
            ExtensionMessage::RemoveChannel { id, channel_id } => {
                assert_eq!(id, "req-11");
                assert_eq!(channel_id, "550e8400-e29b-41d4-a716-446655440000");
            }
            _ => panic!("expected RemoveChannel variant"),
        }
    }

    #[test]
    fn test_daemon_channel_removed_serialization() {
        let msg = DaemonMessage::ChannelRemoved {
            id: "req-11".to_string(),
            channel_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            name: "testchannel".to_string(),
            platform: "twitch".to_string(),
        };

        let json: Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "channel_removed");
        assert_eq!(json["id"], "req-11");
        assert_eq!(json["channel_id"], "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(json["name"], "testchannel");
        assert_eq!(json["platform"], "twitch");
    }

    #[test]
    fn test_daemon_channels_state_serialization() {
        let msg = DaemonMessage::ChannelsState {
            channels: vec![
                ChannelSummary {
                    channel_id: "id-1".to_string(),
                    name: "streamer1".to_string(),
                    platform: "twitch".to_string(),
                    enabled: true,
                    status: "live".to_string(),
                    profile_image_url: Some("https://example.com/img.png".to_string()),
                },
                ChannelSummary {
                    channel_id: "id-2".to_string(),
                    name: "streamer2".to_string(),
                    platform: "kick".to_string(),
                    enabled: false,
                    status: "offline".to_string(),
                    profile_image_url: None,
                },
            ],
        };

        let json: Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "channels_state");
        assert_eq!(json["channels"].as_array().unwrap().len(), 2);
        assert_eq!(json["channels"][0]["name"], "streamer1");
        assert_eq!(json["channels"][0]["platform"], "twitch");
        assert_eq!(json["channels"][0]["enabled"], true);
        assert_eq!(json["channels"][0]["status"], "live");
        assert_eq!(json["channels"][0]["profile_image_url"], "https://example.com/img.png");
        assert_eq!(json["channels"][1]["name"], "streamer2");
        assert_eq!(json["channels"][1]["enabled"], false);
        assert!(json["channels"][1].get("profile_image_url").is_none());
    }
}
