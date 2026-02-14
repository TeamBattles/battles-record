use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Queued,
    ExtractingInfo,
    WaitingForFormat,
    Downloading,
    Processing,
    Paused,
    Complete,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadJob {
    pub id: Uuid,
    pub url: String,
    pub title: Option<String>,
    pub thumbnail: Option<String>,
    pub duration: Option<u64>,
    pub uploader: Option<String>,
    pub platform_name: Option<String>,

    // Storage target
    pub channel_name: String,
    pub source_platform: String,
    pub output_dir: PathBuf,
    pub output_file: Option<PathBuf>,

    // Format selection
    pub format: Option<String>,
    pub quality: Option<String>,
    pub available_formats: Option<Vec<FormatInfo>>,

    // Progress
    pub status: DownloadStatus,
    pub percent: f64,
    pub speed: Option<String>,
    pub eta: Option<u64>,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,

    // Options
    pub options: DownloadOptions,

    // Metadata
    pub requested_by: Uuid,
    pub requested_by_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

/// Lightweight summary excluding `available_formats` and cookies.
/// Used in manager events, queue state responses, and REST endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadJobSummary {
    pub id: Uuid,
    pub url: String,
    pub title: Option<String>,
    pub thumbnail: Option<String>,
    pub platform_name: Option<String>,
    pub channel_name: String,
    pub source_platform: String,
    pub status: DownloadStatus,
    pub percent: f64,
    pub speed: Option<String>,
    pub eta: Option<u64>,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub format: Option<String>,
    pub quality: Option<String>,
    pub requested_by: Uuid,
    pub requested_by_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    #[serde(default)]
    pub update_available: bool,
}

impl From<&DownloadJob> for DownloadJobSummary {
    fn from(job: &DownloadJob) -> Self {
        Self {
            id: job.id,
            url: job.url.clone(),
            title: job.title.clone(),
            thumbnail: job.thumbnail.clone(),
            platform_name: job.platform_name.clone(),
            channel_name: job.channel_name.clone(),
            source_platform: job.source_platform.clone(),
            status: job.status,
            percent: job.percent,
            speed: job.speed.clone(),
            eta: job.eta,
            downloaded_bytes: job.downloaded_bytes,
            total_bytes: job.total_bytes,
            format: job.format.clone(),
            quality: job.quality.clone(),
            requested_by: job.requested_by,
            requested_by_name: job.requested_by_name.clone(),
            created_at: job.created_at,
            completed_at: job.completed_at,
            error: job.error.clone(),
            update_available: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatInfo {
    pub format_id: String,
    pub ext: String,
    pub resolution: Option<String>,
    pub filesize_approx: Option<u64>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub fps: Option<f64>,
    pub tbr: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadOptions {
    #[serde(default = "crate::config::default_true")]
    pub embed_thumbnail: bool,
    #[serde(default = "crate::config::default_true")]
    pub embed_metadata: bool,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            embed_thumbnail: true,
            embed_metadata: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedInfo {
    pub title: String,
    pub duration: Option<u64>,
    pub thumbnail: Option<String>,
    pub uploader: Option<String>,
    pub platform_name: Option<String>,
    pub formats: Vec<FormatInfo>,
    pub webpage_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadRequest {
    pub url: String,
    pub title: Option<String>,
    pub channel_name: String,
    pub source_platform: String,
    pub format: Option<String>,
    pub quality: Option<String>,
    pub options: Option<DownloadOptions>,
    pub cookies: Option<Vec<CookieData>>,
    pub requested_by: Uuid,
    pub requested_by_name: Option<String>,
    #[serde(default)]
    pub auto_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieData {
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub expiration_date: f64,
    pub http_only: bool,
    pub name: String,
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_status_serializes_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&DownloadStatus::Queued).unwrap(),
            "\"queued\""
        );
        assert_eq!(
            serde_json::to_string(&DownloadStatus::ExtractingInfo).unwrap(),
            "\"extracting_info\""
        );
        assert_eq!(
            serde_json::to_string(&DownloadStatus::WaitingForFormat).unwrap(),
            "\"waiting_for_format\""
        );
        assert_eq!(
            serde_json::to_string(&DownloadStatus::Complete).unwrap(),
            "\"complete\""
        );
    }

    #[test]
    fn download_status_deserializes_from_snake_case() {
        let status: DownloadStatus = serde_json::from_str("\"extracting_info\"").unwrap();
        assert_eq!(status, DownloadStatus::ExtractingInfo);

        let status: DownloadStatus = serde_json::from_str("\"waiting_for_format\"").unwrap();
        assert_eq!(status, DownloadStatus::WaitingForFormat);
    }

    #[test]
    fn download_options_defaults_to_true() {
        let opts = DownloadOptions::default();
        assert!(opts.embed_thumbnail);
        assert!(opts.embed_metadata);
    }

    #[test]
    fn download_options_serde_defaults() {
        let json = "{}";
        let opts: DownloadOptions = serde_json::from_str(json).unwrap();
        assert!(opts.embed_thumbnail);
        assert!(opts.embed_metadata);
    }

    #[test]
    fn download_job_summary_from_job() {
        let job = DownloadJob {
            id: Uuid::new_v4(),
            url: "https://youtube.com/watch?v=test".to_string(),
            title: Some("Test Video".to_string()),
            thumbnail: Some("https://img.youtube.com/thumb.jpg".to_string()),
            duration: Some(300),
            uploader: Some("TestChannel".to_string()),
            platform_name: Some("youtube".to_string()),
            channel_name: "test_channel".to_string(),
            source_platform: "youtube".to_string(),
            output_dir: PathBuf::from("/downloads/youtube/test_channel"),
            output_file: None,
            format: Some("bestvideo+bestaudio".to_string()),
            quality: Some("1080p".to_string()),
            available_formats: Some(vec![FormatInfo {
                format_id: "137".to_string(),
                ext: "mp4".to_string(),
                resolution: Some("1920x1080".to_string()),
                filesize_approx: Some(500_000_000),
                vcodec: Some("avc1".to_string()),
                acodec: None,
                fps: Some(30.0),
                tbr: Some(4000.0),
            }]),
            status: DownloadStatus::Downloading,
            percent: 45.5,
            speed: Some("5.2MiB/s".to_string()),
            eta: Some(120),
            downloaded_bytes: 225_000_000,
            total_bytes: Some(500_000_000),
            options: DownloadOptions::default(),
            requested_by: Uuid::new_v4(),
            requested_by_name: Some("Chrome".to_string()),
            created_at: Utc::now(),
            completed_at: None,
            error: None,
        };

        let summary = DownloadJobSummary::from(&job);

        assert_eq!(summary.id, job.id);
        assert_eq!(summary.url, job.url);
        assert_eq!(summary.title, job.title);
        assert_eq!(summary.thumbnail, job.thumbnail);
        assert_eq!(summary.platform_name, job.platform_name);
        assert_eq!(summary.channel_name, job.channel_name);
        assert_eq!(summary.source_platform, job.source_platform);
        assert_eq!(summary.status, DownloadStatus::Downloading);
        assert!((summary.percent - 45.5).abs() < f64::EPSILON);
        assert_eq!(summary.speed, job.speed);
        assert_eq!(summary.eta, job.eta);
        assert_eq!(summary.downloaded_bytes, 225_000_000);
        assert_eq!(summary.total_bytes, Some(500_000_000));
        assert_eq!(summary.format, job.format);
        assert_eq!(summary.requested_by, job.requested_by);
        assert_eq!(summary.created_at, job.created_at);
        assert_eq!(summary.completed_at, None);
        assert_eq!(summary.error, None);
        assert!(!summary.update_available);
    }

    #[test]
    fn summary_update_available_defaults_false_on_deserialize() {
        // Simulate JSON without the update_available field
        let json = serde_json::json!({
            "id": Uuid::new_v4(),
            "url": "https://example.com",
            "title": null,
            "thumbnail": null,
            "platform_name": null,
            "channel_name": "ch",
            "source_platform": "youtube",
            "status": "queued",
            "percent": 0.0,
            "speed": null,
            "eta": null,
            "downloaded_bytes": 0,
            "total_bytes": null,
            "format": null,
            "requested_by": Uuid::new_v4(),
            "created_at": Utc::now(),
            "completed_at": null,
            "error": null
        });

        let summary: DownloadJobSummary = serde_json::from_value(json).unwrap();
        assert!(!summary.update_available);
    }
}
