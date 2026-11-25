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
    explicit_cookies: bool,
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

    if explicit_cookies {
        // Saved yt-dlp config must not add credentials to explicit extension input.
        cmd.arg("--ignore-config").arg("--no-cookies-from-browser");
    }
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
    stdout_handle: Option<tokio::task::JoinHandle<Option<PathBuf>>>,
    stderr_handle: Option<tokio::task::JoinHandle<String>>,
}

/// yt-dlp names files from its own title, which for a bare HLS playlist (a Twitch recording sent
/// by Battles Replay) is just "index-dvr". A client's title replaces it at pre_process, before the
/// output template is evaluated. The replacement goes to Python's `re.sub`, so backslashes are
/// doubled to stay literal.
fn title_override_args(title: Option<&str>) -> Option<[String; 4]> {
    let title = title.map(str::trim).filter(|t| !t.is_empty())?;
    Some([
        "--replace-in-metadata".to_string(),
        "title".to_string(),
        "(?s)^.*$".to_string(),
        title.replace('\\', r"\\"),
    ])
}

/// yt-dlp prints the final path (after merging and moving) on stdout behind this marker, so the
/// daemon records the file this download wrote instead of guessing from its folder, which other
/// downloads for the same channel share.
const OUTPUT_FILE_MARKER: &str = "BR_OUTPUT_FILE ";

/// `--print` implies `--quiet`, which also hides the progress template unless `--progress` is set.
fn output_file_args() -> [&'static str; 3] {
    [
        "--progress",
        "--print",
        "after_move:BR_OUTPUT_FILE %(filepath)s",
    ]
}

fn parse_output_file_line(line: &str) -> Option<PathBuf> {
    let path = line.strip_prefix(OUTPUT_FILE_MARKER)?;
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// Twitch's recording playlist (`.../index-dvr.m3u8`) has no `#EXT-X-ENDLIST` while the stream is
/// still running, so yt-dlp treats it as live and hands it to ffmpeg. Left alone, ffmpeg joins three
/// segments from the live edge (everything earlier is lost) and yt-dlp reports no progress until the
/// broadcast ends. These args start at segment 0, as yt-dlp's own `--live-from-start` does for
/// Twitch, and have ffmpeg write key=value progress to stdout (yt-dlp runs it with stdout inherited).
/// `-nostats` stops ffmpeg's status line piling up in the stderr we keep for error reporting. A
/// finished recording has ENDLIST and goes through yt-dlp's native HLS downloader, which ignores them.
fn live_twitch_recording_args(url: &str) -> Option<[&'static str; 2]> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.ends_with("/index-dvr.m3u8").then_some([
        "--downloader-args",
        "ffmpeg_i:-live_start_index 0 -progress pipe:1 -nostats",
    ])
}

/// Accumulates the key=value blocks ffmpeg writes for `-progress`. Each block ends with a
/// `progress=continue` (or `progress=end`) line.
#[derive(Default)]
struct FfmpegProgress {
    total_size: u64,
}

impl FfmpegProgress {
    /// Returns the bytes written so far once per completed block.
    fn feed(&mut self, line: &str) -> Option<u64> {
        let (key, value) = line.split_once('=')?;
        match key {
            // `N/A` until the muxer has written something.
            "total_size" => {
                if let Ok(bytes) = value.trim().parse() {
                    self.total_size = bytes;
                }
                None
            }
            "progress" => Some(self.total_size),
            _ => None,
        }
    }
}

/// Download rate over a trailing window. ffmpeg following a live playlist fetches each segment
/// (about 10s of video) in a burst and then idles until the next is published, so the rate between
/// two adjacent samples flips between zero and full network speed.
#[derive(Default)]
struct RateWindow {
    samples: std::collections::VecDeque<(Instant, u64)>,
}

impl RateWindow {
    const SPAN: Duration = Duration::from_secs(30);

    fn push(&mut self, now: Instant, bytes: u64) -> Option<f64> {
        self.samples.push_back((now, bytes));
        while self
            .samples
            .get(1)
            .is_some_and(|(t, _)| now.duration_since(*t) >= Self::SPAN)
        {
            self.samples.pop_front();
        }
        let (start, start_bytes) = *self.samples.front()?;
        let secs = now.duration_since(start).as_secs_f64();
        (secs > 0.0).then(|| bytes.saturating_sub(start_bytes) as f64 / secs)
    }
}

/// Formats a rate the way yt-dlp's `_speed_str` does, e.g. `30.52MiB/s`.
fn format_speed(bytes_per_sec: f64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes_per_sec;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.2}{}/s", UNITS[unit])
}

/// Start a yt-dlp download with progress reporting.
#[allow(clippy::too_many_arguments)]
pub async fn start_download(
    ytdlp_path: &Path,
    ffmpeg_path: Option<&Path>,
    url: &str,
    title: Option<&str>,
    format: &str,
    output_dir: &Path,
    output_template: &str,
    options: &DownloadOptions,
    cookie_file: Option<&Path>,
    explicit_cookies: bool,
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
        .args(output_file_args())
        .arg("-f")
        .arg(format);

    if let Some(args) = title_override_args(title) {
        cmd.args(args);
    }
    if let Some(args) = live_twitch_recording_args(url) {
        cmd.args(args);
    }

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
    if explicit_cookies {
        // Saved yt-dlp config must not add credentials to explicit extension input.
        cmd.arg("--ignore-config").arg("--no-cookies-from-browser");
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

    // Take stdout for progress reading and the reported output file
    let stdout_handle = child.stdout.take().map(|stdout| {
        let tx = progress_tx;
        let id = download_id;
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut last_emit = Instant::now() - Duration::from_secs(2);
            let mut ffmpeg = FfmpegProgress::default();
            let mut rate = RateWindow::default();
            let mut output_file = None;

            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(path) = parse_output_file_line(&line) {
                    output_file = Some(path);
                    continue;
                }
                let now = Instant::now();
                let event = if let Some(bytes) = ffmpeg.feed(&line) {
                    // Only a live recording reaches ffmpeg with `-progress`; it has no total.
                    Some(DownloadEvent::Progress {
                        download_id: id,
                        percent: 0.0,
                        speed: rate.push(now, bytes).map(format_speed),
                        eta: None,
                        downloaded_bytes: bytes,
                        total_bytes: None,
                        live: true,
                    })
                } else {
                    parse_progress_line(&line).map(|progress| DownloadEvent::Progress {
                        download_id: id,
                        percent: progress.percent.unwrap_or(0.0),
                        speed: progress.speed,
                        eta: progress.eta,
                        downloaded_bytes: progress.downloaded_bytes.unwrap_or(0),
                        total_bytes: progress.total_bytes,
                        live: false,
                    })
                };
                // Throttle to 1 event per second
                if let Some(event) = event {
                    if now.duration_since(last_emit) >= Duration::from_secs(1) {
                        last_emit = now;
                        let _ = tx.send(event);
                    }
                }
            }
            output_file
        })
    });

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

    Ok(YtdlpDownload {
        child,
        download_id,
        stdout_handle,
        stderr_handle,
    })
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

    /// The file yt-dlp reported writing, available once the process has exited.
    pub async fn output_file(&mut self) -> Option<PathBuf> {
        self.stdout_handle.take()?.await.ok().flatten()
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

    #[test]
    fn title_override_keeps_backslashes_literal() {
        let args = title_override_args(Some(r" SN1 \1 TILL ITS DONE ")).expect("title given");
        assert_eq!(
            args,
            ["--replace-in-metadata", "title", "(?s)^.*$", r"SN1 \\1 TILL ITS DONE"]
        );
    }

    #[test]
    fn title_override_skipped_without_title() {
        assert!(title_override_args(None).is_none());
        assert!(title_override_args(Some("  ")).is_none());
    }

    #[test]
    fn output_file_marker_keeps_whole_path() {
        assert_eq!(
            parse_output_file_line("BR_OUTPUT_FILE /d/My = Title [1080p].mp4"),
            Some(PathBuf::from("/d/My = Title [1080p].mp4"))
        );
        assert_eq!(parse_output_file_line("100.0%|NA|NA|1|1"), None);
        assert_eq!(parse_output_file_line("total_size=5"), None);
        assert_eq!(parse_output_file_line("BR_OUTPUT_FILE "), None);
    }

    #[test]
    fn output_file_args_keep_progress() {
        // --print implies --quiet, which hides the progress template unless --progress is given.
        let args = output_file_args();
        assert!(args.contains(&"--progress"));
        assert!(args.contains(&"--print"));
        assert!(args.contains(&"after_move:BR_OUTPUT_FILE %(filepath)s"));
    }

    #[test]
    fn live_start_args_only_for_twitch_recording_playlists() {
        for url in [
            "https://d1m7jfoe9zdc1j.cloudfront.net/abc_quikkk_xo_322461668698_1795584189/chunked/index-dvr.m3u8",
            "https://vod-secure.twitch.tv/abc/720p60/index-dvr.m3u8?token=x",
        ] {
            assert!(live_twitch_recording_args(url).is_some(), "{url}");
        }
        for url in [
            "https://www.youtube.com/watch?v=abc",
            "https://example.com/live/index.m3u8",
            "https://example.com/watch?next=/index-dvr.m3u8",
        ] {
            assert!(live_twitch_recording_args(url).is_none(), "{url}");
        }
    }

    #[test]
    fn ffmpeg_progress_reports_bytes_once_per_block() {
        let mut progress = FfmpegProgress::default();
        let block = |p: &mut FfmpegProgress, size: &str| {
            [
                "frame=10",
                &format!("total_size={size}"),
                "out_time=00:00:01.000000",
                "speed=27.9x",
            ]
            .into_iter()
            .for_each(|line| assert_eq!(p.feed(line), None));
            p.feed("progress=continue")
        };
        assert_eq!(block(&mut progress, "N/A"), Some(0));
        assert_eq!(block(&mut progress, "8126464"), Some(8_126_464));
        assert_eq!(block(&mut progress, "N/A"), Some(8_126_464));
        assert_eq!(progress.feed("[download] Destination: a=b.mp4"), None);
        assert_eq!(
            progress.feed("100.0%|39.40MiB/s|NA|296224456|296224456"),
            None
        );
    }

    #[test]
    fn rate_window_smooths_segment_bursts() {
        // Following live: an 8 MiB segment lands every 10s, sampled every 0.5s.
        let start = Instant::now();
        let mut rate = RateWindow::default();
        let mut last = None;
        for tick in 0..240u64 {
            let bytes = (tick / 20 + 1) * 8 * 1024 * 1024;
            last = rate.push(start + Duration::from_millis(tick * 500), bytes);
            if tick >= 60 {
                let mib_per_sec = last.expect("rate after first sample") / (1024.0 * 1024.0);
                assert!(
                    (0.5..=1.2).contains(&mib_per_sec),
                    "tick {tick}: {mib_per_sec}"
                );
            }
        }
        assert!(last.is_some());
        assert_eq!(RateWindow::default().push(start, 100), None);
    }

    #[test]
    fn format_speed_matches_ytdlp_units() {
        assert_eq!(format_speed(512.0), "512.00B/s");
        assert_eq!(format_speed(800.0 * 1024.0), "800.00KiB/s");
        assert_eq!(format_speed(30.52 * 1024.0 * 1024.0), "30.52MiB/s");
    }

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
