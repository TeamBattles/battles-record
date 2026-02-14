//! Image download and processing for Jellyfin metadata.
//!
//! Handles downloading profile images from streaming platforms and
//! resizing/formatting them for Jellyfin's expected dimensions.

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use reqwest::Client;
use std::path::Path;
use tracing::{debug, warn};

/** Standard Jellyfin image dimensions. */
pub mod dimensions {
    /** Poster image (portrait, used in library view). */
    pub const POSTER: (u32, u32) = (680, 1000);
    /** Logo image (wide, used for channel branding). */
    pub const LOGO: (u32, u32) = (800, 310);
    /** Banner image (very wide, header image). */
    pub const BANNER: (u32, u32) = (758, 140);
    /** Fanart/background image (16:9 landscape). */
    pub const FANART: (u32, u32) = (1280, 720);
    /** Episode thumbnail. */
    pub const THUMB: (u32, u32) = (320, 180);
}

/** Download an image from a URL. */
pub async fn download_image(client: &Client, url: &str) -> anyhow::Result<Vec<u8>> {
    debug!("Downloading image from: {}", url);
    let resp = client.get(url).send().await?;

    if !resp.status().is_success() {
        anyhow::bail!("Failed to download image: HTTP {}", resp.status());
    }

    let bytes = resp.bytes().await?;
    Ok(bytes.to_vec())
}

/** Load an image from bytes. */
pub fn load_image(data: &[u8]) -> anyhow::Result<DynamicImage> {
    let img = image::load_from_memory(data)?;
    Ok(img)
}

/**
 * Resize an image to fit within the target dimensions, maintaining aspect ratio
 * and centering on a background if needed.
 */
pub fn resize_and_pad(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    background: Rgba<u8>,
) -> DynamicImage {
    // Calculate scaling to fit within target while maintaining aspect ratio
    let img_width = img.width();
    let img_height = img.height();

    let scale_x = target_width as f32 / img_width as f32;
    let scale_y = target_height as f32 / img_height as f32;
    let scale = scale_x.min(scale_y);

    let new_width = (img_width as f32 * scale) as u32;
    let new_height = (img_height as f32 * scale) as u32;

    // Resize the image
    let resized = img.resize_exact(
        new_width,
        new_height,
        image::imageops::FilterType::Lanczos3,
    );

    // Create background canvas
    let mut canvas = RgbaImage::from_pixel(target_width, target_height, background);

    // Calculate position to center the resized image
    let x_offset = (target_width - new_width) / 2;
    let y_offset = (target_height - new_height) / 2;

    // Overlay the resized image onto the canvas
    image::imageops::overlay(&mut canvas, &resized.to_rgba8(), x_offset.into(), y_offset.into());

    DynamicImage::ImageRgba8(canvas)
}

/** Resize an image to exact dimensions (may distort aspect ratio). */
pub fn resize_exact(img: &DynamicImage, width: u32, height: u32) -> DynamicImage {
    img.resize_exact(width, height, image::imageops::FilterType::Lanczos3)
}

/** Crop the center of an image to target dimensions. */
pub fn crop_center(img: &DynamicImage, target_width: u32, target_height: u32) -> DynamicImage {
    let img_width = img.width();
    let img_height = img.height();

    // Scale up to cover target dimensions
    let scale_x = target_width as f32 / img_width as f32;
    let scale_y = target_height as f32 / img_height as f32;
    let scale = scale_x.max(scale_y);

    let new_width = (img_width as f32 * scale).ceil() as u32;
    let new_height = (img_height as f32 * scale).ceil() as u32;

    let resized = img.resize_exact(
        new_width,
        new_height,
        image::imageops::FilterType::Lanczos3,
    );

    // Crop to exact target dimensions from center
    let x_offset = (new_width.saturating_sub(target_width)) / 2;
    let y_offset = (new_height.saturating_sub(target_height)) / 2;

    resized.crop_imm(x_offset, y_offset, target_width, target_height)
}

/** Save an image to a file. */
pub fn save_image(img: &DynamicImage, path: &Path, format: ImageFormat) -> anyhow::Result<()> {
    img.save_with_format(path, format)?;
    Ok(())
}

/**
 * Generate Jellyfin poster from profile image (680x1000).
 * Centers the profile image on a dark background.
 */
pub fn generate_poster(profile_img: &DynamicImage) -> DynamicImage {
    let dark_bg = Rgba([24, 24, 27, 255]); // zinc-900
    resize_and_pad(profile_img, dimensions::POSTER.0, dimensions::POSTER.1, dark_bg)
}

/**
 * Generate Jellyfin logo from profile image (800x310).
 * Centers the profile image on a dark background.
 */
pub fn generate_logo(profile_img: &DynamicImage) -> DynamicImage {
    let dark_bg = Rgba([24, 24, 27, 255]); // zinc-900
    resize_and_pad(profile_img, dimensions::LOGO.0, dimensions::LOGO.1, dark_bg)
}

/** Generate Jellyfin banner from banner image or profile image (758x140). */
pub fn generate_banner(img: &DynamicImage) -> DynamicImage {
    crop_center(img, dimensions::BANNER.0, dimensions::BANNER.1)
}

/** Generate Jellyfin fanart from stream thumbnail (1280x720). */
pub fn generate_fanart(img: &DynamicImage) -> DynamicImage {
    resize_exact(img, dimensions::FANART.0, dimensions::FANART.1)
}

/** Generate episode thumbnail (320x180). */
pub fn generate_thumb(img: &DynamicImage) -> DynamicImage {
    resize_exact(img, dimensions::THUMB.0, dimensions::THUMB.1)
}

/** Download and process all show-level images for a channel. */
pub async fn download_show_images(
    client: &Client,
    profile_url: Option<&str>,
    banner_url: Option<&str>,
    output_dir: &Path,
) -> anyhow::Result<()> {
    // Download profile image if available
    let profile_img = if let Some(url) = profile_url {
        match download_image(client, url).await {
            Ok(data) => match load_image(&data) {
                Ok(img) => Some(img),
                Err(e) => {
                    warn!("Failed to load profile image: {}", e);
                    None
                }
            },
            Err(e) => {
                warn!("Failed to download profile image: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Download banner image if available
    let banner_img = if let Some(url) = banner_url {
        match download_image(client, url).await {
            Ok(data) => match load_image(&data) {
                Ok(img) => Some(img),
                Err(e) => {
                    warn!("Failed to load banner image: {}", e);
                    None
                }
            },
            Err(e) => {
                warn!("Failed to download banner image: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Generate and save images
    if let Some(ref img) = profile_img {
        // Poster
        let poster = generate_poster(img);
        save_image(&poster, &output_dir.join("poster.jpg"), ImageFormat::Jpeg)?;
        debug!("Saved poster.jpg");

        // Logo
        let logo = generate_logo(img);
        save_image(&logo, &output_dir.join("logo.png"), ImageFormat::Png)?;
        debug!("Saved logo.png");
    }

    // Banner (prefer banner image, fall back to profile)
    if let Some(ref img) = banner_img.as_ref().or(profile_img.as_ref()) {
        let banner = generate_banner(img);
        save_image(&banner, &output_dir.join("banner.jpg"), ImageFormat::Jpeg)?;
        debug!("Saved banner.jpg");
    }

    // Fanart (prefer banner for better aspect ratio)
    if let Some(ref img) = banner_img.as_ref().or(profile_img.as_ref()) {
        let fanart = generate_fanart(img);
        save_image(&fanart, &output_dir.join("fanart.jpg"), ImageFormat::Jpeg)?;
        debug!("Saved fanart.jpg");
    }

    Ok(())
}

/** Download and save episode thumbnail. */
pub async fn download_episode_thumb(
    client: &Client,
    thumb_url: &str,
    output_path: &Path,
) -> anyhow::Result<()> {
    let data = download_image(client, thumb_url).await?;
    let img = load_image(&data)?;
    let thumb = generate_thumb(&img);
    save_image(&thumb, output_path, ImageFormat::Jpeg)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resize_and_pad() {
        // Create a small test image
        let img = DynamicImage::new_rgba8(100, 100);
        let result = resize_and_pad(&img, 200, 300, Rgba([0, 0, 0, 255]));
        assert_eq!(result.width(), 200);
        assert_eq!(result.height(), 300);
    }

    #[test]
    fn test_crop_center() {
        let img = DynamicImage::new_rgba8(400, 300);
        let result = crop_center(&img, 200, 150);
        assert_eq!(result.width(), 200);
        assert_eq!(result.height(), 150);
    }
}
