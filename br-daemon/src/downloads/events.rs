use std::path::PathBuf;
use uuid::Uuid;

use super::job::DownloadJobSummary;

/// Internal download events, bridged to ManagerEvent in Phase 4.
#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Queued {
        job: DownloadJobSummary,
    },
    Progress {
        download_id: Uuid,
        percent: f64,
        speed: Option<String>,
        eta: Option<u64>,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Complete {
        download_id: Uuid,
        channel_name: String,
        filepath: PathBuf,
        filesize: u64,
    },
    Failed {
        download_id: Uuid,
        channel_name: String,
        error: String,
        update_available: bool,
    },
    Paused {
        download_id: Uuid,
    },
    Resumed {
        download_id: Uuid,
    },
    Cancelled {
        download_id: Uuid,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloads::job::{DownloadJobSummary, DownloadStatus};
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_summary() -> DownloadJobSummary {
        DownloadJobSummary {
            id: Uuid::new_v4(),
            url: "https://youtube.com/watch?v=test".to_string(),
            title: Some("Test Video".to_string()),
            thumbnail: None,
            platform_name: Some("youtube".to_string()),
            channel_name: "test_channel".to_string(),
            source_platform: "youtube".to_string(),
            status: DownloadStatus::Queued,
            percent: 0.0,
            speed: None,
            eta: None,
            downloaded_bytes: 0,
            total_bytes: None,
            format: None,
            quality: None,
            requested_by: Uuid::new_v4(),
            requested_by_name: None,
            created_at: Utc::now(),
            completed_at: None,
            error: None,
            update_available: false,
        }
    }

    #[test]
    fn download_event_can_be_cloned() {
        let id = Uuid::new_v4();

        let queued = DownloadEvent::Queued {
            job: make_summary(),
        };
        let _ = queued.clone();

        let progress = DownloadEvent::Progress {
            download_id: id,
            percent: 42.5,
            speed: Some("2.1MiB/s".to_string()),
            eta: Some(60),
            downloaded_bytes: 1_000_000,
            total_bytes: Some(10_000_000),
        };
        let _ = progress.clone();

        let complete = DownloadEvent::Complete {
            download_id: id,
            channel_name: "test_channel".to_string(),
            filepath: PathBuf::from("/downloads/video.mp4"),
            filesize: 10_000_000,
        };
        let _ = complete.clone();

        let failed = DownloadEvent::Failed {
            download_id: id,
            channel_name: "test_channel".to_string(),
            error: "Network error".to_string(),
            update_available: false,
        };
        let _ = failed.clone();

        let _ = DownloadEvent::Paused { download_id: id }.clone();
        let _ = DownloadEvent::Resumed { download_id: id }.clone();
        let _ = DownloadEvent::Cancelled { download_id: id }.clone();
    }

    #[test]
    fn download_event_can_be_debug_formatted() {
        let id = Uuid::new_v4();

        let event = DownloadEvent::Progress {
            download_id: id,
            percent: 75.0,
            speed: Some("5MiB/s".to_string()),
            eta: Some(30),
            downloaded_bytes: 7_500_000,
            total_bytes: Some(10_000_000),
        };
        let s = format!("{:?}", event);
        assert!(s.contains("Progress"));
        assert!(s.contains("75"));

        let event = DownloadEvent::Queued {
            job: make_summary(),
        };
        let s = format!("{:?}", event);
        assert!(s.contains("Queued"));

        let event = DownloadEvent::Failed {
            download_id: id,
            channel_name: "test_channel".to_string(),
            error: "timeout".to_string(),
            update_available: true,
        };
        let s = format!("{:?}", event);
        assert!(s.contains("Failed"));
        assert!(s.contains("timeout"));
    }
}
