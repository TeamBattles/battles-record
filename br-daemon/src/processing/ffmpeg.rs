use super::{ProcessingJob, ProcessingMode, ProcessingProgress};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/**
 * Input source for FFmpeg processing.
 *
 * Can be either a concat list (for multiple segment files) or a single file.
 */
pub enum InputSource {
    /** A concat list file containing paths to multiple .ts segments. */
    ConcatList(PathBuf),
    /** A single .ts file (concatenated segments or previous output). */
    SingleFile(PathBuf),
}

/** Executes FFmpeg commands with progress parsing. */
pub struct FfmpegRunner {
    ffmpeg_path: PathBuf,
}

impl FfmpegRunner {
    /**
     * Create a new FFmpeg runner.
     *
     * If `ffmpeg_path` is None, uses "ffmpeg" (PATH lookup).
     */
    pub fn new(ffmpeg_path: Option<PathBuf>) -> Self {
        Self {
            ffmpeg_path: ffmpeg_path.unwrap_or_else(|| PathBuf::from("ffmpeg")),
        }
    }

    /** Check if FFmpeg is available by running `ffmpeg -version`. */
    pub async fn check_available(&self) -> bool {
        match Command::new(&self.ffmpeg_path)
            .arg("-version")
            .output()
            .await
        {
            Ok(output) => output.status.success(),
            Err(e) => {
                warn!("FFmpeg not available: {}", e);
                false
            }
        }
    }

    /** Get the ffprobe path (same directory as ffmpeg, or just "ffprobe" for PATH lookup). */
    fn ffprobe_path(&self) -> PathBuf {
        if self.ffmpeg_path == PathBuf::from("ffmpeg") {
            PathBuf::from("ffprobe")
        } else {
            // Replace "ffmpeg" with "ffprobe" in the path
            let path_str = self.ffmpeg_path.to_string_lossy();
            PathBuf::from(path_str.replace("ffmpeg", "ffprobe"))
        }
    }

    /**
     * Probe a media file to get its duration in seconds.
     *
     * Uses ffprobe to extract duration from container metadata.
     * Returns None if ffprobe fails or duration cannot be determined.
     */
    pub async fn probe_duration(file_path: &Path) -> Option<u64> {
        // Use "ffprobe" from PATH - this is a static method so we don't have access to self
        let output = Command::new("ffprobe")
            .args([
                "-v", "error",
                "-show_entries", "format=duration",
                "-of", "default=noprint_wrappers=1:nokey=1",
            ])
            .arg(file_path)
            .output()
            .await
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let duration_str = stdout.trim();

        // Parse duration (can be float like "3661.234")
        duration_str.parse::<f64>().ok().map(|d| d.round() as u64)
    }

    /**
     * Check if a segment file is readable/valid using ffprobe.
     * Returns Ok(true) if valid, Ok(false) if invalid, Err if ffprobe failed to run.
     */
    async fn probe_segment(file_path: &Path) -> anyhow::Result<bool> {
        let output = Command::new("ffprobe")
            .args([
                "-v", "error",
                "-select_streams", "v:0",
                "-show_entries", "stream=codec_type",
                "-of", "csv=p=0",
            ])
            .arg(file_path)
            .output()
            .await?;

        // If ffprobe returns error (non-zero exit code), the file is likely corrupted
        Ok(output.status.success())
    }

    /** MPEG-TS sync byte (0x47) must appear at byte 0 and every 188 bytes. */
    const TS_SYNC_BYTE: u8 = 0x47;
    const TS_PACKET_SIZE: usize = 188;

    /**
     * Fast validation of TS file by checking sync bytes.
     * Returns Ok(true) if valid, Ok(false) if invalid/corrupted.
     * This is much faster than ffprobe (~1000x) as it only reads a few bytes.
     */
    async fn validate_ts_sync(file_path: &Path) -> anyhow::Result<bool> {
        use tokio::io::AsyncReadExt;

        let mut file = tokio::fs::File::open(file_path).await?;
        let mut buffer = [0u8; Self::TS_PACKET_SIZE * 3]; // Check first 3 packets

        let bytes_read = file.read(&mut buffer).await?;
        if bytes_read < Self::TS_PACKET_SIZE {
            return Ok(false); // Too small to be valid
        }

        // Check sync byte at offset 0
        if buffer[0] != Self::TS_SYNC_BYTE {
            return Ok(false);
        }

        // Check sync byte at offset 188 (if we have enough data)
        if bytes_read >= Self::TS_PACKET_SIZE * 2 && buffer[Self::TS_PACKET_SIZE] != Self::TS_SYNC_BYTE {
            return Ok(false);
        }

        // Check sync byte at offset 376 (if we have enough data)
        if bytes_read >= Self::TS_PACKET_SIZE * 3 && buffer[Self::TS_PACKET_SIZE * 2] != Self::TS_SYNC_BYTE {
            return Ok(false);
        }

        Ok(true)
    }

    /**
     * Minimum segment file size in bytes (10KB).
     * Files smaller than this are likely corrupted/incomplete and will be skipped.
     */
    const MIN_SEGMENT_SIZE: u64 = 10 * 1024;

    /**
     * Find input source for FFmpeg processing.
     *
     * Tries to find input in this order:
     * 1. Numbered .ts segment files (0000001.ts, 0000002.ts, etc.) -> generates concat list
     * 2. Non-numbered .ts file in recording directory (concatenated segments)
     * 3. Previous output file if it's a .ts file
     *
     * For fMP4/CMAF recordings (detected by presence of init.mp4), the init segment
     * is prepended to the concat list and TS sync byte validation is skipped.
     *
     * Returns the input source to use for FFmpeg.
     */
    pub async fn find_input_source(recording_path: &Path, output_file: Option<&Path>) -> anyhow::Result<InputSource> {
        let mut entries = tokio::fs::read_dir(recording_path).await?;
        let mut numbered_ts_files: Vec<(u64, PathBuf, u64)> = Vec::new(); // (seq, path, size)
        let mut other_ts_files: Vec<PathBuf> = Vec::new();

        // Check if this is an fMP4/CMAF recording (has init.mp4)
        let init_segment_path = recording_path.join("init.mp4");
        let is_fmp4 = init_segment_path.exists();

        if is_fmp4 {
            info!("Detected fMP4/CMAF recording (init.mp4 present)");
        }

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "ts" {
                    if let Some(stem) = path.file_stem() {
                        let stem_str = stem.to_string_lossy();
                        if let Ok(seq) = stem_str.parse::<u64>() {
                            // Numbered segment file - get its size
                            let size = tokio::fs::metadata(&path).await.map(|m| m.len()).unwrap_or(0);
                            numbered_ts_files.push((seq, path, size));
                        } else if !stem_str.starts_with("concat_list") {
                            // Non-numbered .ts file (likely concatenated segments)
                            other_ts_files.push(path);
                        }
                    }
                }
            }
        }

        // Priority 1: Use numbered segment files
        if !numbered_ts_files.is_empty() {
            // Sort by sequence number
            numbered_ts_files.sort_by_key(|(seq, _, _)| *seq);

            // Calculate median segment size to detect truncated segments
            let mut sizes: Vec<u64> = numbered_ts_files.iter().map(|(_, _, s)| *s).collect();
            sizes.sort();
            let median_size = sizes[sizes.len() / 2];
            // Segments less than 50% of median are considered truncated
            let min_valid_size = median_size / 2;

            // Filter out corrupted/incomplete segments with multi-layer validation:
            // 1. Size check (minimum 10KB)
            // 2. For MPEG-TS: TS sync byte validation (fast header check for ALL segments)
            // 3. For fMP4: skip sync byte check (segments don't have 0x47 sync bytes)
            let mut valid_segments: Vec<(u64, PathBuf, u64)> = Vec::new();
            for (seq, path, size) in numbered_ts_files {
                if size < Self::MIN_SEGMENT_SIZE {
                    warn!(
                        "Skipping tiny segment {:?} (seq={}, size={} bytes)",
                        path, seq, size
                    );
                    continue;
                }

                if is_fmp4 {
                    // For fMP4, skip TS sync byte validation (segments are fMP4 fragments)
                    valid_segments.push((seq, path, size));
                } else {
                    // For MPEG-TS, validate sync bytes
                    match Self::validate_ts_sync(&path).await {
                        Ok(true) => {
                            valid_segments.push((seq, path, size));
                        }
                        Ok(false) => {
                            warn!(
                                "Skipping corrupted segment {:?} (seq={}, invalid TS sync bytes)",
                                path, seq
                            );
                        }
                        Err(e) => {
                            warn!(
                                "Skipping unreadable segment {:?} (seq={}, error: {})",
                                path, seq, e
                            );
                        }
                    }
                }
            }

            // Check if the last segment is truncated (significantly smaller than median)
            // or corrupted (fails ffprobe validation)
            if valid_segments.len() > 1 {
                let should_skip_last = if let Some((seq, path, size)) = valid_segments.last() {
                    if *size < min_valid_size {
                        warn!(
                            "Skipping truncated last segment {:?} (seq={}, size={} bytes, median={})",
                            path, seq, size, median_size
                        );
                        true
                    } else {
                        // Check if last segment is readable with ffprobe
                        match Self::probe_segment(path).await {
                            Ok(true) => false, // Segment is valid
                            Ok(false) => {
                                warn!(
                                    "Skipping corrupted last segment {:?} (seq={}, failed ffprobe validation)",
                                    path, seq
                                );
                                true
                            }
                            Err(e) => {
                                warn!(
                                    "Skipping unreadable last segment {:?} (seq={}, error: {})",
                                    path, seq, e
                                );
                                true
                            }
                        }
                    }
                } else {
                    false
                };

                if should_skip_last {
                    valid_segments.pop();
                }
            }

            if valid_segments.is_empty() {
                anyhow::bail!("No valid segments found (all segments are corrupted/incomplete)");
            }

            // Generate concat list with valid segments only
            let mut content = String::new();

            // For fMP4/CMAF: prepend init segment to concat list
            if is_fmp4 {
                let init_abs_path = init_segment_path.canonicalize().unwrap_or_else(|_| init_segment_path.clone());
                let init_path_str = init_abs_path.to_string_lossy().replace('\'', "'\\''");
                content.push_str(&format!("file '{}'\n", init_path_str));
                info!("Including init segment in concat list: {:?}", init_abs_path);
            }

            for (_, path, _) in &valid_segments {
                let abs_path = path.canonicalize().unwrap_or_else(|_| path.clone());
                let path_str = abs_path.to_string_lossy().replace('\'', "'\\''");
                content.push_str(&format!("file '{}'\n", path_str));
            }

            let concat_list_path = recording_path.join("concat_list.txt");
            tokio::fs::write(&concat_list_path, &content).await?;

            info!(
                "Generated concat list with {}{} valid segments at {:?}",
                if is_fmp4 { "init + " } else { "" },
                valid_segments.len(),
                concat_list_path
            );

            return Ok(InputSource::ConcatList(concat_list_path));
        }

        // Priority 2: Use non-numbered .ts file in recording directory
        if !other_ts_files.is_empty() {
            // If multiple, use the largest one (likely the most complete)
            let mut largest_file = other_ts_files[0].clone();
            let mut largest_size = 0u64;

            for file in &other_ts_files {
                if let Ok(metadata) = tokio::fs::metadata(file).await {
                    if metadata.len() > largest_size {
                        largest_size = metadata.len();
                        largest_file = file.clone();
                    }
                }
            }

            info!(
                "Using concatenated .ts file as input: {:?} ({} bytes)",
                largest_file, largest_size
            );

            return Ok(InputSource::SingleFile(largest_file));
        }

        // Priority 3: Use previous output file if it's a .ts file
        if let Some(output) = output_file {
            if output.exists() {
                if let Some(ext) = output.extension() {
                    if ext == "ts" {
                        let metadata = tokio::fs::metadata(output).await?;
                        info!(
                            "Using previous output .ts file as input: {:?} ({} bytes)",
                            output, metadata.len()
                        );
                        return Ok(InputSource::SingleFile(output.to_path_buf()));
                    }
                }
            }
        }

        anyhow::bail!(
            "No input source found in {:?}. Expected numbered .ts segments, a concatenated .ts file, or a previous .ts output.",
            recording_path
        )
    }

    /**
     * Generate a concat list file for FFmpeg from .ts segments in a directory.
     *
     * Scans the directory for .ts files, sorts them by sequence number
     * (filename like 0000001.ts), and generates a concat_list.txt file.
     */
    pub async fn generate_concat_list(recording_path: &Path) -> anyhow::Result<PathBuf> {
        match Self::find_input_source(recording_path, None).await? {
            InputSource::ConcatList(path) => Ok(path),
            InputSource::SingleFile(path) => {
                // For backwards compatibility, generate a concat list with the single file
                let concat_list_path = recording_path.join("concat_list.txt");
                let abs_path = path.canonicalize().unwrap_or_else(|_| path.clone());
                let path_str = abs_path.to_string_lossy().replace('\'', "'\\''");
                let content = format!("file '{}'\n", path_str);
                tokio::fs::write(&concat_list_path, &content).await?;

                info!(
                    "Generated concat list with single file at {:?}",
                    concat_list_path
                );

                Ok(concat_list_path)
            }
        }
    }

    /** Build the FFmpeg command for the given job and concat list. */
    pub fn build_command(&self, job: &ProcessingJob, concat_list: &Path) -> Command {
        let mut cmd = Command::new(&self.ffmpeg_path);

        // Global options
        cmd.arg("-y"); // Overwrite output file

        // Input options
        cmd.arg("-f").arg("concat");
        cmd.arg("-safe").arg("0");
        cmd.arg("-i").arg(concat_list);

        // Codec options based on processing mode
        match &job.mode {
            ProcessingMode::Remux { .. } => {
                // Copy both streams without re-encoding
                cmd.arg("-c").arg("copy");
            }
            ProcessingMode::Transcode {
                codec,
                preset,
                crf,
                ..
            } => {
                // Video codec
                let video_codec = match codec.as_str() {
                    "h264" => "libx264",
                    "h265" | "hevc" => "libx265",
                    "av1" => "libaom-av1",
                    other => other, // Allow passing raw codec name
                };
                cmd.arg("-c:v").arg(video_codec);
                cmd.arg("-preset").arg(preset);
                cmd.arg("-crf").arg(crf.to_string());

                // Copy audio without re-encoding
                cmd.arg("-c:a").arg("copy");
            }
        }

        // Progress output to stdout
        cmd.arg("-progress").arg("pipe:1");

        // Output file
        cmd.arg(&job.output_path);

        // Redirect stderr for error capture
        cmd.stderr(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());

        cmd
    }

    /**
     * Run FFmpeg for the given job with progress reporting.
     *
     * The `total_duration_ms` parameter is used to calculate completion percentage.
     * If not provided, progress percentage will not be accurate.
     */
    pub async fn run(
        &self,
        job: &ProcessingJob,
        progress_tx: mpsc::Sender<ProcessingProgress>,
        total_duration_ms: Option<u64>,
    ) -> anyhow::Result<()> {
        // Generate concat list
        let concat_list = Self::generate_concat_list(&job.recording_path).await?;

        // Log diagnostic info about the concat list
        match tokio::fs::read_to_string(&concat_list).await {
            Ok(content) => {
                let line_count = content.lines().count();
                info!(
                    "Concat list contents ({} files):\n{}",
                    line_count,
                    if line_count <= 10 {
                        content.clone()
                    } else {
                        format!(
                            "{}\n... ({} more files) ...\n{}",
                            content.lines().take(5).collect::<Vec<_>>().join("\n"),
                            line_count - 10,
                            content.lines().rev().take(5).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n")
                        )
                    }
                );
            }
            Err(e) => {
                warn!("Failed to read concat list for logging: {}", e);
            }
        }

        // Build and spawn command
        let mut child = self
            .build_command(job, &concat_list)
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn FFmpeg: {}", e))?;

        info!("Started FFmpeg process for job {}", job.id);

        // Get stdout for progress parsing
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture FFmpeg stdout"))?;

        // Get stderr for error capture
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture FFmpeg stderr"))?;

        // Spawn stderr reader task
        let stderr_handle = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            let mut output = String::new();
            while let Ok(Some(line)) = lines.next_line().await {
                output.push_str(&line);
                output.push('\n');
            }
            output
        });

        // Parse progress from stdout
        self.parse_progress(stdout, progress_tx, total_duration_ms)
            .await;

        // Wait for completion
        let status = child.wait().await?;

        // Get stderr output
        let stderr_output = stderr_handle.await.unwrap_or_default();

        // Clean up concat list
        if let Err(e) = tokio::fs::remove_file(&concat_list).await {
            warn!("Failed to clean up concat list: {}", e);
        }

        // Check exit status
        if !status.success() {
            let exit_code = status.code().unwrap_or(-1);

            // Log the full stderr for debugging
            error!(
                "FFmpeg failed for job {} with exit code {}. Full stderr:\n{}",
                job.id, exit_code, stderr_output
            );

            // For the error message, show last 30 lines (skip the version banner at the start)
            let stderr_lines: Vec<&str> = stderr_output.lines().collect();
            let error_lines = if stderr_lines.len() > 30 {
                stderr_lines[stderr_lines.len() - 30..].join("\n")
            } else {
                stderr_lines.join("\n")
            };

            // Also check if there's a specific error pattern
            let error_summary = if let Some(error_line) = stderr_lines.iter().find(|l| {
                l.contains("No such file") ||
                l.contains("Invalid data") ||
                l.contains("Error") ||
                l.contains("error")
            }) {
                format!(" ({})", error_line.trim())
            } else {
                String::new()
            };

            anyhow::bail!(
                "FFmpeg exited with code {}{}: {}",
                exit_code,
                error_summary,
                error_lines
            );
        }

        // Verify output file exists and has content
        let output_metadata = tokio::fs::metadata(&job.output_path).await.map_err(|e| {
            anyhow::anyhow!(
                "Output file {:?} not found after processing: {}",
                job.output_path,
                e
            )
        })?;

        if output_metadata.len() == 0 {
            anyhow::bail!("Output file {:?} is empty", job.output_path);
        }

        info!(
            "FFmpeg completed successfully for job {}: {:?} ({} bytes)",
            job.id,
            job.output_path,
            output_metadata.len()
        );

        Ok(())
    }

    /**
     * Parse FFmpeg progress output from stdout.
     *
     * FFmpeg progress format with `-progress pipe:1`:
     * ```text
     * out_time_ms=60000000    (microseconds)
     * speed=2.5x
     * progress=continue
     * ```
     */
    async fn parse_progress(
        &self,
        stdout: tokio::process::ChildStdout,
        progress_tx: mpsc::Sender<ProcessingProgress>,
        total_duration_ms: Option<u64>,
    ) {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        let mut current_out_time_ms: u64 = 0;
        let mut current_speed: Option<String> = None;

        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim();

            if let Some(value) = line.strip_prefix("out_time_ms=") {
                // out_time_ms is in microseconds, convert to milliseconds
                if let Ok(us) = value.parse::<i64>() {
                    // Can be negative during initialization, clamp to 0
                    current_out_time_ms = (us.max(0) / 1000) as u64;
                }
            } else if let Some(value) = line.strip_prefix("speed=") {
                let speed = value.trim();
                if speed != "N/A" && !speed.is_empty() {
                    current_speed = Some(speed.to_string());
                }
            } else if line.starts_with("progress=") {
                // On progress=continue or progress=end, emit a progress update
                let percent = if let Some(total) = total_duration_ms {
                    if total > 0 {
                        ((current_out_time_ms * 100) / total).min(100) as u8
                    } else {
                        0
                    }
                } else {
                    // Without total duration, we can't calculate percentage
                    0
                };

                let progress = ProcessingProgress {
                    percent,
                    speed: current_speed.clone(),
                    out_time_ms: current_out_time_ms,
                };

                debug!(
                    "FFmpeg progress: {}% at {} ({}ms)",
                    percent,
                    current_speed.as_deref().unwrap_or("N/A"),
                    current_out_time_ms
                );

                if progress_tx.send(progress).await.is_err() {
                    // Receiver dropped, stop parsing
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// Create a fake TS file with valid sync bytes for testing.
    /// Creates a file >= MIN_SEGMENT_SIZE with 0x47 at correct offsets.
    fn create_fake_ts_file(path: &Path) {
        let mut file = std::fs::File::create(path).unwrap();
        // Create buffer with at least MIN_SEGMENT_SIZE bytes
        let size = FfmpegRunner::MIN_SEGMENT_SIZE as usize + 1000;
        let mut buffer = vec![0u8; size];
        // Set sync bytes (0x47) at every 188-byte boundary
        for i in (0..size).step_by(FfmpegRunner::TS_PACKET_SIZE) {
            buffer[i] = FfmpegRunner::TS_SYNC_BYTE;
        }
        file.write_all(&buffer).unwrap();
    }

    #[tokio::test]
    async fn test_generate_concat_list() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        // Create some test .ts files with valid TS headers
        for seq in [1, 5, 3, 2, 4] {
            let filename = format!("{:07}.ts", seq);
            let file_path = dir_path.join(&filename);
            create_fake_ts_file(&file_path);
        }

        // Create a non-.ts file that should be ignored
        std::fs::File::create(dir_path.join("state.json"))
            .unwrap()
            .write_all(b"{}")
            .unwrap();

        let concat_list = FfmpegRunner::generate_concat_list(dir_path)
            .await
            .unwrap();

        assert!(concat_list.exists());

        let content = std::fs::read_to_string(&concat_list).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        // We expect 4 files, not 5, because the last segment (seq 5) gets filtered out
        // by ffprobe validation (our fake TS files don't have valid video streams)
        assert_eq!(lines.len(), 4);

        // Verify files are sorted by sequence number (1-4, since 5 was filtered)
        for (i, line) in lines.iter().enumerate() {
            let expected_seq = i + 1;
            let expected_filename = format!("{:07}.ts", expected_seq);
            assert!(
                line.contains(&expected_filename),
                "Line {} should contain {}, got: {}",
                i,
                expected_filename,
                line
            );
        }
    }

    #[tokio::test]
    async fn test_generate_concat_list_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let result = FfmpegRunner::generate_concat_list(temp_dir.path()).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_build_command_remux() {
        let runner = FfmpegRunner::new(None);
        let job = create_test_job(ProcessingMode::Remux {
            format: "mp4".to_string(),
        });
        let concat_list = PathBuf::from("/tmp/concat_list.txt");

        let cmd = runner.build_command(&job, &concat_list);
        let args: Vec<_> = cmd.as_std().get_args().collect();

        // Check essential arguments are present
        assert!(args.contains(&&std::ffi::OsStr::new("-c")));
        assert!(args.contains(&&std::ffi::OsStr::new("copy")));
        assert!(args.contains(&&std::ffi::OsStr::new("-f")));
        assert!(args.contains(&&std::ffi::OsStr::new("concat")));
    }

    #[test]
    fn test_build_command_transcode_h264() {
        let runner = FfmpegRunner::new(None);
        let job = create_test_job(ProcessingMode::Transcode {
            format: "mp4".to_string(),
            codec: "h264".to_string(),
            preset: "medium".to_string(),
            crf: 23,
        });
        let concat_list = PathBuf::from("/tmp/concat_list.txt");

        let cmd = runner.build_command(&job, &concat_list);
        let args: Vec<_> = cmd.as_std().get_args().collect();

        assert!(args.contains(&&std::ffi::OsStr::new("libx264")));
        assert!(args.contains(&&std::ffi::OsStr::new("-preset")));
        assert!(args.contains(&&std::ffi::OsStr::new("medium")));
        assert!(args.contains(&&std::ffi::OsStr::new("-crf")));
        assert!(args.contains(&&std::ffi::OsStr::new("23")));
    }

    #[test]
    fn test_build_command_transcode_h265() {
        let runner = FfmpegRunner::new(None);
        let job = create_test_job(ProcessingMode::Transcode {
            format: "mkv".to_string(),
            codec: "h265".to_string(),
            preset: "slow".to_string(),
            crf: 28,
        });
        let concat_list = PathBuf::from("/tmp/concat_list.txt");

        let cmd = runner.build_command(&job, &concat_list);
        let args: Vec<_> = cmd.as_std().get_args().collect();

        assert!(args.contains(&&std::ffi::OsStr::new("libx265")));
    }

    #[test]
    fn test_build_command_transcode_av1() {
        let runner = FfmpegRunner::new(None);
        let job = create_test_job(ProcessingMode::Transcode {
            format: "mp4".to_string(),
            codec: "av1".to_string(),
            preset: "4".to_string(),
            crf: 30,
        });
        let concat_list = PathBuf::from("/tmp/concat_list.txt");

        let cmd = runner.build_command(&job, &concat_list);
        let args: Vec<_> = cmd.as_std().get_args().collect();

        assert!(args.contains(&&std::ffi::OsStr::new("libaom-av1")));
    }

    fn create_test_job(mode: ProcessingMode) -> ProcessingJob {
        ProcessingJob::new(
            uuid::Uuid::new_v4(),
            "test_channel".to_string(),
            "twitch".to_string(),
            PathBuf::from("/tmp/recording"),
            mode,
            crate::config::SegmentHandling::Delete,
            None,
        )
    }
}
