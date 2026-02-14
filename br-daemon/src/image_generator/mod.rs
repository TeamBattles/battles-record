//! Image generation for Jellyfin metadata.
//!
//! Generates rich images with gradients, text, and metadata for:
//! - Show posters (Primary)
//! - Season posters
//! - Episode thumbnails (Thumb)
//! - Channel banners
//! - Channel logos

pub mod colors;
pub mod generator;
pub mod gradients;
pub mod templates;
pub mod text;

pub use generator::{ImageGenerator, ImageMetadata, SeasonMetadata, ShowMetadata};
