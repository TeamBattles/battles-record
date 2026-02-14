//! Cleanup worker for managing download storage and retention.

use std::path::Path;

use chrono::Utc;
use uuid::Uuid;

use super::index::{self, DownloadsIndex};
use super::job::DownloadStatus;

/// Worker for cleaning up old downloads based on retention policies.
pub struct DownloadCleanupWorker;

impl DownloadCleanupWorker {
    /// Find downloads eligible for cleanup based on retention.
    /// Only `complete`, `cancelled`, and `failed` jobs are eligible.
    /// `queued`, `extracting_info`, `waiting_for_format`, `downloading`, `processing`, and `paused` are PROTECTED.
    pub fn find_expired(index: &DownloadsIndex, max_age_days: u32) -> Vec<Uuid> {
        let cutoff = Utc::now() - chrono::Duration::days(max_age_days as i64);
        index
            .list()
            .into_iter()
            .filter(|job| {
                Self::is_cleanable(job.status)
                    && job.completed_at.map_or(false, |t| t < cutoff)
            })
            .map(|job| job.id)
            .collect()
    }

    fn is_cleanable(status: DownloadStatus) -> bool {
        matches!(
            status,
            DownloadStatus::Complete | DownloadStatus::Cancelled | DownloadStatus::Failed
        )
    }

    /// Delete download files and remove from index.
    pub async fn delete_download(
        index: &mut DownloadsIndex,
        id: Uuid,
    ) -> Result<(), CleanupError> {
        if let Some(job) = index.get(&id) {
            if let Some(ref output_file) = job.output_file {
                if output_file.exists() {
                    tokio::fs::remove_file(output_file)
                        .await
                        .map_err(|e| CleanupError::Io {
                            context: format!("deleting {}", output_file.display()),
                            source: e,
                        })?;
                }
            }
            let output_dir = job.output_dir.clone();
            if output_dir.exists() {
                Self::cleanup_part_files(&output_dir).await?;
            }
        }
        index.remove(&id);
        Ok(())
    }

    /// Remove .part files (leftover from cancelled/failed downloads).
    async fn cleanup_part_files(dir: &Path) -> Result<(), CleanupError> {
        let mut entries = tokio::fs::read_dir(dir)
            .await
            .map_err(|e| CleanupError::Io {
                context: format!("reading {}", dir.display()),
                source: e,
            })?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| CleanupError::Io {
                context: "reading directory entry".into(),
                source: e,
            })?
        {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "part") {
                let _ = tokio::fs::remove_file(&path).await;
            }
        }
        Ok(())
    }

    /// Run a full cleanup pass, deleting all jobs that have exceeded the retention period.
    /// Returns the number of jobs deleted.
    pub async fn run_cleanup(
        index: &mut DownloadsIndex,
        max_age_days: Option<u32>,
    ) -> Result<u32, CleanupError> {
        let Some(max_age) = max_age_days else {
            return Ok(0);
        };

        let expired = Self::find_expired(index, max_age);
        let count = expired.len() as u32;

        for id in expired {
            if let Err(e) = Self::delete_download(index, id).await {
                tracing::warn!(error = ?e, download_id = %id, "Failed to clean up download");
            }
        }

        if count > 0 {
            index.force_save().await.map_err(CleanupError::Index)?;
        }

        Ok(count)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum CleanupError {
    #[error("IO error: {context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Index error: {0}")]
    Index(#[from] index::IndexError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloads::job::{DownloadJob, DownloadOptions};
    use chrono::{DateTime, Duration};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_job_with_time(
        status: DownloadStatus,
        completed_at: Option<DateTime<Utc>>,
        output_dir: PathBuf,
    ) -> DownloadJob {
        DownloadJob {
            id: Uuid::new_v4(),
            url: "https://example.com/video".to_string(),
            title: Some("Test Video".to_string()),
            thumbnail: None,
            duration: Some(300),
            uploader: None,
            platform_name: None,
            channel_name: "streamer".to_string(),
            source_platform: "youtube".to_string(),
            output_dir,
            output_file: None,
            format: None,
            quality: None,
            available_formats: None,
            status,
            percent: 100.0,
            speed: None,
            eta: None,
            downloaded_bytes: 1000,
            total_bytes: Some(1000),
            options: DownloadOptions::default(),
            requested_by: Uuid::new_v4(),
            requested_by_name: None,
            created_at: Utc::now() - Duration::hours(2),
            completed_at,
            error: None,
        }
    }

    #[test]
    fn is_cleanable_complete() {
        assert!(DownloadCleanupWorker::is_cleanable(DownloadStatus::Complete));
    }

    #[test]
    fn is_cleanable_cancelled() {
        assert!(DownloadCleanupWorker::is_cleanable(DownloadStatus::Cancelled));
    }

    #[test]
    fn is_cleanable_failed() {
        assert!(DownloadCleanupWorker::is_cleanable(DownloadStatus::Failed));
    }

    #[test]
    fn is_not_cleanable_queued() {
        assert!(!DownloadCleanupWorker::is_cleanable(DownloadStatus::Queued));
    }

    #[test]
    fn is_not_cleanable_extracting_info() {
        assert!(!DownloadCleanupWorker::is_cleanable(
            DownloadStatus::ExtractingInfo
        ));
    }

    #[test]
    fn is_not_cleanable_waiting_for_format() {
        assert!(!DownloadCleanupWorker::is_cleanable(
            DownloadStatus::WaitingForFormat
        ));
    }

    #[test]
    fn is_not_cleanable_downloading() {
        assert!(!DownloadCleanupWorker::is_cleanable(
            DownloadStatus::Downloading
        ));
    }

    #[test]
    fn is_not_cleanable_processing() {
        assert!(!DownloadCleanupWorker::is_cleanable(
            DownloadStatus::Processing
        ));
    }

    #[test]
    fn is_not_cleanable_paused() {
        assert!(!DownloadCleanupWorker::is_cleanable(DownloadStatus::Paused));
    }

    #[tokio::test]
    async fn find_expired_returns_only_eligible_expired_jobs() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();

        let now = Utc::now();
        let old = now - Duration::days(40);
        let recent = now - Duration::days(5);

        // Old complete - should be returned
        let old_complete =
            make_job_with_time(DownloadStatus::Complete, Some(old), dir.path().to_path_buf());
        let old_complete_id = old_complete.id;
        index.add(old_complete);

        // Old cancelled - should be returned
        let old_cancelled = make_job_with_time(
            DownloadStatus::Cancelled,
            Some(old),
            dir.path().to_path_buf(),
        );
        let old_cancelled_id = old_cancelled.id;
        index.add(old_cancelled);

        // Old failed - should be returned
        let old_failed =
            make_job_with_time(DownloadStatus::Failed, Some(old), dir.path().to_path_buf());
        let old_failed_id = old_failed.id;
        index.add(old_failed);

        // Recent complete - NOT expired
        let recent_complete = make_job_with_time(
            DownloadStatus::Complete,
            Some(recent),
            dir.path().to_path_buf(),
        );
        let recent_complete_id = recent_complete.id;
        index.add(recent_complete);

        // Old but active (downloading) - PROTECTED
        let old_downloading = make_job_with_time(
            DownloadStatus::Downloading,
            Some(old),
            dir.path().to_path_buf(),
        );
        let old_downloading_id = old_downloading.id;
        index.add(old_downloading);

        // Old but paused - PROTECTED
        let old_paused =
            make_job_with_time(DownloadStatus::Paused, Some(old), dir.path().to_path_buf());
        let old_paused_id = old_paused.id;
        index.add(old_paused);

        let expired = DownloadCleanupWorker::find_expired(&index, 30);

        assert_eq!(expired.len(), 3);
        assert!(expired.contains(&old_complete_id));
        assert!(expired.contains(&old_cancelled_id));
        assert!(expired.contains(&old_failed_id));
        assert!(!expired.contains(&recent_complete_id));
        assert!(!expired.contains(&old_downloading_id));
        assert!(!expired.contains(&old_paused_id));
    }

    #[tokio::test]
    async fn find_expired_no_completed_at_excluded() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();

        // Complete status but no completed_at timestamp - should NOT be returned
        let job = make_job_with_time(DownloadStatus::Complete, None, dir.path().to_path_buf());
        index.add(job);

        let expired = DownloadCleanupWorker::find_expired(&index, 30);
        assert!(expired.is_empty());
    }

    #[tokio::test]
    async fn run_cleanup_with_none_returns_zero() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();

        let old = Utc::now() - Duration::days(100);
        let job =
            make_job_with_time(DownloadStatus::Complete, Some(old), dir.path().to_path_buf());
        index.add(job);

        let count = DownloadCleanupWorker::run_cleanup(&mut index, None)
            .await
            .unwrap();

        assert_eq!(count, 0);
        // Job should still be in index
        assert_eq!(index.list().len(), 1);
    }

    #[tokio::test]
    async fn run_cleanup_deletes_expired_jobs() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();

        let old = Utc::now() - Duration::days(40);
        let recent = Utc::now() - Duration::days(5);

        let old_job =
            make_job_with_time(DownloadStatus::Complete, Some(old), dir.path().to_path_buf());
        let old_id = old_job.id;
        index.add(old_job);

        let recent_job = make_job_with_time(
            DownloadStatus::Complete,
            Some(recent),
            dir.path().to_path_buf(),
        );
        let recent_id = recent_job.id;
        index.add(recent_job);

        let count = DownloadCleanupWorker::run_cleanup(&mut index, Some(30))
            .await
            .unwrap();

        assert_eq!(count, 1);
        assert!(index.get(&old_id).is_none());
        assert!(index.get(&recent_id).is_some());
    }

    #[tokio::test]
    async fn delete_download_removes_part_files() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();

        // Create a .part file in the output directory
        let part_file = dir.path().join("video.part");
        tokio::fs::write(&part_file, b"partial data").await.unwrap();

        let mut job =
            make_job_with_time(DownloadStatus::Cancelled, Some(Utc::now()), dir.path().to_path_buf());
        // output_dir points to the temp dir (where the .part file lives)
        job.output_dir = dir.path().to_path_buf();
        let id = job.id;
        index.add(job);

        DownloadCleanupWorker::delete_download(&mut index, id)
            .await
            .unwrap();

        assert!(index.get(&id).is_none());
        assert!(!part_file.exists());
    }
}
