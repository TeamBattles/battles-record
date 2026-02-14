use std::collections::VecDeque;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::async_runtime::spawn;
use tauri::Manager;
use tauri_plugin_shell::process::CommandChild;
use tauri_plugin_shell::ShellExt;

#[derive(Debug, Clone, serde::Serialize)]
pub enum DaemonStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
}

pub struct DaemonState {
    pub status: DaemonStatus,
    pub port: Option<u16>,
    pub process: Option<CommandChild>,
    pub logs: VecDeque<String>,
    pub config_path: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
    pub library_dir: Option<PathBuf>,
}

impl Default for DaemonState {
    fn default() -> Self {
        Self {
            status: DaemonStatus::Stopped,
            port: None,
            process: None,
            logs: VecDeque::new(),
            config_path: None,
            data_dir: None,
            library_dir: None,
        }
    }
}

pub type DaemonStateMutex = Arc<Mutex<DaemonState>>;

/// Validate a path to prevent directory traversal attacks.
/// Returns an error if the path contains suspicious patterns.
fn validate_path(path: &str) -> Result<(), String> {
    let p = std::path::Path::new(path);

    // Check for path traversal patterns
    for component in p.components() {
        match component {
            std::path::Component::ParentDir => {
                return Err("Path cannot contain '..' (parent directory traversal)".to_string());
            }
            std::path::Component::Normal(s) => {
                let s_str = s.to_string_lossy();
                // Reject suspicious patterns
                if s_str.starts_with('.') && s_str != "." {
                    // Allow hidden directories but be cautious
                    log::warn!("Path contains hidden component: {}", s_str);
                }
            }
            _ => {}
        }
    }

    // Ensure the path is not empty
    if path.is_empty() {
        return Err("Path cannot be empty".to_string());
    }

    Ok(())
}

/// Find an available port starting from base_port
fn find_available_port(base_port: u16) -> Option<u16> {
    for port in base_port..base_port + 100 {
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Some(port);
        }
    }
    None
}

/// Check if daemon is responding on the given port
async fn health_check(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{}/health", port);
    match reqwest::get(&url).await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

#[tauri::command]
pub async fn start_local_daemon(
    app: tauri::AppHandle,
    state: tauri::State<'_, DaemonStateMutex>,
    data_dir: Option<String>,
    library_dir: Option<String>,
) -> Result<u16, String> {
    // Check if already running
    {
        let daemon = state.lock().map_err(|e| e.to_string())?;
        if let DaemonStatus::Running = daemon.status {
            if let Some(port) = daemon.port {
                return Ok(port);
            }
        }
    }

    // Get app data directory for config and recordings
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    // Create app data directory if it doesn't exist
    tokio::fs::create_dir_all(&app_data_dir)
        .await
        .map_err(|e| format!("Failed to create app data dir: {}", e))?;

    // Use custom paths if provided or already set, otherwise use defaults
    let (config_path, resolved_data_dir, resolved_library_dir) = {
        let daemon = state.lock().map_err(|e| e.to_string())?;
        let config = daemon.config_path.clone().unwrap_or_else(|| app_data_dir.join("br-config.toml"));
        // Prefer passed data_dir parameter, then state's data_dir, then default
        let data = data_dir.map(PathBuf::from)
            .or_else(|| daemon.data_dir.clone())
            .unwrap_or_else(|| app_data_dir.join("recordings"));
        // Prefer passed library_dir parameter, then state's library_dir, then None (uses daemon default)
        let library = library_dir.map(PathBuf::from)
            .or_else(|| daemon.library_dir.clone());
        (config, data, library)
    };

    // Create recordings directory if it doesn't exist
    tokio::fs::create_dir_all(&resolved_data_dir)
        .await
        .map_err(|e| format!("Failed to create recordings dir: {}", e))?;

    // Create library directory if specified and doesn't exist
    if let Some(ref lib_dir) = resolved_library_dir {
        tokio::fs::create_dir_all(lib_dir)
            .await
            .map_err(|e| format!("Failed to create library dir: {}", e))?;
    }

    log::info!("Config path: {:?}", config_path);
    log::info!("Data directory: {:?}", resolved_data_dir);
    log::info!("Library directory: {:?}", resolved_library_dir);

    // Find available port
    let port = find_available_port(8080).ok_or("No available port found")?;

    // Update status to starting
    {
        let mut daemon = state.lock().map_err(|e| e.to_string())?;
        daemon.status = DaemonStatus::Starting;
        daemon.port = Some(port);
        daemon.config_path = Some(config_path.clone());
        daemon.data_dir = Some(resolved_data_dir.clone());
        daemon.library_dir = resolved_library_dir.clone();
    }

    // Build args
    let mut args = vec![
        "--port".to_string(), port.to_string(),
        "--config".to_string(), config_path.to_string_lossy().to_string(),
        "--data-dir".to_string(), resolved_data_dir.to_string_lossy().to_string(),
        "--local-only".to_string(),
    ];

    // Add library-dir if specified and different from data_dir
    if let Some(ref lib_dir) = resolved_library_dir {
        if lib_dir != &resolved_data_dir {
            args.push("--library-dir".to_string());
            args.push(lib_dir.to_string_lossy().to_string());
        }
    }

    // Spawn the sidecar with config, data-dir, and local-only flags
    let shell = app.shell();
    let command = shell
        .sidecar("br-daemon")
        .map_err(|e| format!("Failed to create sidecar command: {}", e))?
        .args(&args);

    let (mut rx, child) = command
        .spawn()
        .map_err(|e| format!("Failed to spawn daemon: {}", e))?;

    // Store the process handle
    {
        let mut daemon = state.lock().map_err(|e| e.to_string())?;
        daemon.process = Some(child);
    }

    // Clone state for the log capture task
    let state_clone = Arc::clone(state.inner());

    // Spawn task to read daemon output (prevents buffer deadlock)
    spawn(async move {
        const MAX_LOGS: usize = 500;

        while let Some(event) = rx.recv().await {
            match event {
                tauri_plugin_shell::process::CommandEvent::Stdout(line) => {
                    let line_str = String::from_utf8_lossy(&line).trim().to_string();
                    log::info!("[br-daemon] {}", line_str);
                    if let Ok(mut daemon) = state_clone.lock() {
                        daemon.logs.push_back(format!("[OUT] {}", line_str));
                        if daemon.logs.len() > MAX_LOGS {
                            daemon.logs.pop_front();
                        }
                    }
                }
                tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                    let line_str = String::from_utf8_lossy(&line).trim().to_string();
                    log::warn!("[br-daemon] {}", line_str);
                    if let Ok(mut daemon) = state_clone.lock() {
                        daemon.logs.push_back(format!("[ERR] {}", line_str));
                        if daemon.logs.len() > MAX_LOGS {
                            daemon.logs.pop_front();
                        }
                    }
                }
                tauri_plugin_shell::process::CommandEvent::Terminated(payload) => {
                    log::info!("[br-daemon] terminated with code: {:?}", payload.code);
                    if let Ok(mut daemon) = state_clone.lock() {
                        daemon.logs.push_back(format!("[SYS] Process terminated with code: {:?}", payload.code));
                    }
                    break;
                }
                _ => {}
            }
        }
    });

    // Wait for daemon to be ready (health check)
    let max_attempts = 30;
    for attempt in 0..max_attempts {
        tokio::time::sleep(Duration::from_millis(200)).await;
        if health_check(port).await {
            {
                let mut daemon = state.lock().map_err(|e| e.to_string())?;
                daemon.status = DaemonStatus::Running;
                log::info!("br-daemon started on port {}", port);
            }
            // Update tray status
            crate::tray::update_tray_status(&app);
            return Ok(port);
        }
        log::debug!("Health check attempt {}/{}", attempt + 1, max_attempts);
    }

    // Timeout - kill process and return error
    {
        let mut daemon = state.lock().map_err(|e| e.to_string())?;
        if let Some(child) = daemon.process.take() {
            let _ = child.kill();
        }
        daemon.status = DaemonStatus::Stopped;
        daemon.port = None;
    }

    Err("Daemon failed to start within timeout".to_string())
}

#[tauri::command]
pub async fn stop_local_daemon(
    app: tauri::AppHandle,
    state: tauri::State<'_, DaemonStateMutex>,
) -> Result<(), String> {
    let port = {
        let daemon = state.lock().map_err(|e| e.to_string())?;
        daemon.port
    };

    // Try graceful shutdown via API first
    if let Some(port) = port {
        log::info!("Requesting graceful shutdown via API on port {}", port);

        let client = reqwest::Client::new();
        let shutdown_result = client
            .post(format!("http://localhost:{}/api/shutdown", port))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await;

        if shutdown_result.is_ok() {
            log::info!("Shutdown request sent, waiting for daemon to exit gracefully...");
            // Give the daemon time to gracefully stop recordings (up to 5 seconds)
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        } else {
            log::warn!("Failed to send shutdown request: {:?}", shutdown_result.err());
        }
    }

    // Clean up the process handle (kill if still running)
    {
        let mut daemon = state.lock().map_err(|e| e.to_string())?;

        if let Some(child) = daemon.process.take() {
            daemon.status = DaemonStatus::Stopping;
            // Kill will fail silently if process already exited
            if let Err(e) = child.kill() {
                log::debug!("Kill returned error (process may have already exited): {}", e);
            }
        }

        daemon.status = DaemonStatus::Stopped;
        daemon.port = None;
        log::info!("br-daemon stopped");
    }

    // Update tray status
    crate::tray::update_tray_status(&app);

    Ok(())
}

#[tauri::command]
pub async fn restart_local_daemon(
    app: tauri::AppHandle,
    state: tauri::State<'_, DaemonStateMutex>,
) -> Result<u16, String> {
    // Stop if running
    stop_local_daemon(app.clone(), state.clone()).await?;

    // Brief delay to ensure clean shutdown
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Start again with existing paths (None means use state's stored values)
    start_local_daemon(app, state, None, None).await
}

#[tauri::command]
pub fn get_daemon_port(state: tauri::State<'_, DaemonStateMutex>) -> Option<u16> {
    let daemon = state.lock().ok()?;
    if matches!(daemon.status, DaemonStatus::Running) {
        daemon.port
    } else {
        None
    }
}

#[tauri::command]
pub fn is_daemon_running(state: tauri::State<'_, DaemonStateMutex>) -> bool {
    state
        .lock()
        .map(|d| matches!(d.status, DaemonStatus::Running))
        .unwrap_or(false)
}

#[tauri::command]
pub fn get_daemon_status(
    state: tauri::State<'_, DaemonStateMutex>,
) -> Result<(DaemonStatus, Option<u16>), String> {
    let daemon = state.lock().map_err(|e| e.to_string())?;
    Ok((daemon.status.clone(), daemon.port))
}

#[tauri::command]
pub fn get_daemon_logs(state: tauri::State<'_, DaemonStateMutex>) -> Vec<String> {
    state
        .lock()
        .map(|d| d.logs.iter().cloned().collect())
        .unwrap_or_default()
}

/// Get the local daemon's config and data paths
#[tauri::command]
pub fn get_local_daemon_paths(
    app: tauri::AppHandle,
    state: tauri::State<'_, DaemonStateMutex>,
) -> Result<LocalDaemonPaths, String> {
    // Try to get from state first (if daemon is running)
    if let Ok(daemon) = state.lock() {
        if daemon.config_path.is_some() || daemon.data_dir.is_some() {
            return Ok(LocalDaemonPaths {
                config_path: daemon.config_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                data_dir: daemon.data_dir.as_ref().map(|p| p.to_string_lossy().to_string()),
                library_dir: daemon.library_dir.as_ref().map(|p| p.to_string_lossy().to_string()),
            });
        }
    }

    // Fall back to computing the default paths
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    Ok(LocalDaemonPaths {
        config_path: Some(app_data_dir.join("br-config.toml").to_string_lossy().to_string()),
        data_dir: Some(app_data_dir.join("recordings").to_string_lossy().to_string()),
        library_dir: None, // Default: same as data_dir (daemon will use data_dir)
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LocalDaemonPaths {
    pub config_path: Option<String>,
    pub data_dir: Option<String>,
    pub library_dir: Option<String>,
}

/// Set custom paths for the local daemon and restart it
#[tauri::command]
pub async fn set_local_daemon_paths(
    app: tauri::AppHandle,
    state: tauri::State<'_, DaemonStateMutex>,
    config_path: Option<String>,
    data_dir: Option<String>,
    library_dir: Option<String>,
) -> Result<u16, String> {
    // Validate paths to prevent directory traversal attacks
    if let Some(ref path) = config_path {
        validate_path(path)?;
    }
    if let Some(ref path) = data_dir {
        validate_path(path)?;
    }
    if let Some(ref path) = library_dir {
        validate_path(path)?;
    }

    // Validate data_dir exists or create it
    if let Some(ref dir) = data_dir {
        let path = std::path::Path::new(dir);
        if !path.exists() {
            tokio::fs::create_dir_all(path)
                .await
                .map_err(|e| format!("Cannot create directory: {}", e))?;
        }
    }

    // Validate library_dir exists or create it
    if let Some(ref dir) = library_dir {
        let path = std::path::Path::new(dir);
        if !path.exists() {
            tokio::fs::create_dir_all(path)
                .await
                .map_err(|e| format!("Cannot create library directory: {}", e))?;
        }
    }

    // Validate config_path parent directory exists
    if let Some(ref path) = config_path {
        let p = std::path::Path::new(path);
        if let Some(parent) = p.parent() {
            if !parent.exists() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| format!("Cannot create config directory: {}", e))?;
            }
        }
    }

    // Update daemon state with new paths
    {
        let mut daemon = state.lock().map_err(|e| e.to_string())?;
        if let Some(path) = config_path {
            daemon.config_path = Some(PathBuf::from(path));
        }
        if let Some(dir) = data_dir {
            daemon.data_dir = Some(PathBuf::from(dir));
        }
        if let Some(dir) = library_dir {
            daemon.library_dir = Some(PathBuf::from(dir));
        }
    }

    // Restart daemon with new paths
    restart_local_daemon(app, state).await
}
