//! Main image generator for Jellyfin metadata.
//!
//! Coordinates color extraction, gradient generation, and template rendering
//! to produce rich images for show, season, and episode levels.

use super::colors::{generate_palette, ColorPalette};
use super::templates::{self, EpisodeImageMetadata, SeasonImageMetadata, ShowImageMetadata};
use super::text::TextRenderer;
use chrono::{DateTime, Utc};
use image::{DynamicImage, ImageFormat, RgbaImage};
use reqwest::Client;
use std::path::Path;
use tracing::{debug, warn};

/** Metadata for generating show-level images. */
#[derive(Debug, Clone)]
pub struct ShowMetadata {
    pub channel_name: String,
    pub platform: String,
    pub viewer_count: Option<u64>,
    pub game: Option<String>,
    pub date: DateTime<Utc>,
    pub profile_image_url: Option<String>,
    pub banner_image_url: Option<String>,
}

/** Metadata for generating season-level images. */
#[derive(Debug, Clone)]
pub struct SeasonMetadata {
    pub channel_name: String,
    pub date: DateTime<Utc>,
    pub season_number: u32,
    pub episode_count: u32,
    pub profile_image_url: Option<String>,
}

/** Metadata for generating episode-level images. */
#[derive(Debug, Clone)]
pub struct ImageMetadata {
    pub channel_name: String,
    pub platform: String,
    pub title: String,
    pub viewer_count: Option<u64>,
    pub game: Option<String>,
    pub duration_secs: Option<u64>,
    pub season: u32,
    pub episode: u32,
    pub date: DateTime<Utc>,
    pub profile_image_url: Option<String>,
    pub thumbnail_url: Option<String>,
}

/** Main image generator. */
pub struct ImageGenerator {
    text_renderer: TextRenderer,
    client: Client,
    /** Cached profile images by URL. */
    profile_cache: std::collections::HashMap<String, DynamicImage>,
    /** Cached color palettes by URL. */
    palette_cache: std::collections::HashMap<String, ColorPalette>,
}

impl ImageGenerator {
    /** Create a new image generator. */
    pub fn new() -> anyhow::Result<Self> {
        let text_renderer = TextRenderer::new()?;

        Ok(Self {
            text_renderer,
            client: Client::new(),
            profile_cache: std::collections::HashMap::new(),
            palette_cache: std::collections::HashMap::new(),
        })
    }

    /**
     * Load an image from URL or local file path.
     *
     * If the path starts with "http://" or "https://", it downloads from URL.
     * Otherwise, it loads from a local file path.
     */
    async fn load_image(&self, path_or_url: &str) -> anyhow::Result<DynamicImage> {
        if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
            // Download from HTTP URL
            debug!("Downloading image from URL: {}", path_or_url);
            let resp = self.client.get(path_or_url).send().await?;

            if !resp.status().is_success() {
                anyhow::bail!("Failed to download image: HTTP {}", resp.status());
            }

            let bytes = resp.bytes().await?;
            let img = image::load_from_memory(&bytes)?;
            Ok(img)
        } else {
            // Load from local file path
            debug!("Loading image from file: {}", path_or_url);
            let path = std::path::Path::new(path_or_url);
            if !path.exists() {
                anyhow::bail!("Image file not found: {}", path_or_url);
            }
            let img = image::open(path)?;
            Ok(img)
        }
    }

    /** Get or load profile image (from URL or local file). */
    async fn get_profile_image(&mut self, path_or_url: Option<&str>) -> Option<DynamicImage> {
        let path_or_url = path_or_url?;

        if let Some(cached) = self.profile_cache.get(path_or_url) {
            return Some(cached.clone());
        }

        match self.load_image(path_or_url).await {
            Ok(img) => {
                self.profile_cache.insert(path_or_url.to_string(), img.clone());
                Some(img)
            }
            Err(e) => {
                warn!("Failed to load profile image {}: {}", path_or_url, e);
                None
            }
        }
    }

    /** Get color palette for a profile image. */
    fn get_palette(&mut self, url: Option<&str>, profile_img: Option<&DynamicImage>) -> ColorPalette {
        if let Some(url) = url {
            if let Some(cached) = self.palette_cache.get(url) {
                return cached.clone();
            }
        }

        let palette = match profile_img {
            Some(img) => generate_palette(img),
            None => ColorPalette::default(),
        };

        if let Some(url) = url {
            self.palette_cache.insert(url.to_string(), palette.clone());
        }

        palette
    }

    /**
     * Generate all show-level images.
     *
     * Returns paths to: (poster, banner, logo).
     */
    pub async fn generate_show_images(
        &mut self,
        metadata: &ShowMetadata,
        output_dir: &Path,
    ) -> anyhow::Result<()> {
        tokio::fs::create_dir_all(output_dir).await?;

        // Load profile and banner images (from URL or local file)
        let profile_img = self.get_profile_image(metadata.profile_image_url.as_deref()).await;
        let banner_img = if let Some(path_or_url) = &metadata.banner_image_url {
            match self.load_image(path_or_url).await {
                Ok(img) => Some(img),
                Err(e) => {
                    warn!("Failed to load banner: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let palette = self.get_palette(
            metadata.profile_image_url.as_deref(),
            profile_img.as_ref(),
        );

        let show_meta = ShowImageMetadata {
            channel_name: metadata.channel_name.clone(),
            platform: metadata.platform.clone(),
            viewer_count: metadata.viewer_count,
            game: metadata.game.clone(),
            date: metadata.date,
        };

        // Generate poster (Primary)
        let poster = templates::generate_show_primary(
            &self.text_renderer,
            profile_img.as_ref(),
            &palette,
            &show_meta,
        );
        save_image(&poster, &output_dir.join("poster.jpg"), ImageFormat::Jpeg)?;
        debug!("Generated poster.jpg");

        // Generate banner
        let banner = templates::generate_banner(
            &self.text_renderer,
            profile_img.as_ref(),
            banner_img.as_ref(),
            &palette,
            &metadata.channel_name,
            &metadata.platform,
        );
        save_image(&banner, &output_dir.join("banner.jpg"), ImageFormat::Jpeg)?;
        debug!("Generated banner.jpg");

        // Generate logo
        let logo = templates::generate_logo(
            &self.text_renderer,
            &palette,
            &metadata.channel_name,
            &metadata.platform,
        );
        save_image(&logo, &output_dir.join("logo.png"), ImageFormat::Png)?;
        debug!("Generated logo.png");

        // Generate fanart
        let fanart = templates::generate_fanart(
            &self.text_renderer,
            profile_img.as_ref(),
            banner_img.as_ref(),
            &palette,
            &show_meta,
        );
        save_image(&fanart, &output_dir.join("fanart.jpg"), ImageFormat::Jpeg)?;
        debug!("Generated fanart.jpg");

        // Generate landscape
        let landscape = templates::generate_landscape(
            &self.text_renderer,
            profile_img.as_ref(),
            &palette,
            &show_meta,
        );
        save_image(&landscape, &output_dir.join("landscape.jpg"), ImageFormat::Jpeg)?;
        debug!("Generated landscape.jpg");

        Ok(())
    }

    /** Generate season-level images. */
    pub async fn generate_season_images(
        &mut self,
        metadata: &SeasonMetadata,
        output_dir: &Path,
    ) -> anyhow::Result<()> {
        tokio::fs::create_dir_all(output_dir).await?;

        let profile_img = self.get_profile_image(metadata.profile_image_url.as_deref()).await;
        let palette = self.get_palette(
            metadata.profile_image_url.as_deref(),
            profile_img.as_ref(),
        );

        let season_meta = SeasonImageMetadata {
            channel_name: metadata.channel_name.clone(),
            date: metadata.date,
            season_number: metadata.season_number,
            episode_count: metadata.episode_count,
        };

        // Generate season poster
        let poster = templates::generate_season_primary(
            &self.text_renderer,
            profile_img.as_ref(),
            &palette,
            &season_meta,
        );
        save_image(&poster, &output_dir.join("poster.jpg"), ImageFormat::Jpeg)?;
        debug!("Generated season poster.jpg");

        Ok(())
    }

    /** Generate episode-level thumbnail. */
    pub async fn generate_episode_thumb(
        &mut self,
        metadata: &ImageMetadata,
        output_path: &Path,
    ) -> anyhow::Result<()> {
        let profile_img = self.get_profile_image(metadata.profile_image_url.as_deref()).await;

        let stream_thumb = if let Some(path_or_url) = &metadata.thumbnail_url {
            match self.load_image(path_or_url).await {
                Ok(img) => Some(img),
                Err(e) => {
                    warn!("Failed to load stream thumbnail: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let palette = self.get_palette(
            metadata.profile_image_url.as_deref(),
            profile_img.as_ref(),
        );

        let episode_meta = EpisodeImageMetadata {
            channel_name: metadata.channel_name.clone(),
            platform: metadata.platform.clone(),
            title: metadata.title.clone(),
            viewer_count: metadata.viewer_count,
            game: metadata.game.clone(),
            duration_secs: metadata.duration_secs,
            season: metadata.season,
            episode: metadata.episode,
            date: metadata.date,
        };

        let thumb = templates::generate_thumb(
            &self.text_renderer,
            stream_thumb.as_ref(),
            profile_img.as_ref(),
            &palette,
            &episode_meta,
        );

        // Ensure parent directory exists
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        save_image(&thumb, output_path, ImageFormat::Jpeg)?;
        debug!("Generated episode thumb: {:?}", output_path);

        Ok(())
    }

    /** Clear cached images and palettes. */
    pub fn clear_cache(&mut self) {
        self.profile_cache.clear();
        self.palette_cache.clear();
    }
}

/** Save an RgbaImage to file. */
fn save_image(img: &RgbaImage, path: &Path, format: ImageFormat) -> anyhow::Result<()> {
    let dynamic = DynamicImage::ImageRgba8(img.clone());
    dynamic.save_with_format(path, format)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_generator_creation() {
        let generator = ImageGenerator::new();
        assert!(generator.is_ok());
    }

    #[tokio::test]
    async fn test_generate_show_images_no_network() {
        let mut generator = ImageGenerator::new().unwrap();
        let temp_dir = tempfile::TempDir::new().unwrap();

        let metadata = ShowMetadata {
            channel_name: "TestChannel".to_string(),
            platform: "twitch".to_string(),
            viewer_count: Some(12500),
            game: Some("Just Chatting".to_string()),
            date: Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap(),
            profile_image_url: None,
            banner_image_url: None,
        };

        let result = generator.generate_show_images(&metadata, temp_dir.path()).await;
        assert!(result.is_ok());

        // Check files were created
        assert!(temp_dir.path().join("poster.jpg").exists());
        assert!(temp_dir.path().join("banner.jpg").exists());
        assert!(temp_dir.path().join("logo.png").exists());
        assert!(temp_dir.path().join("fanart.jpg").exists());
        assert!(temp_dir.path().join("landscape.jpg").exists());

        // Verify fanart dimensions (1920×1080)
        let fanart = image::open(temp_dir.path().join("fanart.jpg")).unwrap();
        assert_eq!(fanart.width(), 1920);
        assert_eq!(fanart.height(), 1080);

        // Verify landscape dimensions (500×281)
        let landscape = image::open(temp_dir.path().join("landscape.jpg")).unwrap();
        assert_eq!(landscape.width(), 500);
        assert_eq!(landscape.height(), 281);
    }

    #[tokio::test]
    async fn test_generate_season_images() {
        let mut generator = ImageGenerator::new().unwrap();
        let temp_dir = tempfile::TempDir::new().unwrap();

        let metadata = SeasonMetadata {
            channel_name: "TestChannel".to_string(),
            date: Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap(),
            season_number: 15,
            episode_count: 3,
            profile_image_url: None,
        };

        let result = generator.generate_season_images(&metadata, temp_dir.path()).await;
        assert!(result.is_ok());

        assert!(temp_dir.path().join("poster.jpg").exists());
    }

    #[tokio::test]
    async fn test_generate_episode_thumb() {
        let mut generator = ImageGenerator::new().unwrap();
        let temp_dir = tempfile::TempDir::new().unwrap();

        let metadata = ImageMetadata {
            channel_name: "TestChannel".to_string(),
            platform: "twitch".to_string(),
            title: "Epic Gaming Stream - Day 5 Marathon".to_string(),
            viewer_count: Some(25000),
            game: Some("Elden Ring".to_string()),
            duration_secs: Some(9240),
            season: 15,
            episode: 2,
            date: Utc.with_ymd_and_hms(2026, 1, 15, 18, 30, 0).unwrap(),
            profile_image_url: None,
            thumbnail_url: None,
        };

        let output = temp_dir.path().join("thumb.jpg");
        let result = generator.generate_episode_thumb(&metadata, &output).await;
        assert!(result.is_ok());

        assert!(output.exists());

        // Check image dimensions
        let img = image::open(&output).unwrap();
        assert_eq!(img.width(), 3840);
        assert_eq!(img.height(), 2160);
    }
}
