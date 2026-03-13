//! Storage management for recordings.

mod cleanup;
mod index;
mod quota;

pub use cleanup::CleanupWorker;
pub use index::{RecordingEntry, RecordingStatus, RecordingsIndex};
pub use quota::{QuotaCheckResult, QuotaChecker};

use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::{RetentionConfig, StorageConfig};
use crate::types::Platform;

/**
 * Coordinator for all storage-related operations.
 *
 * StorageManager provides a unified interface for managing recordings,
 * coordinating between the RecordingsIndex, QuotaChecker, and CleanupWorker.
 */
pub struct StorageManager {
    config: StorageConfig,
    index: Arc<RwLock<RecordingsIndex>>,
    quota_checker: QuotaChecker,
}

impl StorageManager {
    /**
     * Create a new StorageManager with the given configuration.
     *
     * This will initialize the recordings index from disk if it exists.
     */
    pub async fn new(config: StorageConfig) -> Result<Self, std::io::Error> {
        // Ensure recordings directory exists
        tokio::fs::create_dir_all(&config.recordings_dir).await?;

        // Initialize the index
        let index =
            RecordingsIndex::new(config.recordings_dir.clone()).map_err(std::io::Error::other)?;

        // Initialize quota checker
        let quota_checker = QuotaChecker::new(config.quotas.clone());

        Ok(Self {
            config,
            index: Arc::new(RwLock::new(index)),
            quota_checker,
        })
    }

    /**
     * Add a new recording to the index (called when recording starts).
     *
     * Returns the UUID of the new recording entry.
     */
    pub async fn add_recording(
        &self,
        channel_name: &str,
        platform: &str,
        path: PathBuf,
        title: Option<String>,
        game: Option<String>,
        thumbnail_url: Option<String>,
    ) -> Result<Uuid, std::io::Error> {
        let platform = Self::parse_platform(platform)?;
        let id = Uuid::new_v4();

        let entry = RecordingEntry {
            id,
            channel_name: channel_name.to_string(),
            platform,
            started_at: Utc::now(),
            ended_at: None,
            duration_secs: None,
            status: RecordingStatus::Recording,
            path,
            size_bytes: 0,
            segment_count: 0,
            title,
            game,
            output_file: None,
            processed_at: None,
            processing_attempts: 0,
            failure_reason: None,
            jellyfin_exported: false,
            jellyfin_path: None,
            thumbnail_url,
        };

        let mut index = self.index.write().await;
        index.add(entry).map_err(std::io::Error::other)?;

        Ok(id)
    }

    /** Get a recording by ID. */
    pub async fn get_recording(&self, id: &Uuid) -> Result<Option<RecordingEntry>, std::io::Error> {
        let index = self.index.read().await;
        Ok(index.get(*id).cloned())
    }

    /** List recordings with optional filters. */
    pub async fn list_recordings(
        &self,
        channel: Option<&str>,
        platform: Option<&str>,
        status: Option<&RecordingStatus>,
    ) -> Vec<RecordingEntry> {
        let platform = platform.and_then(|p| Self::parse_platform(p).ok());

        let index = self.index.read().await;
        index
            .list(channel, platform, status.copied())
            .into_iter()
            .cloned()
            .collect()
    }

    /** Mark a recording as completed. */
    pub async fn complete_recording(
        &self,
        id: &Uuid,
        duration_secs: u64,
        size_bytes: u64,
        segment_count: u32,
    ) -> Result<(), std::io::Error> {
        let mut index = self.index.write().await;
        let entry = index.get(*id).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Recording not found")
        })?;

        let updated = RecordingEntry {
            status: RecordingStatus::Completed,
            ended_at: Some(Utc::now()),
            duration_secs: Some(duration_secs),
            size_bytes,
            segment_count,
            ..entry
        };

        index.update(updated).map_err(std::io::Error::other)
    }

    /** Mark a recording as processing. */
    pub async fn mark_processing(&self, id: &Uuid) -> Result<(), std::io::Error> {
        let mut index = self.index.write().await;
        let entry = index.get(*id).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Recording not found")
        })?;

        let updated = RecordingEntry {
            status: RecordingStatus::Processing,
            ..entry
        };

        index.update(updated).map_err(std::io::Error::other)
    }

    /** Mark a recording as processed. */
    pub async fn mark_processed(
        &self,
        id: &Uuid,
        output_file: PathBuf,
        new_size: Option<u64>,
    ) -> Result<(), std::io::Error> {
        let mut index = self.index.write().await;
        let entry = index.get(*id).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Recording not found")
        })?;

        let updated = RecordingEntry {
            status: RecordingStatus::Processed,
            output_file: Some(output_file),
            size_bytes: new_size.unwrap_or(entry.size_bytes),
            ..entry
        };

        index.update(updated).map_err(std::io::Error::other)
    }

    /** Mark a recording as failed. */
    pub async fn mark_failed(&self, id: &Uuid) -> Result<(), std::io::Error> {
        let mut index = self.index.write().await;
        let entry = index.get(*id).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Recording not found")
        })?;

        let updated = RecordingEntry {
            status: RecordingStatus::Failed,
            ended_at: Some(Utc::now()),
            ..entry
        };

        index.update(updated).map_err(std::io::Error::other)
    }

    /** Mark a recording as stopping (graceful shutdown initiated). */
    pub async fn mark_stopping(&self, id: &Uuid) -> Result<(), std::io::Error> {
        let mut index = self.index.write().await;
        let entry = index.get(*id).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Recording not found")
        })?;

        let updated = RecordingEntry {
            status: RecordingStatus::Stopping,
            ..entry
        };

        index.update(updated).map_err(std::io::Error::other)
    }

    /** Mark a recording as pending processing. */
    pub async fn mark_pending_processing(
        &self,
        id: &Uuid,
        duration_secs: u64,
        size_bytes: u64,
        segment_count: u32,
    ) -> Result<(), std::io::Error> {
        let mut index = self.index.write().await;
        let entry = index.get(*id).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Recording not found")
        })?;

        let updated = RecordingEntry {
            status: RecordingStatus::PendingProcessing,
            ended_at: Some(Utc::now()),
            duration_secs: Some(duration_secs),
            size_bytes,
            segment_count,
            ..entry
        };

        index.update(updated).map_err(std::io::Error::other)
    }

    /** Mark a recording as processing failed. */
    pub async fn mark_processing_failed(
        &self,
        id: &Uuid,
        reason: Option<String>,
    ) -> Result<(), std::io::Error> {
        let mut index = self.index.write().await;
        let entry = index.get(*id).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Recording not found")
        })?;

        let updated = RecordingEntry {
            status: RecordingStatus::ProcessingFailed,
            processing_attempts: entry.processing_attempts + 1,
            failure_reason: reason.or(entry.failure_reason),
            ..entry
        };

        index.update(updated).map_err(std::io::Error::other)
    }

    /** Mark a recording as exported to Jellyfin. */
    pub async fn mark_jellyfin_exported(
        &self,
        id: &Uuid,
        jellyfin_path: PathBuf,
    ) -> Result<(), std::io::Error> {
        let mut index = self.index.write().await;
        let entry = index.get(*id).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Recording not found")
        })?;

        let updated = RecordingEntry {
            jellyfin_exported: true,
            jellyfin_path: Some(jellyfin_path),
            ..entry
        };

        index.update(updated).map_err(std::io::Error::other)
    }

    /** Reset processing attempts for a recording (for manual retry). */
    pub async fn reset_processing_attempts(&self, id: &Uuid) -> Result<(), std::io::Error> {
        let mut index = self.index.write().await;
        let entry = index.get(*id).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Recording not found")
        })?;

        let updated = RecordingEntry {
            processing_attempts: 0,
            ..entry
        };

        index.update(updated).map_err(std::io::Error::other)
    }

    /** Update size, segment count, and optionally duration for a recording (used when reconciling legacy data). */
    pub async fn update_recording_stats(
        &self,
        id: &Uuid,
        size_bytes: u64,
        segment_count: u32,
        duration_secs: Option<u64>,
    ) -> Result<(), std::io::Error> {
        let mut index = self.index.write().await;
        let entry = index.get(*id).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Recording not found")
        })?;

        let updated = RecordingEntry {
            size_bytes,
            segment_count,
            duration_secs: duration_secs.or(entry.duration_secs),
            ..entry
        };

        index.update(updated).map_err(std::io::Error::other)
    }

    /**
     * Get recordings that need post-processing.
     * Returns recordings that are PendingProcessing, ProcessingFailed (< 3 attempts),
     * or Completed/Failed with no output_file.
     * Note: For Failed/Completed, we don't check segment_count since it may be 0
     * even when segments exist on disk (legacy data). The caller should verify disk contents.
     */
    pub async fn get_unprocessed_recordings(&self) -> Vec<RecordingEntry> {
        let index = self.index.read().await;
        let all_recordings: Vec<_> = index.list(None, None, None);

        tracing::debug!(
            "get_unprocessed_recordings: total recordings in index = {}",
            all_recordings.len()
        );

        // Log all recordings and their status for debugging
        for entry in &all_recordings {
            tracing::debug!(
                "  Recording {} ({}) status={:?} output_file={:?} attempts={}",
                entry.channel_name,
                entry.id,
                entry.status,
                entry.output_file,
                entry.processing_attempts
            );
        }

        let result: Vec<_> = all_recordings
            .into_iter()
            .filter(|e| {
                let dominated = matches!(e.status, RecordingStatus::PendingProcessing);
                let failed_retry = matches!(e.status, RecordingStatus::ProcessingFailed) && e.processing_attempts < 5;
                let needs_processing = matches!(e.status, RecordingStatus::Completed | RecordingStatus::Failed)
                    && e.output_file.is_none();

                if dominated || failed_retry || needs_processing {
                    tracing::debug!(
                        "  -> MATCHED for processing: {} ({}) [pending={}, failed_retry={}, needs_proc={}]",
                        e.channel_name, e.id, dominated, failed_retry, needs_processing
                    );
                }

                dominated || failed_retry || needs_processing
            })
            .cloned()
            .collect();

        tracing::debug!(
            "get_unprocessed_recordings: returning {} recordings for processing",
            result.len()
        );

        result
    }

    /** Get IDs of channels that are currently recording. */
    pub async fn get_active_recording_channel_names(&self) -> Vec<String> {
        let index = self.index.read().await;
        index
            .list(None, None, Some(RecordingStatus::Recording))
            .into_iter()
            .map(|e| e.channel_name.clone())
            .collect()
    }

    /**
     * Get all processed recordings.
     *
     * Returns recordings with status Processed that can be checked for
     * remaining .ts segment files for retroactive segment handling.
     */
    pub async fn get_processed_recordings(&self) -> Vec<RecordingEntry> {
        let index = self.index.read().await;
        index
            .list(None, None, Some(RecordingStatus::Processed))
            .into_iter()
            .cloned()
            .collect()
    }

    /**
     * Reset interrupted "Processing" status entries from previous session.
     * These are recordings where processing was started but the daemon crashed/restarted.
     * They are reset to "PendingProcessing" so they will be picked up again.
     */
    pub async fn reset_interrupted_processing(&self) -> usize {
        tracing::info!("reset_interrupted_processing: scanning for stuck Processing entries...");

        let interrupted: Vec<(Uuid, String)> = {
            let index = self.index.read().await;
            let all = index.list(None, None, None);
            tracing::debug!(
                "reset_interrupted_processing: total recordings in index = {}",
                all.len()
            );

            // Log all recordings for debugging
            for e in &all {
                tracing::debug!(
                    "  Recording {} ({}) status={:?}",
                    e.channel_name,
                    e.id,
                    e.status
                );
            }

            index
                .list(None, None, Some(RecordingStatus::Processing))
                .into_iter()
                .map(|e| (e.id, e.channel_name.clone()))
                .collect()
        };

        let count = interrupted.len();
        tracing::info!(
            "reset_interrupted_processing: found {} recordings with Processing status",
            count
        );

        for (id, channel_name) in interrupted {
            if let Err(e) = self.mark_pending_processing_simple(&id).await {
                tracing::warn!(
                    "Failed to reset interrupted processing {} ({}): {}",
                    channel_name,
                    id,
                    e
                );
            } else {
                tracing::info!(
                    "Reset interrupted processing {} ({}) to PendingProcessing",
                    channel_name,
                    id
                );
            }
        }

        count
    }

    /** Mark a recording as pending processing (simple version, preserves existing data). */
    async fn mark_pending_processing_simple(&self, id: &Uuid) -> Result<(), std::io::Error> {
        let mut index = self.index.write().await;
        let entry = index.get(*id).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Recording not found")
        })?;

        let updated = RecordingEntry {
            status: RecordingStatus::PendingProcessing,
            ..entry
        };

        index.update(updated).map_err(std::io::Error::other)
    }

    /**
     * Clean up orphaned "recording" status entries from previous session.
     * - Entries with 0 bytes are deleted (no data was captured)
     * - Entries with data are marked as pending processing (queue for post-processing)
     */
    pub async fn cleanup_orphaned_recordings(&self) -> usize {
        // Get all recordings with status "Recording" along with their size and segment count
        let orphaned: Vec<(Uuid, u64, u32, String)> = {
            let index = self.index.read().await;
            index
                .list(None, None, Some(RecordingStatus::Recording))
                .into_iter()
                .map(|e| (e.id, e.size_bytes, e.segment_count, e.channel_name.clone()))
                .collect()
        };

        let count = orphaned.len();

        for (id, size_bytes, segment_count, channel_name) in orphaned {
            if size_bytes == 0 {
                // No data captured - delete entirely
                if let Err(e) = self.delete_recording(&id, true).await {
                    tracing::warn!(
                        "Failed to delete empty orphaned recording {} ({}): {}",
                        channel_name,
                        id,
                        e
                    );
                } else {
                    tracing::info!("Deleted empty orphaned recording {} ({})", channel_name, id);
                }
            } else {
                // Has data - mark as pending processing
                // Estimate duration from segment count (assuming ~2 sec segments)
                let estimated_duration = (segment_count as u64) * 2;
                if let Err(e) = self
                    .mark_pending_processing(&id, estimated_duration, size_bytes, segment_count)
                    .await
                {
                    tracing::warn!(
                        "Failed to mark orphaned recording {} ({}) as pending: {}",
                        channel_name,
                        id,
                        e
                    );
                } else {
                    tracing::info!("Marked orphaned recording {} ({}) as pending processing ({} bytes, {} segments)",
                        channel_name, id, size_bytes, segment_count);
                }
            }
        }

        count
    }

    /** Update recording size during recording. */
    pub async fn update_size(
        &self,
        id: &Uuid,
        size_bytes: u64,
        segment_count: u32,
    ) -> Result<(), std::io::Error> {
        let mut index = self.index.write().await;
        let entry = index.get(*id).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Recording not found")
        })?;

        let updated = RecordingEntry {
            size_bytes,
            segment_count,
            ..entry
        };

        index.update(updated).map_err(std::io::Error::other)
    }

    /**
     * Refresh stats for all active recordings from disk.
     * Calculates actual directory size and segment count, and estimates duration.
     * Returns the number of recordings updated.
     */
    pub async fn refresh_active_recording_stats(&self) -> usize {
        // Get all active recordings
        let active: Vec<(Uuid, PathBuf, String)> = {
            let index = self.index.read().await;
            index
                .list(None, None, Some(RecordingStatus::Recording))
                .into_iter()
                .map(|e| (e.id, e.path.clone(), e.channel_name.clone()))
                .collect()
        };

        let mut updated_count = 0;

        for (id, path, channel_name) in active {
            // Calculate directory size and count segments
            let (size_bytes, segment_count) = match Self::calculate_dir_stats(&path).await {
                Ok(stats) => stats,
                Err(e) => {
                    tracing::warn!(
                        "Failed to calculate stats for active recording {} ({}): {}",
                        channel_name,
                        id,
                        e
                    );
                    continue;
                }
            };

            // Estimate duration from segment count (~2 seconds per segment)
            let duration_secs = (segment_count as u64) * 2;

            // Update the recording entry
            if let Err(e) = self
                .update_recording_stats(&id, size_bytes, segment_count, Some(duration_secs))
                .await
            {
                tracing::warn!(
                    "Failed to update stats for active recording {} ({}): {}",
                    channel_name,
                    id,
                    e
                );
            } else {
                tracing::debug!(
                    "Updated active recording {} ({}): {} bytes, {} segments, ~{}s",
                    channel_name,
                    id,
                    size_bytes,
                    segment_count,
                    duration_secs
                );
                updated_count += 1;
            }
        }

        if updated_count > 0 {
            tracing::info!("Refreshed stats for {} active recordings", updated_count);
        }

        updated_count
    }

    /** Calculate total size and file count of a directory (for .ts files). */
    async fn calculate_dir_stats(path: &std::path::Path) -> std::io::Result<(u64, u32)> {
        let mut total_size = 0u64;
        let mut segment_count = 0u32;

        if !path.exists() {
            return Ok((0, 0));
        }

        let mut entries = tokio::fs::read_dir(path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;
            if metadata.is_file() {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                // Count .ts segment files
                if file_name_str.ends_with(".ts") {
                    segment_count += 1;
                }
                total_size += metadata.len();
            }
        }

        Ok((total_size, segment_count))
    }

    /**
     * Delete a recording (and optionally files).
     *
     * Returns the number of bytes freed if files were deleted: (recordings_bytes, library_bytes).
     * When include_library is true, also deletes Jellyfin library files.
     */
    pub async fn delete_recording(
        &self,
        id: &Uuid,
        delete_files: bool,
    ) -> Result<Option<u64>, std::io::Error> {
        self.delete_recording_with_options(id, delete_files, true)
            .await
    }

    /**
     * Delete a recording with control over which files to delete.
     *
     * - `delete_files`: Whether to delete recording files
     * - `include_library`: Whether to also delete Jellyfin library files
     *
     * Returns total bytes freed (recordings + library), or None if delete_files is false.
     * Note: Library cleanup is best-effort and doesn't affect the return value when delete_files=false.
     */
    pub async fn delete_recording_with_options(
        &self,
        id: &Uuid,
        delete_files: bool,
        include_library: bool,
    ) -> Result<Option<u64>, std::io::Error> {
        let mut index = self.index.write().await;

        let entry = index.get(*id).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Recording not found")
        })?;

        let mut freed_bytes = 0u64;

        if delete_files {
            freed_bytes +=
                CleanupWorker::delete_recording_files(&self.config.recordings_dir, &entry).await?;
        }

        if include_library {
            freed_bytes += CleanupWorker::delete_jellyfin_files(&entry)
                .await
                .unwrap_or(0);
        }

        index.delete(*id).map_err(std::io::Error::other)?;

        // Return Some(freed_bytes) only if we actually attempted to delete files
        // Preserves backwards compatibility: delete_recording(id, false) returns None
        Ok(if delete_files {
            Some(freed_bytes)
        } else {
            None
        })
    }

    /**
     * Delete only Jellyfin library files for a recording (keep recording in index).
     *
     * This resets the jellyfin_exported flag and clears jellyfin_path.
     *
     * Returns bytes freed from library.
     */
    pub async fn cleanup_jellyfin_files(&self, id: &Uuid) -> Result<u64, std::io::Error> {
        let mut index = self.index.write().await;

        let entry = index.get(*id).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Recording not found")
        })?;

        // Delete Jellyfin files
        let freed_bytes = CleanupWorker::delete_jellyfin_files(&entry)
            .await
            .unwrap_or(0);

        // Reset jellyfin flags on the recording entry
        let updated = RecordingEntry {
            jellyfin_exported: false,
            jellyfin_path: None,
            ..entry
        };

        index.update(updated).map_err(std::io::Error::other)?;

        Ok(freed_bytes)
    }

    /**
     * Delete recording files only (not Jellyfin library files).
     *
     * This removes the recording from the index but leaves any Jellyfin library files.
     *
     * Returns bytes freed from recordings directory.
     */
    pub async fn delete_recording_files_only(&self, id: &Uuid) -> Result<u64, std::io::Error> {
        let mut index = self.index.write().await;

        let entry = index.get(*id).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Recording not found")
        })?;

        let freed_bytes =
            CleanupWorker::delete_recording_files(&self.config.recordings_dir, &entry).await?;

        index.delete(*id).map_err(std::io::Error::other)?;

        Ok(freed_bytes)
    }

    /**
     * Check if recording can proceed based on quotas.
     *
     * Checks both global and per-channel quotas, returning the most restrictive result.
     */
    pub async fn check_quota(
        &self,
        channel: &str,
        _platform: &str,
        channel_limit_gb: Option<u64>,
    ) -> QuotaCheckResult {
        let index = self.index.read().await;

        // Check global quota first
        let total_usage = index.total_size();
        let global_result = self.quota_checker.check_global_quota(total_usage);

        if !global_result.allowed {
            return global_result;
        }

        // Check per-channel quota
        let channel_usage = index.channel_size(channel);
        self.quota_checker
            .check_channel_quota_with_override(channel_usage, channel_limit_gb)
    }

    /** Get current storage usage for a specific channel in bytes. */
    pub async fn get_channel_usage(&self, channel: &str) -> u64 {
        let index = self.index.read().await;
        index.channel_size(channel)
    }

    /** Get storage statistics. */
    pub async fn get_stats(&self) -> StorageStats {
        let index = self.index.read().await;
        let all_recordings = index.list(None, None, None);

        // Group by channel and platform
        let mut per_channel: HashMap<(String, Platform), (usize, u64)> = HashMap::new();

        for recording in &all_recordings {
            let key = (recording.channel_name.clone(), recording.platform);
            let entry = per_channel.entry(key).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += recording.size_bytes;
        }

        let channel_stats: Vec<ChannelStats> = per_channel
            .into_iter()
            .map(
                |((channel_name, platform), (count, size_bytes))| ChannelStats {
                    channel_name,
                    platform: platform.to_string(),
                    count,
                    size_bytes,
                },
            )
            .collect();

        StorageStats {
            total_recordings: all_recordings.len(),
            total_size_bytes: index.total_size(),
            per_channel: channel_stats,
        }
    }

    /**
     * Run cleanup based on retention policy.
     *
     * This finds expired recordings and deletes them along with their files.
     * Also enforces per-channel retention if configured.
     */
    pub async fn run_cleanup(
        &self,
        channels: &[crate::types::Channel],
    ) -> Result<CleanupResult, std::io::Error> {
        // 1. Global retention cleanup - find expired recordings
        let to_delete = {
            let index = self.index.read().await;
            CleanupWorker::find_expired_recordings(&index, &self.config.retention)
        };

        let mut deleted_count = 0;
        let mut freed_bytes = 0u64;

        // Delete each expired recording
        for id in to_delete {
            if let Ok(Some(freed)) = self.delete_recording(&id, true).await {
                deleted_count += 1;
                freed_bytes += freed;
            }
        }

        // 2. Per-channel retention cleanup
        for channel in channels {
            if let Some(retention_days) = channel.retention_days {
                let channel_expired = {
                    let index = self.index.read().await;
                    CleanupWorker::find_channel_expired_recordings(
                        &index,
                        &channel.name,
                        retention_days,
                        self.config.retention.keep_minimum,
                    )
                };

                for id in channel_expired {
                    if let Ok(Some(freed)) = self.delete_recording(&id, true).await {
                        deleted_count += 1;
                        freed_bytes += freed;
                    }
                }
            }
        }

        Ok(CleanupResult {
            deleted_count,
            freed_bytes,
        })
    }

    /** Get retention config. */
    pub fn retention_config(&self) -> &RetentionConfig {
        &self.config.retention
    }

    /** Get recordings directory. */
    pub fn recordings_dir(&self) -> &PathBuf {
        &self.config.recordings_dir
    }

    /** Parse a platform string into a Platform enum. */
    fn parse_platform(platform: &str) -> Result<Platform, std::io::Error> {
        match platform.to_lowercase().as_str() {
            "twitch" => Ok(Platform::Twitch),
            "youtube" => Ok(Platform::YouTube),
            "kick" => Ok(Platform::Kick),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Unknown platform: {}", platform),
            )),
        }
    }
}

/** Storage statistics aggregated from all recordings. */
#[derive(Debug, Clone)]
pub struct StorageStats {
    /** Total number of recordings in the index. */
    pub total_recordings: usize,
    /** Total size of all recordings in bytes. */
    pub total_size_bytes: u64,
    /** Statistics per channel. */
    pub per_channel: Vec<ChannelStats>,
}

/** Statistics for a single channel. */
#[derive(Debug, Clone)]
pub struct ChannelStats {
    /** Name of the channel. */
    pub channel_name: String,
    /** Platform the channel is on. */
    pub platform: String,
    /** Number of recordings for this channel. */
    pub count: usize,
    /** Total size of recordings for this channel in bytes. */
    pub size_bytes: u64,
}

/** Result of a cleanup operation. */
#[derive(Debug, Clone)]
pub struct CleanupResult {
    /** Number of recordings deleted. */
    pub deleted_count: usize,
    /** Total bytes freed by the cleanup. */
    pub freed_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::QuotaConfig;
    use tempfile::TempDir;

    fn create_test_config(recordings_dir: PathBuf) -> StorageConfig {
        StorageConfig {
            recordings_dir: recordings_dir.clone(),
            library_dir: recordings_dir.join("library"),
            images_dir: recordings_dir.join("images"),
            disk_warning_threshold: 90,
            quotas: QuotaConfig {
                global_max_gb: Some(100),
                per_channel_max_gb: Some(10),
                warn_at_percent: 80,
            },
            retention: RetentionConfig {
                max_age_days: Some(30),
                keep_minimum: 2,
                cleanup_interval_hours: 6,
            },
        }
    }

    #[tokio::test]
    async fn test_storage_manager_add_recording() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(temp_dir.path().to_path_buf());
        let manager = StorageManager::new(config).await.unwrap();

        // Add a recording
        let id = manager
            .add_recording(
                "test_channel",
                "twitch",
                PathBuf::from("recordings/test"),
                Some("Test Stream".to_string()),
                Some("Test Game".to_string()),
                None,
            )
            .await
            .unwrap();

        // Verify we can retrieve it
        let recording = manager.get_recording(&id).await.unwrap().unwrap();
        assert_eq!(recording.channel_name, "test_channel");
        assert_eq!(recording.platform, Platform::Twitch);
        assert_eq!(recording.status, RecordingStatus::Recording);
        assert_eq!(recording.title, Some("Test Stream".to_string()));
        assert_eq!(recording.game, Some("Test Game".to_string()));

        // Test listing recordings
        let all = manager.list_recordings(None, None, None).await;
        assert_eq!(all.len(), 1);

        // Test filtering by channel
        let by_channel = manager
            .list_recordings(Some("test_channel"), None, None)
            .await;
        assert_eq!(by_channel.len(), 1);

        let by_wrong_channel = manager
            .list_recordings(Some("nonexistent"), None, None)
            .await;
        assert_eq!(by_wrong_channel.len(), 0);

        // Test filtering by platform
        let by_platform = manager.list_recordings(None, Some("twitch"), None).await;
        assert_eq!(by_platform.len(), 1);

        let by_wrong_platform = manager.list_recordings(None, Some("youtube"), None).await;
        assert_eq!(by_wrong_platform.len(), 0);
    }

    #[tokio::test]
    async fn test_storage_manager_update_recording() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(temp_dir.path().to_path_buf());
        let manager = StorageManager::new(config).await.unwrap();

        // Add a recording
        let id = manager
            .add_recording(
                "test_channel",
                "twitch",
                PathBuf::from("recordings/test"),
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Update size during recording
        manager.update_size(&id, 1000, 10).await.unwrap();
        let recording = manager.get_recording(&id).await.unwrap().unwrap();
        assert_eq!(recording.size_bytes, 1000);
        assert_eq!(recording.segment_count, 10);

        // Complete the recording
        manager
            .complete_recording(&id, 3600, 5000, 50)
            .await
            .unwrap();
        let recording = manager.get_recording(&id).await.unwrap().unwrap();
        assert_eq!(recording.status, RecordingStatus::Completed);
        assert_eq!(recording.duration_secs, Some(3600));
        assert_eq!(recording.size_bytes, 5000);
        assert_eq!(recording.segment_count, 50);
        assert!(recording.ended_at.is_some());

        // Mark as processing
        manager.mark_processing(&id).await.unwrap();
        let recording = manager.get_recording(&id).await.unwrap().unwrap();
        assert_eq!(recording.status, RecordingStatus::Processing);

        // Mark as processed
        manager
            .mark_processed(&id, PathBuf::from("output.mp4"), Some(4500))
            .await
            .unwrap();
        let recording = manager.get_recording(&id).await.unwrap().unwrap();
        assert_eq!(recording.status, RecordingStatus::Processed);
        assert_eq!(recording.output_file, Some(PathBuf::from("output.mp4")));
        assert_eq!(recording.size_bytes, 4500);

        // Add another recording and mark it as failed
        let id2 = manager
            .add_recording(
                "test_channel",
                "twitch",
                PathBuf::from("recordings/test2"),
                None,
                None,
                None,
            )
            .await
            .unwrap();

        manager.mark_failed(&id2).await.unwrap();
        let recording2 = manager.get_recording(&id2).await.unwrap().unwrap();
        assert_eq!(recording2.status, RecordingStatus::Failed);
        assert!(recording2.ended_at.is_some());
    }

    #[tokio::test]
    async fn test_storage_manager_quota_check() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(temp_dir.path().to_path_buf());
        let manager = StorageManager::new(config).await.unwrap();

        // With no recordings, quota should be allowed
        let result = manager.check_quota("test_channel", "twitch", None).await;
        assert!(result.allowed);
        assert!(!result.warning);
        assert!(!result.exceeded);
        assert_eq!(result.usage_bytes, 0);

        // Add a recording with some size
        let id = manager
            .add_recording(
                "test_channel",
                "twitch",
                PathBuf::from("recordings/test"),
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Update size to 1GB
        const GB: u64 = 1024 * 1024 * 1024;
        manager.update_size(&id, GB, 100).await.unwrap();

        // Check quota - should still be allowed (1GB used, 10GB per-channel limit)
        let result = manager.check_quota("test_channel", "twitch", None).await;
        assert!(result.allowed);
        assert_eq!(result.usage_bytes, GB);

        // Check with a custom channel limit (1GB) - should be at limit
        let result = manager.check_quota("test_channel", "twitch", Some(1)).await;
        assert!(!result.allowed);
        assert!(result.exceeded);

        // Check quota for a different channel (no recordings)
        let result = manager.check_quota("other_channel", "twitch", None).await;
        assert!(result.allowed);
        assert_eq!(result.usage_bytes, 0);
    }

    #[tokio::test]
    async fn test_storage_manager_stats() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(temp_dir.path().to_path_buf());
        let manager = StorageManager::new(config).await.unwrap();

        // Add recordings for different channels
        let id1 = manager
            .add_recording(
                "channel_a",
                "twitch",
                PathBuf::from("recordings/1"),
                None,
                None,
                None,
            )
            .await
            .unwrap();
        manager.update_size(&id1, 1000, 10).await.unwrap();

        let id2 = manager
            .add_recording(
                "channel_a",
                "twitch",
                PathBuf::from("recordings/2"),
                None,
                None,
                None,
            )
            .await
            .unwrap();
        manager.update_size(&id2, 2000, 20).await.unwrap();

        let id3 = manager
            .add_recording(
                "channel_b",
                "youtube",
                PathBuf::from("recordings/3"),
                None,
                None,
                None,
            )
            .await
            .unwrap();
        manager.update_size(&id3, 500, 5).await.unwrap();

        // Get stats
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_recordings, 3);
        assert_eq!(stats.total_size_bytes, 3500);
        assert_eq!(stats.per_channel.len(), 2);

        // Find channel_a stats
        let channel_a_stats = stats
            .per_channel
            .iter()
            .find(|s| s.channel_name == "channel_a")
            .unwrap();
        assert_eq!(channel_a_stats.count, 2);
        assert_eq!(channel_a_stats.size_bytes, 3000);
        assert_eq!(channel_a_stats.platform, "twitch");

        // Find channel_b stats
        let channel_b_stats = stats
            .per_channel
            .iter()
            .find(|s| s.channel_name == "channel_b")
            .unwrap();
        assert_eq!(channel_b_stats.count, 1);
        assert_eq!(channel_b_stats.size_bytes, 500);
        assert_eq!(channel_b_stats.platform, "youtube");
    }

    #[tokio::test]
    async fn test_storage_manager_delete_recording() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(temp_dir.path().to_path_buf());
        let manager = StorageManager::new(config).await.unwrap();

        // Add a recording
        let id = manager
            .add_recording(
                "test_channel",
                "twitch",
                PathBuf::from("recordings/test"),
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Verify it exists
        assert!(manager.get_recording(&id).await.unwrap().is_some());

        // Delete without files (files don't exist anyway in this test)
        let result = manager.delete_recording(&id, false).await.unwrap();
        assert_eq!(result, None);

        // Verify it's gone
        assert!(manager.get_recording(&id).await.unwrap().is_none());

        // Deleting non-existent recording should error
        let result = manager.delete_recording(&Uuid::new_v4(), false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_storage_manager_accessors() {
        let temp_dir = TempDir::new().unwrap();
        let recordings_dir = temp_dir.path().to_path_buf();
        let config = create_test_config(recordings_dir.clone());
        let manager = StorageManager::new(config).await.unwrap();

        // Test retention_config accessor
        let retention = manager.retention_config();
        assert_eq!(retention.max_age_days, Some(30));
        assert_eq!(retention.keep_minimum, 2);
        assert_eq!(retention.cleanup_interval_hours, 6);

        // Test recordings_dir accessor
        assert_eq!(manager.recordings_dir(), &recordings_dir);
    }
}
