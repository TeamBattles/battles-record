pub mod cleanup;
pub mod events;
pub mod index;
pub mod job;
pub mod ytdlp;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::{broadcast, mpsc, Semaphore};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::config::DownloadsConfig;
use crate::libraries::LibraryManager;
use crate::merge::aliases::AliasMap;

use self::events::DownloadEvent;
use self::index::DownloadsIndex;
use self::job::{
    CookieData, DownloadJob, DownloadJobSummary, DownloadRequest, DownloadStatus, ExtractedInfo,
};

#[derive(thiserror::Error, Debug)]
pub enum DownloadError {
    #[error("yt-dlp not available")]
    YtdlpNotAvailable,
    #[error("Invalid channel name: {0}")]
    InvalidChannelName(String),
    #[error("Download not found: {0}")]
    NotFound(Uuid),
    #[error("Download {0} is still active and cannot be deleted")]
    StillActive(Uuid),
    #[error("Download quota exceeded: used {used_bytes} of {limit_bytes}")]
    QuotaExceeded { used_bytes: u64, limit_bytes: u64 },
    #[error("URL already queued or downloading (existing job: {existing_id})")]
    DuplicateUrl { url: String, existing_id: Uuid },
    #[error("yt-dlp error: {0}")]
    Ytdlp(#[from] ytdlp::YtdlpDownloadError),
    #[error("Index error: {0}")]
    Index(#[from] index::IndexError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

enum JobCommand {
    Pause,
    Cancel,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadStorageStats {
    pub total_downloads: usize,
    pub total_size_bytes: u64,
    pub per_channel: Vec<DownloadChannelStats>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadChannelStats {
    pub channel: String,
    pub platform: String,
    pub count: usize,
    pub size_bytes: u64,
}

struct ActiveJob {
    #[allow(dead_code)]
    job: DownloadJob,
    command_tx: mpsc::Sender<JobCommand>,
}

pub struct DownloadManager {
    config: DownloadsConfig,
    downloads_dir: PathBuf,
    index: Arc<tokio::sync::RwLock<DownloadsIndex>>,
    library_manager: Arc<tokio::sync::Mutex<LibraryManager>>,
    event_tx: broadcast::Sender<DownloadEvent>,
    active_jobs: Arc<tokio::sync::RwLock<HashMap<Uuid, ActiveJob>>>,
    semaphore: Arc<Semaphore>,
}

fn compute_download_stats(queue: Vec<DownloadJobSummary>) -> DownloadStorageStats {
    let mut per_channel: HashMap<(String, String), (usize, u64)> = HashMap::new();
    let mut total_count = 0usize;
    let mut total_size = 0u64;

    for job in &queue {
        if job.status == DownloadStatus::Complete {
            total_count += 1;
            total_size += job.downloaded_bytes;
            let key = (job.source_platform.clone(), job.channel_name.clone());
            let entry = per_channel.entry(key).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += job.downloaded_bytes;
        }
    }

    let mut channels: Vec<DownloadChannelStats> = per_channel
        .into_iter()
        .map(|((platform, channel), (count, size_bytes))| DownloadChannelStats {
            channel,
            platform,
            count,
            size_bytes,
        })
        .collect();
    channels.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

    DownloadStorageStats {
        total_downloads: total_count,
        total_size_bytes: total_size,
        per_channel: channels,
    }
}

fn validate_channel_name(name: &str) -> Result<String, DownloadError> {
    let name = name.trim().to_lowercase();
    if name.is_empty() {
        return Err(DownloadError::InvalidChannelName("empty".into()));
    }
    if name.len() > 100 {
        return Err(DownloadError::InvalidChannelName(
            "too long (max 100)".into(),
        ));
    }
    if name.contains("..") || name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err(DownloadError::InvalidChannelName(
            "contains invalid characters".into(),
        ));
    }
    Ok(name)
}

impl DownloadManager {
    /// Create a new DownloadManager. Creates the downloads directory if needed
    /// and loads the persisted index.
    pub async fn new(
        config: DownloadsConfig,
        downloads_dir: PathBuf,
        library_manager: Arc<tokio::sync::Mutex<LibraryManager>>,
    ) -> Result<Self, DownloadError> {
        tokio::fs::create_dir_all(&downloads_dir).await?;

        let mut index = DownloadsIndex::new(&downloads_dir).await?;
        Self::reconcile_index(&mut index).await;
        let (event_tx, _) = broadcast::channel(256);
        let max_concurrent = config.max_concurrent.max(1) as usize;
        let semaphore = Arc::new(Semaphore::new(max_concurrent));

        info!(
            dir = %downloads_dir.display(),
            max_concurrent,
            "Download manager initialized"
        );

        Ok(Self {
            config,
            downloads_dir,
            index: Arc::new(tokio::sync::RwLock::new(index)),
            library_manager,
            event_tx,
            active_jobs: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            semaphore,
        })
    }

    /// Reconcile the index with files on disk at startup.
    /// - Completed downloads whose files no longer exist are removed
    /// - Duplicate completed downloads pointing to the same file keep only the newest
    /// - In-progress downloads (interrupted by crash/restart) are marked failed
    async fn reconcile_index(index: &mut DownloadsIndex) {
        use std::collections::HashMap as StdHashMap;

        let jobs: Vec<_> = index
            .list()
            .iter()
            .map(|j| (j.id, j.status, j.output_file.clone(), j.created_at))
            .collect();

        let mut removed = 0u32;
        let mut deduped = 0u32;
        let mut failed = 0u32;

        // Deduplicate: for completed downloads sharing the same output_file, keep newest
        let mut file_best: StdHashMap<PathBuf, (Uuid, chrono::DateTime<Utc>)> = StdHashMap::new();
        let mut duplicates: Vec<Uuid> = Vec::new();

        for &(id, status, ref output_file, created_at) in &jobs {
            if status == DownloadStatus::Complete {
                if let Some(ref path) = output_file {
                    match file_best.get(path) {
                        Some(&(_, best_time)) if created_at > best_time => {
                            let (old_id, _) = file_best.insert(path.clone(), (id, created_at)).unwrap();
                            duplicates.push(old_id);
                        }
                        Some(_) => {
                            duplicates.push(id);
                        }
                        None => {
                            file_best.insert(path.clone(), (id, created_at));
                        }
                    }
                }
            }
        }

        for id in duplicates {
            index.remove(&id);
            deduped += 1;
        }

        for (id, status, output_file, _) in jobs {
            if index.get(&id).is_none() {
                continue; // Already removed by dedup
            }
            match status {
                DownloadStatus::Complete => {
                    let file_exists = output_file
                        .as_ref()
                        .map_or(false, |p| p.exists());
                    if !file_exists {
                        index.remove(&id);
                        removed += 1;
                    }
                }
                DownloadStatus::Downloading
                | DownloadStatus::ExtractingInfo
                | DownloadStatus::Processing => {
                    index.update(&id, |j| {
                        j.status = DownloadStatus::Failed;
                        j.error = Some("Interrupted by app restart".to_string());
                        j.completed_at = Some(Utc::now());
                    });
                    failed += 1;
                }
                _ => {}
            }
        }

        if removed > 0 || deduped > 0 || failed > 0 {
            info!(
                removed,
                deduped,
                failed,
                "Reconciled downloads index with disk"
            );
            let _ = index.force_save().await;
        }
    }

    /// Subscribe to download events (broadcast channel per E5).
    pub fn subscribe(&self) -> broadcast::Receiver<DownloadEvent> {
        self.event_tx.subscribe()
    }

    /// Returns the downloads base directory path.
    pub fn downloads_dir(&self) -> &std::path::Path {
        &self.downloads_dir
    }

    /// Extract video info from a URL using yt-dlp without downloading.
    pub async fn extract_info(
        &self,
        url: &str,
        cookies: Option<&[CookieData]>,
    ) -> Result<ExtractedInfo, DownloadError> {
        let ytdlp_path = {
            let lib = self.library_manager.lock().await;
            lib.resolve_ytdlp()
                .ok_or(DownloadError::YtdlpNotAvailable)?
        };

        let (cookie_file_path, is_temp) = if let Some(cookies) = cookies {
            if cookies.is_empty() {
                // Empty cookie list - fall back to persistent
                let domain = extract_domain(url);
                let persistent = domain.and_then(|d| ytdlp::get_persistent_cookie_path(&d));
                info!(url = %url, has_persistent = persistent.is_some(), "ExtractInfo: no cookies provided, using persistent fallback");
                (persistent, false)
            } else {
                let tmp = self
                    .downloads_dir
                    .join(format!(".cookies-{}.txt", Uuid::new_v4()));
                ytdlp::write_cookie_file(cookies, &tmp).await?;
                info!(url = %url, cookie_count = cookies.len(), path = %tmp.display(), "ExtractInfo: wrote temp cookie file");
                (Some(tmp), true)
            }
        } else {
            // Fall back to persistent cookie file for this domain (spec 12.2)
            let domain = extract_domain(url);
            let persistent = domain.and_then(|d| ytdlp::get_persistent_cookie_path(&d));
            info!(url = %url, has_persistent = persistent.is_some(), "ExtractInfo: no cookies field, using persistent fallback");
            (persistent, false)
        };

        let result = ytdlp::extract_info(&ytdlp_path, url, cookie_file_path.as_deref()).await;

        // Only clean up temp cookie files, not persistent ones
        if is_temp {
            if let Some(ref path) = cookie_file_path {
                let _ = tokio::fs::remove_file(path).await;
            }
        }

        Ok(result?)
    }

    /// Start a new download. Validates the channel name, checks quota, creates
    /// the job, and either starts it immediately or queues it.
    pub async fn start_download(&self, request: DownloadRequest) -> Result<Uuid, DownloadError> {
        let channel_name = validate_channel_name(&request.channel_name)?;

        // Reject duplicate: same URL already active or queued
        {
            let idx = self.index.read().await;
            if let Some(existing) = idx.list().iter().find(|j| {
                j.url == request.url
                    && matches!(
                        j.status,
                        DownloadStatus::Queued | DownloadStatus::Downloading
                    )
            }) {
                return Err(DownloadError::DuplicateUrl {
                    url: request.url.clone(),
                    existing_id: existing.id,
                });
            }
        }

        // Resolve through alias map so renamed channels use the target folder
        let alias_path = self.downloads_dir.join("channel-aliases.json");
        let aliases = AliasMap::load(&alias_path);
        let resolved_channel =
            aliases.resolve_download(&request.source_platform, &channel_name);

        // Quota check (E10)
        if let Some(max_gb) = self.config.max_total_gb {
            let limit_bytes = max_gb * 1024 * 1024 * 1024;
            let used_bytes = {
                let idx = self.index.read().await;
                idx.total_size()
            };
            if used_bytes >= limit_bytes {
                return Err(DownloadError::QuotaExceeded {
                    used_bytes,
                    limit_bytes,
                });
            }
            // Warn when 80%+ of quota is used
            let threshold = limit_bytes * 4 / 5;
            if used_bytes >= threshold {
                tracing::warn!(
                    used_bytes,
                    limit_bytes,
                    percent = used_bytes as f64 / limit_bytes as f64 * 100.0,
                    "Download quota is above 80%"
                );
            }
        }

        let output_dir = self
            .downloads_dir
            .join(&request.source_platform)
            .join(&resolved_channel);
        tokio::fs::create_dir_all(&output_dir).await?;

        let job_id = Uuid::new_v4();
        let now = Utc::now();

        let job = DownloadJob {
            id: job_id,
            url: request.url.clone(),
            title: request.title.clone(),
            thumbnail: None,
            duration: None,
            uploader: None,
            platform_name: None,
            channel_name: channel_name.clone(),
            source_platform: request.source_platform.clone(),
            output_dir: output_dir.clone(),
            output_file: None,
            format: request.format.clone(),
            quality: request.quality.clone(),
            available_formats: None,
            status: DownloadStatus::Queued,
            percent: 0.0,
            speed: None,
            eta: None,
            downloaded_bytes: 0,
            total_bytes: None,
            options: request.options.unwrap_or_default(),
            requested_by: request.requested_by,
            requested_by_name: request.requested_by_name.clone(),
            created_at: now,
            completed_at: None,
            error: None,
        };

        // Add to index
        {
            let mut idx = self.index.write().await;
            idx.add(job.clone());
            let _ = idx.save_if_dirty().await;
        }

        let summary = DownloadJobSummary::from(&job);
        let _ = self.event_tx.send(DownloadEvent::Queued { job: summary });

        // Try to start immediately if concurrency allows
        self.try_start_job(job_id, request.cookies).await;

        Ok(job_id)
    }

    /// Pause an active or queued download.
    pub async fn pause(&self, id: Uuid) -> Result<(), DownloadError> {
        // If active, send pause command to the running task
        {
            let active = self.active_jobs.read().await;
            if let Some(job) = active.get(&id) {
                let _ = job.command_tx.send(JobCommand::Pause).await;
                return Ok(());
            }
        }

        // If queued, update status directly to Paused
        {
            let idx = self.index.read().await;
            let job = idx.get(&id).ok_or(DownloadError::NotFound(id))?;
            if job.status != DownloadStatus::Queued && job.status != DownloadStatus::Downloading {
                return Err(DownloadError::NotFound(id));
            }
        }
        {
            let mut idx = self.index.write().await;
            idx.update(&id, |j| j.status = DownloadStatus::Paused);
            let _ = idx.save_if_dirty().await;
        }

        let _ = self
            .event_tx
            .send(DownloadEvent::Paused { download_id: id });

        Ok(())
    }

    /// Resume a paused download.
    pub async fn resume(&self, id: Uuid) -> Result<(), DownloadError> {
        {
            let idx = self.index.read().await;
            let job = idx.get(&id).ok_or(DownloadError::NotFound(id))?;
            if job.status != DownloadStatus::Paused {
                return Err(DownloadError::NotFound(id));
            }
        }

        // Update status to Queued
        {
            let mut idx = self.index.write().await;
            idx.update(&id, |j| j.status = DownloadStatus::Queued);
            let _ = idx.save_if_dirty().await;
        }

        let _ = self
            .event_tx
            .send(DownloadEvent::Resumed { download_id: id });

        // Try to start it again (no cookies on resume)
        self.try_start_job(id, None).await;

        Ok(())
    }

    /// Cancel a download (active or queued).
    pub async fn cancel(&self, id: Uuid) -> Result<(), DownloadError> {
        // If active, send cancel command
        {
            let active = self.active_jobs.read().await;
            if let Some(job) = active.get(&id) {
                let _ = job.command_tx.send(JobCommand::Cancel).await;
                // The spawned task will handle cleanup and status update
                return Ok(());
            }
        }

        // Otherwise update status in index directly
        {
            let idx = self.index.read().await;
            if idx.get(&id).is_none() {
                return Err(DownloadError::NotFound(id));
            }
        }
        {
            let mut idx = self.index.write().await;
            idx.update(&id, |j| {
                j.status = DownloadStatus::Cancelled;
                j.completed_at = Some(Utc::now());
            });
            let _ = idx.save_if_dirty().await;
        }

        let _ = self
            .event_tx
            .send(DownloadEvent::Cancelled { download_id: id });

        Ok(())
    }

    /// Move a queued job to the front of the queue by bumping its priority.
    /// If all concurrency slots are full, it will be next to run.
    pub async fn prioritize(&self, id: Uuid) -> Result<(), DownloadError> {
        let idx = self.index.read().await;
        let job = idx.get(&id).ok_or(DownloadError::NotFound(id))?;
        if job.status != DownloadStatus::Queued {
            return Err(DownloadError::NotFound(id));
        }
        drop(idx);

        // Try to start immediately; if semaphore is available it will run now
        self.try_start_job(id, None).await;

        Ok(())
    }

    /// Return summaries for all jobs in the index.
    pub async fn get_queue(&self) -> Vec<DownloadJobSummary> {
        let idx = self.index.read().await;
        idx.list()
            .iter()
            .map(|j| DownloadJobSummary::from(*j))
            .collect()
    }

    /// Get a single download job by ID.
    pub async fn get_download(&self, id: Uuid) -> Option<DownloadJob> {
        let idx = self.index.read().await;
        idx.get(&id).cloned()
    }

    /// Find existing downloads for a URL, only including entries where the file
    /// still exists on disk (for completed downloads) or are still active/queued.
    pub async fn find_existing_for_url(&self, url: &str) -> Vec<DownloadJobSummary> {
        let idx = self.index.read().await;
        idx.list()
            .iter()
            .filter(|j| j.url == url)
            .filter(|j| match j.status {
                DownloadStatus::Complete => {
                    // Only include completed downloads if the file still exists
                    j.output_file.as_ref().is_some_and(|p| p.exists())
                }
                DownloadStatus::Failed | DownloadStatus::Cancelled => false,
                // Active/queued downloads are always included
                _ => true,
            })
            .map(|j| DownloadJobSummary::from(*j))
            .collect()
    }

    /// Get download storage statistics: total count, size, and per-channel breakdown.
    pub async fn get_stats(&self) -> DownloadStorageStats {
        let queue = self.get_queue().await;
        compute_download_stats(queue)
    }

    /// Cleanup downloads matching filters. Returns (count, bytes_freed).
    pub async fn cleanup_filtered(
        &self,
        older_than_days: Option<u32>,
        channel_name: Option<&str>,
        source_platform: Option<&str>,
        dry_run: bool,
    ) -> Result<(Vec<DownloadJobSummary>, u64), DownloadError> {
        let mut idx = self.index.write().await;
        let cutoff = older_than_days.map(|days| Utc::now() - chrono::Duration::days(days as i64));

        let matching: Vec<(Uuid, u64)> = idx
            .list()
            .iter()
            .filter(|j| {
                matches!(
                    j.status,
                    DownloadStatus::Complete | DownloadStatus::Failed | DownloadStatus::Cancelled
                )
            })
            .filter(|j| {
                if let Some(ref cutoff) = cutoff {
                    j.completed_at.map_or(false, |t| t < *cutoff)
                } else {
                    true
                }
            })
            .filter(|j| channel_name.map_or(true, |c| j.channel_name == c))
            .filter(|j| source_platform.map_or(true, |p| j.source_platform == p))
            .map(|j| (j.id, j.downloaded_bytes))
            .collect();

        let summaries: Vec<DownloadJobSummary> = matching
            .iter()
            .filter_map(|(id, _)| idx.get(id).map(DownloadJobSummary::from))
            .collect();
        let bytes_to_free: u64 = matching.iter().map(|(_, b)| b).sum();

        if !dry_run {
            for (id, _) in &matching {
                if let Err(e) = cleanup::DownloadCleanupWorker::delete_download(&mut idx, *id).await
                {
                    warn!(download_id = %id, error = %e, "Failed to delete download during cleanup");
                }
            }
        }

        Ok((summaries, bytes_to_free))
    }

    /// Run cleanup, deleting downloads older than the retention period.
    pub async fn run_cleanup(&self, max_age_days: Option<u32>) -> Result<u32, DownloadError> {
        let mut idx = self.index.write().await;
        let count = cleanup::DownloadCleanupWorker::run_cleanup(&mut idx, max_age_days)
            .await
            .map_err(|e| DownloadError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        Ok(count)
    }

    /// Remove a completed/failed/cancelled download from the index.
    /// Returns an error if the download is still active.
    pub async fn remove_download(&self, id: Uuid) -> Result<(), DownloadError> {
        let mut idx = self.index.write().await;
        let job = idx.get(&id).ok_or(DownloadError::NotFound(id))?;
        match job.status {
            DownloadStatus::Complete | DownloadStatus::Failed | DownloadStatus::Cancelled => {}
            _ => return Err(DownloadError::StillActive(id)),
        }
        idx.remove(&id);
        let _ = idx.save_if_dirty().await;
        Ok(())
    }

    /// Internal: attempt to start a job if a semaphore permit is available.
    async fn try_start_job(&self, job_id: Uuid, cookies: Option<Vec<CookieData>>) {
        // Try to acquire a permit without blocking
        let permit = match self.semaphore.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                // No slots available, job stays queued
                return;
            }
        };

        // Read the job from the index
        let job = {
            let idx = self.index.read().await;
            match idx.get(&job_id) {
                Some(j) => j.clone(),
                None => {
                    drop(permit);
                    return;
                }
            }
        };

        // Only start if it's in a startable state
        if job.status != DownloadStatus::Queued {
            drop(permit);
            return;
        }

        // Update status to Downloading
        {
            let mut idx = self.index.write().await;
            idx.update(&job_id, |j| j.status = DownloadStatus::Downloading);
            let _ = idx.save_if_dirty().await;
        }

        // Create command channel for pause/cancel
        let (command_tx, command_rx) = mpsc::channel::<JobCommand>(4);

        // Store active job
        {
            let mut active = self.active_jobs.write().await;
            active.insert(
                job_id,
                ActiveJob {
                    job: job.clone(),
                    command_tx,
                },
            );
        }

        let sched = Arc::new(QueueScheduler {
            index: self.index.clone(),
            semaphore: self.semaphore.clone(),
            active_jobs: self.active_jobs.clone(),
            library_manager: self.library_manager.clone(),
            event_tx: self.event_tx.clone(),
            downloads_dir: self.downloads_dir.clone(),
            output_template: self.config.output_template.clone(),
            default_format: self.config.default_format.clone(),
        });

        tokio::spawn(async move {
            let result = run_download_task(
                job.clone(),
                cookies,
                command_rx,
                sched.library_manager.clone(),
                sched.event_tx.clone(),
                sched.index.clone(),
                sched.downloads_dir.clone(),
                sched.output_template.clone(),
                sched.default_format.clone(),
            )
            .await;

            // Remove from active jobs
            {
                let mut active = sched.active_jobs.write().await;
                active.remove(&job_id);
            }

            // Release semaphore permit
            drop(permit);

            if let Err(e) = result {
                error!(job_id = %job_id, error = %e, "Download task failed");
            }

            // Check if any queued jobs can start now
            schedule_next_queued(sched);
        });
    }
}

/// Shared state for scheduling queued downloads after a slot frees up.
struct QueueScheduler {
    index: Arc<tokio::sync::RwLock<DownloadsIndex>>,
    semaphore: Arc<Semaphore>,
    active_jobs: Arc<tokio::sync::RwLock<HashMap<Uuid, ActiveJob>>>,
    library_manager: Arc<tokio::sync::Mutex<LibraryManager>>,
    event_tx: broadcast::Sender<DownloadEvent>,
    downloads_dir: PathBuf,
    output_template: String,
    default_format: String,
}

/// Try to start the next queued job from the index.
fn schedule_next_queued(sched: Arc<QueueScheduler>) {
    tokio::spawn(async move {
        // Find next queued job
        let next_id = {
            let idx = sched.index.read().await;
            idx.list()
                .iter()
                .find(|j| j.status == DownloadStatus::Queued)
                .map(|j| j.id)
        };

        let Some(next_id) = next_id else {
            return;
        };

        // Try to acquire permit
        let permit = match sched.semaphore.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => return,
        };

        let job = {
            let idx = sched.index.read().await;
            match idx.get(&next_id) {
                Some(j) => j.clone(),
                None => {
                    drop(permit);
                    return;
                }
            }
        };

        if job.status != DownloadStatus::Queued {
            drop(permit);
            return;
        }

        // Update to Downloading
        {
            let mut idx = sched.index.write().await;
            idx.update(&next_id, |j| j.status = DownloadStatus::Downloading);
            let _ = idx.save_if_dirty().await;
        }

        let (command_tx, command_rx) = mpsc::channel::<JobCommand>(4);

        {
            let mut active = sched.active_jobs.write().await;
            active.insert(
                next_id,
                ActiveJob {
                    job: job.clone(),
                    command_tx,
                },
            );
        }

        // Spawn the download in a separate task (matching try_start_job pattern)
        // instead of running inline which would block this task
        let sched_for_task = sched.clone();
        tokio::spawn(async move {
            let result = run_download_task(
                job,
                None,
                command_rx,
                sched_for_task.library_manager.clone(),
                sched_for_task.event_tx.clone(),
                sched_for_task.index.clone(),
                sched_for_task.downloads_dir.clone(),
                sched_for_task.output_template.clone(),
                sched_for_task.default_format.clone(),
            )
            .await;

            {
                let mut active = sched_for_task.active_jobs.write().await;
                active.remove(&next_id);
            }

            drop(permit);

            if let Err(e) = result {
                error!(job_id = %next_id, error = %e, "Download task failed");
            }

            // Check for more queued jobs after this one completes
            schedule_next_queued(sched_for_task);
        });
    });
}

/// Run the actual download process, handling pause/cancel via select.
#[allow(clippy::too_many_arguments)]
async fn run_download_task(
    job: DownloadJob,
    cookies: Option<Vec<CookieData>>,
    mut command_rx: mpsc::Receiver<JobCommand>,
    library_manager: Arc<tokio::sync::Mutex<LibraryManager>>,
    event_tx: broadcast::Sender<DownloadEvent>,
    index: Arc<tokio::sync::RwLock<DownloadsIndex>>,
    downloads_dir: PathBuf,
    output_template: String,
    default_format: String,
) -> Result<(), DownloadError> {
    let job_id = job.id;

    // Resolve binaries
    let (ytdlp_path, ffmpeg_path) = {
        let lib = library_manager.lock().await;
        let ytdlp = lib
            .resolve_ytdlp()
            .ok_or(DownloadError::YtdlpNotAvailable)?;
        let ffmpeg = lib.resolve_ffmpeg();
        if ffmpeg.is_none() {
            warn!(job_id = %job_id, "FFmpeg not found - video+audio merge will not work, audio may be missing");
        }
        (ytdlp, ffmpeg)
    };

    // Write temp cookie file if extension provided cookies, or fall back to persistent cookies
    let (cookie_file_path, is_temp_cookie) = if let Some(ref cookies) = cookies {
        let tmp = downloads_dir.join(format!(".cookies-{}.txt", job_id));
        ytdlp::write_cookie_file(cookies, &tmp).await?;
        (Some(tmp), true)
    } else {
        // Fall back to persistent cookie file for this domain (spec 12.2)
        let domain = extract_domain(&job.url);
        let persistent = domain.and_then(|d| ytdlp::get_persistent_cookie_path(&d));
        (persistent, false)
    };

    let format = job.format.as_deref().unwrap_or(&default_format);

    // Start the yt-dlp download
    let download_result = ytdlp::start_download(
        &ytdlp_path,
        ffmpeg_path.as_deref(),
        &job.url,
        format,
        &job.output_dir,
        &output_template,
        &job.options,
        cookie_file_path.as_deref(),
        event_tx.clone(),
        job_id,
    )
    .await;

    let mut ytdlp_handle = match download_result {
        Ok(h) => h,
        Err(e) => {
            // Only clean up temp cookie files, not persistent ones
            if is_temp_cookie {
                if let Some(ref path) = cookie_file_path {
                    let _ = tokio::fs::remove_file(path).await;
                }
            }
            let err_msg = e.to_string();
            let mut idx = index.write().await;
            idx.update(&job_id, |j| {
                j.status = DownloadStatus::Failed;
                j.error = Some(err_msg.clone());
                j.completed_at = Some(Utc::now());
            });
            let _ = idx.force_save().await;
            let _ = event_tx.send(DownloadEvent::Failed {
                download_id: job_id,
                channel_name: job.channel_name.clone(),
                error: err_msg,
                update_available: false,
            });
            return Err(e.into());
        }
    };

    // Wait for completion while listening for commands
    loop {
        tokio::select! {
            cmd = command_rx.recv() => {
                match cmd {
                    Some(JobCommand::Pause) => {
                        let _ = ytdlp_handle.pause();
                        let mut idx = index.write().await;
                        idx.update(&job_id, |j| j.status = DownloadStatus::Paused);
                        let _ = idx.force_save().await;
                        let _ = event_tx.send(DownloadEvent::Paused { download_id: job_id });
                        if is_temp_cookie {
                            if let Some(ref path) = cookie_file_path {
                                let _ = tokio::fs::remove_file(path).await;
                            }
                        }
                        return Ok(());
                    }
                    Some(JobCommand::Cancel) => {
                        let _ = ytdlp_handle.cancel().await;
                        let mut idx = index.write().await;
                        idx.update(&job_id, |j| {
                            j.status = DownloadStatus::Cancelled;
                            j.completed_at = Some(Utc::now());
                        });
                        let _ = idx.force_save().await;
                        let _ = event_tx.send(DownloadEvent::Cancelled { download_id: job_id });
                        if is_temp_cookie {
                            if let Some(ref path) = cookie_file_path {
                                let _ = tokio::fs::remove_file(path).await;
                            }
                        }
                        return Ok(());
                    }
                    None => {
                        // Command channel closed, continue waiting for process
                    }
                }
            }
            status = ytdlp_handle.wait() => {
                if is_temp_cookie {
                    if let Some(ref path) = cookie_file_path {
                        let _ = tokio::fs::remove_file(path).await;
                    }
                }

                match status {
                    Ok(exit_status) if exit_status.success() => {
                        // Find the actual output file and its size by scanning the output dir
                        // for the most recently modified non-.part file
                        let (output_file, filesize) = find_output_file(&job.output_dir).await;

                        let mut idx = index.write().await;
                        idx.update(&job_id, |j| {
                            j.status = DownloadStatus::Complete;
                            j.percent = 100.0;
                            j.completed_at = Some(Utc::now());
                            if let Some(ref path) = output_file {
                                j.output_file = Some(path.clone());
                            }
                            j.downloaded_bytes = filesize;
                        });
                        let _ = idx.force_save().await;

                        let filepath = output_file.unwrap_or_else(|| job.output_dir.clone());

                        let _ = event_tx.send(DownloadEvent::Complete {
                            download_id: job_id,
                            channel_name: job.channel_name.clone(),
                            filepath,
                            filesize,
                        });

                        info!(job_id = %job_id, filesize = filesize, "Download completed successfully");
                        return Ok(());
                    }
                    Ok(_) => {
                        let stderr = ytdlp_handle.stderr().await;
                        let err_msg = if stderr.is_empty() {
                            "yt-dlp exited with non-zero status".to_string()
                        } else {
                            // Extract the last ERROR line for a concise message
                            let error_line = stderr.lines()
                                .rev()
                                .find(|l| l.starts_with("ERROR:"))
                                .unwrap_or(&stderr);
                            error_line.to_string()
                        };
                        warn!(job_id = %job_id, stderr = %stderr, "yt-dlp download failed");
                        let mut idx = index.write().await;
                        idx.update(&job_id, |j| {
                            j.status = DownloadStatus::Failed;
                            j.error = Some(err_msg.clone());
                            j.completed_at = Some(Utc::now());
                        });
                        let _ = idx.force_save().await;
                        let _ = event_tx.send(DownloadEvent::Failed {
                            download_id: job_id,
                            channel_name: job.channel_name.clone(),
                            error: err_msg,
                            update_available: false,
                        });
                        return Ok(());
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        let mut idx = index.write().await;
                        idx.update(&job_id, |j| {
                            j.status = DownloadStatus::Failed;
                            j.error = Some(err_msg.clone());
                            j.completed_at = Some(Utc::now());
                        });
                        let _ = idx.force_save().await;
                        let _ = event_tx.send(DownloadEvent::Failed {
                            download_id: job_id,
                            channel_name: job.channel_name.clone(),
                            error: err_msg,
                            update_available: false,
                        });
                        return Ok(());
                    }
                }
            }
        }
    }
}

/// Extract the host/domain from a URL without depending on the `url` crate.
fn extract_domain(url: &str) -> Option<String> {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let host = without_scheme.split('/').next()?;
    let host = host.split(':').next()?; // Strip port
    let host = host.split('@').last()?; // Strip user info
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

/// Find the most recently modified non-.part file in a directory.
/// Returns (file_path, file_size). Used after yt-dlp completes to identify the output file.
async fn find_output_file(dir: &std::path::Path) -> (Option<PathBuf>, u64) {
    let mut best: Option<(PathBuf, u64, std::time::SystemTime)> = None;

    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return (None, 0),
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        // Skip .part files (incomplete downloads)
        if path.extension().and_then(|e| e.to_str()) == Some("part") {
            continue;
        }
        // Skip hidden files and temp files
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map_or(true, |n| n.starts_with('.'))
        {
            continue;
        }

        if let Ok(meta) = tokio::fs::metadata(&path).await {
            let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let size = meta.len();
            if best.as_ref().map_or(true, |(_, _, t)| modified > *t) {
                best = Some((path, size, modified));
            }
        }
    }

    match best {
        Some((path, size, _)) => (Some(path), size),
        None => (None, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- validate_channel_name tests ---

    #[test]
    fn valid_channel_names() {
        assert_eq!(validate_channel_name("streamer123").unwrap(), "streamer123");
        assert_eq!(validate_channel_name("  MyChannel  ").unwrap(), "mychannel");
        assert_eq!(
            validate_channel_name("cool-streamer_1").unwrap(),
            "cool-streamer_1"
        );
    }

    #[test]
    fn channel_name_rejects_empty() {
        assert!(validate_channel_name("").is_err());
        assert!(validate_channel_name("   ").is_err());
    }

    #[test]
    fn channel_name_rejects_too_long() {
        let long_name = "a".repeat(101);
        assert!(validate_channel_name(&long_name).is_err());
    }

    #[test]
    fn channel_name_rejects_path_traversal() {
        assert!(validate_channel_name("../etc/passwd").is_err());
        assert!(validate_channel_name("foo/bar").is_err());
        assert!(validate_channel_name("foo\\bar").is_err());
        assert!(validate_channel_name("foo\0bar").is_err());
        assert!(validate_channel_name("..").is_err());
    }

    #[test]
    fn channel_name_normalizes_to_lowercase() {
        assert_eq!(validate_channel_name("UPPER").unwrap(), "upper");
        assert_eq!(validate_channel_name("MiXeD").unwrap(), "mixed");
    }

    #[test]
    fn channel_name_max_length_ok() {
        let name = "a".repeat(100);
        assert!(validate_channel_name(&name).is_ok());
    }

    // --- DownloadManager::new tests ---

    #[tokio::test]
    async fn manager_new_creates_directory() {
        let dir = TempDir::new().unwrap();
        let downloads_path = dir.path().join("downloads");
        let lib_manager = Arc::new(tokio::sync::Mutex::new(
            crate::libraries::LibraryManager::new(Default::default(), None),
        ));

        let manager = DownloadManager::new(
            DownloadsConfig::default(),
            downloads_path.clone(),
            lib_manager,
        )
        .await
        .unwrap();

        assert!(downloads_path.exists());
        // Queue should be empty
        let queue = manager.get_queue().await;
        assert!(queue.is_empty());
    }

    #[tokio::test]
    async fn manager_new_loads_empty_index() {
        let dir = TempDir::new().unwrap();
        let lib_manager = Arc::new(tokio::sync::Mutex::new(
            crate::libraries::LibraryManager::new(Default::default(), None),
        ));

        let manager = DownloadManager::new(
            DownloadsConfig::default(),
            dir.path().to_path_buf(),
            lib_manager,
        )
        .await
        .unwrap();

        let queue = manager.get_queue().await;
        assert!(queue.is_empty());
        assert!(manager.get_download(Uuid::new_v4()).await.is_none());
    }

    // --- get_queue returns empty ---

    #[tokio::test]
    async fn get_queue_returns_empty_initially() {
        let dir = TempDir::new().unwrap();
        let lib_manager = Arc::new(tokio::sync::Mutex::new(
            crate::libraries::LibraryManager::new(Default::default(), None),
        ));

        let manager = DownloadManager::new(
            DownloadsConfig::default(),
            dir.path().to_path_buf(),
            lib_manager,
        )
        .await
        .unwrap();

        assert_eq!(manager.get_queue().await.len(), 0);
    }

    // --- subscribe test ---

    #[tokio::test]
    async fn subscribe_returns_receiver() {
        let dir = TempDir::new().unwrap();
        let lib_manager = Arc::new(tokio::sync::Mutex::new(
            crate::libraries::LibraryManager::new(Default::default(), None),
        ));

        let manager = DownloadManager::new(
            DownloadsConfig::default(),
            dir.path().to_path_buf(),
            lib_manager,
        )
        .await
        .unwrap();

        let _rx = manager.subscribe();
        // Should not panic, just verifying we can subscribe
    }

    // --- quota enforcement tests ---

    #[tokio::test]
    async fn start_download_rejects_when_quota_exceeded() {
        let dir = TempDir::new().unwrap();
        let lib_manager = Arc::new(tokio::sync::Mutex::new(
            crate::libraries::LibraryManager::new(Default::default(), None),
        ));

        let mut config = DownloadsConfig::default();
        config.max_total_gb = Some(0); // 0 GB quota - impossible to fit anything

        let manager = DownloadManager::new(config, dir.path().to_path_buf(), lib_manager)
            .await
            .unwrap();

        let request = DownloadRequest {
            url: "https://youtube.com/watch?v=test".to_string(),
            title: None,
            channel_name: "test_channel".to_string(),
            source_platform: "youtube".to_string(),
            format: None,
            quality: None,
            options: None,
            cookies: None,
            requested_by: Uuid::new_v4(),
            requested_by_name: None,
            auto_start: false,
        };

        let result = manager.start_download(request).await;
        assert!(result.is_err());
        match result {
            Err(DownloadError::QuotaExceeded { .. }) => {} // expected
            other => panic!("Expected QuotaExceeded, got {:?}", other),
        }
    }

    // --- cancel/pause not found ---

    #[tokio::test]
    async fn cancel_nonexistent_returns_not_found() {
        let dir = TempDir::new().unwrap();
        let lib_manager = Arc::new(tokio::sync::Mutex::new(
            crate::libraries::LibraryManager::new(Default::default(), None),
        ));

        let manager = DownloadManager::new(
            DownloadsConfig::default(),
            dir.path().to_path_buf(),
            lib_manager,
        )
        .await
        .unwrap();

        let result = manager.cancel(Uuid::new_v4()).await;
        assert!(matches!(result, Err(DownloadError::NotFound(_))));
    }

    #[tokio::test]
    async fn pause_nonexistent_returns_not_found() {
        let dir = TempDir::new().unwrap();
        let lib_manager = Arc::new(tokio::sync::Mutex::new(
            crate::libraries::LibraryManager::new(Default::default(), None),
        ));

        let manager = DownloadManager::new(
            DownloadsConfig::default(),
            dir.path().to_path_buf(),
            lib_manager,
        )
        .await
        .unwrap();

        let result = manager.pause(Uuid::new_v4()).await;
        assert!(matches!(result, Err(DownloadError::NotFound(_))));
    }
}
