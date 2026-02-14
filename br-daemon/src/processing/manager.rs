use super::{FfmpegRunner, ProcessingJob, ProcessingMode, ProcessingProgress};
use crate::config::SegmentHandling;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock, Semaphore};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/**
 * Events emitted by the ProcessingManager.
 *
 * These events are broadcast to all subscribers and can be used to
 * update UI, send notifications, or trigger other actions.
 */
#[derive(Debug, Clone)]
pub enum ProcessingEvent {
    /** A processing job has started. */
    Started { recording_id: Uuid },
    /** Progress update for a processing job. */
    Progress { recording_id: Uuid, percent: u8 },
    /** Processing completed successfully. */
    Complete {
        recording_id: Uuid,
        output_file: String,
        size_bytes: u64,
    },
    /** Processing failed with an error. */
    Failed { recording_id: Uuid, error: String },
}

/**
 * Manages the post-processing queue and job execution.
 *
 * The ProcessingManager handles:
 * - Queueing processing jobs
 * - Running FFmpeg in background workers (concurrent)
 * - Broadcasting progress and status events
 * - Segment handling after processing (delete, concatenate, or keep)
 */
pub struct ProcessingManager {
    /** FFmpeg command runner. */
    ffmpeg: FfmpegRunner,
    /** Queue of pending jobs. */
    queue: Arc<RwLock<VecDeque<ProcessingJob>>>,
    /** IDs of currently processing jobs. */
    current_jobs: Arc<RwLock<Vec<Uuid>>>,
    /** Broadcast sender for processing events. */
    event_tx: broadcast::Sender<ProcessingEvent>,
    /** Channel to submit new jobs to the worker. */
    job_tx: mpsc::Sender<ProcessingJob>,
    /** Default setting for segment handling. */
    segment_handling_default: SegmentHandling,
    /** Maximum concurrent processing jobs. */
    max_concurrent: u8,
}

impl ProcessingManager {
    /**
     * Create a new ProcessingManager.
     *
     * This spawns a background worker task that processes jobs from the queue.
     *
     * # Arguments
     * * `ffmpeg_path` - Optional path to ffmpeg binary, uses PATH lookup if None
     * * `segment_handling_default` - Default handling for segment files after processing
     * * `max_concurrent` - Maximum number of concurrent processing jobs (default: 1)
     *
     * # Returns
     * A tuple of (ProcessingManager, broadcast::Receiver<ProcessingEvent>)
     */
    pub fn new(
        ffmpeg_path: Option<PathBuf>,
        segment_handling_default: SegmentHandling,
        max_concurrent: u8,
    ) -> (Self, broadcast::Receiver<ProcessingEvent>) {
        let max_concurrent = max_concurrent.max(1); // Ensure at least 1
        let ffmpeg = FfmpegRunner::new(ffmpeg_path.clone());
        let queue = Arc::new(RwLock::new(VecDeque::new()));
        let current_jobs = Arc::new(RwLock::new(Vec::new()));
        let (event_tx, event_rx) = broadcast::channel(256);
        let (job_tx, job_rx) = mpsc::channel(64);

        info!(
            "Processing manager initialized with max_concurrent={}",
            max_concurrent
        );

        // Spawn the worker dispatcher task
        let worker = ProcessingWorker {
            ffmpeg_path,
            queue: Arc::clone(&queue),
            current_jobs: Arc::clone(&current_jobs),
            event_tx: event_tx.clone(),
            job_rx,
            semaphore: Arc::new(Semaphore::new(max_concurrent as usize)),
        };
        tokio::spawn(worker.run());

        let manager = Self {
            ffmpeg,
            queue,
            current_jobs,
            event_tx,
            job_tx,
            segment_handling_default,
            max_concurrent,
        };

        (manager, event_rx)
    }

    /** Check if FFmpeg is available. */
    pub async fn check_ffmpeg(&self) -> bool {
        self.ffmpeg.check_available().await
    }

    /**
     * Queue a new processing job.
     *
     * # Arguments
     * * `recording_id` - ID of the recording to process
     * * `channel_name` - Channel name for display and filename generation
     * * `platform` - Platform name for display
     * * `recording_path` - Directory containing the recording segments
     * * `mode` - Processing mode (remux or transcode)
     * * `segment_handling` - Override for segment handling, uses default if None
     * * `duration_secs` - Duration of the recording in seconds (for progress calculation)
     *
     * # Returns
     * A tuple of (job_id, queue_position) on success.
     */
    pub async fn queue_job(
        &self,
        recording_id: Uuid,
        channel_name: String,
        platform: String,
        recording_path: PathBuf,
        mode: ProcessingMode,
        segment_handling: Option<SegmentHandling>,
        duration_secs: Option<u64>,
    ) -> Result<(Uuid, usize), String> {
        self.queue_job_with_output_dir(
            recording_id,
            channel_name,
            platform,
            recording_path.clone(),
            recording_path, // Default: output to same dir
            mode,
            segment_handling,
            duration_secs,
        )
        .await
    }

    /**
     * Queue a new processing job with a specific output directory.
     *
     * # Arguments
     * * `recording_id` - ID of the recording to process
     * * `channel_name` - Channel name for display and filename generation
     * * `platform` - Platform name for display
     * * `recording_path` - Directory containing the recording segments
     * * `output_dir` - Directory for the processed output file
     * * `mode` - Processing mode (remux or transcode)
     * * `segment_handling` - Override for segment handling, uses default if None
     * * `duration_secs` - Duration of the recording in seconds (for progress calculation)
     *
     * # Returns
     * A tuple of (job_id, queue_position) on success.
     */
    pub async fn queue_job_with_output_dir(
        &self,
        recording_id: Uuid,
        channel_name: String,
        platform: String,
        recording_path: PathBuf,
        output_dir: PathBuf,
        mode: ProcessingMode,
        segment_handling: Option<SegmentHandling>,
        duration_secs: Option<u64>,
    ) -> Result<(Uuid, usize), String> {
        let handling = segment_handling.unwrap_or(self.segment_handling_default);

        let job = ProcessingJob::with_output_dir(
            recording_id,
            channel_name,
            platform,
            recording_path,
            output_dir,
            mode,
            handling,
            duration_secs,
        );

        let job_id = job.id;

        // Add to queue
        let position = {
            let mut queue = self.queue.write().await;
            queue.push_back(job.clone());
            queue.len() - 1
        };

        // Send to worker
        self.job_tx
            .send(job)
            .await
            .map_err(|e| format!("Failed to submit job to worker: {}", e))?;

        info!(
            "Queued processing job {} for recording {} at position {}",
            job_id, recording_id, position
        );

        Ok((job_id, position))
    }

    /**
     * Get the current queue status.
     *
     * # Returns
     * A tuple of (current_job_ids, queued_jobs) where:
     * - current_job_ids is a list of currently processing job IDs
     * - queued_jobs is a list of (job_id, recording_id, position) tuples.
     */
    pub async fn get_queue_status(&self) -> (Vec<Uuid>, Vec<(Uuid, Uuid, usize)>) {
        let current = self.current_jobs.read().await.clone();
        let queue = self.queue.read().await;

        let queued: Vec<(Uuid, Uuid, usize)> = queue
            .iter()
            .enumerate()
            .map(|(i, job)| (job.id, job.recording_id, i))
            .collect();

        (current, queued)
    }

    /** Get the maximum concurrent processing jobs. */
    pub fn get_max_concurrent(&self) -> u8 {
        self.max_concurrent
    }

    /**
     * Cancel a processing job.
     *
     * If the job is currently processing, this will signal cancellation
     * (actual cancellation implementation is TODO).
     * If the job is queued, it will be removed from the queue.
     *
     * # Arguments
     * * `job_id` - ID of the job to cancel
     *
     * # Returns
     * true if the job was found and cancelled/removed, false otherwise.
     */
    pub async fn cancel_job(&self, job_id: Uuid) -> bool {
        // Check if it's a currently running job
        {
            let current_jobs = self.current_jobs.read().await;
            if current_jobs.contains(&job_id) {
                // TODO: Actually implement cancellation (kill FFmpeg process)
                warn!(
                    "Cancellation of running job {} requested but not yet implemented",
                    job_id
                );
                return false;
            }
        }

        // Try to remove from queue
        let mut queue = self.queue.write().await;
        if let Some(pos) = queue.iter().position(|j| j.id == job_id) {
            queue.remove(pos);
            info!("Removed job {} from queue at position {}", job_id, pos);
            return true;
        }

        debug!("Job {} not found in current jobs or queue", job_id);
        false
    }

    /**
     * Subscribe to processing events.
     *
     * Returns a new receiver for processing events. Multiple subscribers
     * can exist simultaneously.
     */
    pub fn subscribe(&self) -> broadcast::Receiver<ProcessingEvent> {
        self.event_tx.subscribe()
    }
}

/**
 * Background worker dispatcher that processes jobs from the queue.
 * Uses a semaphore to limit concurrent FFmpeg processes.
 */
struct ProcessingWorker {
    ffmpeg_path: Option<PathBuf>,
    queue: Arc<RwLock<VecDeque<ProcessingJob>>>,
    current_jobs: Arc<RwLock<Vec<Uuid>>>,
    event_tx: broadcast::Sender<ProcessingEvent>,
    job_rx: mpsc::Receiver<ProcessingJob>,
    semaphore: Arc<Semaphore>,
}

impl ProcessingWorker {
    /**
     * Run the worker dispatcher loop.
     *
     * This continuously waits for jobs from the channel and spawns concurrent
     * tasks to process them, limited by the semaphore.
     */
    async fn run(mut self) {
        info!("Processing worker dispatcher started");

        while let Some(job) = self.job_rx.recv().await {
            // Remove from queue (should be at the front)
            {
                let mut queue = self.queue.write().await;
                if let Some(pos) = queue.iter().position(|j| j.id == job.id) {
                    queue.remove(pos);
                }
            }

            // Clone everything needed for the spawned task
            let semaphore = Arc::clone(&self.semaphore);
            let current_jobs = Arc::clone(&self.current_jobs);
            let event_tx = self.event_tx.clone();
            let ffmpeg_path = self.ffmpeg_path.clone();

            // Spawn a task to process this job
            tokio::spawn(async move {
                // Acquire semaphore permit (blocks if at max concurrent)
                let _permit = match semaphore.acquire().await {
                    Ok(permit) => permit,
                    Err(_) => {
                        tracing::error!(
                            job_id = %job.id,
                            recording_id = %job.recording_id,
                            "Semaphore closed, cannot process job"
                        );
                        return;
                    }
                };

                // Add to current jobs
                {
                    let mut jobs = current_jobs.write().await;
                    jobs.push(job.id);
                }

                // Emit Started event
                let _ = event_tx.send(ProcessingEvent::Started {
                    recording_id: job.recording_id,
                });

                info!(
                    "Starting processing job {} for recording {} ({})",
                    job.id, job.recording_id, job.channel_name
                );

                // Run FFmpeg with progress forwarding
                let result = Self::process_job_static(&job, &event_tx, ffmpeg_path).await;

                match result {
                    Ok((output_file, size_bytes)) => {
                        info!(
                            "Processing job {} completed: {} ({} bytes), segment_handling={:?}",
                            job.id, output_file, size_bytes, job.segment_handling
                        );

                        // Handle segments based on configuration
                        match job.segment_handling {
                            SegmentHandling::Delete => {
                                match delete_segments(&job.recording_path).await {
                                    Ok(count) => {
                                        info!(
                                            "Deleted {} segment files from {:?}",
                                            count, job.recording_path
                                        );
                                    }
                                    Err(e) => {
                                        warn!("Failed to delete segments: {}", e);
                                    }
                                }
                            }
                            SegmentHandling::Concatenate => {
                                match concatenate_segments(&job.recording_path, &job.channel_name)
                                    .await
                                {
                                    Ok(concat_path) => {
                                        info!("Concatenated segments to {:?}", concat_path);
                                    }
                                    Err(e) => {
                                        warn!("Failed to concatenate segments: {}", e);
                                    }
                                }
                            }
                            SegmentHandling::Keep => {
                                info!("Keeping all segment files in {:?}", job.recording_path);
                            }
                        }

                        // Emit Complete event
                        let _ = event_tx.send(ProcessingEvent::Complete {
                            recording_id: job.recording_id,
                            output_file,
                            size_bytes,
                        });
                    }
                    Err(e) => {
                        error!("Processing job {} failed: {}", job.id, e);

                        // Emit Failed event
                        let _ = event_tx.send(ProcessingEvent::Failed {
                            recording_id: job.recording_id,
                            error: e.to_string(),
                        });
                    }
                }

                // Remove from current jobs
                {
                    let mut jobs = current_jobs.write().await;
                    jobs.retain(|&id| id != job.id);
                }

                // Permit is automatically released when _permit is dropped
            });
        }

        info!("Processing worker dispatcher shutting down");
    }

    /**
     * Process a single job (static method for use in spawned tasks).
     *
     * Returns the output file path and size on success.
     */
    async fn process_job_static(
        job: &ProcessingJob,
        event_tx: &broadcast::Sender<ProcessingEvent>,
        ffmpeg_path: Option<PathBuf>,
    ) -> anyhow::Result<(String, u64)> {
        // Ensure output directory exists
        if let Some(parent) = job.output_path.parent() {
            if !parent.exists() {
                info!("Creating output directory: {:?}", parent);
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        let (progress_tx, mut progress_rx) = mpsc::channel::<ProcessingProgress>(32);

        // Spawn a task to forward progress events
        let event_tx_clone = event_tx.clone();
        let recording_id = job.recording_id;
        let progress_forwarder = tokio::spawn(async move {
            let mut last_percent = 0u8;
            while let Some(progress) = progress_rx.recv().await {
                // Only emit if percent changed (avoid flooding)
                if progress.percent != last_percent {
                    last_percent = progress.percent;
                    let _ = event_tx_clone.send(ProcessingEvent::Progress {
                        recording_id,
                        percent: progress.percent,
                    });
                }
            }
        });

        // Create FFmpeg runner and run with duration for progress calculation
        let ffmpeg = FfmpegRunner::new(ffmpeg_path);
        let total_duration_ms = job.duration_secs.map(|s| s * 1000);
        let result = ffmpeg.run(job, progress_tx, total_duration_ms).await;

        // Wait for progress forwarder to finish
        let _ = progress_forwarder.await;

        // If successful, get the output file info
        result?;

        let output_path = job.output_path.to_string_lossy().to_string();
        let metadata = tokio::fs::metadata(&job.output_path).await?;
        let size_bytes = metadata.len();

        Ok((output_path, size_bytes))
    }
}

/**
 * Delete segment files after successful processing.
 *
 * Only deletes numbered segment files (e.g., 0000001.ts, 0000002.ts),
 * not concatenated output files (e.g., channelname_2026-01-28_120312.ts).
 * Also deletes concat_list.txt if it exists.
 *
 * # Arguments
 * * `recording_path` - Directory containing the segment files
 *
 * # Returns
 * The number of files deleted.
 */
pub async fn delete_segments(recording_path: &PathBuf) -> anyhow::Result<u64> {
    let mut count = 0u64;
    let mut entries = tokio::fs::read_dir(recording_path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        // Delete numbered .ts segment files only
        if let Some(ext) = path.extension() {
            if ext == "ts" {
                // Only delete numbered segment files, not concatenated outputs
                if let Some(stem) = path.file_stem() {
                    if stem.to_string_lossy().parse::<u64>().is_ok() {
                        if let Err(e) = tokio::fs::remove_file(&path).await {
                            warn!("Failed to delete segment {:?}: {}", path, e);
                        } else {
                            count += 1;
                        }
                    }
                }
            }
        }

        // Delete concat_list.txt
        if let Some(name) = path.file_name() {
            if name == "concat_list.txt" {
                if let Err(e) = tokio::fs::remove_file(&path).await {
                    warn!("Failed to delete concat list {:?}: {}", path, e);
                } else {
                    count += 1;
                }
            }
        }
    }

    Ok(count)
}

/**
 * Concatenate all .ts segment files into a single .ts file, then delete originals.
 *
 * Uses binary concatenation (cat/copy) since .ts files are designed to be concatenatable.
 *
 * # Arguments
 * * `recording_path` - Directory containing the segment files
 * * `channel_name` - Channel name for the output filename
 *
 * # Returns
 * The path to the concatenated file.
 */
pub async fn concatenate_segments(
    recording_path: &PathBuf,
    channel_name: &str,
) -> anyhow::Result<PathBuf> {
    // Collect and sort .ts files by sequence number
    let mut entries = tokio::fs::read_dir(recording_path).await?;
    let mut ts_files: Vec<(u64, PathBuf)> = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "ts" {
                if let Some(stem) = path.file_stem() {
                    if let Ok(seq) = stem.to_string_lossy().parse::<u64>() {
                        ts_files.push((seq, path));
                    }
                }
            }
        }
    }

    if ts_files.is_empty() {
        anyhow::bail!("No .ts segment files found to concatenate");
    }

    // Sort by sequence number
    ts_files.sort_by_key(|(seq, _)| *seq);

    // Generate output filename
    let output_filename = format!(
        "{}_{}.ts",
        channel_name,
        chrono::Utc::now().format("%Y-%m-%d_%H%M%S")
    );
    let output_path = recording_path.join(&output_filename);

    info!(
        "Concatenating {} segments to {:?}",
        ts_files.len(),
        output_path
    );

    // Concatenate files using async file operations
    let mut output_file = tokio::fs::File::create(&output_path).await?;

    for (_, segment_path) in &ts_files {
        let mut segment_file = tokio::fs::File::open(segment_path).await?;
        tokio::io::copy(&mut segment_file, &mut output_file).await?;
    }

    // Sync to disk
    output_file.sync_all().await?;
    drop(output_file);

    // Get final size for logging
    let metadata = tokio::fs::metadata(&output_path).await?;
    info!(
        "Concatenated {} segments into {:?} ({} bytes)",
        ts_files.len(),
        output_path,
        metadata.len()
    );

    // Delete original segment files
    for (_, segment_path) in &ts_files {
        if let Err(e) = tokio::fs::remove_file(segment_path).await {
            warn!("Failed to delete segment {:?}: {}", segment_path, e);
        }
    }

    // Delete concat_list.txt if it exists
    let concat_list = recording_path.join("concat_list.txt");
    if concat_list.exists() {
        let _ = tokio::fs::remove_file(&concat_list).await;
    }

    Ok(output_path)
}

/**
 * Count .ts segment files in a recording directory.
 *
 * Only counts numbered segment files (e.g., 0000001.ts, 0000002.ts),
 * not concatenated output files (e.g., channelname_2026-01-28_120312.ts).
 *
 * # Arguments
 * * `recording_path` - Directory to scan for .ts files
 *
 * # Returns
 * The number of .ts segment files found.
 */
pub async fn count_segments(recording_path: &PathBuf) -> u64 {
    let Ok(mut entries) = tokio::fs::read_dir(recording_path).await else {
        return 0;
    };

    let mut count = 0u64;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "ts" {
                // Only count numbered segment files, not concatenated outputs
                if let Some(stem) = path.file_stem() {
                    if stem.to_string_lossy().parse::<u64>().is_ok() {
                        count += 1;
                    }
                }
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_processing_manager_creation() {
        let (manager, _rx) = ProcessingManager::new(None, SegmentHandling::Delete, 1);

        // Check initial state
        let (current, queued) = manager.get_queue_status().await;
        assert!(current.is_empty());
        assert!(queued.is_empty());
    }

    #[tokio::test]
    async fn test_queue_job() {
        let (manager, _rx) = ProcessingManager::new(None, SegmentHandling::Delete, 1);

        let recording_id = Uuid::new_v4();
        let result = manager
            .queue_job(
                recording_id,
                "test_channel".to_string(),
                "twitch".to_string(),
                PathBuf::from("/tmp/test_recording"),
                ProcessingMode::Remux {
                    format: "mp4".to_string(),
                },
                None,
                Some(3600), // 1 hour duration for progress calculation
            )
            .await;

        assert!(result.is_ok());
        let (job_id, position) = result.unwrap();
        assert_eq!(position, 0);

        // Give worker a moment to pick up the job
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Job should be picked up by worker (removed from queue, set as current)
        // Note: In a real test we'd check events, but the job will fail quickly
        // because the path doesn't exist
        let _ = job_id; // Silence unused warning
    }

    #[tokio::test]
    async fn test_cancel_queued_job() {
        let (manager, _rx) = ProcessingManager::new(None, SegmentHandling::Delete, 1);

        // We need to queue multiple jobs quickly to have one stay in queue
        // This is a bit tricky because the worker picks them up fast
        // For now, just verify the cancel logic works on an empty queue
        let result = manager.cancel_job(Uuid::new_v4()).await;
        assert!(!result); // Job not found
    }

    #[tokio::test]
    async fn test_subscribe() {
        let (manager, rx1) = ProcessingManager::new(None, SegmentHandling::Delete, 1);
        let rx2 = manager.subscribe();

        // Both receivers should work
        drop(rx1);
        drop(rx2);
    }

    #[tokio::test]
    async fn test_segment_handling_default() {
        // Test with default = Delete
        let (manager1, _) = ProcessingManager::new(None, SegmentHandling::Delete, 1);
        let recording_id = Uuid::new_v4();
        let result = manager1
            .queue_job(
                recording_id,
                "test".to_string(),
                "twitch".to_string(),
                PathBuf::from("/tmp/nonexistent"),
                ProcessingMode::default(),
                None, // Should use default (Delete)
                None, // No duration
            )
            .await;
        assert!(result.is_ok());

        // Test with default = Keep but override to Concatenate
        let (manager2, _) = ProcessingManager::new(None, SegmentHandling::Keep, 1);
        let result = manager2
            .queue_job(
                Uuid::new_v4(),
                "test".to_string(),
                "twitch".to_string(),
                PathBuf::from("/tmp/nonexistent"),
                ProcessingMode::default(),
                Some(SegmentHandling::Concatenate), // Override default
                None,                               // No duration
            )
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_max_concurrent() {
        // Test that max_concurrent is stored correctly
        let (manager, _rx) = ProcessingManager::new(None, SegmentHandling::Delete, 5);
        assert_eq!(manager.get_max_concurrent(), 5);

        // Test that max_concurrent is at least 1
        let (manager2, _rx2) = ProcessingManager::new(None, SegmentHandling::Delete, 0);
        assert_eq!(manager2.get_max_concurrent(), 1);
    }
}
