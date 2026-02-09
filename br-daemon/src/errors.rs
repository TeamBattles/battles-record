//! Error types for br-daemon.
//!
//! This module provides typed error enums for different subsystems,
//! enabling proper error handling without panics.

use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

/** Top-level daemon errors. */
#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("Processing error: {0}")]
    Processing(#[from] ProcessingError),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Notification error: {0}")]
    Notification(#[from] NotificationError),

    #[error("Platform error: {0}")]
    Platform(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/** Storage-related errors. */
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Recording not found: {0}")]
    RecordingNotFound(Uuid),

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Failed to write recording {id}: {source}")]
    WriteFailed {
        id: Uuid,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to read recording {id}: {source}")]
    ReadFailed {
        id: Uuid,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to delete recording {id}: {source}")]
    DeleteFailed {
        id: Uuid,
        #[source]
        source: std::io::Error,
    },

    #[error("Index corrupted: {0}")]
    IndexCorrupted(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/** Processing/FFmpeg errors. */
#[derive(Error, Debug)]
pub enum ProcessingError {
    #[error("FFmpeg failed with exit code {code}: {stderr}")]
    FfmpegFailed { code: i32, stderr: String },

    #[error("FFmpeg not found at {path}")]
    FfmpegNotFound { path: PathBuf },

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("No segments found in {path}")]
    NoSegments { path: PathBuf },

    #[error("Semaphore closed unexpectedly")]
    SemaphoreClosed,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/** Configuration errors. */
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to load config from {path}: {source}")]
    LoadFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse config: {0}")]
    ParseFailed(String),

    #[error("Failed to save config to {path}: {source}")]
    SaveFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Invalid value for {field}: {message}")]
    InvalidValue { field: String, message: String },

    #[error("Password hashing failed: {0}")]
    PasswordHashFailed(String),
}

/** Notification errors. */
#[derive(Error, Debug)]
pub enum NotificationError {
    #[error("Failed to create HTTP client: {0}")]
    ClientCreationFailed(String),

    #[error("Failed to send notification: {0}")]
    SendFailed(String),

    #[error("Invalid webhook URL: {0}")]
    InvalidUrl(String),
}

/** Validation errors for input data. */
#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Channel name is empty")]
    EmptyChannelName,

    #[error("Channel name too long (max {max} chars): {name}")]
    ChannelNameTooLong { name: String, max: usize },

    #[error("Channel name contains invalid characters: {name}")]
    InvalidChannelNameChars { name: String },

    #[error("Channel name contains invalid characters for {platform}: {name}")]
    InvalidChannelNameCharsForPlatform { name: String, platform: String },

    #[error("Invalid path: {0}")]
    InvalidPath(String),
}
