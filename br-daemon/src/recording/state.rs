use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingState {
    pub channel: String,
    pub platform: String,
    pub started_at: DateTime<Utc>,
    pub last_segment: u64,
    pub quality: String,
    pub segments_downloaded: u32,
    pub bytes_downloaded: u64,
}

impl RecordingState {
    pub fn new(channel: &str, platform: &str, quality: &str) -> Self {
        Self {
            channel: channel.to_string(),
            platform: platform.to_string(),
            started_at: Utc::now(),
            last_segment: 0,
            quality: quality.to_string(),
            segments_downloaded: 0,
            bytes_downloaded: 0,
        }
    }

    pub async fn load(path: &Path) -> Option<Self> {
        let content = fs::read_to_string(path).await.ok()?;
        serde_json::from_str(&content).ok()
    }

    pub async fn save(&self, path: &Path) -> std::io::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        let temp_path = path.with_extension("json.tmp");
        fs::write(&temp_path, &content).await?;
        fs::rename(&temp_path, path).await?;
        Ok(())
    }
}
