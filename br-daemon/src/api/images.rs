// br-daemon/src/api/images.rs
//! Channel image management API endpoints.
//!
//! Provides endpoints for:
//! - GET /api/channels/:id/profile - Get channel profile with all image URLs
//! - GET /api/channels/:id/images/profile - Serve custom profile image
//! - POST /api/channels/:id/images/profile - Upload custom profile image
//! - DELETE /api/channels/:id/images/profile - Delete custom profile image
//! - GET /api/channels/:id/images/banner - Serve custom banner image
//! - POST /api/channels/:id/images/banner - Upload custom banner image
//! - DELETE /api/channels/:id/images/banner - Delete custom banner image

use crate::api::auth::{AdminUser, AuthUser};
use crate::api::response::{ApiError, ApiResponse};
use crate::api::AppState;
use crate::types::ChannelProfile;
use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use image::ImageFormat;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tracing::{error, info};
use uuid::Uuid;

/** Maximum file size for profile images (5MB). */
const MAX_PROFILE_SIZE: usize = 5 * 1024 * 1024;
/** Maximum file size for banner images (10MB). */
const MAX_BANNER_SIZE: usize = 10 * 1024 * 1024;
/** Target profile image dimensions. */
const PROFILE_WIDTH: u32 = 300;
const PROFILE_HEIGHT: u32 = 300;
/** Target banner image dimensions. */
const BANNER_WIDTH: u32 = 1200;
const BANNER_HEIGHT: u32 = 400;

/** Allowed image MIME types. */
const ALLOWED_MIME_TYPES: &[&str] = &["image/jpeg", "image/png", "image/webp", "image/gif"];

#[derive(serde::Serialize)]
pub struct ImageUploadResponse {
    pub success: bool,
    pub url: String,
}

#[derive(serde::Serialize)]
pub struct ImageDeleteResponse {
    pub deleted: bool,
}

/** Get channel profile with all image URLs. */
pub async fn get_channel_profile(
    _auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<ChannelProfile>>, (StatusCode, ApiError)> {
    // Get channel from manager
    let channel = state
        .channel_manager
        .get_channel(id)
        .ok_or_else(|| ApiError::not_found("Channel"))?;

    // Get channel config to check for custom images
    let channel_config = state
        .channel_manager
        .get_channel_config(id)
        .ok_or_else(|| ApiError::not_found("Channel"))?;

    // Build image URLs
    let base_url = format!(
        "http://{}:{}/api/channels/{}/images",
        state.config.read().daemon.host,
        state.config.read().daemon.port,
        id
    );

    let custom_profile_url = channel_config
        .custom_profile_image
        .as_ref()
        .map(|_| format!("{}/profile", base_url));

    let custom_banner_url = channel_config
        .custom_banner_image
        .as_ref()
        .map(|_| format!("{}/banner", base_url));

    // Return cached platform image URLs (fetched when channel was added)
    let platform_profile_url = channel_config.platform_profile_url.clone();
    let platform_banner_url = channel_config.platform_banner_url.clone();

    let profile = ChannelProfile {
        channel_id: id,
        display_name: channel.name,
        platform: channel.platform,
        description: None,
        platform_profile_url,
        platform_banner_url,
        custom_profile_url,
        custom_banner_url,
    };

    Ok(Json(ApiResponse::new(profile)))
}

/** Serve custom profile image. */
pub async fn get_profile_image(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Response, (StatusCode, ApiError)> {
    serve_image(&state, id, "profile").await
}

/** Serve custom banner image. */
pub async fn get_banner_image(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Response, (StatusCode, ApiError)> {
    serve_image(&state, id, "banner").await
}

/** Upload custom profile image. */
pub async fn upload_profile_image(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    multipart: Multipart,
) -> Result<Json<ApiResponse<ImageUploadResponse>>, (StatusCode, ApiError)> {
    upload_image(
        &state,
        id,
        "profile",
        PROFILE_WIDTH,
        PROFILE_HEIGHT,
        MAX_PROFILE_SIZE,
        multipart,
    )
    .await
}

/** Upload custom banner image. */
pub async fn upload_banner_image(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    multipart: Multipart,
) -> Result<Json<ApiResponse<ImageUploadResponse>>, (StatusCode, ApiError)> {
    upload_image(
        &state,
        id,
        "banner",
        BANNER_WIDTH,
        BANNER_HEIGHT,
        MAX_BANNER_SIZE,
        multipart,
    )
    .await
}

/** Delete custom profile image. */
pub async fn delete_profile_image(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<ImageDeleteResponse>>, (StatusCode, ApiError)> {
    delete_image(&state, id, "profile").await
}

/** Delete custom banner image. */
pub async fn delete_banner_image(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<ImageDeleteResponse>>, (StatusCode, ApiError)> {
    delete_image(&state, id, "banner").await
}

/** Get the images directory for a channel. */
fn get_channel_images_dir(state: &AppState, channel_id: Uuid) -> PathBuf {
    let config = state.config.read();
    config
        .storage
        .images_dir
        .join("channels")
        .join(channel_id.to_string())
}

/** Serve an image file. */
async fn serve_image(
    state: &AppState,
    channel_id: Uuid,
    image_type: &str,
) -> Result<Response, (StatusCode, ApiError)> {
    // Verify channel exists
    let _channel = state
        .channel_manager
        .get_channel(channel_id)
        .ok_or_else(|| ApiError::not_found("Channel"))?;

    let images_dir = get_channel_images_dir(state, channel_id);
    let image_path = images_dir.join(format!("{}.jpg", image_type));

    if !image_path.exists() {
        return Err(ApiError::not_found("Image"));
    }

    let bytes = fs::read(&image_path)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to read image: {}", e)))?;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/jpeg")
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(Body::from(bytes))
        .map_err(|e| ApiError::internal(format!("Failed to build response: {}", e)))?;

    Ok(response)
}

/** Upload and process an image. */
async fn upload_image(
    state: &AppState,
    channel_id: Uuid,
    image_type: &str,
    target_width: u32,
    target_height: u32,
    max_size: usize,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<ImageUploadResponse>>, (StatusCode, ApiError)> {
    // Verify channel exists
    let _channel = state
        .channel_manager
        .get_channel(channel_id)
        .ok_or_else(|| ApiError::not_found("Channel"))?;

    // Extract file from multipart
    let field = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad_request(format!("Invalid multipart data: {}", e)))?
        .ok_or_else(|| ApiError::bad_request("No file provided"))?;

    let content_type = field
        .content_type()
        .map(|ct| ct.to_string())
        .unwrap_or_default();

    // Validate content type
    if !ALLOWED_MIME_TYPES.contains(&content_type.as_str()) {
        return Err(ApiError::bad_request(format!(
            "Invalid file type '{}'. Allowed: jpg, png, webp, gif",
            content_type
        )));
    }

    // Read file data
    let data = field
        .bytes()
        .await
        .map_err(|e| ApiError::bad_request(format!("Failed to read file: {}", e)))?;

    if data.len() > max_size {
        return Err(ApiError::bad_request(format!(
            "File too large. Maximum size: {} MB",
            max_size / (1024 * 1024)
        )));
    }

    // Process image: resize and convert to JPEG
    let processed_image = process_image(&data, target_width, target_height)
        .map_err(|e| ApiError::bad_request(format!("Failed to process image: {}", e)))?;

    // Ensure directory exists
    let images_dir = get_channel_images_dir(state, channel_id);
    fs::create_dir_all(&images_dir)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to create images directory: {}", e)))?;

    // Write image file
    let image_path = images_dir.join(format!("{}.jpg", image_type));
    fs::write(&image_path, &processed_image)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to save image: {}", e)))?;

    // Update channel config with custom image path
    let relative_path = format!("channels/{}/{}.jpg", channel_id, image_type);
    update_channel_image_config(state, channel_id, image_type, Some(relative_path))
        .map_err(|e| ApiError::internal(format!("Failed to update config: {}", e)))?;

    info!(
        "Uploaded {} image for channel {} ({} bytes)",
        image_type,
        channel_id,
        processed_image.len()
    );

    // Build URL for response
    let base_url = format!(
        "http://{}:{}/api/channels/{}/images/{}",
        state.config.read().daemon.host,
        state.config.read().daemon.port,
        channel_id,
        image_type
    );

    Ok(Json(ApiResponse::new(ImageUploadResponse {
        success: true,
        url: base_url,
    })))
}

/** Delete an image. */
async fn delete_image(
    state: &AppState,
    channel_id: Uuid,
    image_type: &str,
) -> Result<Json<ApiResponse<ImageDeleteResponse>>, (StatusCode, ApiError)> {
    // Verify channel exists
    let _channel = state
        .channel_manager
        .get_channel(channel_id)
        .ok_or_else(|| ApiError::not_found("Channel"))?;

    let images_dir = get_channel_images_dir(state, channel_id);
    let image_path = images_dir.join(format!("{}.jpg", image_type));

    let deleted = if image_path.exists() {
        fs::remove_file(&image_path)
            .await
            .map_err(|e| ApiError::internal(format!("Failed to delete image: {}", e)))?;
        true
    } else {
        false
    };

    // Update channel config to remove custom image path
    update_channel_image_config(state, channel_id, image_type, None)
        .map_err(|e| ApiError::internal(format!("Failed to update config: {}", e)))?;

    if deleted {
        info!("Deleted {} image for channel {}", image_type, channel_id);
    }

    Ok(Json(ApiResponse::new(ImageDeleteResponse { deleted })))
}

/** Process an image: resize to target dimensions and convert to JPEG. */
fn process_image(
    data: &[u8],
    target_width: u32,
    target_height: u32,
) -> Result<Vec<u8>, image::ImageError> {
    // Load image from bytes
    let img = image::load_from_memory(data)?;

    // Resize with aspect ratio preservation, then crop to exact dimensions
    let resized = img.resize_to_fill(
        target_width,
        target_height,
        image::imageops::FilterType::Lanczos3,
    );

    // Encode as JPEG
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    resized.write_to(&mut cursor, ImageFormat::Jpeg)?;

    Ok(buffer)
}

/** Update channel config with custom image path. */
fn update_channel_image_config(
    state: &AppState,
    channel_id: Uuid,
    image_type: &str,
    path: Option<String>,
) -> Result<(), String> {
    state
        .channel_manager
        .update_channel_image(channel_id, image_type, path)
        .ok_or_else(|| "Channel not found".to_string())?;

    // Save config to disk
    save_channels_to_config(state);

    Ok(())
}

/** Sync channels from manager to config and save to disk. */
fn save_channels_to_config(state: &AppState) {
    // Get channel configs directly from the manager
    let channel_configs = state.channel_manager.get_channel_configs();

    // Check if we have a separate channels file configured
    let channels_file = {
        let config = state.config.read();
        config.daemon.channels_file.clone()
    };

    if let Some(channels_path) = channels_file {
        // Save to separate channels file
        if let Err(e) = crate::config::save_channels_file(&channels_path, &channel_configs) {
            error!("Failed to save channels to {:?}: {}", channels_path, e);
        }
    } else {
        // Legacy behavior: save to main config file
        {
            let mut config = state.config.write();
            config.channels = channel_configs;
        }

        let config = state.config.read();
        if let Err(e) = config.save(&state.config_path) {
            error!("Failed to save config: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_image_resize() {
        // Create a simple 100x100 red image
        let mut img = image::RgbImage::new(100, 100);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgb([255, 0, 0]);
        }

        // Convert to bytes
        let mut bytes = Vec::new();
        let mut cursor = Cursor::new(&mut bytes);
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();

        // Process and resize to 50x50
        let result = process_image(&bytes, 50, 50);
        assert!(result.is_ok());

        // Verify the output is a valid JPEG
        let output = result.unwrap();
        let decoded = image::load_from_memory(&output).unwrap();
        assert_eq!(decoded.width(), 50);
        assert_eq!(decoded.height(), 50);
    }

    #[test]
    fn test_allowed_mime_types() {
        assert!(ALLOWED_MIME_TYPES.contains(&"image/jpeg"));
        assert!(ALLOWED_MIME_TYPES.contains(&"image/png"));
        assert!(ALLOWED_MIME_TYPES.contains(&"image/webp"));
        assert!(ALLOWED_MIME_TYPES.contains(&"image/gif"));
        assert!(!ALLOWED_MIME_TYPES.contains(&"image/bmp"));
        assert!(!ALLOWED_MIME_TYPES.contains(&"text/plain"));
    }
}
