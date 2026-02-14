//! Cookie file utilities for platform authentication
//!
//! Provides utilities for managing Netscape-format cookie files used by yt-dlp.

use std::path::PathBuf;
use thiserror::Error;

/** Errors related to cookie file operations. */
#[derive(Error, Debug)]
pub enum CookieError {
    #[error("Cookie file validation failed: {0}")]
    ValidationError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to determine cookies directory")]
    NoCookiesDir,
}

/**
 * Get the cookies directory path based on the platform.
 * Uses the Tauri app data directory structure.
 */
pub fn get_cookies_dir() -> Result<PathBuf, CookieError> {
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return Ok(PathBuf::from(appdata)
                .join("com.battles.record")
                .join("cookies"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return Ok(
                PathBuf::from(home).join("Library/Application Support/com.battles.record/cookies")
            );
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return Ok(PathBuf::from(home).join(".local/share/com.battles.record/cookies"));
        }
    }

    Err(CookieError::NoCookiesDir)
}

/** Get the path for YouTube cookies file. */
pub fn get_youtube_cookie_path() -> Result<PathBuf, CookieError> {
    let dir = get_cookies_dir()?;
    Ok(dir.join("youtube_cookies.txt"))
}

/**
 * Validate that the content is a valid Netscape cookie file format.
 *
 * Netscape cookie format:
 * - Lines starting with # are comments (first line should be "# Netscape HTTP Cookie File" or "# HTTP Cookie File")
 * - Empty lines are ignored
 * - Data lines have 7 tab-separated fields:
 *   domain, flag, path, secure, expiration, name, value
 */
pub fn validate_cookie_file(content: &str) -> Result<(), CookieError> {
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return Err(CookieError::ValidationError(
            "Cookie file is empty".to_string(),
        ));
    }

    // Check for Netscape header (common variants)
    let has_valid_header = lines.iter().any(|line| {
        let lower = line.to_lowercase();
        lower.contains("netscape") && lower.contains("cookie") || lower.contains("http cookie file")
    });

    if !has_valid_header {
        return Err(CookieError::ValidationError(
            "Missing Netscape cookie file header. Expected '# Netscape HTTP Cookie File' or similar.".to_string(),
        ));
    }

    let mut has_data_lines = false;
    let mut has_youtube_cookie = false;

    for line in &lines {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Validate data line format (7 tab-separated fields)
        let fields: Vec<&str> = trimmed.split('\t').collect();
        if fields.len() != 7 {
            return Err(CookieError::ValidationError(format!(
                "Invalid cookie line format. Expected 7 tab-separated fields, got {}. Line: '{}'",
                fields.len(),
                if trimmed.len() > 50 {
                    format!("{}...", &trimmed[..50])
                } else {
                    trimmed.to_string()
                }
            )));
        }

        has_data_lines = true;

        // Check if this is a YouTube cookie
        let domain = fields[0];
        if domain.contains("youtube.com") || domain.contains("google.com") {
            has_youtube_cookie = true;
        }
    }

    if !has_data_lines {
        return Err(CookieError::ValidationError(
            "No cookie data found in file".to_string(),
        ));
    }

    if !has_youtube_cookie {
        return Err(CookieError::ValidationError(
            "No YouTube or Google cookies found. Make sure to export cookies from youtube.com."
                .to_string(),
        ));
    }

    Ok(())
}

/**
 * Save YouTube cookie content to file.
 * Creates the cookies directory if it doesn't exist.
 */
pub async fn save_youtube_cookies(content: &str) -> Result<PathBuf, CookieError> {
    // Validate first
    validate_cookie_file(content)?;

    let cookie_path = get_youtube_cookie_path()?;

    // Create directory if needed
    if let Some(parent) = cookie_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Write atomically: write to temp file, then rename
    let temp_path = cookie_path.with_extension("tmp");
    tokio::fs::write(&temp_path, content).await?;
    tokio::fs::rename(&temp_path, &cookie_path).await?;

    tracing::info!("Saved YouTube cookies to {:?}", cookie_path);
    Ok(cookie_path)
}

/** Check if YouTube cookies file exists. */
pub fn youtube_cookies_exist() -> bool {
    get_youtube_cookie_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}

/** Delete YouTube cookies file. */
pub async fn delete_youtube_cookies() -> Result<(), CookieError> {
    let cookie_path = get_youtube_cookie_path()?;
    if cookie_path.exists() {
        tokio::fs::remove_file(&cookie_path).await?;
        tracing::info!("Deleted YouTube cookies from {:?}", cookie_path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_COOKIE_FILE: &str = r#"# Netscape HTTP Cookie File
# https://curl.haxx.se/rfc/cookie_spec.html
# This is a generated file! Do not edit.

.youtube.com	TRUE	/	TRUE	1700000000	VISITOR_INFO1_LIVE	abc123
.youtube.com	TRUE	/	FALSE	1700000000	PREF	f1=50000000
.google.com	TRUE	/	TRUE	1700000000	SID	xyz789
"#;

    const VALID_COOKIE_HTTP_HEADER: &str = r#"# HTTP Cookie File
.youtube.com	TRUE	/	TRUE	1700000000	LOGIN_INFO	token123
"#;

    #[test]
    fn test_validate_valid_cookie_file() {
        let result = validate_cookie_file(VALID_COOKIE_FILE);
        assert!(result.is_ok(), "Expected valid cookie file: {:?}", result);
    }

    #[test]
    fn test_validate_http_cookie_header() {
        let result = validate_cookie_file(VALID_COOKIE_HTTP_HEADER);
        assert!(
            result.is_ok(),
            "Expected valid cookie file with HTTP header: {:?}",
            result
        );
    }

    #[test]
    fn test_validate_empty_file() {
        let result = validate_cookie_file("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn test_validate_missing_header() {
        let content = ".youtube.com\tTRUE\t/\tTRUE\t1700000000\tCOOKIE\tvalue";
        let result = validate_cookie_file(content);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("header"));
    }

    #[test]
    fn test_validate_no_youtube_cookies() {
        let content = r#"# Netscape HTTP Cookie File
.example.com	TRUE	/	TRUE	1700000000	SESSION	abc123
"#;
        let result = validate_cookie_file(content);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("YouTube"));
    }

    #[test]
    fn test_validate_invalid_field_count() {
        let content = r#"# Netscape HTTP Cookie File
.youtube.com	TRUE	/	TRUE	1700000000
"#;
        let result = validate_cookie_file(content);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("7 tab-separated fields"));
    }

    #[test]
    fn test_validate_only_comments() {
        let content = r#"# Netscape HTTP Cookie File
# This file has no data
# Only comments
"#;
        let result = validate_cookie_file(content);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No cookie data"));
    }

    #[test]
    fn test_get_cookies_dir() {
        // This should succeed on any platform with the expected env var
        let result = get_cookies_dir();
        // We don't assert success because it depends on environment,
        // but we verify it doesn't panic
        if let Ok(path) = result {
            assert!(path.to_string_lossy().contains("com.battles.record"));
        }
    }

    #[test]
    fn test_get_youtube_cookie_path() {
        let result = get_youtube_cookie_path();
        if let Ok(path) = result {
            assert!(path.to_string_lossy().contains("youtube_cookies.txt"));
        }
    }
}
