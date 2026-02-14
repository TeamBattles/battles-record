use crate::config::SegmentHandling;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/** Defines how a recording should be processed after completion. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessingMode {
    /** Remux without re-encoding (fast, lossless). */
    Remux {
        /** Output format: "mp4", "mkv". */
        format: String,
    },
    /** Transcode with re-encoding (slower, can reduce size). */
    Transcode {
        /** Output format: "mp4", "mkv". */
        format: String,
        /** Video codec: "h264", "h265", "av1". */
        codec: String,
        /** FFmpeg preset: "ultrafast" to "veryslow". */
        preset: String,
        /** Constant Rate Factor: 0-51, lower = better quality. */
        crf: u8,
    },
}

impl Default for ProcessingMode {
    fn default() -> Self {
        ProcessingMode::Remux {
            format: "mp4".to_string(),
        }
    }
}

/** A post-processing job for a completed recording. */
#[derive(Debug, Clone)]
pub struct ProcessingJob {
    /** Unique identifier for this job. */
    pub id: Uuid,
    /** ID of the recording this job processes. */
    pub recording_id: Uuid,
    /** Channel name (for display and filename). */
    pub channel_name: String,
    /** Platform name (for display). */
    pub platform: String,
    /** Directory containing the recording segments. */
    pub recording_path: PathBuf,
    /** Path where the processed output will be written. */
    pub output_path: PathBuf,
    /** Processing mode (remux or transcode). */
    pub mode: ProcessingMode,
    /** What to do with segment files after successful processing. */
    pub segment_handling: SegmentHandling,
    /** When this job was created. */
    pub created_at: DateTime<Utc>,
    /** Duration of the recording in seconds (for progress calculation). */
    pub duration_secs: Option<u64>,
}

impl ProcessingJob {
    /**
     * Create a new processing job for a recording.
     *
     * The output path is automatically generated based on the channel name,
     * current timestamp, and the output format from the processing mode.
     * By default, output goes to the same directory as the recording.
     */
    pub fn new(
        recording_id: Uuid,
        channel_name: String,
        platform: String,
        recording_path: PathBuf,
        mode: ProcessingMode,
        segment_handling: SegmentHandling,
        duration_secs: Option<u64>,
    ) -> Self {
        Self::with_output_dir(
            recording_id,
            channel_name,
            platform,
            recording_path.clone(),
            recording_path, // Default: output to same dir as recording
            mode,
            segment_handling,
            duration_secs,
        )
    }

    /**
     * Create a new processing job with a specific output directory.
     *
     * Use this when you want the processed file to go to a different
     * location than the recording (e.g., library_dir for Jellyfin).
     */
    pub fn with_output_dir(
        recording_id: Uuid,
        channel_name: String,
        platform: String,
        recording_path: PathBuf,
        output_dir: PathBuf,
        mode: ProcessingMode,
        segment_handling: SegmentHandling,
        duration_secs: Option<u64>,
    ) -> Self {
        let format = match &mode {
            ProcessingMode::Remux { format } => format,
            ProcessingMode::Transcode { format, .. } => format,
        };

        let output_filename = format!(
            "{}_{}.{}",
            channel_name,
            chrono::Utc::now().format("%Y-%m-%d_%H%M%S"),
            format
        );
        let output_path = output_dir.join(output_filename);

        Self {
            id: Uuid::new_v4(),
            recording_id,
            channel_name,
            platform,
            recording_path,
            output_path,
            mode,
            segment_handling,
            created_at: Utc::now(),
            duration_secs,
        }
    }
}

/** Current status of a processing job. */
#[derive(Debug, Clone, Serialize)]
pub enum JobStatus {
    /** Job is waiting in the queue. */
    Queued {
        /** Position in the queue (0 = next to be processed). */
        position: usize,
    },
    /** Job is currently being processed. */
    Processing {
        /** Completion percentage (0-100). */
        percent: u8,
        /** Processing speed (e.g., "2.5x"). */
        speed: Option<String>,
    },
    /** Job completed successfully. */
    Complete {
        /** Path to the output file. */
        output_path: String,
        /** Size of the output file in bytes. */
        size_bytes: u64,
    },
    /** Job failed with an error. */
    Failed {
        /** Error message describing the failure. */
        error: String,
    },
    /** Job was cancelled by user. */
    Cancelled,
}

/** Progress information during processing. */
#[derive(Debug, Clone)]
pub struct ProcessingProgress {
    /** Completion percentage (0-100). */
    pub percent: u8,
    /** Processing speed (e.g., "2.5x"). */
    pub speed: Option<String>,
    /** Output time in milliseconds (used for progress calculation). */
    pub out_time_ms: u64,
}
