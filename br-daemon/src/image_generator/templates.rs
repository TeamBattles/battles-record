//! Image template implementations for Jellyfin metadata.
//!
//! Implements the 7 template types:
//! 1. Show Primary (680×1000) - poster with profile, name, metadata
//! 2. Season Primary (680×1000) - poster with date prominently displayed
//! 3. Banner (758×140) - wide header
//! 4. Thumb (3840×2160) - episode thumbnail with full metadata
//! 5. Logo (800×310) - text logo on gradient
//! 6. Fanart (1920×1080) - cinematic background with profile and name
//! 7. Landscape (500×281) - thumb card for grid/card views

use super::colors::{darken_color, ColorPalette};
use super::gradients::{create_gradient, create_gradient_with_stops, GradientDirection};
use super::text::{FontWeight, TextAlign, TextRenderer, TextStyle};
use chrono::{DateTime, Datelike, Utc};
use image::{DynamicImage, Rgba, RgbaImage};

/** Standard Jellyfin image dimensions. */
pub mod dimensions {
    /** Primary poster (portrait, main show artwork). */
    pub const PRIMARY: (u32, u32) = (680, 1000);
    /** Logo (wide, channel branding). */
    pub const LOGO: (u32, u32) = (800, 310);
    /** Banner (very wide, header image). */
    pub const BANNER: (u32, u32) = (758, 140);
    /** Thumb (4K episode thumbnail). */
    pub const THUMB: (u32, u32) = (3840, 2160);
    /** Fanart (wide background, 1920x1080). */
    pub const FANART: (u32, u32) = (1920, 1080);
    /** Landscape (thumb card, 500x281 for 16:9). */
    pub const LANDSCAPE: (u32, u32) = (500, 281);
}

/** Metadata for generating show-level images. */
#[derive(Debug, Clone)]
pub struct ShowImageMetadata {
    pub channel_name: String,
    pub platform: String,
    pub viewer_count: Option<u64>,
    pub game: Option<String>,
    pub date: DateTime<Utc>,
}

/** Metadata for generating season-level images. */
#[derive(Debug, Clone)]
pub struct SeasonImageMetadata {
    pub channel_name: String,
    pub date: DateTime<Utc>,
    pub season_number: u32,
    pub episode_count: u32,
}

/** Metadata for generating episode-level images. */
#[derive(Debug, Clone)]
pub struct EpisodeImageMetadata {
    pub channel_name: String,
    pub platform: String,
    pub title: String,
    pub viewer_count: Option<u64>,
    pub game: Option<String>,
    pub duration_secs: Option<u64>,
    pub season: u32,
    pub episode: u32,
    pub date: DateTime<Utc>,
}

/**
 * Generate Show Primary (poster) image (680x1000).
 *
 * Layout:
 * - Gradient background from profile colors
 * - Profile image (rounded) in upper portion
 * - Channel name
 * - Platform badge
 * - Divider
 * - Metadata (viewers, game, date)
 */
pub fn generate_show_primary(
    text_renderer: &TextRenderer,
    profile_img: Option<&DynamicImage>,
    palette: &ColorPalette,
    metadata: &ShowImageMetadata,
) -> RgbaImage {
    let (width, height) = dimensions::PRIMARY;

    // Create gradient background
    let mut canvas = create_gradient(
        width,
        height,
        &[palette.primary, palette.secondary],
        GradientDirection::Vertical,
    );

    // Draw profile image (centered, ~40% down)
    if let Some(profile) = profile_img {
        let profile_size = 280u32;
        let profile_x = (width - profile_size) / 2;
        let profile_y = 120u32;

        let resized = profile.resize_exact(
            profile_size,
            profile_size,
            image::imageops::FilterType::Lanczos3,
        );
        draw_rounded_image(&mut canvas, &resized.to_rgba8(), profile_x as i32, profile_y as i32, 20);
    }

    // Channel name (large, centered)
    let name_style = TextStyle {
        size: 48.0,
        color: palette.text,
        align: TextAlign::Center,
        weight: FontWeight::Bold,
        max_width: Some(width - 60),
        line_height: 1.1,
    };
    text_renderer.draw_text_with_shadow(
        &mut canvas,
        &metadata.channel_name,
        width as i32 / 2,
        460,
        &name_style,
        2,
        darken_color(&palette.primary, 0.3),
    );

    // Platform badge
    let platform_style = TextStyle {
        size: 22.0,
        color: palette.text_muted,
        align: TextAlign::Center,
        weight: FontWeight::Regular,
        ..Default::default()
    };
    let platform_text = format!("[{}]", metadata.platform.to_uppercase());
    text_renderer.draw_text(&mut canvas, &platform_text, width as i32 / 2, 520, &platform_style);

    // Divider line
    draw_horizontal_line(&mut canvas, 80, width - 80, 580, palette.text_muted, 2);

    // Metadata section
    let meta_y_start = 620;
    let line_spacing = 50;
    let _icon_style = TextStyle {
        size: 28.0,
        color: palette.accent,
        align: TextAlign::Center,
        ..Default::default()
    };
    let value_style = TextStyle {
        size: 24.0,
        color: palette.text,
        align: TextAlign::Center,
        ..Default::default()
    };

    // Viewer count (if available)
    let mut current_y = meta_y_start;
    if let Some(viewers) = metadata.viewer_count {
        let viewer_text = format_number(viewers);
        text_renderer.draw_text(&mut canvas, &viewer_text, width as i32 / 2, current_y, &value_style);
        let label_style = TextStyle {
            size: 16.0,
            color: palette.text_muted,
            align: TextAlign::Center,
            ..Default::default()
        };
        text_renderer.draw_text(&mut canvas, "viewers", width as i32 / 2, current_y + 30, &label_style);
        current_y += line_spacing + 20;
    }

    // Game/category
    if let Some(game) = &metadata.game {
        let game_display = if game.len() > 25 {
            format!("{}...", &game[..22])
        } else {
            game.clone()
        };
        text_renderer.draw_text(&mut canvas, &game_display, width as i32 / 2, current_y, &value_style);
        current_y += line_spacing;
    }

    // Date
    let date_str = metadata.date.format("%b %d, %Y").to_string();
    text_renderer.draw_text(&mut canvas, &date_str, width as i32 / 2, current_y, &value_style);

    canvas
}

/**
 * Generate Season Primary (poster) image (680x1000).
 *
 * Layout:
 * - Gradient background
 * - Profile image (smaller)
 * - Channel name
 * - Divider
 * - DATE PROMINENTLY DISPLAYED
 * - Season number and episode count
 */
pub fn generate_season_primary(
    text_renderer: &TextRenderer,
    profile_img: Option<&DynamicImage>,
    palette: &ColorPalette,
    metadata: &SeasonImageMetadata,
) -> RgbaImage {
    let (width, height) = dimensions::PRIMARY;

    // Create gradient background
    let mut canvas = create_gradient(
        width,
        height,
        &[palette.primary, palette.secondary],
        GradientDirection::Vertical,
    );

    // Draw profile image (centered, smaller for season)
    if let Some(profile) = profile_img {
        let profile_size = 220u32;
        let profile_x = (width - profile_size) / 2;
        let profile_y = 100u32;

        let resized = profile.resize_exact(
            profile_size,
            profile_size,
            image::imageops::FilterType::Lanczos3,
        );
        draw_rounded_image(&mut canvas, &resized.to_rgba8(), profile_x as i32, profile_y as i32, 15);
    }

    // Channel name
    let name_style = TextStyle {
        size: 36.0,
        color: palette.text,
        align: TextAlign::Center,
        weight: FontWeight::Bold,
        max_width: Some(width - 60),
        line_height: 1.1,
    };
    text_renderer.draw_text_with_shadow(
        &mut canvas,
        &metadata.channel_name,
        width as i32 / 2,
        360,
        &name_style,
        2,
        darken_color(&palette.primary, 0.3),
    );

    // Divider
    draw_horizontal_line(&mut canvas, 100, width - 100, 430, palette.text_muted, 2);

    // DATE - PROMINENTLY DISPLAYED
    // Month name (large)
    let month_name = month_name(metadata.date.month());
    let month_style = TextStyle {
        size: 72.0,
        color: palette.text,
        align: TextAlign::Center,
        weight: FontWeight::Bold,
        ..Default::default()
    };
    text_renderer.draw_text_with_shadow(
        &mut canvas,
        month_name,
        width as i32 / 2,
        500,
        &month_style,
        3,
        darken_color(&palette.primary, 0.3),
    );

    // Day number (very large)
    let day_style = TextStyle {
        size: 120.0,
        color: palette.accent,
        align: TextAlign::Center,
        weight: FontWeight::Bold,
        ..Default::default()
    };
    text_renderer.draw_text_with_shadow(
        &mut canvas,
        &metadata.date.day().to_string(),
        width as i32 / 2,
        600,
        &day_style,
        4,
        darken_color(&palette.accent, 0.3),
    );

    // Year
    let year_style = TextStyle {
        size: 48.0,
        color: palette.text_muted,
        align: TextAlign::Center,
        weight: FontWeight::Regular,
        ..Default::default()
    };
    text_renderer.draw_text(
        &mut canvas,
        &metadata.date.year().to_string(),
        width as i32 / 2,
        740,
        &year_style,
    );

    // Season info
    let season_style = TextStyle {
        size: 24.0,
        color: palette.text_muted,
        align: TextAlign::Center,
        weight: FontWeight::Regular,
        ..Default::default()
    };
    let season_text = format!("Season {} · {} Episodes", metadata.season_number, metadata.episode_count);
    text_renderer.draw_text(&mut canvas, &season_text, width as i32 / 2, 840, &season_style);

    canvas
}

/**
 * Generate Banner image (758x140).
 *
 * Layout:
 * - Gradient background (horizontal) + optional banner image overlay
 * - Profile image on left
 * - Channel name centered
 * - Platform indicator on right
 */
pub fn generate_banner(
    text_renderer: &TextRenderer,
    profile_img: Option<&DynamicImage>,
    banner_img: Option<&DynamicImage>,
    palette: &ColorPalette,
    channel_name: &str,
    platform: &str,
) -> RgbaImage {
    let (width, height) = dimensions::BANNER;

    // Create base gradient or use banner image
    let mut canvas = if let Some(banner) = banner_img {
        let resized = banner.resize_to_fill(width, height, image::imageops::FilterType::Lanczos3);
        // Add dark overlay for text readability
        let mut base = resized.to_rgba8();
        apply_color_overlay(&mut base, Rgba([0, 0, 0, 128]));
        base
    } else {
        create_gradient(
            width,
            height,
            &[palette.secondary, palette.primary],
            GradientDirection::Horizontal,
        )
    };

    // Profile image on left
    if let Some(profile) = profile_img {
        let profile_size = 100u32;
        let profile_x = 20i32;
        let profile_y = ((height - profile_size) / 2) as i32;

        let resized = profile.resize_exact(
            profile_size,
            profile_size,
            image::imageops::FilterType::Lanczos3,
        );
        draw_rounded_image(&mut canvas, &resized.to_rgba8(), profile_x, profile_y, 10);
    }

    // Channel name (centered)
    let name_style = TextStyle {
        size: 42.0,
        color: palette.text,
        align: TextAlign::Center,
        weight: FontWeight::Bold,
        max_width: Some(width - 300),
        ..Default::default()
    };
    text_renderer.draw_text_with_shadow(
        &mut canvas,
        channel_name,
        width as i32 / 2,
        (height as i32 - 42) / 2,
        &name_style,
        2,
        Rgba([0, 0, 0, 180]),
    );

    // Platform on right
    let platform_style = TextStyle {
        size: 18.0,
        color: palette.text_muted,
        align: TextAlign::Right,
        weight: FontWeight::Regular,
        ..Default::default()
    };
    text_renderer.draw_text(
        &mut canvas,
        &platform.to_uppercase(),
        width as i32 - 25,
        (height as i32 - 18) / 2,
        &platform_style,
    );

    canvas
}

/**
 * Generate Episode Thumb image (3840x2160) - 4K.
 *
 * Layout:
 * - Stream thumbnail as background with gradient overlay
 * - Profile image and channel name in upper area
 * - Stream title (large, wrapping)
 * - Metadata bar at bottom (viewers, game, duration, episode number)
 * - Date in corner
 */
pub fn generate_thumb(
    text_renderer: &TextRenderer,
    stream_thumb: Option<&DynamicImage>,
    profile_img: Option<&DynamicImage>,
    palette: &ColorPalette,
    metadata: &EpisodeImageMetadata,
) -> RgbaImage {
    let (width, height) = dimensions::THUMB;

    // Use stream thumbnail or gradient as background
    let mut canvas = if let Some(thumb) = stream_thumb {
        let resized = thumb.resize_to_fill(width, height, image::imageops::FilterType::Lanczos3);
        resized.to_rgba8()
    } else {
        create_gradient(
            width,
            height,
            &[palette.primary, darken_color(&palette.secondary, 0.7)],
            GradientDirection::Diagonal,
        )
    };

    // Apply gradient overlay for text readability (dark at bottom)
    let overlay = create_gradient_with_stops(
        width,
        height,
        &[
            (0.0, Rgba([0, 0, 0, 0])),
            (0.4, Rgba([0, 0, 0, 0])),
            (0.7, Rgba([0, 0, 0, 180])),
            (1.0, Rgba([0, 0, 0, 230])),
        ],
        GradientDirection::Vertical,
    );
    blend_images(&mut canvas, &overlay);

    // Also add top overlay for profile area
    let top_overlay = create_gradient_with_stops(
        width,
        height / 3,
        &[
            (0.0, Rgba([0, 0, 0, 200])),
            (1.0, Rgba([0, 0, 0, 0])),
        ],
        GradientDirection::Vertical,
    );
    blend_images_at(&mut canvas, &top_overlay, 0, 0);

    // Profile image (top left)
    let profile_size = 200u32;
    if let Some(profile) = profile_img {
        let resized = profile.resize_exact(
            profile_size,
            profile_size,
            image::imageops::FilterType::Lanczos3,
        );
        draw_rounded_image(&mut canvas, &resized.to_rgba8(), 80, 80, 20);
    }

    // Channel name (next to profile)
    let channel_style = TextStyle {
        size: 72.0,
        color: Rgba([255, 255, 255, 255]),
        align: TextAlign::Left,
        weight: FontWeight::Bold,
        ..Default::default()
    };
    text_renderer.draw_text_with_shadow(
        &mut canvas,
        &metadata.channel_name,
        80 + profile_size as i32 + 40,
        130,
        &channel_style,
        3,
        Rgba([0, 0, 0, 200]),
    );

    // Platform badge
    let platform_style = TextStyle {
        size: 36.0,
        color: Rgba([180, 180, 180, 255]),
        align: TextAlign::Left,
        weight: FontWeight::Regular,
        ..Default::default()
    };
    text_renderer.draw_text(
        &mut canvas,
        &format!("[{}]", metadata.platform.to_uppercase()),
        80 + profile_size as i32 + 40,
        210,
        &platform_style,
    );

    // Stream title (large, centered, with wrapping)
    let title_style = TextStyle {
        size: 96.0,
        color: Rgba([255, 255, 255, 255]),
        align: TextAlign::Center,
        weight: FontWeight::Bold,
        max_width: Some(width - 200),
        line_height: 1.15,
    };
    text_renderer.draw_text_with_shadow(
        &mut canvas,
        &metadata.title,
        width as i32 / 2,
        height as i32 / 2 - 100,
        &title_style,
        4,
        Rgba([0, 0, 0, 220]),
    );

    // Bottom metadata bar
    let bar_y = height as i32 - 200;
    let bar_height = 80;

    // Draw semi-transparent bar background
    draw_rect(&mut canvas, 0, bar_y, width, bar_height as u32, Rgba([0, 0, 0, 150]));

    // Metadata items
    let meta_style = TextStyle {
        size: 48.0,
        color: Rgba([255, 255, 255, 255]),
        align: TextAlign::Center,
        weight: FontWeight::Regular,
        ..Default::default()
    };
    let label_style = TextStyle {
        size: 28.0,
        color: Rgba([150, 150, 150, 255]),
        align: TextAlign::Center,
        weight: FontWeight::Regular,
        ..Default::default()
    };

    let mut items: Vec<(String, String)> = Vec::new();

    if let Some(viewers) = metadata.viewer_count {
        items.push((format_number(viewers), "viewers".to_string()));
    }
    if let Some(game) = &metadata.game {
        let game_short = if game.len() > 20 {
            format!("{}...", &game[..17])
        } else {
            game.clone()
        };
        items.push((game_short, "playing".to_string()));
    }
    if let Some(duration) = metadata.duration_secs {
        items.push((format_duration(duration), "duration".to_string()));
    }
    items.push((
        format!("S{:02}E{:02}", metadata.season, metadata.episode),
        "episode".to_string(),
    ));

    let item_width = width / items.len() as u32;
    for (i, (value, label)) in items.iter().enumerate() {
        let x = (item_width * i as u32 + item_width / 2) as i32;
        text_renderer.draw_text(&mut canvas, value, x, bar_y + 25, &meta_style);
        text_renderer.draw_text(&mut canvas, label, x, bar_y + 55, &label_style);
    }

    // Date in bottom right
    let date_style = TextStyle {
        size: 48.0,
        color: Rgba([200, 200, 200, 255]),
        align: TextAlign::Right,
        weight: FontWeight::Regular,
        ..Default::default()
    };
    let date_str = metadata.date.format("%B %d, %Y").to_string();
    text_renderer.draw_text(&mut canvas, &date_str, width as i32 - 60, height as i32 - 70, &date_style);

    canvas
}

/**
 * Generate Logo image (800x310).
 *
 * Layout:
 * - Gradient background
 * - Stylized channel name (large, centered)
 * - Platform indicator below
 */
pub fn generate_logo(
    text_renderer: &TextRenderer,
    palette: &ColorPalette,
    channel_name: &str,
    platform: &str,
) -> RgbaImage {
    let (width, height) = dimensions::LOGO;

    // Create gradient background
    let mut canvas = create_gradient(
        width,
        height,
        &[palette.primary, palette.secondary],
        GradientDirection::Horizontal,
    );

    // Channel name (large, centered)
    let name_style = TextStyle {
        size: 96.0,
        color: palette.text,
        align: TextAlign::Center,
        weight: FontWeight::Bold,
        max_width: Some(width - 80),
        ..Default::default()
    };
    text_renderer.draw_text_with_shadow(
        &mut canvas,
        channel_name,
        width as i32 / 2,
        (height as i32 - 96) / 2 - 20,
        &name_style,
        3,
        darken_color(&palette.primary, 0.3),
    );

    // Platform indicator
    let platform_style = TextStyle {
        size: 28.0,
        color: palette.text_muted,
        align: TextAlign::Center,
        weight: FontWeight::Regular,
        ..Default::default()
    };
    text_renderer.draw_text(
        &mut canvas,
        &platform.to_uppercase(),
        width as i32 / 2,
        height as i32 - 60,
        &platform_style,
    );

    canvas
}

/**
 * Generate Fanart image (1920x1080).
 *
 * Layout:
 * - Banner image as background (if available) or gradient
 * - Dark vignette overlay for cinematic effect
 * - Profile image (circular, ~300px) positioned bottom-left
 * - Channel name (large, bold) next to profile
 * - Platform badge below channel name
 * - Game/category in top-right corner (if available)
 */
pub fn generate_fanart(
    text_renderer: &TextRenderer,
    profile_img: Option<&DynamicImage>,
    banner_img: Option<&DynamicImage>,
    palette: &ColorPalette,
    metadata: &ShowImageMetadata,
) -> RgbaImage {
    let (width, height) = dimensions::FANART;

    // Use banner image as background or create gradient
    let mut canvas = if let Some(banner) = banner_img {
        let resized = banner.resize_to_fill(width, height, image::imageops::FilterType::Lanczos3);
        resized.to_rgba8()
    } else {
        create_gradient(
            width,
            height,
            &[palette.primary, darken_color(&palette.secondary, 0.6)],
            GradientDirection::Diagonal,
        )
    };

    // Apply vignette overlay (dark edges, lighter center) for cinematic effect
    let vignette = create_vignette(width, height);
    blend_images(&mut canvas, &vignette);

    // Apply bottom gradient for text readability
    let bottom_overlay = create_gradient_with_stops(
        width,
        height,
        &[
            (0.0, Rgba([0, 0, 0, 0])),
            (0.5, Rgba([0, 0, 0, 0])),
            (0.75, Rgba([0, 0, 0, 120])),
            (1.0, Rgba([0, 0, 0, 200])),
        ],
        GradientDirection::Vertical,
    );
    blend_images(&mut canvas, &bottom_overlay);

    // Profile image (bottom-left, circular)
    let profile_size = 280u32;
    let profile_x = 80i32;
    let profile_y = (height - profile_size - 80) as i32;

    if let Some(profile) = profile_img {
        let resized = profile.resize_exact(
            profile_size,
            profile_size,
            image::imageops::FilterType::Lanczos3,
        );
        draw_circular_image(&mut canvas, &resized.to_rgba8(), profile_x, profile_y);
    }

    // Channel name (next to profile)
    let name_x = profile_x + profile_size as i32 + 40;
    let name_y = profile_y + 80;
    let name_style = TextStyle {
        size: 72.0,
        color: Rgba([255, 255, 255, 255]),
        align: TextAlign::Left,
        weight: FontWeight::Bold,
        max_width: Some(width - name_x as u32 - 100),
        ..Default::default()
    };
    text_renderer.draw_text_with_shadow(
        &mut canvas,
        &metadata.channel_name,
        name_x,
        name_y,
        &name_style,
        3,
        Rgba([0, 0, 0, 200]),
    );

    // Platform badge
    let platform_style = TextStyle {
        size: 32.0,
        color: Rgba([180, 180, 180, 255]),
        align: TextAlign::Left,
        weight: FontWeight::Regular,
        ..Default::default()
    };
    text_renderer.draw_text(
        &mut canvas,
        &format!("[{}]", metadata.platform.to_uppercase()),
        name_x,
        name_y + 80,
        &platform_style,
    );

    // Game/category in top-right corner (if available)
    if let Some(game) = &metadata.game {
        let game_display = if game.len() > 35 {
            format!("{}...", &game[..32])
        } else {
            game.clone()
        };

        let game_style = TextStyle {
            size: 28.0,
            color: Rgba([200, 200, 200, 255]),
            align: TextAlign::Right,
            weight: FontWeight::Regular,
            ..Default::default()
        };
        text_renderer.draw_text_with_shadow(
            &mut canvas,
            &format!("Playing: {}", game_display),
            width as i32 - 60,
            60,
            &game_style,
            2,
            Rgba([0, 0, 0, 180]),
        );
    }

    canvas
}

/**
 * Generate Landscape image (500x281).
 *
 * Layout:
 * - Gradient background from profile colors
 * - Profile image (circular, ~100px) on left side
 * - Channel name (bold, sized to fit)
 * - Platform indicator (small, below name)
 */
pub fn generate_landscape(
    text_renderer: &TextRenderer,
    profile_img: Option<&DynamicImage>,
    palette: &ColorPalette,
    metadata: &ShowImageMetadata,
) -> RgbaImage {
    let (width, height) = dimensions::LANDSCAPE;

    // Create angled gradient background
    let mut canvas = create_gradient(
        width,
        height,
        &[palette.primary, palette.secondary],
        GradientDirection::Diagonal,
    );

    // Profile image (left side, circular)
    let profile_size = 120u32;
    let profile_x = 40i32;
    let profile_y = ((height - profile_size) / 2) as i32;

    if let Some(profile) = profile_img {
        let resized = profile.resize_exact(
            profile_size,
            profile_size,
            image::imageops::FilterType::Lanczos3,
        );
        draw_circular_image(&mut canvas, &resized.to_rgba8(), profile_x, profile_y);
    }

    // Channel name (right of profile)
    let text_x = profile_x + profile_size as i32 + 30;
    let name_style = TextStyle {
        size: 42.0,
        color: palette.text,
        align: TextAlign::Left,
        weight: FontWeight::Bold,
        max_width: Some(width - text_x as u32 - 30),
        ..Default::default()
    };
    text_renderer.draw_text_with_shadow(
        &mut canvas,
        &metadata.channel_name,
        text_x,
        (height as i32 / 2) - 25,
        &name_style,
        2,
        darken_color(&palette.primary, 0.3),
    );

    // Platform indicator
    let platform_style = TextStyle {
        size: 18.0,
        color: palette.text_muted,
        align: TextAlign::Left,
        weight: FontWeight::Regular,
        ..Default::default()
    };
    text_renderer.draw_text(
        &mut canvas,
        &format!("[{}]", metadata.platform.to_uppercase()),
        text_x,
        (height as i32 / 2) + 25,
        &platform_style,
    );

    canvas
}

// Helper functions

/** Create a vignette overlay (dark edges, clear center) for cinematic effect. */
fn create_vignette(width: u32, height: u32) -> RgbaImage {
    let mut vignette = RgbaImage::new(width, height);
    let center_x = width as f32 / 2.0;
    let center_y = height as f32 / 2.0;
    let max_dist = ((center_x * center_x) + (center_y * center_y)).sqrt();

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - center_x;
            let dy = y as f32 - center_y;
            let dist = (dx * dx + dy * dy).sqrt();
            let normalized = dist / max_dist;

            // Quadratic falloff: starts dark at edges, clear in center
            // Only apply vignette in outer 40% of the image
            let alpha = if normalized > 0.6 {
                let edge_dist = (normalized - 0.6) / 0.4;
                (edge_dist * edge_dist * 180.0) as u8
            } else {
                0
            };

            vignette.put_pixel(x, y, Rgba([0, 0, 0, alpha]));
        }
    }

    vignette
}

/** Draw a circular image onto canvas. */
fn draw_circular_image(canvas: &mut RgbaImage, img: &RgbaImage, x: i32, y: i32) {
    let img_w = img.width();
    let img_h = img.height();
    let radius = img_w.min(img_h) / 2;
    let center_x = img_w / 2;
    let center_y = img_h / 2;

    for iy in 0..img_h {
        for ix in 0..img_w {
            // Check if pixel is within circle
            let dx = ix as i32 - center_x as i32;
            let dy = iy as i32 - center_y as i32;
            let dist_sq = (dx * dx + dy * dy) as u32;

            if dist_sq > radius * radius {
                continue;
            }

            let canvas_x = x + ix as i32;
            let canvas_y = y + iy as i32;

            if canvas_x >= 0
                && canvas_x < canvas.width() as i32
                && canvas_y >= 0
                && canvas_y < canvas.height() as i32
            {
                let src = img.get_pixel(ix, iy);
                if src[3] > 0 {
                    let dst = canvas.get_pixel(canvas_x as u32, canvas_y as u32);
                    let blended = blend_pixel(dst, src);
                    canvas.put_pixel(canvas_x as u32, canvas_y as u32, blended);
                }
            }
        }
    }
}

/** Draw a rounded rectangle image onto canvas. */
fn draw_rounded_image(canvas: &mut RgbaImage, img: &RgbaImage, x: i32, y: i32, radius: u32) {
    let img_w = img.width();
    let img_h = img.height();

    for iy in 0..img_h {
        for ix in 0..img_w {
            // Check if pixel is within rounded corners
            let in_corner =
                (ix < radius && iy < radius && distance(ix, iy, radius, radius) > radius)
                    || (ix >= img_w - radius
                        && iy < radius
                        && distance(ix, iy, img_w - radius - 1, radius) > radius)
                    || (ix < radius
                        && iy >= img_h - radius
                        && distance(ix, iy, radius, img_h - radius - 1) > radius)
                    || (ix >= img_w - radius
                        && iy >= img_h - radius
                        && distance(ix, iy, img_w - radius - 1, img_h - radius - 1) > radius);

            if in_corner {
                continue;
            }

            let canvas_x = x + ix as i32;
            let canvas_y = y + iy as i32;

            if canvas_x >= 0 && canvas_x < canvas.width() as i32 &&
               canvas_y >= 0 && canvas_y < canvas.height() as i32 {
                let src = img.get_pixel(ix, iy);
                if src[3] > 0 {
                    let dst = canvas.get_pixel(canvas_x as u32, canvas_y as u32);
                    let blended = blend_pixel(dst, src);
                    canvas.put_pixel(canvas_x as u32, canvas_y as u32, blended);
                }
            }
        }
    }
}

fn distance(x1: u32, y1: u32, x2: u32, y2: u32) -> u32 {
    let dx = x1 as f32 - x2 as f32;
    let dy = y1 as f32 - y2 as f32;
    (dx * dx + dy * dy).sqrt() as u32
}

/** Draw a horizontal line. */
fn draw_horizontal_line(canvas: &mut RgbaImage, x1: u32, x2: u32, y: i32, color: Rgba<u8>, thickness: u32) {
    if y < 0 || y >= canvas.height() as i32 {
        return;
    }
    for t in 0..thickness {
        let py = (y + t as i32) as u32;
        if py < canvas.height() {
            for x in x1..x2 {
                if x < canvas.width() {
                    canvas.put_pixel(x, py, color);
                }
            }
        }
    }
}

/** Draw a filled rectangle. */
fn draw_rect(canvas: &mut RgbaImage, x: i32, y: i32, width: u32, height: u32, color: Rgba<u8>) {
    for dy in 0..height {
        for dx in 0..width {
            let px = x + dx as i32;
            let py = y + dy as i32;
            if px >= 0 && px < canvas.width() as i32 && py >= 0 && py < canvas.height() as i32 {
                let existing = canvas.get_pixel(px as u32, py as u32);
                let blended = blend_pixel(existing, &color);
                canvas.put_pixel(px as u32, py as u32, blended);
            }
        }
    }
}

/** Blend source pixel over destination. */
fn blend_pixel(dst: &Rgba<u8>, src: &Rgba<u8>) -> Rgba<u8> {
    let sa = src[3] as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);

    if out_a == 0.0 {
        return Rgba([0, 0, 0, 0]);
    }

    let blend = |s: u8, d: u8| -> u8 {
        ((s as f32 * sa + d as f32 * da * (1.0 - sa)) / out_a) as u8
    };

    Rgba([
        blend(src[0], dst[0]),
        blend(src[1], dst[1]),
        blend(src[2], dst[2]),
        (out_a * 255.0) as u8,
    ])
}

/** Apply a solid color overlay to entire image. */
fn apply_color_overlay(img: &mut RgbaImage, color: Rgba<u8>) {
    for pixel in img.pixels_mut() {
        *pixel = blend_pixel(pixel, &color);
    }
}

/** Blend one image over another (modifying dst in place). */
fn blend_images(dst: &mut RgbaImage, src: &RgbaImage) {
    let width = dst.width().min(src.width());
    let height = dst.height().min(src.height());

    for y in 0..height {
        for x in 0..width {
            let src_pixel = src.get_pixel(x, y);
            let dst_pixel = dst.get_pixel(x, y);
            dst.put_pixel(x, y, blend_pixel(dst_pixel, src_pixel));
        }
    }
}

/** Blend src image onto dst at specified position. */
fn blend_images_at(dst: &mut RgbaImage, src: &RgbaImage, x: i32, y: i32) {
    for sy in 0..src.height() {
        for sx in 0..src.width() {
            let dx = x + sx as i32;
            let dy = y + sy as i32;
            if dx >= 0 && dx < dst.width() as i32 && dy >= 0 && dy < dst.height() as i32 {
                let src_pixel = src.get_pixel(sx, sy);
                let dst_pixel = dst.get_pixel(dx as u32, dy as u32);
                dst.put_pixel(dx as u32, dy as u32, blend_pixel(dst_pixel, src_pixel));
            }
        }
    }
}

/** Format large numbers with K/M suffix. */
fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/** Format duration in seconds to human-readable string. */
fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;

    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

/** Get month name from number. */
fn month_name(month: u32) -> &'static str {
    match month {
        1 => "JANUARY",
        2 => "FEBRUARY",
        3 => "MARCH",
        4 => "APRIL",
        5 => "MAY",
        6 => "JUNE",
        7 => "JULY",
        8 => "AUGUST",
        9 => "SEPTEMBER",
        10 => "OCTOBER",
        11 => "NOVEMBER",
        12 => "DECEMBER",
        _ => "UNKNOWN",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(500), "500");
        assert_eq!(format_number(1500), "1.5K");
        assert_eq!(format_number(1_500_000), "1.5M");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(300), "5m");
        assert_eq!(format_duration(3700), "1h 1m");
        assert_eq!(format_duration(7200), "2h 0m");
    }

    #[test]
    fn test_month_name() {
        assert_eq!(month_name(1), "JANUARY");
        assert_eq!(month_name(12), "DECEMBER");
    }

    #[test]
    fn test_dimensions() {
        // Verify all image dimensions
        assert_eq!(dimensions::PRIMARY, (680, 1000));
        assert_eq!(dimensions::LOGO, (800, 310));
        assert_eq!(dimensions::BANNER, (758, 140));
        assert_eq!(dimensions::THUMB, (3840, 2160));
        assert_eq!(dimensions::FANART, (1920, 1080));
        assert_eq!(dimensions::LANDSCAPE, (500, 281));
    }

    #[test]
    fn test_vignette_creation() {
        let vignette = create_vignette(100, 100);
        assert_eq!(vignette.width(), 100);
        assert_eq!(vignette.height(), 100);

        // Center should be transparent
        let center = vignette.get_pixel(50, 50);
        assert_eq!(center[3], 0);

        // Corner should have some darkness
        let corner = vignette.get_pixel(0, 0);
        assert!(corner[3] > 0);
    }
}
