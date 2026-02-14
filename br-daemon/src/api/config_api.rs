use crate::api::auth::{AdminUser, AuthUser};
use crate::api::response::{ApiError, ApiResponse};
use crate::api::AppState;
use crate::config::{DownloadsConfig, PostProcessingConfig, SegmentHandling};
use crate::processing::{concatenate_segments, count_segments, delete_segments};
use crate::storage::StorageManager;
use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/** Public config view (excludes sensitive fields). */
#[derive(Debug, Serialize)]
pub struct PublicConfig {
    pub polling: PollingView,
    pub storage: StorageView,
    pub post_processing: PostProcessingView,
    pub notifications: NotificationsView,
}

#[derive(Debug, Serialize)]
pub struct PollingView {
    pub default_interval: u64,
    pub playlist_interval: u64,
}

#[derive(Debug, Serialize)]
pub struct StorageView {
    pub recordings_dir: String,
    pub library_dir: String,
    pub disk_warning_threshold: u8,
}

#[derive(Debug, Serialize)]
pub struct PostProcessingView {
    pub auto_process: bool,
    /** "delete", "concatenate", or "keep". */
    pub segment_handling: String,
    pub format: String,
    pub mode: String,
}

#[derive(Debug, Serialize)]
pub struct NotificationsView {
    pub discord_enabled: bool,
    pub telegram_enabled: bool,
    pub webhook_enabled: bool,
}

/** Request to update config. */
#[derive(Debug, Deserialize)]
pub struct UpdateConfigRequest {
    pub polling: Option<UpdatePolling>,
    pub storage: Option<UpdateStorage>,
    pub post_processing: Option<UpdatePostProcessing>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePolling {
    pub default_interval: Option<u64>,
    pub playlist_interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStorage {
    pub recordings_dir: Option<String>,
    pub library_dir: Option<String>,
    pub disk_warning_threshold: Option<u8>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePostProcessing {
    pub auto_process: Option<bool>,
    /** "delete", "concatenate", or "keep". */
    pub segment_handling: Option<String>,
}

/** Get current configuration (admin only). */
pub async fn get_config(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<PublicConfig>> {
    let config = state.config.read();

    Json(ApiResponse::new(PublicConfig {
        polling: PollingView {
            default_interval: config.polling.default_interval,
            playlist_interval: config.polling.playlist_interval,
        },
        storage: StorageView {
            recordings_dir: config.storage.recordings_dir.to_string_lossy().to_string(),
            library_dir: config.storage.library_dir.to_string_lossy().to_string(),
            disk_warning_threshold: config.storage.disk_warning_threshold,
        },
        post_processing: PostProcessingView {
            auto_process: config.post_processing.enabled,
            segment_handling: config.post_processing.get_segment_handling().to_string(),
            format: config.post_processing.output_format.clone(),
            mode: "remux".to_string(),
        },
        notifications: NotificationsView {
            discord_enabled: config.notifications.discord.is_some(),
            telegram_enabled: config.notifications.telegram.is_some(),
            webhook_enabled: config.notifications.webhook.is_some(),
        },
    }))
}

/** Update configuration (admin only). */
pub async fn update_config(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Json(request): Json<UpdateConfigRequest>,
) -> Result<Json<ApiResponse<PublicConfig>>, (StatusCode, ApiError)> {
    {
        let mut config = state.config.write();

        if let Some(polling) = request.polling {
            if let Some(interval) = polling.default_interval {
                config.polling.default_interval = interval;
            }
            if let Some(interval) = polling.playlist_interval {
                config.polling.playlist_interval = interval;
            }
        }

        if let Some(storage) = request.storage {
            if let Some(dir) = storage.recordings_dir {
                config.storage.recordings_dir = std::path::PathBuf::from(dir);
            }
            if let Some(dir) = storage.library_dir {
                config.storage.library_dir = std::path::PathBuf::from(dir);
            }
            if let Some(threshold) = storage.disk_warning_threshold {
                config.storage.disk_warning_threshold = threshold;
            }
        }

        if let Some(pp) = request.post_processing {
            if let Some(auto) = pp.auto_process {
                config.post_processing.enabled = auto;
            }
            if let Some(handling) = pp.segment_handling {
                if let Ok(parsed) = handling.parse() {
                    config.post_processing.segment_handling = parsed;
                }
            }
        }
    }

    // Return updated config
    let config = state.config.read();
    Ok(Json(ApiResponse::new(PublicConfig {
        polling: PollingView {
            default_interval: config.polling.default_interval,
            playlist_interval: config.polling.playlist_interval,
        },
        storage: StorageView {
            recordings_dir: config.storage.recordings_dir.to_string_lossy().to_string(),
            library_dir: config.storage.library_dir.to_string_lossy().to_string(),
            disk_warning_threshold: config.storage.disk_warning_threshold,
        },
        post_processing: PostProcessingView {
            auto_process: config.post_processing.enabled,
            segment_handling: config.post_processing.get_segment_handling().to_string(),
            format: config.post_processing.output_format.clone(),
            mode: "remux".to_string(),
        },
        notifications: NotificationsView {
            discord_enabled: config.notifications.discord.is_some(),
            telegram_enabled: config.notifications.telegram.is_some(),
            webhook_enabled: config.notifications.webhook.is_some(),
        },
    })))
}

/** Response for post-processing config (matches TypeScript interface). */
#[derive(Debug, Serialize)]
pub struct PostProcessingConfigResponse {
    pub enabled: bool,
    pub check_interval_minutes: u32,
    pub output_format: String,
    /** What to do with segment files after processing: "delete", "concatenate", "keep". */
    pub segment_handling: String,
    pub encoding: EncodingConfigResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ffmpeg_path: Option<String>,
    pub max_concurrent: u8,
}

#[derive(Debug, Serialize)]
pub struct EncodingConfigResponse {
    pub crf: u8,
    pub preset: String,
    pub video_codec: String,
    pub audio_codec: String,
    pub audio_bitrate: String,
}

impl From<&PostProcessingConfig> for PostProcessingConfigResponse {
    fn from(config: &PostProcessingConfig) -> Self {
        Self {
            enabled: config.enabled,
            check_interval_minutes: config.check_interval_minutes,
            output_format: config.output_format.clone(),
            segment_handling: config.get_segment_handling().to_string(),
            encoding: EncodingConfigResponse {
                crf: config.encoding.crf,
                preset: config.encoding.preset.clone(),
                video_codec: config.encoding.video_codec.clone(),
                audio_codec: config.encoding.audio_codec.clone(),
                audio_bitrate: config.encoding.audio_bitrate.clone(),
            },
            ffmpeg_path: config
                .ffmpeg_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            max_concurrent: config.max_concurrent,
        }
    }
}

/** Request to update post-processing config. */
#[derive(Debug, Deserialize)]
pub struct UpdatePostProcessingConfigRequest {
    pub enabled: Option<bool>,
    pub check_interval_minutes: Option<u32>,
    pub output_format: Option<String>,
    /** What to do with segment files: "delete", "concatenate", "keep". */
    pub segment_handling: Option<String>,
    pub encoding: Option<UpdateEncodingConfig>,
    pub max_concurrent: Option<u8>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEncodingConfig {
    pub crf: Option<u8>,
    pub preset: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub audio_bitrate: Option<String>,
}

/**
 * GET /api/config/post-processing
 * Get current post-processing configuration.
 */
pub async fn get_post_processing_config(
    _auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<PostProcessingConfigResponse>> {
    let config = state.config.read();
    Json(ApiResponse::new(PostProcessingConfigResponse::from(
        &config.post_processing,
    )))
}

/**
 * PUT /api/config/post-processing
 * Update post-processing configuration (admin only).
 */
pub async fn update_post_processing_config(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Json(request): Json<UpdatePostProcessingConfigRequest>,
) -> Result<Json<ApiResponse<PostProcessingConfigResponse>>, (StatusCode, ApiError)> {
    // Track if segment_handling changed
    let new_segment_handling: Option<SegmentHandling> = request
        .segment_handling
        .as_ref()
        .and_then(|s| s.parse().ok());

    {
        let mut config = state.config.write();

        if let Some(enabled) = request.enabled {
            config.post_processing.enabled = enabled;
        }
        if let Some(interval) = request.check_interval_minutes {
            config.post_processing.check_interval_minutes = interval;
        }
        if let Some(format) = request.output_format {
            config.post_processing.output_format = format;
        }
        if let Some(handling) = new_segment_handling {
            config.post_processing.segment_handling = handling;
        }
        if let Some(max) = request.max_concurrent {
            config.post_processing.max_concurrent = max;
        }

        if let Some(encoding) = request.encoding {
            if let Some(crf) = encoding.crf {
                config.post_processing.encoding.crf = crf;
            }
            if let Some(preset) = encoding.preset {
                config.post_processing.encoding.preset = preset;
            }
            if let Some(codec) = encoding.video_codec {
                config.post_processing.encoding.video_codec = codec;
            }
            if let Some(codec) = encoding.audio_codec {
                config.post_processing.encoding.audio_codec = codec;
            }
            if let Some(bitrate) = encoding.audio_bitrate {
                config.post_processing.encoding.audio_bitrate = bitrate;
            }
        }

        // Save to disk
        if let Err(e) = config.save(&state.config_path) {
            tracing::error!("Failed to save config: {}", e);
            return Err(ApiError::internal(&format!("Failed to save config: {}", e)));
        }
    }

    // If segment handling was changed to delete or concatenate, apply retroactively
    if let Some(handling) = new_segment_handling {
        if matches!(
            handling,
            SegmentHandling::Delete | SegmentHandling::Concatenate
        ) {
            let storage = state.storage_manager.clone();
            tokio::spawn(async move {
                apply_segment_handling_retroactively(storage, handling).await;
            });
        }
    }

    // Return updated config
    let config = state.config.read();
    Ok(Json(ApiResponse::new(PostProcessingConfigResponse::from(
        &config.post_processing,
    ))))
}

/** Response for downloads config (matches TypeScript DownloadsConfig interface). */
#[derive(Debug, Serialize)]
pub struct DownloadsConfigResponse {
    pub directory: String,
    pub max_concurrent: u8,
    pub default_format: String,
    pub embed_thumbnail: bool,
    pub embed_metadata: bool,
    pub output_template: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_total_gb: Option<u64>,
    pub retention: DownloadRetentionView,
}

#[derive(Debug, Serialize)]
pub struct DownloadRetentionView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_age_days: Option<u32>,
    pub cleanup_interval_hours: u32,
}

impl From<&DownloadsConfig> for DownloadsConfigResponse {
    fn from(config: &DownloadsConfig) -> Self {
        Self {
            directory: config.directory.to_string_lossy().to_string(),
            max_concurrent: config.max_concurrent,
            default_format: config.default_format.clone(),
            embed_thumbnail: config.embed_thumbnail,
            embed_metadata: config.embed_metadata,
            output_template: config.output_template.clone(),
            max_total_gb: config.max_total_gb,
            retention: DownloadRetentionView {
                max_age_days: config.retention.max_age_days,
                cleanup_interval_hours: config.retention.cleanup_interval_hours,
            },
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateDownloadsConfigRequest {
    pub directory: Option<String>,
    pub max_concurrent: Option<u8>,
    pub default_format: Option<String>,
    pub embed_thumbnail: Option<bool>,
    pub embed_metadata: Option<bool>,
    pub output_template: Option<String>,
    pub max_total_gb: Option<u64>,
    pub retention: Option<UpdateRetention>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRetention {
    pub max_age_days: Option<u32>,
    pub cleanup_interval_hours: Option<u32>,
}

/// GET /api/config/downloads
pub async fn get_downloads_config(
    _auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<DownloadsConfigResponse>> {
    let config = state.config.read();
    Json(ApiResponse::new(DownloadsConfigResponse::from(&config.downloads)))
}

/// PUT /api/config/downloads
pub async fn update_downloads_config(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Json(request): Json<UpdateDownloadsConfigRequest>,
) -> Result<Json<ApiResponse<DownloadsConfigResponse>>, (StatusCode, ApiError)> {
    {
        let mut config = state.config.write();

        if let Some(dir) = request.directory {
            config.downloads.directory = std::path::PathBuf::from(dir);
        }
        if let Some(max) = request.max_concurrent {
            config.downloads.max_concurrent = max;
        }
        if let Some(fmt) = request.default_format {
            config.downloads.default_format = fmt;
        }
        if let Some(thumb) = request.embed_thumbnail {
            config.downloads.embed_thumbnail = thumb;
        }
        if let Some(meta) = request.embed_metadata {
            config.downloads.embed_metadata = meta;
        }
        if let Some(tmpl) = request.output_template {
            config.downloads.output_template = tmpl;
        }
        if let Some(gb) = request.max_total_gb {
            config.downloads.max_total_gb = Some(gb);
        }
        if let Some(ret) = request.retention {
            if let Some(days) = ret.max_age_days {
                config.downloads.retention.max_age_days = Some(days);
            }
            if let Some(hours) = ret.cleanup_interval_hours {
                config.downloads.retention.cleanup_interval_hours = hours;
            }
        }

        if let Err(e) = config.save(&state.config_path) {
            tracing::error!("Failed to save config: {}", e);
            return Err(ApiError::internal(&format!("Failed to save config: {}", e)));
        }
    }

    let config = state.config.read();
    Ok(Json(ApiResponse::new(DownloadsConfigResponse::from(&config.downloads))))
}

/** Apply segment handling retroactively to all processed recordings that still have .ts segments. */
async fn apply_segment_handling_retroactively(
    storage: Arc<StorageManager>,
    handling: SegmentHandling,
) {
    tracing::info!(
        "Applying segment handling {:?} retroactively to processed recordings",
        handling
    );

    let recordings = storage.get_processed_recordings().await;
    let mut processed_count = 0;

    for recording in recordings {
        // Check if there are .ts segment files in the recording path
        let segment_count = count_segments(&recording.path).await;
        if segment_count == 0 {
            continue; // No segments to process
        }

        tracing::info!(
            "Retroactively applying {:?} to {} ({}) with {} segments",
            handling,
            recording.channel_name,
            recording.id,
            segment_count
        );

        match handling {
            SegmentHandling::Delete => match delete_segments(&recording.path).await {
                Ok(count) => {
                    tracing::info!(
                        "Retroactively deleted {} segment files from {} ({})",
                        count,
                        recording.channel_name,
                        recording.id
                    );
                    processed_count += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to retroactively delete segments for {} ({}): {}",
                        recording.channel_name,
                        recording.id,
                        e
                    );
                }
            },
            SegmentHandling::Concatenate => {
                match concatenate_segments(&recording.path, &recording.channel_name).await {
                    Ok(path) => {
                        tracing::info!(
                            "Retroactively concatenated segments for {} ({}) to {:?}",
                            recording.channel_name,
                            recording.id,
                            path
                        );
                        processed_count += 1;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to retroactively concatenate segments for {} ({}): {}",
                            recording.channel_name,
                            recording.id,
                            e
                        );
                    }
                }
            }
            SegmentHandling::Keep => {
                // Nothing to do
            }
        }
    }

    tracing::info!(
        "Retroactive segment handling complete: processed {} recordings",
        processed_count
    );
}
