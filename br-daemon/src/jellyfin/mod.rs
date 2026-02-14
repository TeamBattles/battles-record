//! Jellyfin-compatible media library export.
//!
//! This module handles exporting stream recordings to a Jellyfin-compatible
//! folder structure with NFO metadata files and images.

pub mod episode_tracker;
pub mod exporter;
pub mod nfo;

pub use episode_tracker::EpisodeTracker;
pub use exporter::JellyfinExporter;
