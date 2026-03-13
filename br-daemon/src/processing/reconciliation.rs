//! Background worker for post-processing reconciliation.
//!
//! Scans for unprocessed recordings on startup and periodically,
//! then queues them for post-processing.

use crate::config::PostProcessingConfig;
use crate::storage::{RecordingEntry, StorageManager};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, warn};

use super::{FfmpegRunner, InputSource, ProcessingManager, ProcessingMode};

/** Background worker that reconciles unprocessed recordings. */
pub struct ReconciliationWorker {
    storage: Arc<StorageManager>,
    processing: Arc<ProcessingManager>,
    config: PostProcessingConfig,
    /** Directory for processed output files (library_dir from storage config). */
    library_dir: PathBuf,
    shutdown_rx: mpsc::Receiver<()>,
}

impl ReconciliationWorker {
    pub fn new(
        storage: Arc<StorageManager>,
        processing: Arc<ProcessingManager>,
        config: PostProcessingConfig,
        library_dir: PathBuf,
        shutdown_rx: mpsc::Receiver<()>,
    ) -> Self {
        Self {
            storage,
            processing,
            config,
            library_dir,
            shutdown_rx,
        }
    }

    /**
     * Run the reconciliation worker.
     *
     * Performs initial scan on startup, then runs periodically based on config.
     */
    pub async fn run(mut self) {
        if !self.config.enabled {
            info!("Post-processing reconciliation disabled");
            return;
        }

        info!(
            "Post-processing reconciliation worker started (interval: {} minutes)",
            self.config.check_interval_minutes
        );

        // Initial scan on startup
        self.reconcile().await;

        let interval = Duration::from_secs(self.config.check_interval_minutes as u64 * 60);

        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {
                    self.reconcile().await;
                }
                _ = self.shutdown_rx.recv() => {
                    info!("Reconciliation worker received shutdown signal");
                    break;
                }
            }
        }

        info!("Reconciliation worker stopped");
    }

    /**
     * Scan for unprocessed recordings and queue them.
     * Also updates stats for processed recordings that are missing duration.
     */
    async fn reconcile(&self) {
        info!("Running post-processing reconciliation scan...");

        // First, update stats for any processed recordings missing duration
        self.update_missing_stats().await;

        // Get recordings that need processing
        let unprocessed = self.storage.get_unprocessed_recordings().await;

        info!(
            "Reconciliation: found {} recordings needing post-processing",
            unprocessed.len()
        );

        if unprocessed.is_empty() {
            return;
        }

        // Log each recording found
        for entry in &unprocessed {
            info!(
                "  - {} ({}) status={:?} path={:?}",
                entry.channel_name, entry.id, entry.status, entry.path
            );
        }

        // Get channels that are currently recording (skip their recordings)
        let active_channels = self.storage.get_active_recording_channel_names().await;
        if !active_channels.is_empty() {
            info!(
                "Active recording channels (will skip): {:?}",
                active_channels
            );
        }

        for entry in unprocessed {
            // Skip if channel is currently recording
            if active_channels.contains(&entry.channel_name) {
                info!(
                    "Skipping {} ({}) - channel is currently recording",
                    entry.channel_name, entry.id
                );
                continue;
            }

            // Queue for processing
            info!(
                "Attempting to queue {} ({}) for processing...",
                entry.channel_name, entry.id
            );
            if let Err(e) = self.queue_recording(&entry).await {
                warn!(
                    "Failed to queue recording {} ({}) for processing: {}",
                    entry.channel_name, entry.id, e
                );
            }
        }
    }

    /** Queue a single recording for post-processing. */
    async fn queue_recording(&self, entry: &RecordingEntry) -> Result<(), String> {
        info!(
            "queue_recording: {} ({}) checking disk at {:?}",
            entry.channel_name, entry.id, entry.path
        );

        // Check if there's a valid input source (segments, concatenated file, or output file)
        let input_source =
            FfmpegRunner::find_input_source(&entry.path, entry.output_file.as_deref()).await;

        let (segment_count, total_size) = match &input_source {
            Ok(InputSource::ConcatList(_)) => {
                // We have numbered segments, scan them
                let stats = Self::scan_segments(&entry.path).await;
                info!(
                    "queue_recording: {} ({}) - found {} numbered segments ({} bytes) on disk",
                    entry.channel_name, entry.id, stats.0, stats.1
                );
                stats
            }
            Ok(InputSource::SingleFile(path)) => {
                // We have a single file (concatenated or previous output)
                let size = tokio::fs::metadata(path)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(0);
                info!(
                    "queue_recording: {} ({}) - found concatenated/output file {:?} ({} bytes)",
                    entry.channel_name, entry.id, path, size
                );
                (1, size) // Count as 1 "segment" for estimation purposes
            }
            Err(e) => {
                // No valid input source - mark as failed so it won't be retried forever
                let reason = e.to_string();
                warn!(
                    "queue_recording: {} ({}) - NO valid input source found: {}",
                    entry.channel_name, entry.id, reason
                );

                // Mark as processing failed to increment attempt counter
                // After 5 attempts, it won't be retried anymore
                if let Err(mark_err) = self
                    .storage
                    .mark_processing_failed(&entry.id, Some(reason.clone()))
                    .await
                {
                    warn!(
                        "Failed to mark recording {} as failed: {}",
                        entry.id, mark_err
                    );
                } else {
                    info!(
                        "Marked recording {} ({}) as processing failed (no valid input): {}",
                        entry.channel_name, entry.id, reason
                    );
                }

                return Ok(());
            }
        };

        // Update size/segment_count/duration in index if they were 0 (legacy data)
        let needs_size_update = entry.size_bytes == 0 || entry.segment_count == 0;
        let needs_duration_update = entry.duration_secs.is_none() || entry.duration_secs == Some(0);

        if needs_size_update || needs_duration_update {
            // Estimate duration from segment count (assuming ~2 second segments)
            let estimated_duration = if needs_duration_update {
                Some((segment_count as u64) * 2)
            } else {
                None
            };

            if let Err(e) = self
                .storage
                .update_recording_stats(
                    &entry.id,
                    total_size,
                    segment_count as u32,
                    estimated_duration,
                )
                .await
            {
                warn!("Failed to update recording stats: {}", e);
            } else {
                info!(
                    "Updated recording {} stats: {} bytes, {} segments, duration={:?}s",
                    entry.id, total_size, segment_count, estimated_duration
                );
            }
        }

        let mode = self.build_processing_mode();
        let segment_handling = self.config.get_segment_handling();
        info!(
            "queue_recording: {} ({}) - segment_handling={:?}",
            entry.channel_name, entry.id, segment_handling
        );

        // Mark as processing first
        self.storage
            .mark_processing(&entry.id)
            .await
            .map_err(|e| e.to_string())?;

        // Use entry duration if available, otherwise estimate from segment count
        let duration = entry
            .duration_secs
            .or_else(|| Some((segment_count as u64) * 2));

        // Build output directory: library_dir/{platform}/{channel}/
        let output_dir = self
            .library_dir
            .join(entry.platform.to_string())
            .join(&entry.channel_name);

        info!(
            "queue_recording: {} ({}) - output_dir={:?}",
            entry.channel_name, entry.id, output_dir
        );

        self.processing
            .queue_job_with_output_dir(
                entry.id,
                entry.channel_name.clone(),
                entry.platform.to_string(),
                entry.path.clone(),
                output_dir,
                mode,
                Some(segment_handling),
                duration,
            )
            .await?;

        info!(
            "Queued recording {} ({}) for post-processing",
            entry.channel_name, entry.id
        );

        Ok(())
    }

    /** Update stats (size, duration) for processed recordings that are missing them. */
    async fn update_missing_stats(&self) {
        let processed = self.storage.get_processed_recordings().await;

        for entry in processed {
            // Check if duration is missing
            let needs_duration = entry.duration_secs.is_none() || entry.duration_secs == Some(0);

            if !needs_duration {
                continue;
            }

            // Try multiple methods to get duration:
            // 1. Probe output file with ffprobe (most accurate)
            // 2. Estimate from segment count if segments still exist

            let mut probed_duration: Option<u64> = None;

            // Method 1: Probe output file
            if let Some(ref output_file) = entry.output_file {
                if output_file.exists() {
                    if let Some(duration) = FfmpegRunner::probe_duration(output_file).await {
                        probed_duration = Some(duration);
                        info!(
                            "Probed duration for {} ({}): {}s from {:?}",
                            entry.channel_name, entry.id, duration, output_file
                        );
                    }
                }
            }

            // Method 2: Estimate from segments if probing failed
            if probed_duration.is_none() {
                let (segment_count, _) = Self::scan_segments(&entry.path).await;
                if segment_count > 0 {
                    let estimated_duration = (segment_count as u64) * 2;
                    probed_duration = Some(estimated_duration);
                    info!(
                        "Estimated duration for {} ({}) from {} segments: {}s",
                        entry.channel_name, entry.id, segment_count, estimated_duration
                    );
                }
            }

            // Update if we found a duration
            if let Some(duration) = probed_duration {
                if let Err(e) = self
                    .storage
                    .update_recording_stats(
                        &entry.id,
                        entry.size_bytes,
                        entry.segment_count,
                        Some(duration),
                    )
                    .await
                {
                    warn!(
                        "Failed to update duration for {} ({}): {}",
                        entry.channel_name, entry.id, e
                    );
                } else {
                    info!(
                        "Updated {} ({}) duration: {}s",
                        entry.channel_name, entry.id, duration
                    );
                }
            }
        }
    }

    /**
     * Scan .ts segment files in a recording directory.
     * Only counts numbered segment files (e.g., 0000001.ts), not concatenated outputs.
     * Returns (segment_count, total_size_bytes).
     */
    async fn scan_segments(path: &Path) -> (usize, u64) {
        if !path.exists() {
            return (0, 0);
        }

        let mut count = 0usize;
        let mut total_size = 0u64;

        let mut entries = match tokio::fs::read_dir(path).await {
            Ok(entries) => entries,
            Err(_) => return (0, 0),
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let entry_path = entry.path();
            // Only count .ts files with numeric filenames (segment files)
            let is_ts = entry_path
                .extension()
                .map(|ext| ext == "ts")
                .unwrap_or(false);
            let is_numeric = entry_path
                .file_stem()
                .and_then(|s| s.to_string_lossy().parse::<u64>().ok())
                .is_some();

            if is_ts && is_numeric {
                if let Ok(metadata) = entry.metadata().await {
                    total_size += metadata.len();
                }
                count += 1;
            }
        }

        (count, total_size)
    }

    /** Build ProcessingMode from config. */
    fn build_processing_mode(&self) -> ProcessingMode {
        match self.config.output_format.as_str() {
            "mp4_copy" => ProcessingMode::Remux {
                format: "mp4".to_string(),
            },
            "ts_concat" => ProcessingMode::Remux {
                format: "ts".to_string(),
            },
            _ => ProcessingMode::Transcode {
                format: "mp4".to_string(),
                codec: self.config.encoding.video_codec.clone(),
                preset: self.config.encoding.preset.clone(),
                crf: self.config.encoding.crf,
            },
        }
    }
}
