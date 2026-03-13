//! Persistent index for tracking recordings on disk.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

use crate::types::Platform;

/** Status of a recording in the index. */
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingStatus {
    Recording,
    Stopping,          // Graceful shutdown in progress
    PendingProcessing, // Waiting for post-processing
    Processing,
    Processed,
    ProcessingFailed, // FFmpeg failed (can retry)
    Failed,
    Completed, // Re-added for backwards compat
}

/** A single recording entry in the index. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingEntry {
    pub id: Uuid,
    pub channel_name: String,
    pub platform: Platform,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<u64>,
    pub status: RecordingStatus,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub segment_count: u32,
    pub title: Option<String>,
    pub game: Option<String>,
    pub output_file: Option<PathBuf>,
    #[serde(default)]
    pub processed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub processing_attempts: u32,
    /** Reason for processing failure (if status is ProcessingFailed). */
    #[serde(default)]
    pub failure_reason: Option<String>,
    /** Whether this recording has been exported to Jellyfin library. */
    #[serde(default)]
    pub jellyfin_exported: bool,
    /** Path to the exported file in Jellyfin library. */
    #[serde(default)]
    pub jellyfin_path: Option<PathBuf>,
    /** Stream thumbnail URL captured at recording start. */
    #[serde(default)]
    pub thumbnail_url: Option<String>,
}

/** Index for tracking all recordings, persisted to disk. */
#[derive(Debug)]
pub struct RecordingsIndex {
    base_path: PathBuf,
    entries: HashMap<Uuid, RecordingEntry>,
}

impl RecordingsIndex {
    const INDEX_FILENAME: &'static str = ".recordings_index.json";

    /** Create a new RecordingsIndex, loading from disk if file exists. */
    pub fn new(base_path: PathBuf) -> anyhow::Result<Self> {
        let index_path = base_path.join(Self::INDEX_FILENAME);
        let entries = if index_path.exists() {
            let content = fs::read_to_string(&index_path)?;
            serde_json::from_str(&content)?
        } else {
            HashMap::new()
        };

        Ok(Self { base_path, entries })
    }

    /** Add a new recording entry to the index. */
    pub fn add(&mut self, entry: RecordingEntry) -> anyhow::Result<()> {
        self.entries.insert(entry.id, entry);
        self.save()
    }

    /** Get a recording entry by ID. */
    pub fn get(&self, id: Uuid) -> Option<&RecordingEntry> {
        self.entries.get(&id)
    }

    /** Update an existing recording entry. */
    pub fn update(&mut self, entry: RecordingEntry) -> anyhow::Result<()> {
        if !self.entries.contains_key(&entry.id) {
            anyhow::bail!("Recording {} not found", entry.id);
        }
        self.entries.insert(entry.id, entry);
        self.save()
    }

    /** Delete a recording entry by ID. */
    pub fn delete(&mut self, id: Uuid) -> anyhow::Result<()> {
        if self.entries.remove(&id).is_none() {
            anyhow::bail!("Recording {} not found", id);
        }
        self.save()
    }

    /** List all recordings, optionally filtered. */
    pub fn list(
        &self,
        channel: Option<&str>,
        platform: Option<Platform>,
        status: Option<RecordingStatus>,
    ) -> Vec<&RecordingEntry> {
        self.entries
            .values()
            .filter(|entry| {
                let channel_match = channel.map(|c| entry.channel_name == c).unwrap_or(true);
                let platform_match = platform.map(|p| entry.platform == p).unwrap_or(true);
                let status_match = status.map(|s| entry.status == s).unwrap_or(true);
                channel_match && platform_match && status_match
            })
            .collect()
    }

    /** Calculate total size of all recordings in bytes. */
    pub fn total_size(&self) -> u64 {
        self.entries.values().map(|e| e.size_bytes).sum()
    }

    /** Calculate total size of recordings for a specific channel. */
    pub fn channel_size(&self, channel_name: &str) -> u64 {
        self.entries
            .values()
            .filter(|e| e.channel_name == channel_name)
            .map(|e| e.size_bytes)
            .sum()
    }

    /** Save the index to disk using atomic write. */
    fn save(&self) -> anyhow::Result<()> {
        let index_path = self.base_path.join(Self::INDEX_FILENAME);
        let tmp_path = self.base_path.join(".recordings_index.json.tmp");

        // Write to temp file
        let content = serde_json::to_string_pretty(&self.entries)?;
        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;

        // Atomic rename
        fs::rename(&tmp_path, &index_path)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_entry(id: Uuid, channel_name: &str, platform: Platform) -> RecordingEntry {
        RecordingEntry {
            id,
            channel_name: channel_name.to_string(),
            platform,
            started_at: Utc::now(),
            ended_at: None,
            duration_secs: None,
            status: RecordingStatus::Recording,
            path: PathBuf::from("/test/path"),
            size_bytes: 1000,
            segment_count: 10,
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
    fn test_index_add_and_list() {
        let temp_dir = TempDir::new().unwrap();
        let mut index = RecordingsIndex::new(temp_dir.path().to_path_buf()).unwrap();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let entry1 = create_test_entry(id1, "streamer1", Platform::Twitch);
        let entry2 = create_test_entry(id2, "streamer2", Platform::YouTube);

        index.add(entry1).unwrap();
        index.add(entry2).unwrap();

        let all = index.list(None, None, None);
        assert_eq!(all.len(), 2);

        // Verify we can get entries by ID
        assert!(index.get(id1).is_some());
        assert!(index.get(id2).is_some());
        assert!(index.get(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_index_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();

        let id = Uuid::new_v4();

        // Create index and add entry
        {
            let mut index = RecordingsIndex::new(base_path.clone()).unwrap();
            let entry = create_test_entry(id, "persistent_streamer", Platform::Kick);
            index.add(entry).unwrap();
        }

        // Reload index and verify entry persists
        {
            let index = RecordingsIndex::new(base_path).unwrap();
            let all = index.list(None, None, None);
            assert_eq!(all.len(), 1);

            let entry = index.get(id).unwrap();
            assert_eq!(entry.channel_name, "persistent_streamer");
            assert_eq!(entry.platform, Platform::Kick);
        }
    }

    #[test]
    fn test_index_update() {
        let temp_dir = TempDir::new().unwrap();
        let mut index = RecordingsIndex::new(temp_dir.path().to_path_buf()).unwrap();

        let id = Uuid::new_v4();
        let mut entry = create_test_entry(id, "streamer", Platform::Twitch);
        index.add(entry.clone()).unwrap();

        // Update the entry
        entry.status = RecordingStatus::Completed;
        entry.ended_at = Some(Utc::now());
        entry.duration_secs = Some(3600);
        entry.size_bytes = 5000;

        index.update(entry).unwrap();

        let updated = index.get(id).unwrap();
        assert_eq!(updated.status, RecordingStatus::Completed);
        assert!(updated.ended_at.is_some());
        assert_eq!(updated.duration_secs, Some(3600));
        assert_eq!(updated.size_bytes, 5000);
    }

    #[test]
    fn test_index_delete() {
        let temp_dir = TempDir::new().unwrap();
        let mut index = RecordingsIndex::new(temp_dir.path().to_path_buf()).unwrap();

        let id = Uuid::new_v4();
        let entry = create_test_entry(id, "to_delete", Platform::Twitch);
        index.add(entry).unwrap();

        assert!(index.get(id).is_some());

        index.delete(id).unwrap();

        assert!(index.get(id).is_none());
        assert_eq!(index.list(None, None, None).len(), 0);

        // Deleting non-existent entry should error
        assert!(index.delete(Uuid::new_v4()).is_err());
    }

    #[test]
    fn test_index_filter_by_channel() {
        let temp_dir = TempDir::new().unwrap();
        let mut index = RecordingsIndex::new(temp_dir.path().to_path_buf()).unwrap();

        // Add multiple entries for different channels
        for _ in 0..3 {
            let entry = create_test_entry(Uuid::new_v4(), "channel_a", Platform::Twitch);
            index.add(entry).unwrap();
        }

        for _ in 0..2 {
            let entry = create_test_entry(Uuid::new_v4(), "channel_b", Platform::YouTube);
            index.add(entry).unwrap();
        }

        // Filter by channel
        let channel_a = index.list(Some("channel_a"), None, None);
        assert_eq!(channel_a.len(), 3);

        let channel_b = index.list(Some("channel_b"), None, None);
        assert_eq!(channel_b.len(), 2);

        // Filter by platform
        let twitch = index.list(None, Some(Platform::Twitch), None);
        assert_eq!(twitch.len(), 3);

        let youtube = index.list(None, Some(Platform::YouTube), None);
        assert_eq!(youtube.len(), 2);

        // Combined filters
        let channel_a_twitch = index.list(Some("channel_a"), Some(Platform::Twitch), None);
        assert_eq!(channel_a_twitch.len(), 3);

        let channel_a_youtube = index.list(Some("channel_a"), Some(Platform::YouTube), None);
        assert_eq!(channel_a_youtube.len(), 0);
    }

    #[test]
    fn test_index_total_size() {
        let temp_dir = TempDir::new().unwrap();
        let mut index = RecordingsIndex::new(temp_dir.path().to_path_buf()).unwrap();

        let mut entry1 = create_test_entry(Uuid::new_v4(), "channel1", Platform::Twitch);
        entry1.size_bytes = 1000;

        let mut entry2 = create_test_entry(Uuid::new_v4(), "channel1", Platform::Twitch);
        entry2.size_bytes = 2000;

        let mut entry3 = create_test_entry(Uuid::new_v4(), "channel2", Platform::YouTube);
        entry3.size_bytes = 500;

        index.add(entry1).unwrap();
        index.add(entry2).unwrap();
        index.add(entry3).unwrap();

        assert_eq!(index.total_size(), 3500);
        assert_eq!(index.channel_size("channel1"), 3000);
        assert_eq!(index.channel_size("channel2"), 500);
        assert_eq!(index.channel_size("nonexistent"), 0);
    }

    #[test]
    fn test_index_filter_by_status() {
        let temp_dir = TempDir::new().unwrap();
        let mut index = RecordingsIndex::new(temp_dir.path().to_path_buf()).unwrap();

        let mut entry1 = create_test_entry(Uuid::new_v4(), "channel", Platform::Twitch);
        entry1.status = RecordingStatus::Recording;

        let mut entry2 = create_test_entry(Uuid::new_v4(), "channel", Platform::Twitch);
        entry2.status = RecordingStatus::Completed;

        let mut entry3 = create_test_entry(Uuid::new_v4(), "channel", Platform::Twitch);
        entry3.status = RecordingStatus::Processed;

        index.add(entry1).unwrap();
        index.add(entry2).unwrap();
        index.add(entry3).unwrap();

        let recording = index.list(None, None, Some(RecordingStatus::Recording));
        assert_eq!(recording.len(), 1);

        let completed = index.list(None, None, Some(RecordingStatus::Completed));
        assert_eq!(completed.len(), 1);

        let processed = index.list(None, None, Some(RecordingStatus::Processed));
        assert_eq!(processed.len(), 1);
    }
}
