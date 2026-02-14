use crate::api::auth::AuthUser;
use crate::api::response::ApiResponse;
use crate::api::AppState;
use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

/// Minimum client version required to connect to this daemon.
/// Bump when making breaking API changes that older clients can't handle.
const MIN_CLIENT_VERSION: &str = "1.0.0";

/// Maximum client version this daemon supports.
/// Bump when newer clients rely on features this daemon doesn't have.
const MAX_CLIENT_VERSION: &str = "1.99.99";

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub version: String,
    pub uptime_secs: u64,
    pub min_client_version: String,
    pub max_client_version: String,
    pub update: Option<UpdateInfo>,
    pub disk: DiskStatus,
    pub channels: ChannelStats,
    pub processing_queue: ProcessingQueueStatus,
}

#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub release_notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DiskStatus {
    pub recordings_path: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub usage_percent: f32,
}

#[derive(Debug, Serialize)]
pub struct ChannelStats {
    pub total: usize,
    pub enabled: usize,
    pub recording: usize,
    pub live_not_recording: usize,
}

#[derive(Debug, Serialize)]
pub struct ProcessingQueueStatus {
    pub active: Option<String>,
    pub active_progress_percent: Option<u8>,
    pub queued: usize,
}

pub async fn get_status(
    _auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<StatusResponse>> {
    let config = state.config.read();
    let channels = state.channel_manager.get_channels();

    let uptime_secs = state.started_at.elapsed().as_secs();

    let recordings_path = config.storage.recordings_dir.to_string_lossy().to_string();
    let (total_bytes, used_bytes, usage_percent) = get_disk_usage(&config.storage.recordings_dir);

    let total = channels.len();
    let enabled = channels.iter().filter(|c| c.enabled).count();
    let recording = channels
        .iter()
        .filter(|c| c.status == crate::types::ChannelStatus::Recording)
        .count();
    let live_not_recording = channels
        .iter()
        .filter(|c| c.status == crate::types::ChannelStatus::Live)
        .count();

    let update = {
        let info = state.version_checker.get_info();
        if info.last_check.is_some() {
            Some(UpdateInfo {
                latest_version: info.latest_version,
                update_available: info.update_available,
                release_url: info.release_url,
                release_notes: info.release_notes,
            })
        } else {
            None
        }
    };

    Json(ApiResponse::new(StatusResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_secs,
        min_client_version: MIN_CLIENT_VERSION.to_string(),
        max_client_version: MAX_CLIENT_VERSION.to_string(),
        update,
        disk: DiskStatus {
            recordings_path,
            total_bytes,
            used_bytes,
            usage_percent,
        },
        channels: ChannelStats {
            total,
            enabled,
            recording,
            live_not_recording,
        },
        processing_queue: ProcessingQueueStatus {
            active: None,
            active_progress_percent: None,
            queued: 0,
        },
    }))
}

#[cfg(windows)]
fn get_disk_usage(path: &std::path::Path) -> (u64, u64, f32) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let path_str = path.to_string_lossy();
    let root = if path_str.len() >= 2 && path_str.chars().nth(1) == Some(':') {
        format!("{}\\", &path_str[..2])
    } else {
        "C:\\".to_string()
    };

    let wide: Vec<u16> = OsStr::new(&root)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut free_bytes: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut _total_free: u64 = 0;

    unsafe {
        if windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_bytes as *mut u64,
            &mut total_bytes as *mut u64,
            &mut _total_free as *mut u64,
        ) != 0
        {
            let used = total_bytes.saturating_sub(free_bytes);
            let percent = if total_bytes > 0 {
                (used as f64 / total_bytes as f64 * 100.0) as f32
            } else {
                0.0
            };
            return (total_bytes, used, percent);
        }
    }
    (0, 0, 0.0)
}

#[cfg(not(windows))]
fn get_disk_usage(path: &std::path::Path) -> (u64, u64, f32) {
    use std::ffi::CString;
    use std::mem::MaybeUninit;

    let path_cstr = match CString::new(path.to_string_lossy().as_bytes()) {
        Ok(s) => s,
        Err(_) => return (0, 0, 0.0),
    };

    let mut stat: MaybeUninit<libc::statvfs> = MaybeUninit::uninit();

    unsafe {
        if libc::statvfs(path_cstr.as_ptr(), stat.as_mut_ptr()) == 0 {
            let stat = stat.assume_init();
            let total = stat.f_blocks as u64 * stat.f_frsize as u64;
            let free = stat.f_bfree as u64 * stat.f_frsize as u64;
            let used = total.saturating_sub(free);
            let percent = if total > 0 {
                (used as f64 / total as f64 * 100.0) as f32
            } else {
                0.0
            };
            return (total, used, percent);
        }
    }
    (0, 0, 0.0)
}
