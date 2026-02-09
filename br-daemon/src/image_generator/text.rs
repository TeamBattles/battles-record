//! Text rendering utilities for image generation.
//!
//! Renders text using ab_glyph with embedded JetBrains Mono font,
//! matching the project's monospace design system.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};

/**
 * Embedded JetBrains Mono Regular font.
 * Font licensed under OFL 1.1: https://www.jetbrains.com/lp/mono/
 */
const JETBRAINS_MONO_REGULAR: &[u8] = include_bytes!("fonts/JetBrainsMono-Regular.ttf");
const JETBRAINS_MONO_BOLD: &[u8] = include_bytes!("fonts/JetBrainsMono-Bold.ttf");

/** Text alignment options. */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/** Text weight/style. */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Regular,
    Bold,
}

/** Configuration for drawing text. */
#[derive(Debug, Clone)]
pub struct TextStyle {
    pub size: f32,
    pub color: Rgba<u8>,
    pub align: TextAlign,
    pub weight: FontWeight,
    /** Maximum width before wrapping (None = no wrap). */
    pub max_width: Option<u32>,
    /** Line height multiplier (1.0 = no extra spacing). */
    pub line_height: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            size: 24.0,
            color: Rgba([255, 255, 255, 255]),
            align: TextAlign::Left,
            weight: FontWeight::Regular,
            max_width: None,
            line_height: 1.2,
        }
    }
}

impl TextStyle {
    pub fn new(size: f32, color: Rgba<u8>) -> Self {
        Self {
            size,
            color,
            ..Default::default()
        }
    }

    pub fn centered(mut self) -> Self {
        self.align = TextAlign::Center;
        self
    }

    pub fn right_aligned(mut self) -> Self {
        self.align = TextAlign::Right;
        self
    }

    pub fn bold(mut self) -> Self {
        self.weight = FontWeight::Bold;
        self
    }

    pub fn with_max_width(mut self, width: u32) -> Self {
        self.max_width = Some(width);
        self
    }

    pub fn with_line_height(mut self, height: f32) -> Self {
        self.line_height = height;
        self
    }
}

/** Text renderer using embedded JetBrains Mono font. */
pub struct TextRenderer {
    font_regular: FontRef<'static>,
    font_bold: FontRef<'static>,
}

impl TextRenderer {
    /** Create a new text renderer with embedded fonts. */
    pub fn new() -> anyhow::Result<Self> {
        let font_regular = FontRef::try_from_slice(JETBRAINS_MONO_REGULAR)
            .map_err(|e| anyhow::anyhow!("Failed to load regular font: {}", e))?;
        let font_bold = FontRef::try_from_slice(JETBRAINS_MONO_BOLD)
            .map_err(|e| anyhow::anyhow!("Failed to load bold font: {}", e))?;

        Ok(Self {
            font_regular,
            font_bold,
        })
    }

    fn get_font(&self, weight: FontWeight) -> &FontRef<'static> {
        match weight {
            FontWeight::Regular => &self.font_regular,
            FontWeight::Bold => &self.font_bold,
        }
    }

    /** Measure the width of text. */
    pub fn measure_text(&self, text: &str, style: &TextStyle) -> (f32, f32) {
        let font = self.get_font(style.weight);
        let scale = PxScale::from(style.size);
        let scaled_font = font.as_scaled(scale);

        let mut width = 0.0f32;
        let mut prev_glyph_id = None;

        for c in text.chars() {
            let glyph_id = font.glyph_id(c);

            if let Some(prev) = prev_glyph_id {
                width += scaled_font.kern(prev, glyph_id);
            }

            width += scaled_font.h_advance(glyph_id);
            prev_glyph_id = Some(glyph_id);
        }

        let height = scaled_font.height();
        (width, height)
    }

    /** Wrap text to fit within max_width. */
    pub fn wrap_text(&self, text: &str, style: &TextStyle) -> Vec<String> {
        let max_width = match style.max_width {
            Some(w) => w as f32,
            None => return vec![text.to_string()],
        };

        let mut lines = Vec::new();
        let mut current_line = String::new();
        let mut current_width = 0.0f32;

        let font = self.get_font(style.weight);
        let scale = PxScale::from(style.size);
        let scaled_font = font.as_scaled(scale);
        let space_width = scaled_font.h_advance(font.glyph_id(' '));

        for word in text.split_whitespace() {
            let (word_width, _) = self.measure_text(word, style);

            if current_width + word_width > max_width && !current_line.is_empty() {
                lines.push(current_line.trim_end().to_string());
                current_line = String::new();
                current_width = 0.0;
            }

            if !current_line.is_empty() {
                current_line.push(' ');
                current_width += space_width;
            }
            current_line.push_str(word);
            current_width += word_width;
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }

        if lines.is_empty() {
            lines.push(String::new());
        }

        lines
    }

    /** Draw text onto an image. */
    pub fn draw_text(&self, img: &mut RgbaImage, text: &str, x: i32, y: i32, style: &TextStyle) {
        let font = self.get_font(style.weight);
        let scale = PxScale::from(style.size);
        let scaled_font = font.as_scaled(scale);

        // Handle text wrapping
        let lines = self.wrap_text(text, style);
        let line_spacing = (style.size * style.line_height) as i32;

        for (line_idx, line) in lines.iter().enumerate() {
            let line_y = y + (line_idx as i32 * line_spacing);

            // Calculate x offset for alignment
            let (line_width, _) = self.measure_text(line, style);
            let x_offset = match style.align {
                TextAlign::Left => 0.0,
                TextAlign::Center => -line_width / 2.0,
                TextAlign::Right => -line_width,
            };

            let mut cursor_x = x as f32 + x_offset;
            let mut prev_glyph_id = None;

            for c in line.chars() {
                let glyph_id = font.glyph_id(c);

                if let Some(prev) = prev_glyph_id {
                    cursor_x += scaled_font.kern(prev, glyph_id);
                }

                if let Some(outlined) = scaled_font.outline_glyph(
                    ab_glyph::Glyph {
                        id: glyph_id,
                        scale,
                        position: ab_glyph::point(cursor_x, line_y as f32 + scaled_font.ascent()),
                    }
                ) {
                    let bounds = outlined.px_bounds();
                    outlined.draw(|px_x, px_y, coverage| {
                        let img_x = (bounds.min.x as i32 + px_x as i32) as u32;
                        let img_y = (bounds.min.y as i32 + px_y as i32) as u32;

                        if img_x < img.width() && img_y < img.height() {
                            let alpha = (coverage * style.color[3] as f32) as u8;
                            if alpha > 0 {
                                let bg = img.get_pixel(img_x, img_y);
                                let blended = blend_pixels(bg, &style.color, alpha);
                                img.put_pixel(img_x, img_y, blended);
                            }
                        }
                    });
                }

                cursor_x += scaled_font.h_advance(glyph_id);
                prev_glyph_id = Some(glyph_id);
            }
        }
    }

    /** Draw text with a shadow for better readability. */
    pub fn draw_text_with_shadow(
        &self,
        img: &mut RgbaImage,
        text: &str,
        x: i32,
        y: i32,
        style: &TextStyle,
        shadow_offset: i32,
        shadow_color: Rgba<u8>,
    ) {
        // Draw shadow first
        let shadow_style = TextStyle {
            color: shadow_color,
            ..style.clone()
        };
        self.draw_text(img, text, x + shadow_offset, y + shadow_offset, &shadow_style);

        // Draw main text
        self.draw_text(img, text, x, y, style);
    }

    /** Draw text with an outline for maximum readability on varied backgrounds. */
    pub fn draw_text_with_outline(
        &self,
        img: &mut RgbaImage,
        text: &str,
        x: i32,
        y: i32,
        style: &TextStyle,
        outline_width: i32,
        outline_color: Rgba<u8>,
    ) {
        let outline_style = TextStyle {
            color: outline_color,
            ..style.clone()
        };

        // Draw outline in all directions
        for dx in -outline_width..=outline_width {
            for dy in -outline_width..=outline_width {
                if dx != 0 || dy != 0 {
                    self.draw_text(img, text, x + dx, y + dy, &outline_style);
                }
            }
        }

        // Draw main text
        self.draw_text(img, text, x, y, style);
    }

    /** Get the height of a line of text. */
    pub fn line_height(&self, style: &TextStyle) -> f32 {
        let font = self.get_font(style.weight);
        let scale = PxScale::from(style.size);
        font.as_scaled(scale).height()
    }
}

/** Blend two pixels with alpha. */
fn blend_pixels(bg: &Rgba<u8>, fg: &Rgba<u8>, alpha: u8) -> Rgba<u8> {
    let alpha_f = alpha as f32 / 255.0;
    let inv_alpha = 1.0 - alpha_f;

    Rgba([
        (fg[0] as f32 * alpha_f + bg[0] as f32 * inv_alpha) as u8,
        (fg[1] as f32 * alpha_f + bg[1] as f32 * inv_alpha) as u8,
        (fg[2] as f32 * alpha_f + bg[2] as f32 * inv_alpha) as u8,
        (alpha as f32 + bg[3] as f32 * inv_alpha).min(255.0) as u8,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_renderer_creation() {
        let renderer = TextRenderer::new();
        assert!(renderer.is_ok());
    }

    #[test]
    fn test_measure_text() {
        let renderer = TextRenderer::new().unwrap();
        let style = TextStyle::new(24.0, Rgba([255, 255, 255, 255]));

        let (width, height) = renderer.measure_text("Hello", &style);
        assert!(width > 0.0);
        assert!(height > 0.0);
    }

    #[test]
    fn test_text_wrapping() {
        let renderer = TextRenderer::new().unwrap();
        let style = TextStyle::new(24.0, Rgba([255, 255, 255, 255])).with_max_width(100);

        let lines = renderer.wrap_text("This is a long text that should wrap", &style);
        assert!(lines.len() > 1);
    }

    #[test]
    fn test_draw_text() {
        let renderer = TextRenderer::new().unwrap();
        let mut img = RgbaImage::new(200, 50);
        let style = TextStyle::new(20.0, Rgba([255, 255, 255, 255]));

        renderer.draw_text(&mut img, "Test", 10, 10, &style);

        // Check that some pixels were drawn (not all black)
        let has_white = img.pixels().any(|p| p[0] > 0 || p[1] > 0 || p[2] > 0);
        assert!(has_white);
    }
}
