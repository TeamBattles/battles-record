//! Gradient generation utilities for image backgrounds.
//!
//! Creates linear gradients using tiny-skia for professional-looking
//! image backgrounds.

use image::{Rgba, RgbaImage};
use tiny_skia::{
    Color, GradientStop, LinearGradient, Paint, Pixmap, Point, Rect, SpreadMode, Transform,
};

/** Direction of a linear gradient. */
#[derive(Debug, Clone, Copy)]
pub enum GradientDirection {
    /** Top to bottom. */
    Vertical,
    /** Left to right. */
    Horizontal,
    /** Top-left to bottom-right. */
    Diagonal,
    /** Custom angle in degrees (0 = right, 90 = down). */
    Angle(f32),
}

/** Create a linear gradient background. */
pub fn create_gradient(
    width: u32,
    height: u32,
    colors: &[Rgba<u8>],
    direction: GradientDirection,
) -> RgbaImage {
    if colors.is_empty() {
        return RgbaImage::from_pixel(width, height, Rgba([24, 24, 27, 255]));
    }

    if colors.len() == 1 {
        return RgbaImage::from_pixel(width, height, colors[0]);
    }

    let mut pixmap = Pixmap::new(width, height).unwrap_or_else(|| {
        // Fallback for very large images
        Pixmap::new(1, 1).unwrap()
    });

    // Calculate start and end points based on direction
    let (start, end) = match direction {
        GradientDirection::Vertical => (
            Point::from_xy(width as f32 / 2.0, 0.0),
            Point::from_xy(width as f32 / 2.0, height as f32),
        ),
        GradientDirection::Horizontal => (
            Point::from_xy(0.0, height as f32 / 2.0),
            Point::from_xy(width as f32, height as f32 / 2.0),
        ),
        GradientDirection::Diagonal => (
            Point::from_xy(0.0, 0.0),
            Point::from_xy(width as f32, height as f32),
        ),
        GradientDirection::Angle(degrees) => {
            let radians = degrees.to_radians();
            let cx = width as f32 / 2.0;
            let cy = height as f32 / 2.0;
            let diag = (width as f32).hypot(height as f32) / 2.0;
            (
                Point::from_xy(
                    cx - diag * radians.cos(),
                    cy - diag * radians.sin(),
                ),
                Point::from_xy(
                    cx + diag * radians.cos(),
                    cy + diag * radians.sin(),
                ),
            )
        }
    };

    // Create gradient stops
    let stops: Vec<GradientStop> = colors
        .iter()
        .enumerate()
        .map(|(i, color)| {
            let position = if colors.len() == 1 {
                0.0
            } else {
                i as f32 / (colors.len() - 1) as f32
            };
            GradientStop::new(
                position,
                Color::from_rgba8(color[0], color[1], color[2], color[3]),
            )
        })
        .collect();

    let gradient = LinearGradient::new(start, end, stops, SpreadMode::Pad, Transform::identity());

    if let Some(gradient) = gradient {
        let mut paint = Paint::default();
        paint.shader = gradient;

        let rect = Rect::from_xywh(0.0, 0.0, width as f32, height as f32).unwrap();
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    }

    // Convert pixmap to RgbaImage
    let data = pixmap.data();
    let mut img = RgbaImage::new(width, height);

    for (i, pixel) in img.pixels_mut().enumerate() {
        let offset = i * 4;
        if offset + 3 < data.len() {
            // tiny-skia uses premultiplied alpha, convert back
            let a = data[offset + 3];
            if a > 0 {
                let factor = 255.0 / a as f32;
                *pixel = Rgba([
                    (data[offset] as f32 * factor).min(255.0) as u8,
                    (data[offset + 1] as f32 * factor).min(255.0) as u8,
                    (data[offset + 2] as f32 * factor).min(255.0) as u8,
                    a,
                ]);
            } else {
                *pixel = Rgba([0, 0, 0, 0]);
            }
        }
    }

    img
}

/** Create a gradient with a custom position mapping. */
pub fn create_gradient_with_stops(
    width: u32,
    height: u32,
    stops: &[(f32, Rgba<u8>)], // (position 0.0-1.0, color)
    direction: GradientDirection,
) -> RgbaImage {
    if stops.is_empty() {
        return RgbaImage::from_pixel(width, height, Rgba([24, 24, 27, 255]));
    }

    let mut pixmap = Pixmap::new(width, height).unwrap_or_else(|| Pixmap::new(1, 1).unwrap());

    let (start, end) = match direction {
        GradientDirection::Vertical => (
            Point::from_xy(width as f32 / 2.0, 0.0),
            Point::from_xy(width as f32 / 2.0, height as f32),
        ),
        GradientDirection::Horizontal => (
            Point::from_xy(0.0, height as f32 / 2.0),
            Point::from_xy(width as f32, height as f32 / 2.0),
        ),
        GradientDirection::Diagonal => (
            Point::from_xy(0.0, 0.0),
            Point::from_xy(width as f32, height as f32),
        ),
        GradientDirection::Angle(degrees) => {
            let radians = degrees.to_radians();
            let cx = width as f32 / 2.0;
            let cy = height as f32 / 2.0;
            let diag = (width as f32).hypot(height as f32) / 2.0;
            (
                Point::from_xy(cx - diag * radians.cos(), cy - diag * radians.sin()),
                Point::from_xy(cx + diag * radians.cos(), cy + diag * radians.sin()),
            )
        }
    };

    let gradient_stops: Vec<GradientStop> = stops
        .iter()
        .map(|(pos, color)| {
            GradientStop::new(
                *pos,
                Color::from_rgba8(color[0], color[1], color[2], color[3]),
            )
        })
        .collect();

    let gradient = LinearGradient::new(start, end, gradient_stops, SpreadMode::Pad, Transform::identity());

    if let Some(gradient) = gradient {
        let mut paint = Paint::default();
        paint.shader = gradient;

        let rect = Rect::from_xywh(0.0, 0.0, width as f32, height as f32).unwrap();
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    }

    // Convert to RgbaImage
    let data = pixmap.data();
    let mut img = RgbaImage::new(width, height);

    for (i, pixel) in img.pixels_mut().enumerate() {
        let offset = i * 4;
        if offset + 3 < data.len() {
            let a = data[offset + 3];
            if a > 0 {
                let factor = 255.0 / a as f32;
                *pixel = Rgba([
                    (data[offset] as f32 * factor).min(255.0) as u8,
                    (data[offset + 1] as f32 * factor).min(255.0) as u8,
                    (data[offset + 2] as f32 * factor).min(255.0) as u8,
                    a,
                ]);
            } else {
                *pixel = Rgba([0, 0, 0, 0]);
            }
        }
    }

    img
}

/**
 * Create a gradient overlay for compositing over images
 * (e.g., fading to dark at bottom for text readability).
 */
pub fn create_fade_overlay(
    width: u32,
    height: u32,
    direction: GradientDirection,
    fade_color: Rgba<u8>,
    start_opacity: u8,
    end_opacity: u8,
) -> RgbaImage {
    let start_color = Rgba([fade_color[0], fade_color[1], fade_color[2], start_opacity]);
    let end_color = Rgba([fade_color[0], fade_color[1], fade_color[2], end_opacity]);

    create_gradient(width, height, &[start_color, end_color], direction)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertical_gradient() {
        let gradient = create_gradient(
            100,
            100,
            &[Rgba([255, 0, 0, 255]), Rgba([0, 0, 255, 255])],
            GradientDirection::Vertical,
        );
        assert_eq!(gradient.width(), 100);
        assert_eq!(gradient.height(), 100);

        // Top should be redder
        let top_pixel = gradient.get_pixel(50, 0);
        let bottom_pixel = gradient.get_pixel(50, 99);
        assert!(top_pixel[0] > bottom_pixel[0]); // More red at top
        assert!(top_pixel[2] < bottom_pixel[2]); // Less blue at top
    }

    #[test]
    fn test_horizontal_gradient() {
        let gradient = create_gradient(
            100,
            100,
            &[Rgba([255, 0, 0, 255]), Rgba([0, 0, 255, 255])],
            GradientDirection::Horizontal,
        );

        // Left should be redder
        let left_pixel = gradient.get_pixel(0, 50);
        let right_pixel = gradient.get_pixel(99, 50);
        assert!(left_pixel[0] > right_pixel[0]);
        assert!(left_pixel[2] < right_pixel[2]);
    }

    #[test]
    fn test_single_color() {
        let gradient = create_gradient(50, 50, &[Rgba([128, 128, 128, 255])], GradientDirection::Vertical);
        let pixel = gradient.get_pixel(25, 25);
        assert_eq!(pixel[0], 128);
    }

    #[test]
    fn test_fade_overlay() {
        let overlay = create_fade_overlay(
            100,
            100,
            GradientDirection::Vertical,
            Rgba([0, 0, 0, 255]),
            0,
            255,
        );

        // Top should be transparent
        let top_pixel = overlay.get_pixel(50, 0);
        // Bottom should be opaque
        let bottom_pixel = overlay.get_pixel(50, 99);

        assert!(top_pixel[3] < bottom_pixel[3]);
    }
}
