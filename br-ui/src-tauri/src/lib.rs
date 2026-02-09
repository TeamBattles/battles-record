mod daemon;
mod dependencies;
mod tray;
mod youtube_bundle;

use daemon::{DaemonState, DaemonStateMutex};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, RunEvent};
use tauri_plugin_window_state::StateFlags;
#[cfg(desktop)]
use tauri_plugin_deep_link::DeepLinkExt;

#[tauri::command]
fn show_in_folder(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .args(["/select,", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(std::path::Path::new(&path).parent().unwrap_or(std::path::Path::new(&path)))
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_oauth::init())
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(StateFlags::all() & !StateFlags::VISIBLE)
                .build(),
        )
        .manage(Arc::new(Mutex::new(DaemonState::default())) as DaemonStateMutex)
        .invoke_handler(tauri::generate_handler![
            daemon::start_local_daemon,
            daemon::stop_local_daemon,
            daemon::restart_local_daemon,
            daemon::get_daemon_port,
            daemon::is_daemon_running,
            daemon::get_daemon_status,
            daemon::get_daemon_logs,
            daemon::get_local_daemon_paths,
            daemon::set_local_daemon_paths,
            dependencies::check_youtube_dependencies,
            dependencies::install_youtube_dependencies,
            show_in_folder,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Ensure window starts hidden and has no decorations
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
                // Programmatically remove decorations (backup for config)
                let _ = window.set_decorations(false);
            }

            // Extract bundled YouTube deps if present (YouTube-bundled variant)
            youtube_bundle::extract_youtube_deps(&app.handle());

            // Setup system tray
            tray::setup_tray(app)?;

            // Setup deep link handler for OAuth callbacks
            #[cfg(desktop)]
            {
                let handle = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    let urls = event.urls();
                    for url in urls {
                        if url.scheme() == "battles-record" && url.path() == "/oauth/callback" {
                            if let Some(query) = url.query() {
                                let _ = handle.emit("oauth-callback", query);
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let state = app.state::<DaemonStateMutex>();

                // Check if daemon is running
                let daemon_running = state
                    .lock()
                    .map(|d| matches!(d.status, daemon::DaemonStatus::Running))
                    .unwrap_or(false);

                if daemon_running {
                    // Prevent close and emit event to frontend
                    api.prevent_close();
                    let _ = window.emit("close-requested", ());
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // Run with exit handler to ensure daemon cleanup
    app.run(|app_handle, event| {
        if let RunEvent::Exit = event {
            // Ensure daemon is stopped when app exits for any reason
            let state = app_handle.state::<DaemonStateMutex>();
            let mut daemon = match state.lock() {
                Ok(d) => d,
                Err(_) => return,
            };
            if let Some(child) = daemon.process.take() {
                log::info!("Stopping daemon on app exit...");
                let _ = child.kill();
                daemon.status = daemon::DaemonStatus::Stopped;
                daemon.port = None;
            }
        }
    });
}
