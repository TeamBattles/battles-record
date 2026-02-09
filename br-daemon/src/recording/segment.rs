use std::path::PathBuf;
use thiserror::Error;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

#[derive(Error, Debug)]
pub enum SegmentError {
    #[error("Download failed: {0}")]
    Download(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct SegmentWriter {
    output_dir: PathBuf,
}

impl SegmentWriter {
    pub fn new(output_dir: PathBuf) -> Self {
        Self { output_dir }
    }

    /**
     * Download a segment and write it atomically to disk.
     * Returns the final file path.
     */
    pub async fn download_and_write(
        &self,
        client: &reqwest::Client,
        url: &str,
        sequence: u64,
    ) -> Result<PathBuf, SegmentError> {
        // Download segment into memory
        let response = client.get(url).send().await?;
        let bytes = response.bytes().await?;

        // Write atomically
        self.write_segment(sequence, &bytes).await
    }

    /** Write segment data atomically: write to .tmp, fsync, rename. */
    pub async fn write_segment(&self, sequence: u64, data: &[u8]) -> Result<PathBuf, SegmentError> {
        let filename = format!("{:07}.ts", sequence);
        self.write_file(&filename, data).await
    }

    /**
     * Write initialization segment for fMP4/CMAF streams.
     * This file contains the ftyp and moov boxes needed to decode media segments.
     */
    pub async fn write_init_segment(&self, data: &[u8]) -> Result<PathBuf, SegmentError> {
        self.write_file("init.mp4", data).await
    }

    /** Download and write the initialization segment. */
    pub async fn download_and_write_init(
        &self,
        client: &reqwest::Client,
        url: &str,
    ) -> Result<PathBuf, SegmentError> {
        let response = client.get(url).send().await?;
        let bytes = response.bytes().await?;
        self.write_init_segment(&bytes).await
    }

    /** Check if init segment exists. */
    pub async fn has_init_segment(&self) -> bool {
        self.output_dir.join("init.mp4").exists()
    }

    /** Write a file atomically: write to .tmp, fsync, rename. */
    async fn write_file(&self, filename: &str, data: &[u8]) -> Result<PathBuf, SegmentError> {
        let final_path = self.output_dir.join(filename);
        let temp_path = self.output_dir.join(format!("{}.tmp", filename));

        // Ensure output directory exists
        fs::create_dir_all(&self.output_dir).await?;

        // Write to temp file
        let mut file = File::create(&temp_path).await?;
        file.write_all(data).await?;
        file.sync_all().await?; // fsync to ensure data is on disk
        drop(file);

        // Atomic rename
        fs::rename(&temp_path, &final_path).await?;

        Ok(final_path)
    }

    /** Get the highest sequence number from existing segment files. */
    pub async fn get_last_sequence(&self) -> Option<u64> {
        let mut entries = match fs::read_dir(&self.output_dir).await {
            Ok(e) => e,
            Err(_) => return None,
        };

        let mut max_seq: Option<u64> = None;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let filename = entry.file_name();
            let name = filename.to_string_lossy();

            // Match pattern: 0000001.ts
            if name.ends_with(".ts") && !name.ends_with(".tmp") {
                if let Some(seq_str) = name.strip_suffix(".ts") {
                    if let Ok(seq) = seq_str.parse::<u64>() {
                        max_seq = Some(max_seq.map_or(seq, |m| m.max(seq)));
                    }
                }
            }
        }

        max_seq
    }

    /** Clean up any incomplete .tmp files. */
    pub async fn cleanup_temp_files(&self) -> Result<u32, SegmentError> {
        let mut entries = match fs::read_dir(&self.output_dir).await {
            Ok(e) => e,
            Err(_) => return Ok(0),
        };

        let mut count = 0;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let filename = entry.file_name();
            if filename.to_string_lossy().ends_with(".tmp") {
                fs::remove_file(entry.path()).await?;
                count += 1;
            }
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_write_segment() {
        let temp_dir = TempDir::new().unwrap();
        let writer = SegmentWriter::new(temp_dir.path().to_path_buf());

        let data = b"test segment data";
        let path = writer.write_segment(123, data).await.unwrap();

        assert!(path.exists());
        assert_eq!(path.file_name().unwrap(), "0000123.ts");

        let content = std::fs::read(&path).unwrap();
        assert_eq!(content, data);
    }

    #[tokio::test]
    async fn test_get_last_sequence() {
        let temp_dir = TempDir::new().unwrap();
        let writer = SegmentWriter::new(temp_dir.path().to_path_buf());

        // No files yet
        assert_eq!(writer.get_last_sequence().await, None);

        // Write some segments
        writer.write_segment(5, b"data").await.unwrap();
        writer.write_segment(10, b"data").await.unwrap();
        writer.write_segment(3, b"data").await.unwrap();

        assert_eq!(writer.get_last_sequence().await, Some(10));
    }

    #[tokio::test]
    async fn test_cleanup_temp_files() {
        let temp_dir = TempDir::new().unwrap();
        let writer = SegmentWriter::new(temp_dir.path().to_path_buf());

        // Create some temp files
        std::fs::create_dir_all(temp_dir.path()).unwrap();
        std::fs::write(temp_dir.path().join("0000001.ts.tmp"), "temp").unwrap();
        std::fs::write(temp_dir.path().join("0000002.ts.tmp"), "temp").unwrap();
        std::fs::write(temp_dir.path().join("0000003.ts"), "complete").unwrap();

        let cleaned = writer.cleanup_temp_files().await.unwrap();
        assert_eq!(cleaned, 2);

        // Verify temp files removed, complete file remains
        assert!(!temp_dir.path().join("0000001.ts.tmp").exists());
        assert!(!temp_dir.path().join("0000002.ts.tmp").exists());
        assert!(temp_dir.path().join("0000003.ts").exists());
    }
}
