use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use super::job::{DownloadJob, DownloadStatus};

const INDEX_FILENAME: &str = "downloads-index.json";
const DEBOUNCE_SECS: u64 = 5;

#[derive(thiserror::Error, Debug)]
pub enum IndexError {
    #[error("IO error: {context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Async JSON persistence layer for download jobs with debounced saves.
pub struct DownloadsIndex {
    entries: HashMap<Uuid, DownloadJob>,
    file_path: PathBuf,
    dirty: bool,
    last_save: tokio::time::Instant,
}

impl DownloadsIndex {
    /// Load from `{downloads_dir}/downloads-index.json`, or start empty.
    pub async fn new(downloads_dir: &Path) -> Result<Self, IndexError> {
        let file_path = downloads_dir.join(INDEX_FILENAME);
        let entries = if file_path.exists() {
            match tokio::fs::read_to_string(&file_path).await {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(map) => map,
                    Err(e) => {
                        tracing::warn!(
                            path = %file_path.display(),
                            error = %e,
                            "Malformed downloads index, starting fresh"
                        );
                        HashMap::new()
                    }
                },
                Err(e) => {
                    return Err(IndexError::Io {
                        context: format!("reading {}", file_path.display()),
                        source: e,
                    });
                }
            }
        } else {
            HashMap::new()
        };

        Ok(Self {
            entries,
            file_path,
            dirty: false,
            last_save: tokio::time::Instant::now(),
        })
    }

    pub fn add(&mut self, job: DownloadJob) {
        self.entries.insert(job.id, job);
        self.dirty = true;
    }

    pub fn get(&self, id: &Uuid) -> Option<&DownloadJob> {
        self.entries.get(id)
    }

    pub fn update(&mut self, id: &Uuid, f: impl FnOnce(&mut DownloadJob)) {
        if let Some(job) = self.entries.get_mut(id) {
            f(job);
            self.dirty = true;
        }
    }

    pub fn remove(&mut self, id: &Uuid) -> Option<DownloadJob> {
        let removed = self.entries.remove(id);
        if removed.is_some() {
            self.dirty = true;
        }
        removed
    }

    pub fn list(&self) -> Vec<&DownloadJob> {
        self.entries.values().collect()
    }

    pub fn list_filtered(
        &self,
        status: Option<DownloadStatus>,
        platform: Option<&str>,
        channel: Option<&str>,
    ) -> Vec<&DownloadJob> {
        self.entries
            .values()
            .filter(|job| {
                let status_ok = status.map(|s| job.status == s).unwrap_or(true);
                let platform_ok = platform.map(|p| job.source_platform == p).unwrap_or(true);
                let channel_ok = channel.map(|c| job.channel_name == c).unwrap_or(true);
                status_ok && platform_ok && channel_ok
            })
            .collect()
    }

    /// Sum of downloaded_bytes for completed downloads.
    pub fn total_size(&self) -> u64 {
        self.entries
            .values()
            .filter(|job| job.status == DownloadStatus::Complete)
            .map(|job| job.downloaded_bytes)
            .sum()
    }

    /// Save only if dirty and at least 5 seconds since last save.
    pub async fn save_if_dirty(&mut self) -> Result<(), IndexError> {
        if !self.dirty {
            return Ok(());
        }
        if self.last_save.elapsed() < std::time::Duration::from_secs(DEBOUNCE_SECS) {
            return Ok(());
        }
        self.do_save().await
    }

    /// Save regardless of dirty flag or debounce timer.
    pub async fn force_save(&mut self) -> Result<(), IndexError> {
        self.do_save().await
    }

    async fn do_save(&mut self) -> Result<(), IndexError> {
        let data = serde_json::to_string_pretty(&self.entries)?;
        atomic_save(&self.file_path, data.as_bytes()).await?;
        self.dirty = false;
        self.last_save = tokio::time::Instant::now();
        Ok(())
    }
}

async fn atomic_save(path: &Path, data: &[u8]) -> Result<(), IndexError> {
    let tmp_path = path.with_extension("json.tmp");
    let mut file = tokio::fs::File::create(&tmp_path)
        .await
        .map_err(|e| IndexError::Io {
            context: format!("creating temp file {}", tmp_path.display()),
            source: e,
        })?;
    file.write_all(data).await.map_err(|e| IndexError::Io {
        context: format!("writing temp file {}", tmp_path.display()),
        source: e,
    })?;
    file.sync_all().await.map_err(|e| IndexError::Io {
        context: format!("syncing temp file {}", tmp_path.display()),
        source: e,
    })?;
    tokio::fs::rename(&tmp_path, path)
        .await
        .map_err(|e| IndexError::Io {
            context: format!("renaming {} to {}", tmp_path.display(), path.display()),
            source: e,
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloads::job::DownloadOptions;
    use chrono::Utc;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_job(channel: &str, platform: &str, status: DownloadStatus) -> DownloadJob {
        DownloadJob {
            id: Uuid::new_v4(),
            url: "https://example.com/video".to_string(),
            title: Some("Test Video".to_string()),
            thumbnail: None,
            duration: Some(300),
            uploader: None,
            platform_name: Some(platform.to_string()),
            channel_name: channel.to_string(),
            source_platform: platform.to_string(),
            output_dir: PathBuf::from("/downloads"),
            output_file: None,
            format: None,
            quality: None,
            available_formats: None,
            status,
            percent: 0.0,
            speed: None,
            eta: None,
            downloaded_bytes: 0,
            total_bytes: None,
            options: DownloadOptions::default(),
            requested_by: Uuid::new_v4(),
            requested_by_name: None,
            created_at: Utc::now(),
            completed_at: None,
            error: None,
        }
    }

    #[tokio::test]
    async fn add_and_retrieve() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();

        let job = make_job("streamer", "twitch", DownloadStatus::Queued);
        let id = job.id;
        index.add(job);

        let got = index.get(&id).unwrap();
        assert_eq!(got.channel_name, "streamer");
        assert_eq!(got.status, DownloadStatus::Queued);
    }

    #[tokio::test]
    async fn update_job() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();

        let job = make_job("ch", "youtube", DownloadStatus::Downloading);
        let id = job.id;
        index.add(job);

        index.update(&id, |j| {
            j.percent = 50.0;
            j.downloaded_bytes = 1_000_000;
            j.status = DownloadStatus::Downloading;
        });

        let got = index.get(&id).unwrap();
        assert!((got.percent - 50.0).abs() < f64::EPSILON);
        assert_eq!(got.downloaded_bytes, 1_000_000);
    }

    #[tokio::test]
    async fn update_nonexistent_is_noop() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();
        let bogus = Uuid::new_v4();
        index.update(&bogus, |j| j.percent = 99.0);
        assert!(index.get(&bogus).is_none());
    }

    #[tokio::test]
    async fn remove_job() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();

        let job = make_job("ch", "kick", DownloadStatus::Complete);
        let id = job.id;
        index.add(job);

        let removed = index.remove(&id);
        assert!(removed.is_some());
        assert!(index.get(&id).is_none());

        // Removing again returns None
        assert!(index.remove(&id).is_none());
    }

    #[tokio::test]
    async fn list_all() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();

        index.add(make_job("a", "twitch", DownloadStatus::Queued));
        index.add(make_job("b", "youtube", DownloadStatus::Downloading));
        index.add(make_job("c", "kick", DownloadStatus::Complete));

        assert_eq!(index.list().len(), 3);
    }

    #[tokio::test]
    async fn list_filtered_by_status() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();

        index.add(make_job("a", "twitch", DownloadStatus::Queued));
        index.add(make_job("b", "twitch", DownloadStatus::Downloading));
        index.add(make_job("c", "twitch", DownloadStatus::Complete));

        let queued = index.list_filtered(Some(DownloadStatus::Queued), None, None);
        assert_eq!(queued.len(), 1);

        let complete = index.list_filtered(Some(DownloadStatus::Complete), None, None);
        assert_eq!(complete.len(), 1);
    }

    #[tokio::test]
    async fn list_filtered_by_platform() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();

        index.add(make_job("a", "twitch", DownloadStatus::Queued));
        index.add(make_job("b", "youtube", DownloadStatus::Queued));
        index.add(make_job("c", "youtube", DownloadStatus::Queued));

        let yt = index.list_filtered(None, Some("youtube"), None);
        assert_eq!(yt.len(), 2);

        let tw = index.list_filtered(None, Some("twitch"), None);
        assert_eq!(tw.len(), 1);
    }

    #[tokio::test]
    async fn list_filtered_by_channel() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();

        index.add(make_job("alice", "twitch", DownloadStatus::Queued));
        index.add(make_job("alice", "twitch", DownloadStatus::Complete));
        index.add(make_job("bob", "twitch", DownloadStatus::Queued));

        let alice = index.list_filtered(None, None, Some("alice"));
        assert_eq!(alice.len(), 2);

        let bob = index.list_filtered(None, None, Some("bob"));
        assert_eq!(bob.len(), 1);
    }

    #[tokio::test]
    async fn list_filtered_combined() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();

        index.add(make_job("alice", "twitch", DownloadStatus::Queued));
        index.add(make_job("alice", "youtube", DownloadStatus::Complete));
        index.add(make_job("bob", "twitch", DownloadStatus::Complete));

        let result =
            index.list_filtered(Some(DownloadStatus::Complete), Some("twitch"), Some("bob"));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].channel_name, "bob");

        // No match
        let empty = index.list_filtered(
            Some(DownloadStatus::Complete),
            Some("twitch"),
            Some("alice"),
        );
        assert_eq!(empty.len(), 0);
    }

    #[tokio::test]
    async fn total_size_only_counts_complete() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();

        let mut j1 = make_job("a", "twitch", DownloadStatus::Complete);
        j1.downloaded_bytes = 1000;

        let mut j2 = make_job("b", "twitch", DownloadStatus::Complete);
        j2.downloaded_bytes = 2000;

        // This one is still downloading, should NOT count
        let mut j3 = make_job("c", "twitch", DownloadStatus::Downloading);
        j3.downloaded_bytes = 5000;

        index.add(j1);
        index.add(j2);
        index.add(j3);

        assert_eq!(index.total_size(), 3000);
    }

    #[tokio::test]
    async fn debounce_skips_save_within_window() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();
        let index_file = dir.path().join(INDEX_FILENAME);

        let job = make_job("ch", "twitch", DownloadStatus::Queued);
        index.add(job);

        // First save should work (initial last_save is "now" from construction,
        // but we just created the index so we need to wait or force)
        // Actually last_save is set at construction time, so save_if_dirty may
        // be debounced. Use force_save to establish baseline, then test debounce.
        index.force_save().await.unwrap();
        assert!(index_file.exists());

        // Now add another job and try save_if_dirty immediately - should be debounced
        index.add(make_job("ch2", "youtube", DownloadStatus::Queued));

        // Remove the file so we can check if save_if_dirty actually writes
        tokio::fs::remove_file(&index_file).await.unwrap();

        index.save_if_dirty().await.unwrap();

        // File should NOT exist because debounce suppressed the write
        assert!(!index_file.exists());
    }

    #[tokio::test]
    async fn force_save_always_writes() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();
        let index_file = dir.path().join(INDEX_FILENAME);

        index.add(make_job("ch", "twitch", DownloadStatus::Queued));

        // force_save ignores debounce
        index.force_save().await.unwrap();
        assert!(index_file.exists());

        // Even without dirty flag, force_save writes
        tokio::fs::remove_file(&index_file).await.unwrap();
        index.force_save().await.unwrap();
        assert!(index_file.exists());
    }

    #[tokio::test]
    async fn persistence_round_trip() {
        let dir = TempDir::new().unwrap();

        let id;
        {
            let mut index = DownloadsIndex::new(dir.path()).await.unwrap();
            let mut job = make_job("streamer", "kick", DownloadStatus::Complete);
            job.downloaded_bytes = 42_000;
            id = job.id;
            index.add(job);
            index.force_save().await.unwrap();
        }

        // Reload from disk
        let index = DownloadsIndex::new(dir.path()).await.unwrap();
        let got = index.get(&id).unwrap();
        assert_eq!(got.channel_name, "streamer");
        assert_eq!(got.source_platform, "kick");
        assert_eq!(got.status, DownloadStatus::Complete);
        assert_eq!(got.downloaded_bytes, 42_000);
    }

    #[tokio::test]
    async fn malformed_json_starts_fresh() {
        let dir = TempDir::new().unwrap();
        let index_file = dir.path().join(INDEX_FILENAME);

        tokio::fs::write(&index_file, b"not valid json {{{")
            .await
            .unwrap();

        let index = DownloadsIndex::new(dir.path()).await.unwrap();
        assert_eq!(index.list().len(), 0);
    }

    #[tokio::test]
    async fn empty_index_loads_clean() {
        let dir = TempDir::new().unwrap();
        let index = DownloadsIndex::new(dir.path()).await.unwrap();
        assert_eq!(index.list().len(), 0);
        assert_eq!(index.total_size(), 0);
    }

    #[tokio::test]
    async fn save_if_dirty_noop_when_clean() {
        let dir = TempDir::new().unwrap();
        let mut index = DownloadsIndex::new(dir.path()).await.unwrap();
        let index_file = dir.path().join(INDEX_FILENAME);

        // No mutations, save_if_dirty should be a no-op
        index.save_if_dirty().await.unwrap();
        assert!(!index_file.exists());
    }
}
