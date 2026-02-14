//! Cleanup worker for managing recording storage and retention.

use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use uuid::Uuid;

use crate::config::RetentionConfig;
use crate::storage::{RecordingEntry, RecordingStatus, RecordingsIndex};

/** Worker for cleaning up old recordings based on retention policies. */
pub struct CleanupWorker;

impl CleanupWorker {
    /**
     * Find recordings that have exceeded the retention period.
     *
     * Returns Vec<Uuid> of recording IDs to delete.
     * Respects keep_minimum per channel.
     * Never returns active recordings (Recording, Processing status).
     */
    pub fn find_expired_recordings(
        index: &RecordingsIndex,
        retention: &RetentionConfig,
    ) -> Vec<Uuid> {
        // If no max age is set, nothing expires
        let max_age_days = match retention.max_age_days {
            Some(days) => days,
            None => return Vec::new(),
        };

        let cutoff = Utc::now() - Duration::days(max_age_days as i64);
        let keep_minimum = retention.keep_minimum;

        // Get all recordings and group by channel
        let all_recordings = index.list(None, None, None);
        let mut by_channel: HashMap<String, Vec<&RecordingEntry>> = HashMap::new();

        for recording in all_recordings {
            by_channel
                .entry(recording.channel_name.clone())
                .or_default()
                .push(recording);
        }

        let mut to_delete = Vec::new();

        for (_channel, mut recordings) in by_channel {
            // Sort by started_at (oldest first)
            recordings.sort_by(|a, b| a.started_at.cmp(&b.started_at));

            // Filter out active recordings (Recording or Processing)
            let inactive_recordings: Vec<_> = recordings
                .iter()
                .filter(|r| !Self::is_active(r.status))
                .collect();

            // Find expired recordings (older than cutoff)
            let expired: Vec<_> = inactive_recordings
                .iter()
                .filter(|r| r.started_at < cutoff)
                .collect();

            // How many inactive recordings we have total
            let total_inactive = inactive_recordings.len();

            // How many expired recordings we can delete while respecting keep_minimum
            // We need to keep at least keep_minimum inactive recordings per channel
            let can_delete = total_inactive.saturating_sub(keep_minimum as usize);

            // Delete oldest expired recordings up to can_delete limit
            for (i, recording) in expired.iter().enumerate() {
                if i >= can_delete {
                    break;
                }
                to_delete.push(recording.id);
            }
        }

        to_delete
    }

    /**
     * Find recordings that have exceeded a channel-specific retention period.
     *
     * Returns Vec<Uuid> of recording IDs to delete.
     * Respects keep_minimum.
     * Never returns active recordings.
     */
    pub fn find_channel_expired_recordings(
        index: &RecordingsIndex,
        channel: &str,
        retention_days: u32,
        keep_minimum: u32,
    ) -> Vec<Uuid> {
        let cutoff = Utc::now() - Duration::days(retention_days as i64);

        let recordings = index.list(Some(channel), None, None);
        let mut inactive: Vec<_> = recordings
            .into_iter()
            .filter(|r| !Self::is_active(r.status))
            .collect();

        // Sort by started_at (oldest first)
        inactive.sort_by(|a, b| a.started_at.cmp(&b.started_at));

        let expired: Vec<_> = inactive.iter().filter(|r| r.started_at < cutoff).collect();

        let can_delete = inactive.len().saturating_sub(keep_minimum as usize);

        expired.iter().take(can_delete).map(|r| r.id).collect()
    }

    /**
     * Find recordings to delete when channel quota is exceeded.
     *
     * Deletes oldest first until under quota.
     * Respects keep_minimum.
     * Never returns active recordings.
     */
    pub fn find_quota_exceeded_recordings(
        index: &RecordingsIndex,
        channel: &str,
        _platform: &str,
        quota_gb: u64,
        keep_minimum: u32,
    ) -> Vec<Uuid> {
        const BYTES_PER_GB: u64 = 1024 * 1024 * 1024;
        let quota_bytes = quota_gb * BYTES_PER_GB;

        // Get all recordings for this channel
        let recordings = index.list(Some(channel), None, None);

        // Filter out active recordings and collect
        let mut inactive: Vec<_> = recordings
            .into_iter()
            .filter(|r| !Self::is_active(r.status))
            .collect();

        // Sort by started_at (oldest first) so we delete oldest
        inactive.sort_by(|a, b| a.started_at.cmp(&b.started_at));

        // Calculate current total size for the channel
        let current_size: u64 = index.channel_size(channel);

        // If under quota, nothing to delete
        if current_size <= quota_bytes {
            return Vec::new();
        }

        let bytes_to_free = current_size - quota_bytes;
        let mut freed_bytes = 0u64;
        let mut to_delete = Vec::new();

        // We must keep at least keep_minimum recordings
        let max_deletable = inactive.len().saturating_sub(keep_minimum as usize);

        for recording in inactive.iter().take(max_deletable) {
            if freed_bytes >= bytes_to_free {
                break;
            }
            to_delete.push(recording.id);
            freed_bytes += recording.size_bytes;
        }

        to_delete
    }

    /**
     * Delete a recording's files from disk.
     *
     * Returns freed bytes.
     */
    pub async fn delete_recording_files(
        base_path: &Path,
        recording: &RecordingEntry,
    ) -> std::io::Result<u64> {
        let recording_path = base_path.join(&recording.path);
        let mut freed_bytes = 0u64;

        // Delete the main recording directory if it exists
        if recording_path.exists() {
            freed_bytes += Self::calculate_dir_size(&recording_path).await?;
            fs::remove_dir_all(&recording_path).await?;
        }

        // Delete the output file if it exists and is separate from the recording path
        if let Some(ref output_file) = recording.output_file {
            let output_path = base_path.join(output_file);
            if output_path.exists() {
                let metadata = fs::metadata(&output_path).await?;
                freed_bytes += metadata.len();
                fs::remove_file(&output_path).await?;
            }
        }

        Ok(freed_bytes)
    }

    /**
     * Delete Jellyfin library files for a recording.
     *
     * Deletes:
     * - Video file at jellyfin_path (.mkv, .mp4, .ts)
     * - NFO file (same path, .nfo extension)
     * - Thumbnail file (base name + -thumb.jpg)
     *
     * Also cleans up empty season directories (but preserves show directory with metadata).
     *
     * Returns bytes freed.
     */
    pub async fn delete_jellyfin_files(recording: &RecordingEntry) -> std::io::Result<u64> {
        // Check if recording was exported to Jellyfin
        if !recording.jellyfin_exported {
            return Ok(0);
        }

        let jellyfin_path = match &recording.jellyfin_path {
            Some(path) => path,
            None => return Ok(0),
        };

        let mut freed_bytes = 0u64;

        // Delete the main video file
        if jellyfin_path.exists() {
            if let Ok(metadata) = fs::metadata(jellyfin_path).await {
                freed_bytes += metadata.len();
            }
            if let Err(e) = fs::remove_file(jellyfin_path).await {
                tracing::warn!(
                    "Failed to delete Jellyfin video file {:?}: {}",
                    jellyfin_path,
                    e
                );
            }
        }

        // Delete the NFO file (same path but with .nfo extension)
        let nfo_path = jellyfin_path.with_extension("nfo");
        if nfo_path.exists() {
            if let Ok(metadata) = fs::metadata(&nfo_path).await {
                freed_bytes += metadata.len();
            }
            if let Err(e) = fs::remove_file(&nfo_path).await {
                tracing::warn!("Failed to delete Jellyfin NFO file {:?}: {}", nfo_path, e);
            }
        }

        // Delete the thumbnail file (base name + -thumb.jpg)
        // Example: "channel - S01E01 - title.mp4" -> "channel - S01E01 - title-thumb.jpg"
        if let Some(stem) = jellyfin_path.file_stem().and_then(|s| s.to_str()) {
            let parent = jellyfin_path.parent();
            if let Some(parent_dir) = parent {
                let thumb_path = parent_dir.join(format!("{}-thumb.jpg", stem));
                if thumb_path.exists() {
                    if let Ok(metadata) = fs::metadata(&thumb_path).await {
                        freed_bytes += metadata.len();
                    }
                    if let Err(e) = fs::remove_file(&thumb_path).await {
                        tracing::warn!(
                            "Failed to delete Jellyfin thumbnail {:?}: {}",
                            thumb_path,
                            e
                        );
                    }
                }
            }
        }

        // Clean up empty season directory (but NOT the show directory)
        if let Some(season_dir) = jellyfin_path.parent() {
            if Self::is_directory_empty(season_dir).await {
                if let Err(e) = fs::remove_dir(season_dir).await {
                    tracing::debug!("Could not remove season directory {:?}: {}", season_dir, e);
                } else {
                    tracing::debug!("Removed empty season directory {:?}", season_dir);
                }
            }
        }

        Ok(freed_bytes)
    }

    /** Check if a directory is empty (no files or subdirectories). */
    async fn is_directory_empty(path: &Path) -> bool {
        if !path.exists() {
            return true;
        }

        match fs::read_dir(path).await {
            Ok(mut entries) => {
                // If we can get even one entry, it's not empty
                matches!(entries.next_entry().await, Ok(None))
            }
            Err(_) => false,
        }
    }

    /** Check if a recording status is considered "active" (should not be deleted). */
    fn is_active(status: RecordingStatus) -> bool {
        matches!(
            status,
            RecordingStatus::Recording | RecordingStatus::Processing
        )
    }

    /** Calculate total size of a directory recursively. */
    async fn calculate_dir_size(path: &Path) -> std::io::Result<u64> {
        let mut total = 0u64;
        let mut entries = fs::read_dir(path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;
            if metadata.is_dir() {
                total += Box::pin(Self::calculate_dir_size(&entry.path())).await?;
            } else {
                total += metadata.len();
            }
        }

        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Platform;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_entry(
        id: Uuid,
        channel_name: &str,
        platform: Platform,
        status: RecordingStatus,
        started_at: chrono::DateTime<Utc>,
        size_bytes: u64,
    ) -> RecordingEntry {
        RecordingEntry {
            id,
            channel_name: channel_name.to_string(),
            platform,
            started_at,
            ended_at: Some(started_at + Duration::hours(1)),
            duration_secs: Some(3600),
            status,
            path: PathBuf::from(format!("recordings/{}", id)),
            size_bytes,
            segment_count: 100,
            title: Some("Test Stream".to_string()),
            game: Some("Test Game".to_string()),
            output_file: None,
            processed_at: None,
            processing_attempts: 0,
            failure_reason: None,
            jellyfin_exported: false,
            jellyfin_path: None,
            thumbnail_url: None,
        }
    }

    #[test]
    fn test_cleanup_by_retention() {
        let temp_dir = TempDir::new().unwrap();
        let mut index = RecordingsIndex::new(temp_dir.path().to_path_buf()).unwrap();

        let now = Utc::now();

        // Add old recordings (40 days ago - should be expired with 30 day retention)
        let old_id1 = Uuid::new_v4();
        let old_id2 = Uuid::new_v4();
        let old_entry1 = create_test_entry(
            old_id1,
            "channel_a",
            Platform::Twitch,
            RecordingStatus::Completed,
            now - Duration::days(40),
            1000,
        );
        let old_entry2 = create_test_entry(
            old_id2,
            "channel_a",
            Platform::Twitch,
            RecordingStatus::Completed,
            now - Duration::days(35),
            1000,
        );

        // Add recent recording (5 days ago - should not be expired)
        let recent_id = Uuid::new_v4();
        let recent_entry = create_test_entry(
            recent_id,
            "channel_a",
            Platform::Twitch,
            RecordingStatus::Completed,
            now - Duration::days(5),
            1000,
        );

        index.add(old_entry1).unwrap();
        index.add(old_entry2).unwrap();
        index.add(recent_entry).unwrap();

        let retention = RetentionConfig {
            max_age_days: Some(30),
            keep_minimum: 0, // No minimum for this test
            cleanup_interval_hours: 6,
        };

        let to_delete = CleanupWorker::find_expired_recordings(&index, &retention);

        // Should delete the two old recordings
        assert_eq!(to_delete.len(), 2);
        assert!(to_delete.contains(&old_id1));
        assert!(to_delete.contains(&old_id2));
        // Should not delete recent recording
        assert!(!to_delete.contains(&recent_id));
    }

    #[test]
    fn test_cleanup_respects_minimum() {
        let temp_dir = TempDir::new().unwrap();
        let mut index = RecordingsIndex::new(temp_dir.path().to_path_buf()).unwrap();

        let now = Utc::now();

        // Add 5 old recordings (all expired)
        let mut old_ids = Vec::new();
        for i in 0..5 {
            let id = Uuid::new_v4();
            old_ids.push(id);
            let entry = create_test_entry(
                id,
                "channel_a",
                Platform::Twitch,
                RecordingStatus::Completed,
                now - Duration::days(40 + i),
                1000,
            );
            index.add(entry).unwrap();
        }

        let retention = RetentionConfig {
            max_age_days: Some(30),
            keep_minimum: 3, // Must keep at least 3
            cleanup_interval_hours: 6,
        };

        let to_delete = CleanupWorker::find_expired_recordings(&index, &retention);

        // Should only delete 2 (5 total - 3 minimum = 2 can delete)
        assert_eq!(to_delete.len(), 2);
    }

    #[test]
    fn test_cleanup_quota_exceeded() {
        let temp_dir = TempDir::new().unwrap();
        let mut index = RecordingsIndex::new(temp_dir.path().to_path_buf()).unwrap();

        let now = Utc::now();
        const GB: u64 = 1024 * 1024 * 1024;

        // Add 5 recordings, each 1GB, for total of 5GB
        let mut ids = Vec::new();
        for i in 0..5 {
            let id = Uuid::new_v4();
            ids.push(id);
            let entry = create_test_entry(
                id,
                "channel_a",
                Platform::Twitch,
                RecordingStatus::Completed,
                now - Duration::days(5 - i as i64), // Oldest first
                GB,
            );
            index.add(entry).unwrap();
        }

        // Quota is 3GB, so we need to free 2GB (delete 2 recordings)
        let to_delete = CleanupWorker::find_quota_exceeded_recordings(
            &index,
            "channel_a",
            "twitch",
            3, // 3GB quota
            0, // No minimum
        );

        // Should delete 2 oldest recordings
        assert_eq!(to_delete.len(), 2);
        // Should be the oldest two
        assert!(to_delete.contains(&ids[0])); // Oldest
        assert!(to_delete.contains(&ids[1])); // Second oldest
    }

    #[test]
    fn test_cleanup_never_deletes_recording() {
        let temp_dir = TempDir::new().unwrap();
        let mut index = RecordingsIndex::new(temp_dir.path().to_path_buf()).unwrap();

        let now = Utc::now();

        // Add an active recording (Recording status) that is old
        let active_id = Uuid::new_v4();
        let active_entry = create_test_entry(
            active_id,
            "channel_a",
            Platform::Twitch,
            RecordingStatus::Recording, // Active!
            now - Duration::days(100),  // Very old
            1000,
        );

        // Add a processing recording that is old
        let processing_id = Uuid::new_v4();
        let processing_entry = create_test_entry(
            processing_id,
            "channel_a",
            Platform::Twitch,
            RecordingStatus::Processing, // Active!
            now - Duration::days(100),   // Very old
            1000,
        );

        // Add a completed old recording
        let completed_id = Uuid::new_v4();
        let completed_entry = create_test_entry(
            completed_id,
            "channel_a",
            Platform::Twitch,
            RecordingStatus::Completed, // Not active
            now - Duration::days(100),  // Very old
            1000,
        );

        index.add(active_entry).unwrap();
        index.add(processing_entry).unwrap();
        index.add(completed_entry).unwrap();

        let retention = RetentionConfig {
            max_age_days: Some(30),
            keep_minimum: 0,
            cleanup_interval_hours: 6,
        };

        let to_delete = CleanupWorker::find_expired_recordings(&index, &retention);

        // Should only delete the completed recording
        assert_eq!(to_delete.len(), 1);
        assert!(to_delete.contains(&completed_id));
        // Should never delete active recordings
        assert!(!to_delete.contains(&active_id));
        assert!(!to_delete.contains(&processing_id));

        // Also test quota exceeded - should not delete active recordings
        let to_delete_quota = CleanupWorker::find_quota_exceeded_recordings(
            &index,
            "channel_a",
            "twitch",
            0, // 0GB quota - everything should be deleted if possible
            0,
        );

        // Should only delete the completed recording
        assert_eq!(to_delete_quota.len(), 1);
        assert!(to_delete_quota.contains(&completed_id));
        assert!(!to_delete_quota.contains(&active_id));
        assert!(!to_delete_quota.contains(&processing_id));
    }

    #[tokio::test]
    async fn test_delete_recording_files() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create a test recording directory with some files
        let recording_id = Uuid::new_v4();
        let recording_subdir = format!("recordings/{}", recording_id);
        let recording_path = base_path.join(&recording_subdir);
        fs::create_dir_all(&recording_path).await.unwrap();

        // Create some segment files
        fs::write(recording_path.join("segment_001.ts"), vec![0u8; 1000])
            .await
            .unwrap();
        fs::write(recording_path.join("segment_002.ts"), vec![0u8; 2000])
            .await
            .unwrap();
        fs::write(recording_path.join("playlist.m3u8"), b"#EXTM3U\n")
            .await
            .unwrap();

        let entry = RecordingEntry {
            id: recording_id,
            channel_name: "test_channel".to_string(),
            platform: Platform::Twitch,
            started_at: Utc::now(),
            ended_at: None,
            duration_secs: None,
            status: RecordingStatus::Completed,
            path: PathBuf::from(&recording_subdir),
            size_bytes: 3008, // 1000 + 2000 + 8
            segment_count: 2,
            title: None,
            game: None,
            output_file: None,
            processed_at: None,
            processing_attempts: 0,
            failure_reason: None,
            jellyfin_exported: false,
            jellyfin_path: None,
            thumbnail_url: None,
        };

        let freed = CleanupWorker::delete_recording_files(base_path, &entry)
            .await
            .unwrap();

        // Should have freed some bytes
        assert!(freed > 0);

        // Directory should be deleted
        assert!(!recording_path.exists());
    }

    #[test]
    fn test_cleanup_no_retention_set() {
        let temp_dir = TempDir::new().unwrap();
        let mut index = RecordingsIndex::new(temp_dir.path().to_path_buf()).unwrap();

        let now = Utc::now();

        // Add an old recording
        let id = Uuid::new_v4();
        let entry = create_test_entry(
            id,
            "channel_a",
            Platform::Twitch,
            RecordingStatus::Completed,
            now - Duration::days(1000), // Very old
            1000,
        );
        index.add(entry).unwrap();

        // No max_age_days set - nothing should expire
        let retention = RetentionConfig {
            max_age_days: None,
            keep_minimum: 0,
            cleanup_interval_hours: 6,
        };

        let to_delete = CleanupWorker::find_expired_recordings(&index, &retention);

        // Should not delete anything
        assert!(to_delete.is_empty());
    }

    #[test]
    fn test_cleanup_quota_respects_minimum() {
        let temp_dir = TempDir::new().unwrap();
        let mut index = RecordingsIndex::new(temp_dir.path().to_path_buf()).unwrap();

        let now = Utc::now();
        const GB: u64 = 1024 * 1024 * 1024;

        // Add 5 recordings, each 1GB
        for i in 0..5 {
            let entry = create_test_entry(
                Uuid::new_v4(),
                "channel_a",
                Platform::Twitch,
                RecordingStatus::Completed,
                now - Duration::days(5 - i as i64),
                GB,
            );
            index.add(entry).unwrap();
        }

        // Quota is 1GB, so we'd need to delete 4 recordings
        // But keep_minimum is 3, so we can only delete 2
        let to_delete = CleanupWorker::find_quota_exceeded_recordings(
            &index,
            "channel_a",
            "twitch",
            1, // 1GB quota
            3, // Keep at least 3
        );

        // Should only delete 2 (5 - 3 = 2 can delete)
        assert_eq!(to_delete.len(), 2);
    }

    #[test]
    fn test_cleanup_under_quota() {
        let temp_dir = TempDir::new().unwrap();
        let mut index = RecordingsIndex::new(temp_dir.path().to_path_buf()).unwrap();

        let now = Utc::now();
        const GB: u64 = 1024 * 1024 * 1024;

        // Add 2 recordings, each 1GB (total 2GB)
        for i in 0..2 {
            let entry = create_test_entry(
                Uuid::new_v4(),
                "channel_a",
                Platform::Twitch,
                RecordingStatus::Completed,
                now - Duration::days(i as i64),
                GB,
            );
            index.add(entry).unwrap();
        }

        // Quota is 5GB, we only have 2GB - nothing to delete
        let to_delete = CleanupWorker::find_quota_exceeded_recordings(
            &index,
            "channel_a",
            "twitch",
            5, // 5GB quota
            0,
        );

        assert!(to_delete.is_empty());
    }

    #[tokio::test]
    async fn test_delete_jellyfin_files_basic() {
        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path();

        // Create a mock Jellyfin library structure:
        // library/twitch/test_channel/Season 01/test_channel - S01E01 - Test Stream.mp4
        let season_dir = library_path.join("twitch/test_channel/Season 01");
        fs::create_dir_all(&season_dir).await.unwrap();

        let video_path = season_dir.join("test_channel - S01E01 - Test Stream.mp4");
        let nfo_path = season_dir.join("test_channel - S01E01 - Test Stream.nfo");
        let thumb_path = season_dir.join("test_channel - S01E01 - Test Stream-thumb.jpg");

        // Create the files
        fs::write(&video_path, vec![0u8; 5000]).await.unwrap();
        fs::write(&nfo_path, b"<episodedetails>...</episodedetails>")
            .await
            .unwrap();
        fs::write(&thumb_path, vec![0u8; 1000]).await.unwrap();

        // Create recording entry with jellyfin_exported = true
        let recording_id = Uuid::new_v4();
        let entry = RecordingEntry {
            id: recording_id,
            channel_name: "test_channel".to_string(),
            platform: Platform::Twitch,
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            duration_secs: Some(3600),
            status: RecordingStatus::Processed,
            path: PathBuf::from("recordings/test"),
            size_bytes: 5000,
            segment_count: 100,
            title: Some("Test Stream".to_string()),
            game: None,
            output_file: None,
            processed_at: None,
            processing_attempts: 0,
            failure_reason: None,
            jellyfin_exported: true,
            jellyfin_path: Some(video_path.clone()),
            thumbnail_url: None,
        };

        // Call the delete function
        let freed = CleanupWorker::delete_jellyfin_files(&entry).await.unwrap();

        // Should have freed some bytes
        assert!(freed > 0);

        // All three files should be deleted
        assert!(!video_path.exists());
        assert!(!nfo_path.exists());
        assert!(!thumb_path.exists());

        // Season directory should be deleted (it was empty after files removed)
        assert!(!season_dir.exists());
    }

    #[tokio::test]
    async fn test_delete_jellyfin_files_not_exported() {
        // Recording not exported to Jellyfin should return 0 bytes freed
        let recording_id = Uuid::new_v4();
        let entry = RecordingEntry {
            id: recording_id,
            channel_name: "test_channel".to_string(),
            platform: Platform::Twitch,
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            duration_secs: Some(3600),
            status: RecordingStatus::Completed,
            path: PathBuf::from("recordings/test"),
            size_bytes: 5000,
            segment_count: 100,
            title: None,
            game: None,
            output_file: None,
            processed_at: None,
            processing_attempts: 0,
            failure_reason: None,
            jellyfin_exported: false, // Not exported
            jellyfin_path: None,
            thumbnail_url: None,
        };

        let freed = CleanupWorker::delete_jellyfin_files(&entry).await.unwrap();
        assert_eq!(freed, 0);
    }

    #[tokio::test]
    async fn test_delete_jellyfin_files_missing_files() {
        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path();

        // Create a path that doesn't exist
        let video_path = library_path.join("nonexistent/Season 01/video.mp4");

        let recording_id = Uuid::new_v4();
        let entry = RecordingEntry {
            id: recording_id,
            channel_name: "test_channel".to_string(),
            platform: Platform::Twitch,
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            duration_secs: Some(3600),
            status: RecordingStatus::Processed,
            path: PathBuf::from("recordings/test"),
            size_bytes: 5000,
            segment_count: 100,
            title: None,
            game: None,
            output_file: None,
            processed_at: None,
            processing_attempts: 0,
            failure_reason: None,
            jellyfin_exported: true,
            jellyfin_path: Some(video_path),
            thumbnail_url: None,
        };

        // Should not panic, just return 0
        let freed = CleanupWorker::delete_jellyfin_files(&entry).await.unwrap();
        assert_eq!(freed, 0);
    }

    #[tokio::test]
    async fn test_delete_jellyfin_preserves_show_dir() {
        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path();

        // Create a mock Jellyfin library structure with show-level files
        let show_dir = library_path.join("twitch/test_channel");
        let season_dir = show_dir.join("Season 01");
        fs::create_dir_all(&season_dir).await.unwrap();

        // Create show-level metadata (should be preserved)
        let tvshow_nfo = show_dir.join("tvshow.nfo");
        let poster = show_dir.join("poster.jpg");
        fs::write(&tvshow_nfo, b"<tvshow>...</tvshow>")
            .await
            .unwrap();
        fs::write(&poster, vec![0u8; 2000]).await.unwrap();

        // Create episode file
        let video_path = season_dir.join("test_channel - S01E01 - Test.mp4");
        fs::write(&video_path, vec![0u8; 5000]).await.unwrap();

        let recording_id = Uuid::new_v4();
        let entry = RecordingEntry {
            id: recording_id,
            channel_name: "test_channel".to_string(),
            platform: Platform::Twitch,
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            duration_secs: Some(3600),
            status: RecordingStatus::Processed,
            path: PathBuf::from("recordings/test"),
            size_bytes: 5000,
            segment_count: 100,
            title: Some("Test".to_string()),
            game: None,
            output_file: None,
            processed_at: None,
            processing_attempts: 0,
            failure_reason: None,
            jellyfin_exported: true,
            jellyfin_path: Some(video_path.clone()),
            thumbnail_url: None,
        };

        let freed = CleanupWorker::delete_jellyfin_files(&entry).await.unwrap();
        assert!(freed > 0);

        // Episode file should be deleted
        assert!(!video_path.exists());

        // Season dir should be deleted (was empty after episode removed)
        assert!(!season_dir.exists());

        // Show-level files should be preserved
        assert!(tvshow_nfo.exists());
        assert!(poster.exists());
        assert!(show_dir.exists());
    }
}
