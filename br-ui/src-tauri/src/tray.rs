use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

use crate::daemon::{DaemonStateMutex, DaemonStatus};

pub fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    // Get initial status (handle poisoned mutex gracefully)
    let initial_status: &'static str = {
        let state = app.state::<DaemonStateMutex>();
        let status = state.lock().ok().map(|d| d.status.clone());
        match status {
            Some(DaemonStatus::Stopped) => "Service: Stopped",
            Some(DaemonStatus::Starting) => "Service: Starting...",
            Some(DaemonStatus::Running) => "Service: Running",
            Some(DaemonStatus::Stopping) => "Service: Stopping...",
            None => "Service: Unknown",
        }
    };

    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "status", initial_status, false, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "open", "Open App", true, None::<&str>)?,
            &MenuItem::with_id(app, "stop_service", "Stop Service", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "exit", "Exit", true, None::<&str>)?,
        ],
    )?;

    // Build tray icon - use default window icon if available, otherwise skip icon
    let mut tray_builder = TrayIconBuilder::new();
    if let Some(icon) = app.default_window_icon() {
        tray_builder = tray_builder.icon(icon.clone());
    }

    let _tray = tray_builder
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "stop_service" => {
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<DaemonStateMutex>();
                    if let Err(e) = crate::daemon::stop_local_daemon(app_handle.clone(), state).await {
                        log::error!("Failed to stop daemon from tray: {}", e);
                    }
                });
            }
            "exit" => {
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    // Stop the daemon first
                    let state = app_handle.state::<DaemonStateMutex>();
                    if let Err(e) = crate::daemon::stop_local_daemon(app_handle.clone(), state).await {
                        log::error!("Failed to stop daemon during exit: {}", e);
                    }
                    // Then exit the app
                    app_handle.exit(0);
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// Update the tray menu status item to reflect current daemon state
/// Note: This is a placeholder - dynamic tray menu updates require storing a reference
pub fn update_tray_status(_app: &tauri::AppHandle) {
    // Tray menu items cannot be easily updated in Tauri v2
    // The status is shown when the menu is initially created
    // For dynamic updates, we would need to rebuild the menu
}
