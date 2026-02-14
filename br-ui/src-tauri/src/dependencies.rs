use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Serialize, Deserialize)]
pub struct DependencyStatus {
    pub bun_available: bool,
    pub bun_version: Option<String>,
    pub ytdlp_available: bool,
    pub ytdlp_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallProgress {
    pub dependency: String,
    pub status: String,
    pub progress: Option<f32>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InstallResult {
    pub bun_installed: bool,
    pub ytdlp_installed: bool,
    pub errors: Vec<String>,
}

fn get_bin_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    Ok(data_dir.join("bin"))
}

fn get_dependency_path(app: &AppHandle, name: &str) -> Option<PathBuf> {
    if let Ok(bin_dir) = get_bin_dir(app) {
        #[cfg(target_os = "windows")]
        let binary_name = format!("{}.exe", name);
        #[cfg(not(target_os = "windows"))]
        let binary_name = name.to_string();

        let app_path = bin_dir.join(&binary_name);
        if app_path.exists() {
            return Some(app_path);
        }
    }
    which::which(name).ok()
}

fn check_dependency(app: &AppHandle, name: &str) -> (bool, Option<String>) {
    let path = match get_dependency_path(app, name) {
        Some(p) => p,
        None => return (false, None),
    };

    let output = std::process::Command::new(&path).arg("--version").output();

    match output {
        Ok(o) if o.status.success() => {
            let version = String::from_utf8_lossy(&o.stdout).trim().to_string();
            (true, Some(version))
        }
        _ => (false, None),
    }
}

#[tauri::command]
pub fn check_youtube_dependencies(app: AppHandle) -> DependencyStatus {
    let (bun_available, bun_version) = check_dependency(&app, "bun");
    let (ytdlp_available, ytdlp_version) = check_dependency(&app, "yt-dlp");

    DependencyStatus {
        bun_available,
        bun_version,
        ytdlp_available,
        ytdlp_version,
    }
}

#[tauri::command]
pub async fn install_youtube_dependencies(
    app: AppHandle,
    install_bun: bool,
    install_ytdlp: bool,
) -> Result<InstallResult, String> {
    let bin_dir = get_bin_dir(&app)?;
    std::fs::create_dir_all(&bin_dir)
        .map_err(|e| format!("Failed to create bin directory: {}", e))?;

    let mut errors = Vec::new();
    let mut bun_installed = false;
    let mut ytdlp_installed = false;

    if install_bun {
        match install_bun_impl(&app, &bin_dir).await {
            Ok(_) => bun_installed = true,
            Err(e) => errors.push(format!("Bun: {}", e)),
        }
    }

    if install_ytdlp {
        match install_ytdlp_impl(&app, &bin_dir).await {
            Ok(_) => ytdlp_installed = true,
            Err(e) => errors.push(format!("yt-dlp: {}", e)),
        }
    }

    Ok(InstallResult {
        bun_installed,
        ytdlp_installed,
        errors,
    })
}

async fn install_bun_impl(app: &AppHandle, bin_dir: &PathBuf) -> Result<(), String> {
    let _ = app.emit(
        "dependency-progress",
        InstallProgress {
            dependency: "bun".to_string(),
            status: "downloading".to_string(),
            progress: Some(0.0),
            error: None,
        },
    );

    let asset_name = get_bun_asset_name()?;
    let download_url = format!(
        "https://github.com/oven-sh/bun/releases/latest/download/{}",
        asset_name
    );

    let response = reqwest::get(&download_url)
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Download failed: HTTP {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    let _ = app.emit(
        "dependency-progress",
        InstallProgress {
            dependency: "bun".to_string(),
            status: "extracting".to_string(),
            progress: Some(50.0),
            error: None,
        },
    );

    let cursor = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Failed to open zip: {}", e))?;

    #[cfg(target_os = "windows")]
    let binary_name = "bun.exe";
    #[cfg(not(target_os = "windows"))]
    let binary_name = "bun";

    let mut found = false;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        if file.name().ends_with(binary_name) && !file.name().contains("bundler") {
            let outpath = bin_dir.join(binary_name);
            let mut outfile = std::fs::File::create(&outpath)
                .map_err(|e| format!("Failed to create file: {}", e))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to write file: {}", e))?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(0o755))
                    .map_err(|e| format!("Failed to set permissions: {}", e))?;
            }

            found = true;
            break;
        }
    }

    if !found {
        return Err("Bun binary not found in archive".to_string());
    }

    let _ = app.emit(
        "dependency-progress",
        InstallProgress {
            dependency: "bun".to_string(),
            status: "complete".to_string(),
            progress: Some(100.0),
            error: None,
        },
    );

    Ok(())
}

async fn install_ytdlp_impl(app: &AppHandle, bin_dir: &PathBuf) -> Result<(), String> {
    let _ = app.emit(
        "dependency-progress",
        InstallProgress {
            dependency: "ytdlp".to_string(),
            status: "downloading".to_string(),
            progress: Some(0.0),
            error: None,
        },
    );

    let (asset_name, binary_name) = get_ytdlp_asset_name()?;
    let download_url = format!(
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/{}",
        asset_name
    );

    let response = reqwest::get(&download_url)
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Download failed: HTTP {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    let _ = app.emit(
        "dependency-progress",
        InstallProgress {
            dependency: "ytdlp".to_string(),
            status: "extracting".to_string(),
            progress: Some(50.0),
            error: None,
        },
    );

    let outpath = bin_dir.join(binary_name);
    std::fs::write(&outpath, &bytes).map_err(|e| format!("Failed to write file: {}", e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    let _ = app.emit(
        "dependency-progress",
        InstallProgress {
            dependency: "ytdlp".to_string(),
            status: "complete".to_string(),
            progress: Some(100.0),
            error: None,
        },
    );

    Ok(())
}

fn get_bun_asset_name() -> Result<&'static str, String> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return Ok("bun-windows-x64.zip");

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return Ok("bun-darwin-x64.zip");

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return Ok("bun-darwin-aarch64.zip");

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return Ok("bun-linux-x64.zip");

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return Ok("bun-linux-aarch64.zip");

    #[allow(unreachable_code)]
    Err("Unsupported platform".to_string())
}

fn get_ytdlp_asset_name() -> Result<(&'static str, &'static str), String> {
    #[cfg(target_os = "windows")]
    return Ok(("yt-dlp.exe", "yt-dlp.exe"));

    #[cfg(target_os = "macos")]
    return Ok(("yt-dlp_macos", "yt-dlp"));

    #[cfg(target_os = "linux")]
    return Ok(("yt-dlp_linux", "yt-dlp"));

    #[allow(unreachable_code)]
    Err("Unsupported platform".to_string())
}
