use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Extracts bundled YouTube dependencies (yt-dlp, bun) from Tauri resources to app bin directory.
/// Called on first launch for YouTube-bundled variants.
pub fn extract_youtube_deps(app: &AppHandle) {
    if let Err(e) = extract_youtube_deps_impl(app) {
        log::warn!("Failed to extract bundled YouTube deps: {}", e);
    }
}

fn extract_youtube_deps_impl(app: &AppHandle) -> Result<(), String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource dir: {}", e))?;

    let youtube_deps_dir = resource_dir.join("youtube-deps");

    // Not a bundled variant - skip
    if !youtube_deps_dir.exists() {
        return Ok(());
    }

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    let bin_dir = data_dir.join("bin");

    // Create bin dir if needed
    std::fs::create_dir_all(&bin_dir)
        .map_err(|e| format!("Failed to create bin dir: {}", e))?;

    // Copy yt-dlp if not already present
    #[cfg(target_os = "windows")]
    let ytdlp_name = "yt-dlp.exe";
    #[cfg(not(target_os = "windows"))]
    let ytdlp_name = "yt-dlp";

    copy_if_missing(&youtube_deps_dir, &bin_dir, ytdlp_name)?;

    // Copy bun if not already present
    #[cfg(target_os = "windows")]
    let bun_name = "bun.exe";
    #[cfg(not(target_os = "windows"))]
    let bun_name = "bun";

    copy_if_missing(&youtube_deps_dir, &bin_dir, bun_name)?;

    Ok(())
}

fn copy_if_missing(src_dir: &PathBuf, dest_dir: &PathBuf, filename: &str) -> Result<(), String> {
    let src_path = src_dir.join(filename);
    let dest_path = dest_dir.join(filename);

    // Skip if source doesn't exist (maybe only some deps bundled)
    if !src_path.exists() {
        return Ok(());
    }

    // Skip if destination already exists (respect user-installed versions)
    if dest_path.exists() {
        return Ok(());
    }

    log::info!("Extracting bundled {}", filename);

    std::fs::copy(&src_path, &dest_path)
        .map_err(|e| format!("Failed to copy {}: {}", filename, e))?;

    // Set executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set permissions on {}: {}", filename, e))?;
    }

    Ok(())
}
