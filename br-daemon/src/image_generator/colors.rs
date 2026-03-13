//! Color extraction and manipulation for image generation.
//!
//! Extracts dominant colors from profile images using k-means clustering
//! and generates complementary colors for gradients and text.

use image::{DynamicImage, Rgba};
use kmeans_colors::{get_kmeans, Kmeans};
use palette::Srgb;

/** Extracted color palette from an image. */
#[derive(Debug, Clone)]
pub struct ColorPalette {
    /** Primary/dominant color. */
    pub primary: Rgba<u8>,
    /** Secondary color (for gradients). */
    pub secondary: Rgba<u8>,
    /** Accent color. */
    pub accent: Rgba<u8>,
    /** Text color (contrasts with primary). */
    pub text: Rgba<u8>,
    /** Muted text color. */
    pub text_muted: Rgba<u8>,
}

impl Default for ColorPalette {
    fn default() -> Self {
        // Default dark theme colors matching design system
        Self {
            primary: Rgba([24, 24, 27, 255]),       // zinc-900
            secondary: Rgba([39, 39, 42, 255]),     // zinc-800
            accent: Rgba([16, 185, 129, 255]),      // emerald-500
            text: Rgba([250, 250, 250, 255]),       // zinc-50
            text_muted: Rgba([113, 113, 122, 255]), // zinc-500
        }
    }
}

/** Extract dominant colors from an image using k-means clustering. */
pub fn extract_dominant_colors(img: &DynamicImage, num_colors: usize) -> Vec<Rgba<u8>> {
    let img_rgba = img.to_rgba8();
    let (width, height) = img_rgba.dimensions();

    // Sample pixels (skip if image is large)
    let step = if width * height > 10000 { 4 } else { 1 };
    let mut pixels: Vec<Srgb<f32>> = Vec::new();

    for y in (0..height).step_by(step as usize) {
        for x in (0..width).step_by(step as usize) {
            let pixel = img_rgba.get_pixel(x, y);
            // Skip transparent pixels
            if pixel[3] < 128 {
                continue;
            }
            // Convert to Srgb for clustering
            pixels.push(Srgb::new(
                pixel[0] as f32 / 255.0,
                pixel[1] as f32 / 255.0,
                pixel[2] as f32 / 255.0,
            ));
        }
    }

    if pixels.is_empty() {
        return vec![Rgba([24, 24, 27, 255])]; // Default dark color
    }

    // Check if all pixels are identical (edge case that breaks kmeans)
    let first = &pixels[0];
    if pixels
        .iter()
        .all(|p| p.red == first.red && p.green == first.green && p.blue == first.blue)
    {
        return vec![Rgba([
            (first.red * 255.0).clamp(0.0, 255.0) as u8,
            (first.green * 255.0).clamp(0.0, 255.0) as u8,
            (first.blue * 255.0).clamp(0.0, 255.0) as u8,
            255,
        ])];
    }

    // Run k-means clustering
    let max_iterations = 20;
    let converge = 1.0;
    let verbose = false;
    let seed = 42;

    let result: Kmeans<Srgb<f32>> =
        get_kmeans(num_colors, max_iterations, converge, verbose, &pixels, seed);

    // The actual number of centroids may be less than requested
    let actual_centroids = result.centroids.len();

    // Sort centroids by frequency (most common first)
    let mut centroid_counts: Vec<(usize, usize)> = result
        .indices
        .iter()
        .fold(vec![0usize; actual_centroids], |mut acc, &idx| {
            let idx = idx as usize;
            if idx < acc.len() {
                acc[idx] += 1;
            }
            acc
        })
        .into_iter()
        .enumerate()
        .collect();
    centroid_counts.sort_by(|a, b| b.1.cmp(&a.1));

    // Convert centroids back to Rgba
    centroid_counts
        .iter()
        .filter(|(idx, _)| *idx < actual_centroids)
        .map(|(idx, _)| {
            let c = &result.centroids[*idx];
            Rgba([
                (c.red * 255.0).clamp(0.0, 255.0) as u8,
                (c.green * 255.0).clamp(0.0, 255.0) as u8,
                (c.blue * 255.0).clamp(0.0, 255.0) as u8,
                255,
            ])
        })
        .collect()
}

/** Generate a color palette from an image. */
pub fn generate_palette(img: &DynamicImage) -> ColorPalette {
    let colors = extract_dominant_colors(img, 5);

    if colors.is_empty() {
        return ColorPalette::default();
    }

    let primary = colors[0];

    // Create a darker version for secondary
    let secondary = darken_color(&primary, 0.7);

    // Find the most vibrant color for accent
    let accent = colors
        .iter()
        .max_by(|a, b| {
            color_saturation(a)
                .partial_cmp(&color_saturation(b))
                .unwrap()
        })
        .cloned()
        .unwrap_or(Rgba([16, 185, 129, 255])); // fallback to emerald

    // Determine text color based on primary luminance
    let lum = luminance(&primary);
    let text = if lum > 0.5 {
        Rgba([24, 24, 27, 255]) // Dark text on light bg
    } else {
        Rgba([250, 250, 250, 255]) // Light text on dark bg
    };

    let text_muted = if lum > 0.5 {
        Rgba([82, 82, 91, 255]) // zinc-600
    } else {
        Rgba([161, 161, 170, 255]) // zinc-400
    };

    ColorPalette {
        primary,
        secondary,
        accent,
        text,
        text_muted,
    }
}

/** Calculate relative luminance of a color (0.0 - 1.0). */
pub fn luminance(color: &Rgba<u8>) -> f32 {
    let r = color[0] as f32 / 255.0;
    let g = color[1] as f32 / 255.0;
    let b = color[2] as f32 / 255.0;

    // sRGB to linear conversion
    let r = if r <= 0.03928 {
        r / 12.92
    } else {
        ((r + 0.055) / 1.055).powf(2.4)
    };
    let g = if g <= 0.03928 {
        g / 12.92
    } else {
        ((g + 0.055) / 1.055).powf(2.4)
    };
    let b = if b <= 0.03928 {
        b / 12.92
    } else {
        ((b + 0.055) / 1.055).powf(2.4)
    };

    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/** Calculate saturation of a color (0.0 - 1.0). */
fn color_saturation(color: &Rgba<u8>) -> f32 {
    let r = color[0] as f32 / 255.0;
    let g = color[1] as f32 / 255.0;
    let b = color[2] as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);

    if max == 0.0 {
        0.0
    } else {
        (max - min) / max
    }
}

/** Darken a color by a factor (0.0 = black, 1.0 = original). */
pub fn darken_color(color: &Rgba<u8>, factor: f32) -> Rgba<u8> {
    Rgba([
        (color[0] as f32 * factor).clamp(0.0, 255.0) as u8,
        (color[1] as f32 * factor).clamp(0.0, 255.0) as u8,
        (color[2] as f32 * factor).clamp(0.0, 255.0) as u8,
        color[3],
    ])
}

/** Lighten a color by a factor (1.0 = original, 2.0 = max bright). */
#[allow(dead_code)]
pub fn lighten_color(color: &Rgba<u8>, factor: f32) -> Rgba<u8> {
    let lighten = |c: u8| -> u8 {
        let f = c as f32 / 255.0;
        let adjusted = 1.0 - (1.0 - f) / factor;
        (adjusted * 255.0).clamp(0.0, 255.0) as u8
    };

    Rgba([
        lighten(color[0]),
        lighten(color[1]),
        lighten(color[2]),
        color[3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_palette() {
        let palette = ColorPalette::default();
        assert_eq!(palette.primary, Rgba([24, 24, 27, 255]));
    }

    #[test]
    fn test_luminance() {
        // Black should have low luminance
        assert!(luminance(&Rgba([0, 0, 0, 255])) < 0.1);
        // White should have high luminance
        assert!(luminance(&Rgba([255, 255, 255, 255])) > 0.9);
    }

    #[test]
    fn test_darken_color() {
        let white = Rgba([255, 255, 255, 255]);
        let darkened = darken_color(&white, 0.5);
        assert_eq!(darkened[0], 127);
        assert_eq!(darkened[1], 127);
        assert_eq!(darkened[2], 127);
    }

    #[test]
    fn test_extract_colors_from_solid_image() {
        // Create a simple red image
        let img =
            DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(10, 10, Rgba([255, 0, 0, 255])));
        let colors = extract_dominant_colors(&img, 3);
        assert!(!colors.is_empty());
        // Should be mostly red
        assert!(colors[0][0] > 200);
    }
}
