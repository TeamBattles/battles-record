use crate::types::{Channel, Platform};
use serde::Serialize;
use uuid::Uuid;

/** Events sent over WebSocket to clients. */
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    /** Sent on initial WebSocket connection. */
    Connected {
        channels: Vec<Channel>,
        active_recordings: Vec<ActiveRecordingInfo>,
    },

    /** Channel status changed. */
    ChannelStatus {
        channel_id: Uuid,
        name: String,
        platform: Platform,
        status: String,
        stream: Option<StreamData>,
    },

    /** Error for a specific channel. */
    ChannelError {
        channel_id: Uuid,
        name: String,
        error: String,
    },

    /** Recording started. */
    RecordingStarted {
        recording_id: Uuid,
        channel_id: Uuid,
        channel_name: String,
        platform: Platform,
        quality: String,
    },

    /** Segment downloaded (per-segment granularity). */
    SegmentDownloaded {
        recording_id: Uuid,
        sequence: u32,
        size_bytes: u64,
        total_segments: u32,
        total_bytes: u64,
    },

    /** Recording ended. */
    RecordingEnded {
        recording_id: Uuid,
        duration_secs: u64,
        size_bytes: u64,
        segment_count: u32,
        reason: String,
    },

    /** Post-processing started. */
    ProcessingStarted { recording_id: Uuid },

    /** Post-processing progress. */
    ProcessingProgress { recording_id: Uuid, percent: u8 },

    /** Post-processing complete. */
    ProcessingComplete {
        recording_id: Uuid,
        output_file: String,
        size_bytes: u64,
    },

    /** Post-processing failed. */
    ProcessingFailed { recording_id: Uuid, error: String },

    /** Disk space warning. */
    DiskWarning { usage_percent: f32, free_bytes: u64 },

    /** Config was reloaded. */
    ConfigReloaded { sections: Vec<String> },

    /** Recording skipped due to schedule rules. */
    ScheduleSkip {
        channel_id: Uuid,
        channel_name: String,
        platform: String,
    },

    /** Recording skipped due to filter rules. */
    FilterSkip {
        channel_id: Uuid,
        channel_name: String,
        platform: String,
        reason: serde_json::Value,
    },

    /** Recording skipped due to quota exceeded. */
    QuotaSkip {
        channel_id: Uuid,
        channel_name: String,
        platform: String,
        quota_used_bytes: u64,
        quota_limit_bytes: u64,
    },

    /** Quota status changed for a channel. */
    QuotaStatusChanged {
        channel_id: Uuid,
        channel_name: String,
        quota_status: crate::types::QuotaStatus,
        quota_used_bytes: u64,
        quota_percent: u8,
    },

    /** Platform authentication updated (OAuth connected or token refreshed). */
    PlatformAuthUpdated {
        platform: Platform,
        status: String,
        username: Option<String>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    },

    /** Platform authentication expired (refresh failed). */
    PlatformAuthExpired { platform: Platform, reason: String },
}

/** Stream info for WebSocket events. */
#[derive(Debug, Clone, Serialize)]
pub struct StreamData {
    pub title: String,
    pub game: Option<String>,
    pub viewers: u32,
}

/** Active recording info for initial state. */
#[derive(Debug, Clone, Serialize)]
pub struct ActiveRecordingInfo {
    pub recording_id: Uuid,
    pub channel_id: Uuid,
    pub channel_name: String,
    pub platform: Platform,
    pub duration_secs: u64,
    pub size_bytes: u64,
    pub segments: u32,
}
