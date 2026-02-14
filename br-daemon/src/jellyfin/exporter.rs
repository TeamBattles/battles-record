//! Main Jellyfin export orchestration.
//!
//! Coordinates the export of processed recordings to a Jellyfin-compatible
//! folder structure with NFO metadata and images.

use super::episode_tracker::EpisodeTracker;
use super::nfo::{self, EpisodeMetadata};
use crate::config::JellyfinConfig;
use crate::image_generator::{ImageGenerator, ImageMetadata, SeasonMetadata, ShowMetadata};
use crate::platforms::ChannelProfile;
use crate::storage::RecordingEntry;
use crate::types::Platform;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/** Result of a Jellyfin export operation. */
#[derive(Debug)]
pub struct ExportResult {
    /** Path to the exported video file. */
    pub video_path: PathBuf,
    /** Path to the episode NFO file. */
    pub nfo_path: PathBuf,
    /** Season number used. */
    pub season: u32,
    /** Episode number assigned. */
    pub episode: u32,
}

/** Handles exporting recordings to Jellyfin library structure. */
pub struct JellyfinExporter {
    /** Configuration. */
    config: JellyfinConfig,
    /** Library root directory. */
    library_dir: PathBuf,
    /** Episode number tracker. */
    tracker: EpisodeTracker,
    /** Image generator for rich thumbnails. */
    image_generator: ImageGenerator,
}

impl JellyfinExporter {
    /**
     * Create a new JellyfinExporter.
     *
     * Note: This constructor uses blocking I/O to create the library directory.
     * It should be called during startup, not in hot paths.
     */
    pub fn new(config: JellyfinConfig, library_dir: PathBuf) -> anyhow::Result<Self> {
        // Ensure library directory exists (blocking - called at startup only)
        std::fs::create_dir_all(&library_dir)?;

        let tracker = EpisodeTracker::new(library_dir.clone())?;
        let image_generator = ImageGenerator::new()?;

        Ok(Self {
            config,
            library_dir,
            tracker,
            image_generator,
        })
    }

    /** Check if Jellyfin export is enabled. */
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /**
     * Export a recording to the Jellyfin library.
     *
     * This will:
     * 1. Create the show/season directory structure
     * 2. Copy the processed video file with Jellyfin naming
     * 3. Generate episode NFO
     * 4. Ensure show-level NFO and images exist
     * 5. Generate rich episode thumbnail
     * 6. Ensure season-level images exist
     */
    pub async fn export_recording(
        &mut self,
        recording: &RecordingEntry,
        processed_file: &Path,
        channel_profile: &ChannelProfile,
    ) -> anyhow::Result<ExportResult> {
        let platform_str = recording.platform.to_string();

        info!(
            "Exporting {} recording {} to Jellyfin library",
            recording.channel_name, recording.id
        );

        // Create directory structure: library/{platform}/{channel}/Season XX/
        let show_dir = self
            .library_dir
            .join(&platform_str)
            .join(&recording.channel_name);

        // Get episode numbering (season = month)
        let (season, episode) = self.tracker.get_next_episode(
            &platform_str,
            &recording.channel_name,
            recording.started_at,
        );

        let season_dir = show_dir.join(format!("Season {}", season));
        tokio::fs::create_dir_all(&season_dir).await?;

        // Ensure show-level metadata exists
        self.ensure_show_metadata(&show_dir, recording, channel_profile)
            .await?;

        // Ensure season-level metadata exists
        self.ensure_season_metadata(
            &season_dir,
            season,
            recording.started_at,
            channel_profile,
            &platform_str,
        )
        .await?;

        // Generate Jellyfin-compatible filename
        let title = recording.title.as_deref().unwrap_or("Stream");
        let sanitized_title = truncate_title(&sanitize_filename(title), 80);
        let date_str = recording.started_at.format("%Y-%m-%d").to_string();
        let extension = processed_file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mp4");

        let video_filename = format!(
            "{} - S{}E{:03} - {} - {}.{}",
            recording.channel_name, season, episode, date_str, sanitized_title, extension
        );
        let video_path = season_dir.join(&video_filename);

        // Move the processed file to library (copy then delete original)
        // This prevents duplicates showing up in Jellyfin
        info!("Moving {} to {:?}", processed_file.display(), video_path);
        tokio::fs::copy(processed_file, &video_path).await?;

        // Delete the original file after successful copy to avoid duplicates
        // Only delete if the source is within our library directory (not recordings dir)
        // Use canonicalize for proper path comparison on Windows (handles UNC paths, case, etc.)
        let should_delete = match (
            processed_file.canonicalize(),
            self.library_dir.canonicalize(),
        ) {
            (Ok(canonical_file), Ok(canonical_lib)) => {
                let result = canonical_file.starts_with(&canonical_lib);
                debug!(
                    "Path comparison: file={:?}, library={:?}, should_delete={}",
                    canonical_file, canonical_lib, result
                );
                result
            }
            (Err(e1), _) => {
                warn!(
                    "Failed to canonicalize processed file path {}: {}",
                    processed_file.display(),
                    e1
                );
                false
            }
            (_, Err(e2)) => {
                warn!(
                    "Failed to canonicalize library dir path {:?}: {}",
                    self.library_dir, e2
                );
                false
            }
        };

        if should_delete {
            if let Err(e) = tokio::fs::remove_file(processed_file).await {
                warn!(
                    "Failed to remove original file after Jellyfin export: {}",
                    e
                );
            } else {
                debug!(
                    "Removed original file {} after export to season folder",
                    processed_file.display()
                );
            }
        }

        // Generate episode NFO
        let duration_minutes = recording.duration_secs.unwrap_or(0) / 60;
        let nfo_filename = format!(
            "{} - S{}E{:03} - {} - {}.nfo",
            recording.channel_name, season, episode, date_str, sanitized_title
        );
        let nfo_path = season_dir.join(&nfo_filename);

        let episode_metadata = EpisodeMetadata {
            title,
            show_title: &channel_profile.display_name,
            season,
            episode,
            aired: recording.started_at,
            game: recording.game.as_deref(),
            duration_minutes,
            recording_id: &recording.id.to_string(),
        };
        let nfo_content = nfo::generate_episode_nfo(&episode_metadata);
        nfo::write_nfo(&nfo_path, &nfo_content).await?;

        // Generate rich episode thumbnail if enabled
        if self.config.generate_thumbnails {
            let thumb_filename = format!(
                "{} - S{}E{:03} - {} - {}-thumb.jpg",
                recording.channel_name, season, episode, date_str, sanitized_title
            );
            let thumb_path = season_dir.join(&thumb_filename);

            let image_metadata = ImageMetadata {
                channel_name: recording.channel_name.clone(),
                platform: platform_str.clone(),
                title: title.to_string(),
                viewer_count: None, // TODO: Store viewer count in recording
                game: recording.game.clone(),
                duration_secs: recording.duration_secs,
                season,
                episode,
                date: recording.started_at,
                profile_image_url: channel_profile.profile_image_url.clone(),
                thumbnail_url: recording.thumbnail_url.clone(),
            };

            if let Err(e) = self
                .image_generator
                .generate_episode_thumb(&image_metadata, &thumb_path)
                .await
            {
                warn!("Failed to generate episode thumbnail: {}", e);
            }
        }

        // Save tracker state
        self.tracker.save()?;

        info!(
            "Exported {} S{}E{:03} to Jellyfin library",
            recording.channel_name, season, episode
        );

        Ok(ExportResult {
            video_path,
            nfo_path,
            season,
            episode,
        })
    }

    /** Ensure show-level metadata (tvshow.nfo, images) exists. */
    async fn ensure_show_metadata(
        &mut self,
        show_dir: &Path,
        recording: &RecordingEntry,
        profile: &ChannelProfile,
    ) -> anyhow::Result<()> {
        tokio::fs::create_dir_all(show_dir).await?;

        let nfo_path = show_dir.join("tvshow.nfo");

        // Only generate if not exists
        if !nfo_path.exists() {
            let platform_str = recording.platform.to_string();
            let nfo_content = nfo::generate_tvshow_nfo(
                &profile.display_name,
                &platform_str,
                profile.description.as_deref(),
                recording.started_at,
            );
            nfo::write_nfo(&nfo_path, &nfo_content).await?;
            debug!("Created tvshow.nfo for {}", recording.channel_name);
        }

        // Generate rich images if enabled and not already present
        if self.config.fetch_profile_images {
            let poster_path = show_dir.join("poster.jpg");
            if !poster_path.exists() {
                let show_metadata = ShowMetadata {
                    channel_name: profile.display_name.clone(),
                    platform: recording.platform.to_string(),
                    viewer_count: None,
                    game: recording.game.clone(),
                    date: recording.started_at,
                    profile_image_url: profile.profile_image_url.clone(),
                    banner_image_url: profile.banner_image_url.clone(),
                };

                if let Err(e) = self
                    .image_generator
                    .generate_show_images(&show_metadata, show_dir)
                    .await
                {
                    warn!(
                        "Failed to generate show images for {}: {}",
                        recording.channel_name, e
                    );
                }
            }
        }

        Ok(())
    }

    /** Ensure season-level metadata exists. */
    async fn ensure_season_metadata(
        &mut self,
        season_dir: &Path,
        season: u32,
        recording_date: DateTime<Utc>,
        profile: &ChannelProfile,
        platform: &str,
    ) -> anyhow::Result<()> {
        let nfo_path = season_dir.join("season.nfo");

        if !nfo_path.exists() {
            let nfo_content = nfo::generate_season_nfo(season);
            nfo::write_nfo(&nfo_path, &nfo_content).await?;
            debug!("Created season.nfo for Season {}", season);
        }

        // Generate season poster if not exists
        let poster_path = season_dir.join("poster.jpg");
        if !poster_path.exists() && self.config.fetch_profile_images {
            let episode_count = self.tracker.count_episodes_for_season(
                platform,
                &profile.display_name,
                recording_date,
            );

            let season_metadata = SeasonMetadata {
                channel_name: profile.display_name.clone(),
                date: recording_date,
                season_number: season,
                episode_count: episode_count.max(1), // At least 1 (the current one)
                profile_image_url: profile.profile_image_url.clone(),
            };

            if let Err(e) = self
                .image_generator
                .generate_season_images(&season_metadata, season_dir)
                .await
            {
                warn!("Failed to generate season images: {}", e);
            }
        }

        Ok(())
    }

    /** Get the path where a recording would be exported. */
    pub fn get_export_path(
        &self,
        platform: Platform,
        channel_name: &str,
        recording_date: DateTime<Utc>,
        title: Option<&str>,
        extension: &str,
    ) -> PathBuf {
        let platform_str = platform.to_string();
        let (season, episode) =
            self.tracker
                .peek_next_episode(&platform_str, channel_name, recording_date);

        let title = title.unwrap_or("Stream");
        let sanitized_title = truncate_title(&sanitize_filename(title), 80);
        let date_str = recording_date.format("%Y-%m-%d").to_string();

        let filename = format!(
            "{} - S{}E{:03} - {} - {}.{}",
            channel_name, season, episode, date_str, sanitized_title, extension
        );

        self.library_dir
            .join(&platform_str)
            .join(channel_name)
            .join(format!("Season {}", season))
            .join(filename)
    }
}

/** Sanitize a string for use in filenames. */
fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn truncate_title(s: &str, max_chars: usize) -> String {
    s.chars()
        .take(max_chars)
        .collect::<String>()
        .trim_end()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Hello World"), "Hello World");
        assert_eq!(sanitize_filename("Test: Episode"), "Test_ Episode");
        assert_eq!(sanitize_filename("A/B\\C"), "A_B_C");
        assert_eq!(sanitize_filename("What?!"), "What_!");
    }

    #[test]
    fn test_truncate_title() {
        assert_eq!(truncate_title("Short title", 80), "Short title");

        let long = "A".repeat(100);
        let truncated = truncate_title(&long, 80);
        assert_eq!(truncated.len(), 80);

        // Unicode safety
        let emoji_title = "Stream 🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮🎮";
        let truncated = truncate_title(emoji_title, 20);
        assert!(truncated.is_char_boundary(truncated.len()));
    }
}
