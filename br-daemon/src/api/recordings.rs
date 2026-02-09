use crate::api::auth::{AdminUser, AuthUser};
use crate::api::response::{ApiError, ApiResponse};
use crate::api::AppState;
use crate::processing::ProcessingMode;
use crate::storage::{RecordingEntry, RecordingStatus};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use sysinfo::Disks;
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ListRecordingsParams {
    pub channel: Option<String>,
    pub platform: Option<String>,
    pub status: Option<RecordingStatus>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteRecordingParams {
    pub keep_files: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ListRecordingsResponse {
    pub recordings: Vec<RecordingEntry>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Serialize)]
pub struct DeleteRecordingResponse {
    pub deleted: bool,
    pub freed_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct StorageStatsResponse {
    pub total_recordings: usize,
    pub total_size_bytes: u64,
    pub disk_free_bytes: u64,
    pub disk_total_bytes: u64,
    pub per_channel: Vec<ChannelStorageStats>,
    pub recordings_dir: String,
    pub library_dir: String,
    /** Size of files in the library directory. */
    pub library_size_bytes: u64,
    /** Library disk stats (only present if library_dir is on a different disk than recordings_dir). */
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_disk_free_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_disk_total_bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct ChannelStorageStats {
    pub channel: String,
    pub platform: String,
    pub count: usize,
    pub size_bytes: u64,
}

#[derive(Debug, Deserialize)]
pub struct ProcessRecordingRequest {
    pub format: Option<String>,
    pub mode: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProcessRecordingResponse {
    pub status: String,
    pub queue_position: usize,
}

/** Location to clean up: Recordings directory, Library directory, or Both. */
#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CleanupLocation {
    /** Only delete files in recordings_dir. */
    Recordings,
    /** Only delete files in library_dir (Jellyfin exports). */
    Library,
    /** Delete files in both directories. */
    #[default]
    Both,
}

#[derive(Debug, Deserialize)]
pub struct CleanupRequest {
    /** Delete recordings older than this many days. */
    pub older_than_days: Option<u32>,
    /** Filter by specific channel ID. */
    pub channel_id: Option<Uuid>,
    /** Filter by channel name. */
    pub channel_name: Option<String>,
    /** Filter by recording status (e.g., "completed", "processed", "failed"). */
    pub status: Option<RecordingStatus>,
    /** Which location to clean up: "recordings", "library", or "both" (default). */
    #[serde(default)]
    pub location: CleanupLocation,
    /** If true, don't actually delete, just return what would be deleted. */
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Serialize)]
pub struct CleanupResponse {
    /** Number of recordings that would be/were deleted. */
    pub recordings_affected: usize,
    /** Total bytes that would be/were freed (sum of recordings + library). */
    pub bytes_to_free: u64,
    /** List of recordings affected (only included in dry_run mode). */
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recordings: Option<Vec<RecordingEntry>>,
    /** Whether this was a dry run. */
    pub dry_run: bool,
    /** Bytes freed from recordings directory (only in non-dry_run mode). */
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recordings_bytes_freed: Option<u64>,
    /** Bytes freed from library directory (only in non-dry_run mode). */
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_bytes_freed: Option<u64>,
}

/** List all recordings with optional filters. */
pub async fn list_recordings(
    _auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListRecordingsParams>,
) -> Json<ApiResponse<ListRecordingsResponse>> {
    // Get status as reference for filtering
    let status_ref = params.status.as_ref();

    // Fetch recordings from storage manager
    let all_recordings = state
        .storage_manager
        .list_recordings(
            params.channel.as_deref(),
            params.platform.as_deref(),
            status_ref,
        )
        .await;

    let total = all_recordings.len();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);

    // Apply pagination
    let recordings: Vec<RecordingEntry> = all_recordings
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect();

    Json(ApiResponse::new(ListRecordingsResponse {
        recordings,
        total,
        limit,
        offset,
    }))
}

/** Get a specific recording by ID. */
pub async fn get_recording(
    _auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<RecordingEntry>>, (StatusCode, ApiError)> {
    let recording = state
        .storage_manager
        .get_recording(&id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Recording"))?;

    Ok(Json(ApiResponse::new(recording)))
}

/** Delete a recording (admin only). */
pub async fn delete_recording(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Query(params): Query<DeleteRecordingParams>,
) -> Result<Json<ApiResponse<DeleteRecordingResponse>>, (StatusCode, ApiError)> {
    // If keep_files is true, we don't delete files (delete_files = false)
    let delete_files = !params.keep_files.unwrap_or(false);

    let freed_bytes = state
        .storage_manager
        .delete_recording(&id, delete_files)
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ApiError::not_found("Recording")
            } else {
                ApiError::internal(e.to_string())
            }
        })?;

    Ok(Json(ApiResponse::new(DeleteRecordingResponse {
        deleted: true,
        freed_bytes: freed_bytes.unwrap_or(0),
    })))
}

/** Get storage statistics. */
pub async fn get_storage_stats(
    _auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<StorageStatsResponse>> {
    let stats = state.storage_manager.get_stats().await;

    let per_channel: Vec<ChannelStorageStats> = stats
        .per_channel
        .into_iter()
        .map(|cs| ChannelStorageStats {
            channel: cs.channel_name,
            platform: cs.platform,
            count: cs.count,
            size_bytes: cs.size_bytes,
        })
        .collect();

    // Get directory paths from config
    let config = state.config.read();
    let recordings_dir_path = config.storage.recordings_dir.clone();
    let library_dir_path = config.storage.library_dir.clone();
    let recordings_dir_str = recordings_dir_path.to_string_lossy().to_string();
    let library_dir_str = library_dir_path.to_string_lossy().to_string();
    drop(config);

    // Get disk stats for the recordings directory
    let (disk_free_bytes, disk_total_bytes) = get_disk_stats(&recordings_dir_path);

    // Calculate library directory size
    // Note: This is a blocking operation but only called once per request for stats
    let library_size_bytes = calculate_directory_size(&library_dir_path);

    // Check if library_dir is on a different disk
    let (library_disk_free_bytes, library_disk_total_bytes) =
        if recordings_dir_str != library_dir_str {
            let (lib_free, lib_total) = get_disk_stats(&library_dir_path);

            // Compare using device ID (works in Docker and on real filesystems)
            let same_disk = are_same_device(&recordings_dir_path, &library_dir_path);
            tracing::info!("Storage stats: same_disk={} (recordings_dir={}, library_dir={})",
                same_disk, recordings_dir_str, library_dir_str);

            if !same_disk {
                (Some(lib_free), Some(lib_total))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

    Json(ApiResponse::new(StorageStatsResponse {
        total_recordings: stats.total_recordings,
        total_size_bytes: stats.total_size_bytes,
        disk_free_bytes,
        disk_total_bytes,
        per_channel,
        recordings_dir: recordings_dir_str,
        library_dir: library_dir_str,
        library_size_bytes,
        library_disk_free_bytes,
        library_disk_total_bytes,
    }))
}

/** Calculate the total size of all files in a directory (recursively). */
fn calculate_directory_size(path: &std::path::Path) -> u64 {
    if !path.exists() {
        return 0;
    }

    let mut total_size = 0u64;

    fn visit_dir(dir: &std::path::Path, total: &mut u64) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    visit_dir(&path, total);
                } else if let Ok(metadata) = entry.metadata() {
                    *total += metadata.len();
                }
            }
        }
    }

    visit_dir(path, &mut total_size);
    total_size
}

/** Get disk free and total space for the given path. */
fn get_disk_stats(path: &std::path::Path) -> (u64, u64) {
    let disks = Disks::new_with_refreshed_list();

    // Try to get an absolute path for comparison
    let check_path = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => path.to_path_buf(),
    };

    // Convert to string and strip Windows extended-length path prefix (\\?\) if present
    let mut check_str = check_path.to_string_lossy().to_lowercase();
    if check_str.starts_with(r"\\?\") {
        check_str = check_str[4..].to_string();
    }

    // Find the disk that contains the path
    // Sort disks by mount point length (longest first) to find the most specific match
    let mut disk_list: Vec<_> = disks.list().iter().collect();
    disk_list.sort_by(|a, b| {
        b.mount_point()
            .as_os_str()
            .len()
            .cmp(&a.mount_point().as_os_str().len())
    });

    for disk in &disk_list {
        let mount_point = disk.mount_point();
        let mount_str = mount_point.to_string_lossy().to_lowercase();

        // Case-insensitive comparison (works for Windows drive letters)
        if check_str.starts_with(&mount_str) {
            return (disk.available_space(), disk.total_space());
        }
    }

    // Fallback: try to find the root disk or any disk
    if let Some(disk) = disks.list().first() {
        return (disk.available_space(), disk.total_space());
    }

    (0, 0)
}

/**
 * Check if two paths are on the same device/filesystem.
 * Uses disk stats comparison which works reliably in Docker containers
 * (device IDs don't work correctly with Docker bind mounts).
 */
fn are_same_device(path1: &std::path::Path, path2: &std::path::Path) -> bool {
    // Compare disk stats - if total bytes match and are non-zero, they're on the same disk
    // This works reliably in Docker containers where device IDs can differ for bind mounts
    // from the same underlying filesystem
    let (_free1, total1) = get_disk_stats(path1);
    let (_free2, total2) = get_disk_stats(path2);

    // If total bytes match and are non-zero, they're on the same disk
    let same = total1 == total2 && total1 > 0;
    tracing::info!("are_same_device: path1={:?} (total={}), path2={:?} (total={}) -> {}",
        path1, total1, path2, total2, same);
    same
}

/** Trigger post-processing for a recording. */
pub async fn process_recording(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(request): Json<ProcessRecordingRequest>,
) -> Result<Json<ApiResponse<ProcessRecordingResponse>>, (StatusCode, ApiError)> {
    // Get recording from storage manager
    let recording = state
        .storage_manager
        .get_recording(&id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Recording"))?;

    // Build processing mode from request
    let mode = match request.mode.as_deref() {
        Some("transcode") => ProcessingMode::Transcode {
            format: request.format.clone().unwrap_or_else(|| "mp4".to_string()),
            codec: "h265".to_string(),
            preset: "medium".to_string(),
            crf: 23,
        },
        _ => ProcessingMode::Remux {
            format: request.format.unwrap_or_else(|| "mp4".to_string()),
        },
    };

    // Queue the job
    let (_job_id, position) = state
        .processing_manager
        .queue_job(
            id,
            recording.channel_name.clone(),
            recording.platform.to_string(),
            recording.path.clone(),
            mode,
            None, // Use default cleanup setting
            recording.duration_secs,
        )
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(ApiResponse::new(ProcessRecordingResponse {
        status: "queued".to_string(),
        queue_position: position,
    })))
}

/**
 * Cleanup recordings based on criteria (admin only).
 *
 * Supports dry_run mode to preview what would be deleted.
 * Supports location parameter to control what gets deleted:
 * - "recordings": Only delete recording files, keep library files
 * - "library": Only delete library files (Jellyfin exports), keep recordings in index
 * - "both" (default): Delete both recording and library files.
 */
pub async fn cleanup_storage(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Json(request): Json<CleanupRequest>,
) -> Result<Json<ApiResponse<CleanupResponse>>, (StatusCode, ApiError)> {
    // Build filter criteria
    let cutoff_date = request
        .older_than_days
        .map(|days| Utc::now() - Duration::days(days as i64));

    // Get all recordings that match the criteria
    let all_recordings = state
        .storage_manager
        .list_recordings(
            request.channel_name.as_deref(),
            None, // platform filter not needed for cleanup
            request.status.as_ref(),
        )
        .await;

    // Filter by age and channel_id if provided
    // For "library" location, also filter to only recordings that have been exported
    let mut to_cleanup: Vec<RecordingEntry> = all_recordings
        .into_iter()
        .filter(|recording| {
            // Skip active recordings (Recording or Processing status)
            if matches!(
                recording.status,
                RecordingStatus::Recording | RecordingStatus::Processing
            ) {
                return false;
            }

            // Filter by age
            if let Some(cutoff) = cutoff_date {
                if recording.started_at >= cutoff {
                    return false;
                }
            }

            // Filter by channel_id if provided
            if let Some(ref channel_id) = request.channel_id {
                // Try to match by looking up the channel
                let channels = state.channel_manager.get_channels();
                let channel_match = channels
                    .iter()
                    .find(|c| &c.id == channel_id)
                    .map(|c| c.name == recording.channel_name)
                    .unwrap_or(false);
                if !channel_match {
                    return false;
                }
            }

            // For "library" location, only include recordings that have been exported to Jellyfin
            if request.location == CleanupLocation::Library && !recording.jellyfin_exported {
                return false;
            }

            true
        })
        .collect();

    // Sort by age (oldest first)
    to_cleanup.sort_by(|a, b| a.started_at.cmp(&b.started_at));

    let recordings_affected = to_cleanup.len();
    // Calculate estimated bytes to free based on what we're deleting
    let bytes_to_free: u64 = to_cleanup.iter().map(|r| r.size_bytes).sum();

    if request.dry_run {
        // Return preview without deleting
        Ok(Json(ApiResponse::new(CleanupResponse {
            recordings_affected,
            bytes_to_free,
            recordings: Some(to_cleanup),
            dry_run: true,
            recordings_bytes_freed: None,
            library_bytes_freed: None,
        })))
    } else {
        // Actually delete based on location
        let mut recordings_freed = 0u64;
        let mut library_freed = 0u64;
        let mut actual_deleted = 0usize;

        for recording in &to_cleanup {
            let result = match request.location {
                CleanupLocation::Recordings => {
                    // Only delete recording files, remove from index
                    match state.storage_manager.delete_recording_files_only(&recording.id).await {
                        Ok(freed) => {
                            recordings_freed += freed;
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                }
                CleanupLocation::Library => {
                    // Only delete Jellyfin files, keep recording in index
                    match state.storage_manager.cleanup_jellyfin_files(&recording.id).await {
                        Ok(freed) => {
                            library_freed += freed;
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                }
                CleanupLocation::Both => {
                    // Delete both recording and Jellyfin files
                    match state.storage_manager.delete_recording(&recording.id, true).await {
                        Ok(Some(freed)) => {
                            // We don't have separate tracking here, so we estimate
                            // For "both" mode, recording files are the bulk of the size
                            recordings_freed += freed;
                            Ok(())
                        }
                        Ok(None) => Ok(()),
                        Err(e) => Err(e),
                    }
                }
            };

            match result {
                Ok(()) => {
                    actual_deleted += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to cleanup recording {} during cleanup: {}",
                        recording.id,
                        e
                    );
                }
            }
        }

        let total_freed = recordings_freed + library_freed;

        Ok(Json(ApiResponse::new(CleanupResponse {
            recordings_affected: actual_deleted,
            bytes_to_free: total_freed,
            recordings: None,
            dry_run: false,
            recordings_bytes_freed: Some(recordings_freed),
            library_bytes_freed: Some(library_freed),
        })))
    }
}

/**
 * POST /api/recordings/:id/reprocess
 * Manually trigger reprocessing for a specific recording.
 */
pub async fn reprocess_recording(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<ProcessRecordingResponse>>, (StatusCode, ApiError)> {
    // Get the recording
    let recording = state
        .storage_manager
        .get_recording(&id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Recording"))?;

    // Reset processing attempts
    state
        .storage_manager
        .reset_processing_attempts(&id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    // Mark as processing
    state
        .storage_manager
        .mark_processing(&id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    // Build processing mode from config
    let (mode, segment_handling, library_dir) = {
        let config = state.config.read();
        let pp_config = &config.post_processing;
        let mode = match pp_config.output_format.as_str() {
            "mp4_copy" => ProcessingMode::Remux {
                format: "mp4".to_string(),
            },
            "ts_concat" => ProcessingMode::Remux {
                format: "ts".to_string(),
            },
            _ => ProcessingMode::Transcode {
                format: "mp4".to_string(),
                codec: pp_config.encoding.video_codec.clone(),
                preset: pp_config.encoding.preset.clone(),
                crf: pp_config.encoding.crf,
            },
        };
        let segment_handling = pp_config.get_segment_handling();
        let library_dir = config.storage.library_dir.clone();
        (mode, segment_handling, library_dir)
    };

    // Build output directory: library_dir/{platform}/{channel}/
    let output_dir = library_dir
        .join(recording.platform.to_string())
        .join(&recording.channel_name);

    // Queue the job immediately
    let (_job_id, position) = state
        .processing_manager
        .queue_job_with_output_dir(
            id,
            recording.channel_name.clone(),
            recording.platform.to_string(),
            recording.path.clone(),
            output_dir,
            mode,
            Some(segment_handling),
            recording.duration_secs,
        )
        .await
        .map_err(ApiError::internal)?;

    info!(
        "Manually triggered reprocessing for recording {} (queue position: {})",
        id, position
    );

    Ok(Json(ApiResponse::new(ProcessRecordingResponse {
        status: "queued".to_string(),
        queue_position: position,
    })))
}
