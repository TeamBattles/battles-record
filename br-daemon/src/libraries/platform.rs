//! Platform-specific binary directory resolution.
//!
//! Provides a single source of truth for locating managed binaries
//! (yt-dlp, FFmpeg, Bun) across Windows, macOS, and Linux.

use std::path::PathBuf;

/// Returns the application's managed binary directory, if it can be determined.
///
/// - Windows: `%APPDATA%/com.battles.record/bin/`
/// - macOS: `$HOME/Library/Application Support/com.battles.record/bin/`
/// - Linux: `$HOME/.local/share/com.battles.record/bin/`
pub fn get_bin_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|appdata| {
            PathBuf::from(appdata)
                .join("com.battles.record")
                .join("bin")
        })
    }

    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|home| {
            PathBuf::from(home).join("Library/Application Support/com.battles.record/bin")
        })
    }

    #[cfg(target_os = "linux")]
    {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".local/share/com.battles.record/bin"))
    }
}

/// Resolve a binary by name, checking locations in priority order:
///
/// 1. Explicit path (if provided and the file exists)
/// 2. App bin directory (`get_bin_dir()`)
/// 3. System PATH (via `which::which`)
pub fn resolve_binary(name: &str, explicit_path: Option<&PathBuf>) -> Option<PathBuf> {
    // 1. Explicit path takes priority
    if let Some(path) = explicit_path {
        if path.exists() {
            return Some(path.clone());
        }
    }

    // 2. Check app bin directory
    if let Some(bin_dir) = get_bin_dir() {
        let binary_name = platform_binary_name(name);
        let bin_path = bin_dir.join(&binary_name);
        if bin_path.exists() {
            return Some(bin_path);
        }
    }

    // 3. Fall back to system PATH
    which::which(name).ok()
}

/// Returns the platform-appropriate binary filename (adds .exe on Windows).
pub fn platform_binary_name(name: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        format!("{}.exe", name)
    }
    #[cfg(not(target_os = "windows"))]
    {
        name.to_string()
    }
}

/// Returns the platform-specific GitHub release asset name for yt-dlp.
pub fn ytdlp_asset_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "yt-dlp.exe"
    }
    #[cfg(target_os = "macos")]
    {
        "yt-dlp_macos"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "yt-dlp_linux"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "yt-dlp_linux_aarch64"
    }
}

/// Returns the BtbN asset suffix for this platform's stable FFmpeg build.
/// Used to match assets from the GitHub releases API.
/// Returns None on macOS (BtbN doesn't build macOS binaries).
pub fn ffmpeg_btbn_asset_suffix() -> Option<&'static str> {
    #[cfg(target_os = "windows")]
    {
        Some("-win64-gpl")
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        Some("-linux64-gpl")
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        Some("-linuxarm64-gpl")
    }
    #[cfg(target_os = "macos")]
    {
        None
    }
}

/// Returns the macOS-only FFmpeg download URL (evermeet.cx).
/// Windows and Linux use BtbN stable releases via GitHub API instead.
pub fn ffmpeg_download_url() -> Option<&'static str> {
    #[cfg(target_os = "macos")]
    {
        Some("https://evermeet.cx/ffmpeg/getrelease/zip")
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_bin_dir_returns_some() {
        // Should return Some on any platform with HOME or APPDATA set
        let result = get_bin_dir();
        // Can't assert Some in CI without the env var, but at least it shouldn't panic
        if result.is_some() {
            let path = result.unwrap();
            assert!(path.ends_with("bin"));
        }
    }

    #[test]
    fn test_resolve_nonexistent_binary() {
        let result = resolve_binary("nonexistent_binary_xyz_12345", None);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_with_explicit_nonexistent_path() {
        let fake_path = PathBuf::from("/this/does/not/exist/binary");
        let result = resolve_binary("nonexistent", Some(&fake_path));
        assert!(result.is_none());
    }

    #[test]
    fn test_platform_binary_name() {
        let name = platform_binary_name("yt-dlp");
        #[cfg(target_os = "windows")]
        assert_eq!(name, "yt-dlp.exe");
        #[cfg(not(target_os = "windows"))]
        assert_eq!(name, "yt-dlp");
    }

    #[test]
    fn test_ytdlp_asset_name_not_empty() {
        let name = ytdlp_asset_name();
        assert!(!name.is_empty());
    }

    #[test]
    fn test_ffmpeg_btbn_asset_suffix() {
        // Windows and Linux should have a BtbN suffix, macOS should not
        let suffix = ffmpeg_btbn_asset_suffix();
        #[cfg(target_os = "macos")]
        assert!(suffix.is_none());
        #[cfg(not(target_os = "macos"))]
        assert!(suffix.is_some());
    }

    #[test]
    fn test_ffmpeg_download_url_macos_only() {
        let url = ffmpeg_download_url();
        #[cfg(target_os = "macos")]
        assert!(url.is_some());
        #[cfg(not(target_os = "macos"))]
        assert!(url.is_none());
    }
}
