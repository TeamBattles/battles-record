use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::events::DownloadEvent;
use super::job::{CookieData, DownloadOptions, ExtractedInfo, FormatInfo};

/// Build an enriched PATH that includes directories containing JS runtimes
/// (Node.js, Bun) so yt-dlp can solve YouTube signature challenges.
fn enriched_path() -> std::ffi::OsString {
    let mut dirs: Vec<PathBuf> = Vec::new();

    // Add the app's managed bin dir (contains bun)
    if let Some(bin_dir) = crate::libraries::platform::get_bin_dir() {
        dirs.push(bin_dir);
    }

    // Add common Node.js locations
    #[cfg(target_os = "windows")]
    {
        if let Some(pf) = std::env::var_os("ProgramFiles") {
            let node_dir = PathBuf::from(&pf).join("nodejs");
            if node_dir.exists() {
                dirs.push(node_dir);
            }
        }
    }

    // Build new PATH: extra dirs + existing PATH
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let mut new_path = std::env::join_paths(dirs).unwrap_or_default();
    if !existing.is_empty() {
        let sep = if cfg!(windows) { ";" } else { ":" };
        new_path.push(sep);
        new_path.push(&existing);
    }
    new_path
}

const EXTRACT_TIMEOUT_SECS: u64 = 60;

#[derive(thiserror::Error, Debug)]
pub enum YtdlpDownloadError {
    #[error("yt-dlp not found at {0}")]
    NotFound(PathBuf),
    #[error("Extract info failed: {0}")]
    ExtractFailed(String),
    #[error("Extract info timed out after {0} seconds")]
    Timeout(u64),
    #[error("Download failed: {0}")]
    DownloadFailed(String),
    #[error("Process error: {0}")]
    Process(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),
    #[error("Signal error: {0}")]
    Signal(String),
    #[error("Cookie file error: {0}")]
    CookieFile(String),
}

/// Raw JSON shape from yt-dlp --dump-json
#[derive(Debug, Deserialize)]
struct RawYtdlpInfo {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    thumbnail: Option<String>,
    #[serde(default)]
    uploader: Option<String>,
    #[serde(default)]
    extractor_key: Option<String>,
    #[serde(default)]
    webpage_url: Option<String>,
    #[serde(default)]
    formats: Option<Vec<RawFormatInfo>>,
}

#[derive(Debug, Deserialize)]
struct RawFormatInfo {
    #[serde(default)]
    format_id: Option<String>,
    #[serde(default)]
    ext: Option<String>,
    #[serde(default)]
    resolution: Option<String>,
    #[serde(default)]
    filesize: Option<u64>,
    #[serde(default)]
    filesize_approx: Option<u64>,
    #[serde(default)]
    vcodec: Option<String>,
    #[serde(default)]
    acodec: Option<String>,
    #[serde(default)]
    fps: Option<f64>,
    #[serde(default)]
    tbr: Option<f64>,
}

impl From<RawYtdlpInfo> for ExtractedInfo {
    fn from(raw: RawYtdlpInfo) -> Self {
        let formats = raw
            .formats
            .unwrap_or_default()
            .into_iter()
            .filter_map(|f| {
                let format_id = f.format_id?;
                let ext = f.ext.unwrap_or_else(|| "unknown".to_string());
                Some(FormatInfo {
                    format_id,
                    ext,
                    resolution: f.resolution,
                    filesize_approx: f.filesize_approx.or(f.filesize),
                    vcodec: f.vcodec,
                    acodec: f.acodec,
                    fps: f.fps,
                    tbr: f.tbr,
                })
            })
            .collect();

        Self {
            title: raw.title.unwrap_or_else(|| "Untitled".to_string()),
            duration: raw.duration.map(|d| d as u64),
            thumbnail: raw.thumbnail,
            uploader: raw.uploader,
            platform_name: raw.extractor_key,
            formats,
            webpage_url: raw.webpage_url,
        }
    }
}

/// Run yt-dlp --dump-json to extract video info without downloading.
pub async fn extract_info(
    ytdlp_path: &Path,
    url: &str,
    cookie_file: Option<&Path>,
) -> Result<ExtractedInfo, YtdlpDownloadError> {
    if !ytdlp_path.exists() {
        return Err(YtdlpDownloadError::NotFound(ytdlp_path.to_path_buf()));
    }

    let mut cmd = Command::new(ytdlp_path);
    cmd.env("PATH", enriched_path())
        .arg("--dump-json")
        .arg("--no-download")
        .arg("--js-runtimes")
        .arg("node")
        .arg("--js-runtimes")
        .arg("bun")
        .arg("--js-runtimes")
        .arg("deno")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(cookie_path) = cookie_file {
        cmd.arg("--cookies").arg(cookie_path);
    }

    cmd.arg(url);

    let output = tokio::time::timeout(Duration::from_secs(EXTRACT_TIMEOUT_SECS), cmd.output())
        .await
        .map_err(|_| YtdlpDownloadError::Timeout(EXTRACT_TIMEOUT_SECS))?
        .map_err(YtdlpDownloadError::Process)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(url = %url, stderr = %stderr, "yt-dlp extract_info failed");
        return Err(YtdlpDownloadError::ExtractFailed(stderr.into_owned()));
    }

    let raw: RawYtdlpInfo = serde_json::from_slice(&output.stdout)?;
    Ok(raw.into())
}

/// Handle to a running yt-dlp download process.
pub struct YtdlpDownload {
    child: tokio::process::Child,
    #[allow(dead_code)]
    download_id: Uuid,
    stderr_handle: Option<tokio::task::JoinHandle<String>>,
}

/// Start a yt-dlp download with progress reporting.
#[allow(clippy::too_many_arguments)]
pub async fn start_download(
    ytdlp_path: &Path,
    ffmpeg_path: Option<&Path>,
    url: &str,
    format: &str,
    output_dir: &Path,
    output_template: &str,
    options: &DownloadOptions,
    cookie_file: Option<&Path>,
    progress_tx: broadcast::Sender<DownloadEvent>,
    download_id: Uuid,
) -> Result<YtdlpDownload, YtdlpDownloadError> {
    if !ytdlp_path.exists() {
        return Err(YtdlpDownloadError::NotFound(ytdlp_path.to_path_buf()));
    }

    let mut cmd = Command::new(ytdlp_path);
    cmd.env("PATH", enriched_path())
        .arg("--js-runtimes")
        .arg("node")
        .arg("--js-runtimes")
        .arg("bun")
        .arg("--js-runtimes")
        .arg("deno")
        .arg("--progress-template")
        .arg("download:%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s|%(progress.downloaded_bytes)s|%(progress.total_bytes)s")
        .arg("--newline")
        .arg("--no-warnings")
        .arg("-f")
        .arg(format);

    // Only force mp4 merge for video downloads, not audio-only
    let is_audio_only = format.starts_with("bestaudio")
        || format.contains("audio")
        || ["139", "140", "249", "251"].contains(&format);
    if !is_audio_only {
        cmd.arg("--merge-output-format").arg("mp4");
    }

    cmd
        .arg("-o")
        .arg(output_dir.join(output_template));

    // Thumbnail embedding doesn't support webm - only embed when output will be mp4/mkv/m4a/ogg
    if options.embed_thumbnail && !is_audio_only {
        cmd.arg("--embed-thumbnail");
    }
    if options.embed_metadata {
        cmd.arg("--embed-metadata");
    }
    if let Some(cookie_path) = cookie_file {
        cmd.arg("--cookies").arg(cookie_path);
    }
    if let Some(ffmpeg) = ffmpeg_path {
        cmd.arg("--ffmpeg-location").arg(ffmpeg);
    }

    cmd.arg(url);

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    #[cfg(windows)]
    {
        cmd.creation_flags(0x00000200); // CREATE_NEW_PROCESS_GROUP
    }

    let mut child = cmd.spawn().map_err(YtdlpDownloadError::Process)?;

    // Take stdout for progress reading
    if let Some(stdout) = child.stdout.take() {
        let tx = progress_tx;
        let id = download_id;
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut last_emit = Instant::now() - Duration::from_secs(2);

            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(progress) = parse_progress_line(&line) {
                    // Throttle to 1 event per second
                    let now = Instant::now();
                    if now.duration_since(last_emit) >= Duration::from_secs(1) {
                        last_emit = now;
                        let _ = tx.send(DownloadEvent::Progress {
                            download_id: id,
                            percent: progress.percent.unwrap_or(0.0),
                            speed: progress.speed,
                            eta: progress.eta,
                            downloaded_bytes: progress.downloaded_bytes.unwrap_or(0),
                            total_bytes: progress.total_bytes,
                        });
                    }
                }
            }
        });
    }

    // Capture stderr for error reporting
    let stderr_handle = child.stderr.take().map(|stderr| {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            let mut output = String::new();
            while let Ok(Some(line)) = lines.next_line().await {
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str(&line);
            }
            output
        })
    });

    Ok(YtdlpDownload { child, download_id, stderr_handle })
}

impl YtdlpDownload {
    /// Pause the download (SIGINT on Unix, Ctrl+C event on Windows).
    pub fn pause(&self) -> Result<(), YtdlpDownloadError> {
        #[cfg(unix)]
        {
            if let Some(pid) = self.child.id() {
                // SAFETY: libc::kill is a standard POSIX function; we pass a valid pid
                let ret = unsafe { libc::kill(pid as i32, libc::SIGINT) };
                if ret != 0 {
                    return Err(YtdlpDownloadError::Signal(
                        std::io::Error::last_os_error().to_string(),
                    ));
                }
            }
        }
        #[cfg(windows)]
        {
            if let Some(pid) = self.child.id() {
                // SAFETY: GenerateConsoleCtrlEvent is a standard Win32 API
                let ret = unsafe {
                    windows_sys::Win32::System::Console::GenerateConsoleCtrlEvent(
                        windows_sys::Win32::System::Console::CTRL_C_EVENT,
                        pid,
                    )
                };
                if ret == 0 {
                    return Err(YtdlpDownloadError::Signal(
                        std::io::Error::last_os_error().to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Cancel the download (kill the process).
    pub async fn cancel(&mut self) -> Result<(), YtdlpDownloadError> {
        let _ = self.child.kill().await;
        Ok(())
    }

    /// Wait for the process to complete.
    pub async fn wait(&mut self) -> Result<std::process::ExitStatus, YtdlpDownloadError> {
        self.child.wait().await.map_err(YtdlpDownloadError::Process)
    }

    /// Collect captured stderr output after process exits.
    pub async fn stderr(&mut self) -> String {
        match self.stderr_handle.take() {
            Some(handle) => handle.await.unwrap_or_default(),
            None => String::new(),
        }
    }
}

/// Parsed progress data from a single yt-dlp output line.
#[derive(Debug, PartialEq)]
struct ProgressInfo {
    percent: Option<f64>,
    speed: Option<String>,
    eta: Option<u64>,
    downloaded_bytes: Option<u64>,
    total_bytes: Option<u64>,
}

/// Parse a yt-dlp progress line like:
/// `download:  45.2%|5.2MiB/s|00:23|24000000|52000000`
fn parse_progress_line(line: &str) -> Option<ProgressInfo> {
    // yt-dlp --progress-template "download:TEMPLATE" outputs TEMPLATE without the "download:" prefix
    let content = line.strip_prefix("download:").unwrap_or(line);
    let parts: Vec<&str> = content.split('|').collect();
    if parts.len() != 5 {
        return None;
    }

    let percent = parse_percent(parts[0]);
    let speed = parse_optional_string(parts[1]);
    let eta = parse_eta(parts[2]);
    let downloaded_bytes = parse_optional_u64(parts[3]);
    let total_bytes = parse_optional_u64(parts[4]);

    Some(ProgressInfo {
        percent,
        speed,
        eta,
        downloaded_bytes,
        total_bytes,
    })
}

fn parse_percent(s: &str) -> Option<f64> {
    let trimmed = s.trim();
    if trimmed == "NA" || trimmed.is_empty() {
        return None;
    }
    let stripped = trimmed.trim_end_matches('%').trim();
    stripped.parse::<f64>().ok()
}

fn parse_optional_string(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed == "NA" || trimmed.is_empty() || trimmed.starts_with("Unknown") {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_eta(s: &str) -> Option<u64> {
    let trimmed = s.trim();
    if trimmed == "NA" || trimmed.is_empty() {
        return None;
    }
    // ETA can be in HH:MM:SS or MM:SS format
    let parts: Vec<&str> = trimmed.split(':').collect();
    match parts.len() {
        2 => {
            let mins = parts[0].parse::<u64>().ok()?;
            let secs = parts[1].parse::<u64>().ok()?;
            Some(mins * 60 + secs)
        }
        3 => {
            let hours = parts[0].parse::<u64>().ok()?;
            let mins = parts[1].parse::<u64>().ok()?;
            let secs = parts[2].parse::<u64>().ok()?;
            Some(hours * 3600 + mins * 60 + secs)
        }
        _ => None,
    }
}

fn parse_optional_u64(s: &str) -> Option<u64> {
    let trimmed = s.trim();
    if trimmed == "NA" || trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<u64>().ok()
}

/// Write cookies in Netscape cookie file format for yt-dlp.
pub async fn write_cookie_file(
    cookies: &[CookieData],
    output_path: &Path,
) -> Result<(), YtdlpDownloadError> {
    let mut content = String::from("# Netscape HTTP Cookie File\n");

    for cookie in cookies {
        let include_subdomains = if cookie.domain.starts_with('.') {
            "TRUE"
        } else {
            "FALSE"
        };
        let secure = if cookie.secure { "TRUE" } else { "FALSE" };
        let expiration = cookie.expiration_date as i64;

        content.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            cookie.domain,
            include_subdomains,
            cookie.path,
            secure,
            expiration,
            cookie.name,
            cookie.value,
        ));
    }

    tokio::fs::write(output_path, content.as_bytes())
        .await
        .map_err(|e| {
            YtdlpDownloadError::CookieFile(format!("Failed to write cookie file: {}", e))
        })?;

    Ok(())
}

/// Build a cookie file path for a domain from persistent cookies, if available.
/// Falls back to the existing YouTube cookie pattern in {APPDATA}/com.battles.record/cookies/.
pub fn get_persistent_cookie_path(domain: &str) -> Option<PathBuf> {
    use crate::libraries::platform::get_bin_dir;

    // Persistent cookies are stored alongside binaries in the app data dir
    let bin_dir = get_bin_dir()?;
    let cookies_dir = bin_dir.parent()?.join("cookies");

    // Check for domain-specific cookie files
    // YouTube cookies: youtube_cookies.txt (existing pattern from cookie_utils.rs)
    let cookie_file = if domain.contains("youtube") || domain.contains("googlevideo") {
        cookies_dir.join("youtube_cookies.txt")
    } else {
        // Generic: {domain}_cookies.txt
        let safe_domain = domain.replace('.', "_");
        cookies_dir.join(format!("{}_cookies.txt", safe_domain))
    };

    if cookie_file.exists() {
        Some(cookie_file)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- Progress line parsing ---

    #[test]
    fn parse_progress_full_values() {
        let line = "download:  45.2%|5.2MiB/s|00:23|24000000|52000000";
        let info = parse_progress_line(line).expect("should parse");
        assert!((info.percent.unwrap() - 45.2).abs() < 0.01);
        assert_eq!(info.speed, Some("5.2MiB/s".to_string()));
        assert_eq!(info.eta, Some(23));
        assert_eq!(info.downloaded_bytes, Some(24000000));
        assert_eq!(info.total_bytes, Some(52000000));
    }

    #[test]
    fn parse_progress_all_na() {
        let line = "download:NA|NA|NA|NA|NA";
        let info = parse_progress_line(line).expect("should parse");
        assert_eq!(info.percent, None);
        assert_eq!(info.speed, None);
        assert_eq!(info.eta, None);
        assert_eq!(info.downloaded_bytes, None);
        assert_eq!(info.total_bytes, None);
    }

    #[test]
    fn parse_progress_mixed_na() {
        let line = "download: 100.0%|NA|NA|500000|NA";
        let info = parse_progress_line(line).expect("should parse");
        assert!((info.percent.unwrap() - 100.0).abs() < 0.01);
        assert_eq!(info.speed, None);
        assert_eq!(info.eta, None);
        assert_eq!(info.downloaded_bytes, Some(500000));
        assert_eq!(info.total_bytes, None);
    }

    #[test]
    fn parse_progress_eta_hours() {
        let line = "download:  10.0%|1.0MiB/s|01:30:45|1000000|10000000";
        let info = parse_progress_line(line).expect("should parse");
        assert_eq!(info.eta, Some(1 * 3600 + 30 * 60 + 45));
    }

    #[test]
    fn parse_progress_without_prefix() {
        let line = "  45.2%|5.2MiB/s|00:23|24000000|52000000";
        let info = parse_progress_line(line).expect("should parse without download: prefix");
        assert!((info.percent.unwrap() - 45.2).abs() < 0.01);
        assert_eq!(info.speed, Some("5.2MiB/s".to_string()));
        assert_eq!(info.eta, Some(23));
        assert_eq!(info.downloaded_bytes, Some(24000000));
        assert_eq!(info.total_bytes, Some(52000000));
    }

    #[test]
    fn parse_progress_not_download_prefix() {
        let line = "some random output line";
        assert!(parse_progress_line(line).is_none());
    }

    #[test]
    fn parse_progress_wrong_field_count() {
        let line = "download:45%|fast|00:10";
        assert!(parse_progress_line(line).is_none());
    }

    #[test]
    fn parse_progress_zero_percent() {
        let line = "download:   0.0%|NA|NA|0|NA";
        let info = parse_progress_line(line).expect("should parse");
        assert!((info.percent.unwrap()).abs() < 0.01);
        assert_eq!(info.downloaded_bytes, Some(0));
    }

    // --- Cookie file generation ---

    #[tokio::test]
    async fn write_cookie_file_basic() {
        let dir = TempDir::new().unwrap();
        let cookie_path = dir.path().join("cookies.txt");

        let cookies = vec![
            CookieData {
                domain: ".youtube.com".to_string(),
                path: "/".to_string(),
                secure: true,
                expiration_date: 1740000000.0,
                http_only: true,
                name: "LOGIN_INFO".to_string(),
                value: "some_token".to_string(),
            },
            CookieData {
                domain: "example.com".to_string(),
                path: "/api".to_string(),
                secure: false,
                expiration_date: 1750000000.0,
                http_only: false,
                name: "SESSION".to_string(),
                value: "abc123".to_string(),
            },
        ];

        write_cookie_file(&cookies, &cookie_path).await.expect("should write");

        let content = std::fs::read_to_string(&cookie_path).expect("should read back");

        assert!(content.starts_with("# Netscape HTTP Cookie File\n"));

        let lines: Vec<&str> = content.lines().collect();
        // Header + 2 cookie lines
        assert_eq!(lines.len(), 3);

        // First cookie: domain starts with '.', so include_subdomains=TRUE, secure=TRUE
        let fields: Vec<&str> = lines[1].split('\t').collect();
        assert_eq!(fields.len(), 7);
        assert_eq!(fields[0], ".youtube.com");
        assert_eq!(fields[1], "TRUE"); // include_subdomains
        assert_eq!(fields[2], "/");
        assert_eq!(fields[3], "TRUE"); // secure
        assert_eq!(fields[4], "1740000000");
        assert_eq!(fields[5], "LOGIN_INFO");
        assert_eq!(fields[6], "some_token");

        // Second cookie: domain doesn't start with '.', so include_subdomains=FALSE, secure=FALSE
        let fields: Vec<&str> = lines[2].split('\t').collect();
        assert_eq!(fields[0], "example.com");
        assert_eq!(fields[1], "FALSE");
        assert_eq!(fields[2], "/api");
        assert_eq!(fields[3], "FALSE");
    }

    #[tokio::test]
    async fn write_cookie_file_empty() {
        let dir = TempDir::new().unwrap();
        let cookie_path = dir.path().join("cookies.txt");

        write_cookie_file(&[], &cookie_path).await.expect("should write");

        let content = std::fs::read_to_string(&cookie_path).expect("should read back");
        assert_eq!(content, "# Netscape HTTP Cookie File\n");
    }

    // --- ExtractedInfo deserialization ---

    #[test]
    fn extracted_info_from_full_json() {
        let json = r#"{
            "title": "My Cool Video",
            "duration": 3600.5,
            "thumbnail": "https://img.youtube.com/thumb.jpg",
            "uploader": "CoolChannel",
            "extractor_key": "Youtube",
            "webpage_url": "https://www.youtube.com/watch?v=abc123",
            "formats": [
                {
                    "format_id": "137",
                    "ext": "mp4",
                    "resolution": "1920x1080",
                    "filesize_approx": 500000000,
                    "vcodec": "avc1.640028",
                    "acodec": "none",
                    "fps": 30.0,
                    "tbr": 4000.5
                },
                {
                    "format_id": "140",
                    "ext": "m4a",
                    "resolution": "audio only",
                    "filesize": 12000000,
                    "vcodec": "none",
                    "acodec": "mp4a.40.2",
                    "tbr": 128.0
                }
            ]
        }"#;

        let raw: RawYtdlpInfo = serde_json::from_str(json).unwrap();
        let info: ExtractedInfo = raw.into();

        assert_eq!(info.title, "My Cool Video");
        assert_eq!(info.duration, Some(3600));
        assert_eq!(
            info.thumbnail,
            Some("https://img.youtube.com/thumb.jpg".to_string())
        );
        assert_eq!(info.uploader, Some("CoolChannel".to_string()));
        assert_eq!(info.platform_name, Some("Youtube".to_string()));
        assert_eq!(
            info.webpage_url,
            Some("https://www.youtube.com/watch?v=abc123".to_string())
        );
        assert_eq!(info.formats.len(), 2);

        // First format should use filesize_approx
        assert_eq!(info.formats[0].format_id, "137");
        assert_eq!(info.formats[0].ext, "mp4");
        assert_eq!(info.formats[0].resolution, Some("1920x1080".to_string()));
        assert_eq!(info.formats[0].filesize_approx, Some(500000000));
        assert_eq!(info.formats[0].vcodec, Some("avc1.640028".to_string()));
        assert!((info.formats[0].fps.unwrap() - 30.0).abs() < f64::EPSILON);

        // Second format should fall back to filesize since filesize_approx is absent
        assert_eq!(info.formats[1].format_id, "140");
        assert_eq!(info.formats[1].filesize_approx, Some(12000000));
        assert_eq!(info.formats[1].acodec, Some("mp4a.40.2".to_string()));
    }

    #[test]
    fn extracted_info_from_minimal_json() {
        let json = r#"{}"#;
        let raw: RawYtdlpInfo = serde_json::from_str(json).unwrap();
        let info: ExtractedInfo = raw.into();

        assert_eq!(info.title, "Untitled");
        assert_eq!(info.duration, None);
        assert_eq!(info.thumbnail, None);
        assert_eq!(info.uploader, None);
        assert_eq!(info.platform_name, None);
        assert!(info.formats.is_empty());
        assert_eq!(info.webpage_url, None);
    }

    #[test]
    fn extracted_info_skips_formats_without_id() {
        let json = r#"{
            "title": "Test",
            "formats": [
                {"format_id": "137", "ext": "mp4"},
                {"ext": "webm"},
                {"format_id": "140", "ext": "m4a"}
            ]
        }"#;

        let raw: RawYtdlpInfo = serde_json::from_str(json).unwrap();
        let info: ExtractedInfo = raw.into();

        // The format without format_id should be filtered out
        assert_eq!(info.formats.len(), 2);
        assert_eq!(info.formats[0].format_id, "137");
        assert_eq!(info.formats[1].format_id, "140");
    }

    // --- parse helpers ---

    #[test]
    fn parse_percent_various() {
        assert!((parse_percent("  45.2%").unwrap() - 45.2).abs() < 0.01);
        assert!((parse_percent("100%").unwrap() - 100.0).abs() < 0.01);
        assert!((parse_percent("  0.0%  ").unwrap()).abs() < 0.01);
        assert_eq!(parse_percent("NA"), None);
        assert_eq!(parse_percent(""), None);
    }

    #[test]
    fn parse_eta_various() {
        assert_eq!(parse_eta("00:23"), Some(23));
        assert_eq!(parse_eta("05:30"), Some(330));
        assert_eq!(parse_eta("01:00:00"), Some(3600));
        assert_eq!(parse_eta("NA"), None);
        assert_eq!(parse_eta(""), None);
        assert_eq!(parse_eta("not_a_time"), None);
    }

    #[test]
    fn parse_optional_u64_various() {
        assert_eq!(parse_optional_u64("24000000"), Some(24000000));
        assert_eq!(parse_optional_u64("0"), Some(0));
        assert_eq!(parse_optional_u64("NA"), None);
        assert_eq!(parse_optional_u64(""), None);
        assert_eq!(parse_optional_u64("not_a_number"), None);
    }

    #[test]
    fn parse_optional_string_various() {
        assert_eq!(
            parse_optional_string("5.2MiB/s"),
            Some("5.2MiB/s".to_string())
        );
        assert_eq!(parse_optional_string("NA"), None);
        assert_eq!(parse_optional_string(""), None);
        assert_eq!(parse_optional_string(" Unknown B/s"), None);
        assert_eq!(parse_optional_string("Unknown"), None);
        assert_eq!(
            parse_optional_string("  trimmed  "),
            Some("trimmed".to_string())
        );
    }
}
