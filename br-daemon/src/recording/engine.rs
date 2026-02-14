use super::{parse_media_playlist, HlsSegment, RecordingState, SegmentWriter};
use crate::platforms::StreamUrl;
use reqwest::Client;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, info, warn};

/** Priority for segment downloads. */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentPriority {
    High,   // Live edge - download immediately
    Normal, // Backfill - download when workers available
}

#[derive(Debug, Clone)]
pub struct QueuedSegment {
    pub segment: HlsSegment,
    pub priority: SegmentPriority,
}

impl PartialEq for QueuedSegment {
    fn eq(&self, other: &Self) -> bool {
        self.segment.sequence == other.segment.sequence
    }
}

impl Eq for QueuedSegment {}

impl PartialOrd for QueuedSegment {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueuedSegment {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher priority first, then higher sequence number
        match (&self.priority, &other.priority) {
            (SegmentPriority::High, SegmentPriority::Normal) => std::cmp::Ordering::Greater,
            (SegmentPriority::Normal, SegmentPriority::High) => std::cmp::Ordering::Less,
            _ => self.segment.sequence.cmp(&other.segment.sequence),
        }
    }
}

/** Events emitted by the recording engine. */
#[derive(Debug, Clone)]
pub enum RecordingEvent {
    /** Init segment downloaded (for fMP4/CMAF streams). */
    InitSegmentDownloaded {
        size_bytes: u64,
    },
    SegmentDownloaded {
        sequence: u64,
        size_bytes: u64,
    },
    PlaylistRefreshed {
        new_segments: u32,
    },
    StreamEnded,
    Error {
        message: String,
    },
}

pub struct RecordingEngine {
    client: Client,
    stream_url: StreamUrl,
    output_dir: PathBuf,
    state: Arc<RwLock<RecordingState>>,
    downloaded: Arc<RwLock<HashSet<u64>>>,
    /** Whether the init segment has been downloaded (for fMP4/CMAF streams). */
    init_downloaded: Arc<RwLock<bool>>,
    event_tx: broadcast::Sender<RecordingEvent>,
    shutdown_rx: mpsc::Receiver<()>,
    worker_count: usize,
    /** Seconds with no new segments before assuming stream ended. */
    stale_timeout_secs: u64,
}

impl RecordingEngine {
    pub fn new(
        stream_url: StreamUrl,
        output_dir: PathBuf,
        state: RecordingState,
        shutdown_rx: mpsc::Receiver<()>,
        stale_timeout_secs: u64,
    ) -> (Self, broadcast::Receiver<RecordingEvent>) {
        let (event_tx, event_rx) = broadcast::channel(100);

        let engine = Self {
            client: Client::new(),
            stream_url,
            output_dir,
            state: Arc::new(RwLock::new(state)),
            downloaded: Arc::new(RwLock::new(HashSet::new())),
            init_downloaded: Arc::new(RwLock::new(false)),
            event_tx,
            shutdown_rx,
            worker_count: 4,
            stale_timeout_secs,
        };

        (engine, event_rx)
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        let segment_writer = SegmentWriter::new(self.output_dir.clone());

        // Clean up any temp files from previous crash
        let cleaned = segment_writer.cleanup_temp_files().await?;
        if cleaned > 0 {
            info!("Cleaned up {} incomplete segment files", cleaned);
        }

        // Load existing segments to avoid re-downloading
        if let Some(last_seq) = segment_writer.get_last_sequence().await {
            let mut downloaded = self.downloaded.write().await;
            for seq in 0..=last_seq {
                downloaded.insert(seq);
            }
            let mut state = self.state.write().await;
            state.last_segment = last_seq;
            info!("Resuming from segment {}", last_seq);
        }

        // Check if init segment already exists (for fMP4/CMAF streams)
        if segment_writer.has_init_segment().await {
            *self.init_downloaded.write().await = true;
            info!("Found existing init segment");
        }

        // Create segment queue
        let (queue_tx, queue_rx) = async_channel::bounded::<QueuedSegment>(100);

        // Spawn worker tasks
        let workers: Vec<_> = (0..self.worker_count)
            .map(|id| {
                let rx = queue_rx.clone();
                let client = self.client.clone();
                let writer = SegmentWriter::new(self.output_dir.clone());
                let event_tx = self.event_tx.clone();
                let state = self.state.clone();
                let downloaded = self.downloaded.clone();

                tokio::spawn(async move {
                    Self::worker_loop(id, rx, client, writer, event_tx, state, downloaded).await
                })
            })
            .collect();

        // Main loop: poll playlist and queue segments
        let mut consecutive_failures = 0;
        let mut no_new_segments_count = 0;

        loop {
            tokio::select! {
                biased;

                _ = self.shutdown_rx.recv() => {
                    info!("Shutdown signal received");
                    break;
                }

                result = Self::poll_playlist_static(
                    &self.client,
                    &self.stream_url.url,
                    &self.downloaded,
                    &queue_tx,
                ) => {
                    match result {
                        Ok((new_count, init_segment_uri)) => {
                            consecutive_failures = 0;
                            if new_count == 0 {
                                no_new_segments_count += 1;
                            } else {
                                no_new_segments_count = 0;
                            }

                            // Download init segment if present and not already downloaded
                            if let Some(init_uri) = init_segment_uri {
                                let init_downloaded = *self.init_downloaded.read().await;
                                if !init_downloaded {
                                    info!("Detected fMP4/CMAF stream, downloading init segment");
                                    match segment_writer.download_and_write_init(&self.client, &init_uri).await {
                                        Ok(path) => {
                                            let size = tokio::fs::metadata(&path)
                                                .await
                                                .map(|m| m.len())
                                                .unwrap_or(0);
                                            *self.init_downloaded.write().await = true;
                                            info!("Downloaded init segment ({} bytes)", size);
                                            let _ = self.event_tx.send(RecordingEvent::InitSegmentDownloaded {
                                                size_bytes: size,
                                            });
                                        }
                                        Err(e) => {
                                            warn!("Failed to download init segment: {}", e);
                                            let _ = self.event_tx.send(RecordingEvent::Error {
                                                message: format!("Init segment download failed: {}", e),
                                            });
                                        }
                                    }
                                }
                            }

                            let _ = self.event_tx.send(RecordingEvent::PlaylistRefreshed {
                                new_segments: new_count,
                            });
                        }
                        Err(e) => {
                            consecutive_failures += 1;
                            warn!("Playlist fetch failed ({}): {}", consecutive_failures, e);

                            let _ = self.event_tx.send(RecordingEvent::Error {
                                message: e.to_string(),
                            });

                            if consecutive_failures >= 15 {
                                info!("Too many consecutive failures, assuming stream ended");
                                break;
                            }
                        }
                    }

                    // Check for stream end conditions
                    if no_new_segments_count >= self.stale_timeout_secs {
                        info!("No new segments for {} seconds, assuming stream ended", self.stale_timeout_secs);
                        break;
                    }

                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }

        // Signal workers to stop
        drop(queue_tx);

        // Wait for workers to finish
        for worker in workers {
            let _ = worker.await;
        }

        // Save final state
        let state = self.state.read().await;
        let state_path = self.output_dir.join("state.json");
        state.save(&state_path).await?;

        let _ = self.event_tx.send(RecordingEvent::StreamEnded);

        info!(
            "Recording complete: {} segments, {} bytes",
            state.segments_downloaded, state.bytes_downloaded
        );

        Ok(())
    }

    /**
     * Poll the playlist and queue new segments for download.
     * Returns (new_segment_count, init_segment_uri) where init_segment_uri is
     * Some if this is an fMP4/CMAF stream with an EXT-X-MAP tag.
     */
    async fn poll_playlist_static(
        client: &Client,
        playlist_url: &str,
        downloaded: &Arc<RwLock<HashSet<u64>>>,
        queue_tx: &async_channel::Sender<QueuedSegment>,
    ) -> anyhow::Result<(u32, Option<String>)> {
        let response = client.get(playlist_url).send().await?;
        let content = response.text().await?;

        let playlist = parse_media_playlist(&content, playlist_url)?;

        if playlist.is_endlist {
            info!("Playlist has ENDLIST tag, stream has ended");
            return Err(anyhow::anyhow!("Stream ended"));
        }

        let downloaded_set = downloaded.read().await;
        let mut new_count = 0;

        for (i, segment) in playlist.segments.iter().enumerate() {
            if downloaded_set.contains(&segment.sequence) {
                continue;
            }

            // Prioritize the most recent segment (live edge)
            let priority = if i == playlist.segments.len() - 1 {
                SegmentPriority::High
            } else {
                SegmentPriority::Normal
            };

            let queued = QueuedSegment {
                segment: segment.clone(),
                priority,
            };

            if queue_tx.send(queued).await.is_err() {
                break; // Queue closed
            }
            new_count += 1;
        }

        Ok((new_count, playlist.init_segment_uri))
    }

    async fn worker_loop(
        id: usize,
        queue_rx: async_channel::Receiver<QueuedSegment>,
        client: Client,
        writer: SegmentWriter,
        event_tx: broadcast::Sender<RecordingEvent>,
        state: Arc<RwLock<RecordingState>>,
        downloaded: Arc<RwLock<HashSet<u64>>>,
    ) {
        debug!("Worker {} started", id);

        while let Ok(queued) = queue_rx.recv().await {
            let segment = queued.segment;

            // Check if already downloaded (race condition prevention)
            {
                let dl = downloaded.read().await;
                if dl.contains(&segment.sequence) {
                    continue;
                }
            }

            // Download segment
            match writer
                .download_and_write(&client, &segment.uri, segment.sequence)
                .await
            {
                Ok(path) => {
                    let size = tokio::fs::metadata(&path)
                        .await
                        .map(|m| m.len())
                        .unwrap_or(0);

                    // Mark as downloaded
                    {
                        let mut dl = downloaded.write().await;
                        dl.insert(segment.sequence);
                    }

                    // Update state
                    {
                        let mut s = state.write().await;
                        s.last_segment = s.last_segment.max(segment.sequence);
                        s.segments_downloaded += 1;
                        s.bytes_downloaded += size;
                    }

                    let _ = event_tx.send(RecordingEvent::SegmentDownloaded {
                        sequence: segment.sequence,
                        size_bytes: size,
                    });

                    debug!(
                        "Worker {}: downloaded segment {} ({} bytes)",
                        id, segment.sequence, size
                    );
                }
                Err(e) => {
                    warn!(
                        "Worker {}: failed to download segment {}: {}",
                        id, segment.sequence, e
                    );
                }
            }
        }

        debug!("Worker {} stopped", id);
    }
}
