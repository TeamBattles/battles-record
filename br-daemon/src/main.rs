use anyhow::Context;
use br_daemon::api::AppState;
use br_daemon::config;
use br_daemon::jellyfin::JellyfinExporter;
use br_daemon::manager::{ChannelManager, ManagerEvent};
use br_daemon::notifications::NotificationManager;
use br_daemon::platforms::{ChannelProfile, StreamPlatform, TwitchPlatform};
use br_daemon::processing::{ProcessingEvent, ProcessingManager, ReconciliationWorker};
use br_daemon::storage::StorageManager;
use br_daemon::types::{Platform, QuotaStatus};
use parking_lot::RwLock;
use std::time::Instant;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Handle --hash-password before any other initialization
    // Used by Docker entrypoint to hash admin passwords
    if let Some(password) = std::env::args()
        .skip_while(|a| a != "--hash-password")
        .nth(1)
    {
        let hash = bcrypt::hash(&password, bcrypt::DEFAULT_COST)
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;
        println!("{}", hash);
        return Ok(());
    }

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse --config flag, fall back to BR_CONFIG env var, then default
    let config_path = std::env::args()
        .skip_while(|a| a != "--config")
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| std::env::var("BR_CONFIG").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("br-config.toml"));

    let config = config::Config::load_or_default(&config_path);
    tracing::info!("Loaded config from {:?}", config_path);

    // Allow --port flag to override config
    let mut config = config;
    if let Some(port_str) = std::env::args()
        .skip_while(|a| a != "--port")
        .nth(1)
    {
        if let Ok(port) = port_str.parse::<u16>() {
            tracing::info!("Port override via --port flag: {}", port);
            config.daemon.port = port;
        } else {
            tracing::warn!("Invalid --port value '{}', using config value", port_str);
        }
    }

    // Allow --data-dir flag to override recordings directory
    if let Some(data_dir) = std::env::args()
        .skip_while(|a| a != "--data-dir")
        .nth(1)
    {
        tracing::info!("Data directory override via --data-dir flag: {}", data_dir);
        config.storage.recordings_dir = PathBuf::from(data_dir);
    }

    // Allow --library-dir flag to override library directory
    let library_dir_specified = std::env::args()
        .skip_while(|a| a != "--library-dir")
        .nth(1);
    if let Some(library_dir) = library_dir_specified.clone() {
        tracing::info!("Library directory override via --library-dir flag: {}", library_dir);
        config.storage.library_dir = PathBuf::from(library_dir);
    }

    // Check for --local-only flag (skips auth for localhost connections)
    let local_only = std::env::args().any(|a| a == "--local-only");
    if local_only {
        tracing::info!("Running in local-only mode (auth disabled for localhost)");
        // Force bind to localhost only for security
        config.daemon.host = "127.0.0.1".to_string();

        // If running in local-only mode and --library-dir was NOT specified,
        // default library_dir to recordings_dir (keeps them the same)
        if library_dir_specified.is_none() {
            tracing::info!("Defaulting library_dir to recordings_dir in local-only mode");
            config.storage.library_dir = config.storage.recordings_dir.clone();
        }
    }

    let addr: SocketAddr = format!("{}:{}", config.daemon.host, config.daemon.port).parse()?;

    let jwt_secret = config.auth.jwt_secret.clone().unwrap_or_else(|| {
        let secret = uuid::Uuid::new_v4().to_string();
        tracing::warn!("No JWT secret configured, generated temporary secret");
        secret
    });

    // Create storage manager (must be created before ChannelManager)
    let storage_manager = StorageManager::new(config.storage.clone())
        .await
        .context("Failed to initialize storage manager")?;

    // Clean up orphaned recordings from previous session
    let orphaned = storage_manager.cleanup_orphaned_recordings().await;
    if orphaned > 0 {
        tracing::info!("Cleaned up {} orphaned recordings from previous session", orphaned);
    }

    // Reset any recordings that were mid-processing when the daemon stopped
    let interrupted = storage_manager.reset_interrupted_processing().await;
    if interrupted > 0 {
        tracing::info!("Reset {} interrupted processing jobs from previous session", interrupted);
    }

    let storage_manager = Arc::new(storage_manager);

    // Wrap config in Arc<RwLock> for sharing with ChannelManager and AppState
    let config = Arc::new(RwLock::new(config));

    // Create channel manager (with access to config for platform auth tokens)
    let (channel_manager, manager_events) = ChannelManager::new(
        config.read().storage.recordings_dir.clone(),
        config.read().polling.default_interval,
        storage_manager.clone(),
        config.clone(),
    );
    let channel_manager = Arc::new(channel_manager);

    // Create event bus for WebSocket
    let (event_tx, _) = broadcast::channel::<ManagerEvent>(256);

    // Subscribe channel manager events to the bus
    let event_tx_clone = event_tx.clone();
    tokio::spawn(async move {
        let mut rx = manager_events;
        while let Ok(event) = rx.recv().await {
            let _ = event_tx_clone.send(event);
        }
    });

    // Create processing manager
    let (processing_manager, processing_events) = {
        let cfg = config.read();
        ProcessingManager::new(
            cfg.post_processing.ffmpeg_path.clone(),
            cfg.post_processing.get_segment_handling(),
            cfg.post_processing.max_concurrent,
        )
    };
    let processing_manager = Arc::new(processing_manager);

    // Create Jellyfin exporter if enabled
    let jellyfin_exporter = {
        let cfg = config.read();
        if cfg.jellyfin.enabled {
            match JellyfinExporter::new(cfg.jellyfin.clone(), cfg.storage.library_dir.clone()) {
                Ok(exporter) => {
                    tracing::info!("Jellyfin exporter initialized (library_dir: {:?})", cfg.storage.library_dir);
                    Some(Arc::new(tokio::sync::Mutex::new(exporter)))
                }
                Err(e) => {
                    tracing::error!("Failed to initialize Jellyfin exporter: {}", e);
                    None
                }
            }
        } else {
            tracing::info!("Jellyfin export disabled");
            None
        }
    };

    // Forward processing events to the main event bus AND update storage index
    let event_tx_processing = event_tx.clone();
    let storage_for_processing = Arc::clone(&storage_manager);
    let jellyfin_for_processing = jellyfin_exporter.clone();
    let channel_manager_for_processing = Arc::clone(&channel_manager);
    let images_dir_for_processing = config.read().storage.images_dir.clone();
    tokio::spawn(async move {
        let mut rx = processing_events;
        while let Ok(event) = rx.recv().await {
            let manager_event = match &event {
                ProcessingEvent::Started { recording_id } => {
                    ManagerEvent::ProcessingStarted { recording_id: *recording_id }
                }
                ProcessingEvent::Progress {
                    recording_id,
                    percent,
                } => ManagerEvent::ProcessingProgress {
                    recording_id: *recording_id,
                    percent: *percent,
                },
                ProcessingEvent::Complete {
                    recording_id,
                    output_file,
                    size_bytes,
                } => ManagerEvent::ProcessingComplete {
                    recording_id: *recording_id,
                    output_file: output_file.clone(),
                    size_bytes: *size_bytes,
                },
                ProcessingEvent::Failed {
                    recording_id,
                    error,
                } => ManagerEvent::ProcessingFailed {
                    recording_id: *recording_id,
                    error: error.clone(),
                },
            };
            let _ = event_tx_processing.send(manager_event);

            // Update storage index based on processing result
            match event {
                ProcessingEvent::Complete {
                    recording_id,
                    output_file,
                    size_bytes,
                } => {
                    if let Err(e) = storage_for_processing
                        .mark_processed(&recording_id, PathBuf::from(&output_file), Some(size_bytes))
                        .await
                    {
                        tracing::error!(
                            recording_id = %recording_id,
                            error = %e,
                            "Failed to update storage index after processing completed"
                        );
                    } else {
                        tracing::info!(
                            recording_id = %recording_id,
                            output_file = %output_file,
                            size_bytes = size_bytes,
                            "Storage index updated: recording marked as processed"
                        );

                        // Export to Jellyfin if enabled
                        if let Some(ref jellyfin) = jellyfin_for_processing {
                            // Get the recording entry
                            if let Ok(Some(recording)) = storage_for_processing.get_recording(&recording_id).await {
                                // Try to get cached profile from channel config first
                                let profile = if let Some(channel_config) = channel_manager_for_processing.get_channel_config_by_name(&recording.channel_name, recording.platform) {
                                    // Resolve custom images first (path relative to images_dir), fallback to platform URLs
                                    let profile_image = channel_config.custom_profile_image.as_ref()
                                        .map(|rel| images_dir_for_processing.join(rel).to_string_lossy().to_string())
                                        .or(channel_config.platform_profile_url.clone());

                                    let banner_image = channel_config.custom_banner_image.as_ref()
                                        .map(|rel| images_dir_for_processing.join(rel).to_string_lossy().to_string())
                                        .or(channel_config.platform_banner_url.clone());

                                    // Log image status for debugging
                                    if profile_image.is_some() || banner_image.is_some() {
                                        let using_custom = channel_config.custom_profile_image.is_some() || channel_config.custom_banner_image.is_some();
                                        tracing::debug!(
                                            channel = %recording.channel_name,
                                            using_custom = using_custom,
                                            "Using {} images for Jellyfin export",
                                            if using_custom { "custom" } else { "platform" }
                                        );
                                    } else {
                                        tracing::debug!(
                                            channel = %recording.channel_name,
                                            "No images available for Jellyfin export, proceeding without images"
                                        );
                                    }

                                    // Always create profile - images are optional for Jellyfin export
                                    Some(ChannelProfile {
                                        display_name: recording.channel_name.clone(),
                                        description: None,
                                        profile_image_url: profile_image,
                                        banner_image_url: banner_image,
                                    })
                                } else {
                                    None
                                };

                                // If no cached profile, fetch from platform API
                                let profile = match profile {
                                    Some(p) => Some(p),
                                    None => {
                                        let platform: Option<Box<dyn StreamPlatform + Send>> = match recording.platform {
                                            Platform::Twitch => Some(Box::new(TwitchPlatform::new())),
                                            Platform::YouTube => {
                                                tracing::warn!("Jellyfin export: YouTube platform not yet implemented");
                                                None
                                            }
                                            Platform::Kick => {
                                                tracing::warn!("Jellyfin export: Kick platform not yet implemented");
                                                None
                                            }
                                        };

                                        if let Some(platform) = platform {
                                            match platform.get_channel_profile(&recording.channel_name).await {
                                                Ok(p) => Some(p),
                                                Err(e) => {
                                                    tracing::error!(
                                                        recording_id = %recording_id,
                                                        channel = %recording.channel_name,
                                                        error = %e,
                                                        "Failed to fetch channel profile for Jellyfin export"
                                                    );
                                                    None
                                                }
                                            }
                                        } else {
                                            None
                                        }
                                    }
                                };

                                if let Some(profile) = profile {
                                    let mut exporter = jellyfin.lock().await;
                                    let output_path = PathBuf::from(&output_file);
                                    match exporter.export_recording(&recording, &output_path, &profile).await {
                                        Ok(result) => {
                                            tracing::info!(
                                                recording_id = %recording_id,
                                                channel = %recording.channel_name,
                                                season = result.season,
                                                episode = result.episode,
                                                "Exported to Jellyfin library"
                                            );

                                            // Update recording with Jellyfin export status
                                            if let Err(e) = storage_for_processing
                                                .mark_jellyfin_exported(&recording_id, result.video_path)
                                                .await
                                            {
                                                tracing::warn!(
                                                    recording_id = %recording_id,
                                                    error = %e,
                                                    "Failed to update Jellyfin export status"
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!(
                                                recording_id = %recording_id,
                                                error = %e,
                                                "Failed to export to Jellyfin"
                                            );
                                        }
                                    }
                                } else {
                                    tracing::warn!(
                                        recording_id = %recording_id,
                                        channel = %recording.channel_name,
                                        "Jellyfin export skipped: could not create channel profile (channel not found in config and platform API failed)"
                                    );
                                }
                            } else {
                                tracing::warn!(
                                    recording_id = %recording_id,
                                    "Could not find recording for Jellyfin export"
                                );
                            }
                        }
                    }
                }
                ProcessingEvent::Failed {
                    recording_id,
                    error,
                } => {
                    if let Err(e) = storage_for_processing
                        .mark_processing_failed(&recording_id, Some(error.clone()))
                        .await
                    {
                        tracing::error!(
                            recording_id = %recording_id,
                            error = %e,
                            "Failed to update storage index after processing failed"
                        );
                    } else {
                        tracing::warn!(
                            recording_id = %recording_id,
                            processing_error = %error,
                            "Storage index updated: recording marked as processing failed"
                        );
                    }
                }
                _ => {}
            }
        }
    });

    // Start reconciliation worker for post-processing incomplete recordings
    let (reconciliation_shutdown_tx, reconciliation_shutdown_rx) = mpsc::channel::<()>(1);
    let reconciliation_worker = {
        let cfg = config.read();
        ReconciliationWorker::new(
            Arc::clone(&storage_manager),
            Arc::clone(&processing_manager),
            cfg.post_processing.clone(),
            cfg.storage.library_dir.clone(),
            reconciliation_shutdown_rx,
        )
    };
    tokio::spawn(reconciliation_worker.run());
    tracing::info!("Reconciliation worker started (library_dir: {:?})", config.read().storage.library_dir);

    // Start notification manager
    let notification_manager = Arc::new(NotificationManager::new(
        config.read().notifications.clone(),
    ));
    notification_manager.clone().start(event_tx.subscribe());

    // Spawn cleanup worker (only if interval is configured)
    let cleanup_interval_hours = config.read().storage.retention.cleanup_interval_hours;
    if cleanup_interval_hours > 0 {
        let cleanup_storage_manager = storage_manager.clone();
        let cleanup_channel_manager = channel_manager.clone();
        let cleanup_interval = std::time::Duration::from_secs(cleanup_interval_hours as u64 * 3600);

        tokio::spawn(async move {
            // Wait a bit before first cleanup to allow daemon to start
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;

            let mut interval = tokio::time::interval(cleanup_interval);
            loop {
                interval.tick().await;

                // Get current channels for per-channel retention
                let channels = cleanup_channel_manager.get_channels();

                match cleanup_storage_manager.run_cleanup(&channels).await {
                    Ok(result) => {
                        if result.deleted_count > 0 {
                            tracing::info!(
                                deleted = result.deleted_count,
                                freed_bytes = result.freed_bytes,
                                "Storage cleanup completed"
                            );
                        } else {
                            tracing::debug!("Storage cleanup: nothing to delete");
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Storage cleanup failed");
                    }
                }
            }
        });
    } else {
        tracing::info!("Storage cleanup disabled (cleanup_interval_hours = 0)");
    }

    // Spawn quota refresh task (updates quota status every 5 minutes)
    {
        let quota_channel_manager = channel_manager.clone();
        let quota_storage_manager = storage_manager.clone();
        let quota_event_tx = event_tx.clone();
        let quota_refresh_interval = std::time::Duration::from_secs(5 * 60);

        tokio::spawn(async move {
            // Wait a bit before first refresh
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;

            let mut interval = tokio::time::interval(quota_refresh_interval);
            loop {
                interval.tick().await;

                let channels = quota_channel_manager.get_channels();
                for channel in channels {
                    if let Some(quota_gb) = channel.quota_gb {
                        let usage = quota_storage_manager.get_channel_usage(&channel.name).await;
                        let limit_bytes = quota_gb as u64 * 1024 * 1024 * 1024;
                        let percent = if limit_bytes > 0 {
                            ((usage as f64 / limit_bytes as f64) * 100.0).min(255.0) as u8
                        } else {
                            0
                        };

                        let new_status = if percent >= 100 {
                            QuotaStatus::Exceeded
                        } else if percent >= 90 {
                            QuotaStatus::Warning
                        } else {
                            QuotaStatus::Ok
                        };

                        let old_status = channel.quota_status;
                        if new_status != old_status || channel.quota_used_bytes != usage {
                            quota_channel_manager.update_quota_status(
                                channel.id,
                                new_status,
                                usage,
                                percent,
                            );

                            if new_status != old_status {
                                let _ = quota_event_tx.send(ManagerEvent::QuotaStatusChanged {
                                    channel_id: channel.id,
                                    channel_name: channel.name.clone(),
                                    quota_status: new_status,
                                    quota_used_bytes: usage,
                                    quota_percent: percent,
                                });
                                tracing::info!(
                                    channel = %channel.name,
                                    status = ?new_status,
                                    percent = percent,
                                    "Quota status changed"
                                );
                            }
                        }
                    }
                }
            }
        });
        tracing::info!("Quota refresh task started (interval: 5 minutes)");
    }

    // Spawn active recording stats refresh task (updates size/duration every minute)
    {
        let stats_storage_manager = storage_manager.clone();
        let stats_refresh_interval = std::time::Duration::from_secs(60); // 1 minute

        tokio::spawn(async move {
            // Wait a bit before first refresh
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;

            let mut interval = tokio::time::interval(stats_refresh_interval);
            loop {
                interval.tick().await;
                stats_storage_manager.refresh_active_recording_stats().await;
            }
        });
        tracing::info!("Active recording stats refresh task started (interval: 1 minute)");
    }

    // Load channels from separate file if configured, otherwise use config.channels
    let channels_to_add = {
        let cfg = config.read();
        if let Some(ref channels_file) = cfg.daemon.channels_file {
            let loaded = config::load_channels_file(channels_file);
            if loaded.is_empty() && !cfg.channels.is_empty() {
                // Migrate existing channels from config to the new file
                tracing::info!("Migrating {} channels from config to {:?}", cfg.channels.len(), channels_file);
                if let Err(e) = config::save_channels_file(channels_file, &cfg.channels) {
                    tracing::error!("Failed to migrate channels: {}", e);
                }
                cfg.channels.clone()
            } else {
                loaded
            }
        } else {
            cfg.channels.clone()
        }
    };

    // Add channels to manager
    for channel_config in &channels_to_add {
        channel_manager.add_channel(channel_config.clone());
    }

    tracing::info!("Added {} channels", channels_to_add.len());

    // Do initial poll BEFORE starting the HTTP server
    // This ensures channels have correct status when clients connect
    if !channels_to_add.is_empty() {
        tracing::info!("Running initial channel check...");
        channel_manager.poll_all_channels().await;
        tracing::info!("Initial channel check complete");

        // DEBUG: Log channel statuses after initial poll
        let channels = channel_manager.get_channels();
        for ch in &channels {
            tracing::info!(
                channel = %ch.name,
                status = ?ch.status,
                enabled = ch.enabled,
                "Channel status after initial poll"
            );
        }
    }

    // Start polling loop (for subsequent polls)
    let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
    let manager_clone = channel_manager.clone();
    tokio::spawn(async move {
        manager_clone.run_polling_loop(shutdown_rx).await;
    });

    // Create API shutdown channel (for graceful shutdown via API)
    let (api_shutdown_tx, mut api_shutdown_rx) = mpsc::channel::<()>(1);

    let state = Arc::new(AppState {
        config: config.clone(),
        config_path: config_path.clone(),
        jwt_secret,
        local_only,
        channel_manager,
        processing_manager,
        storage_manager,
        event_tx,
        started_at: Instant::now(),
        session_store: Arc::new(br_daemon::api::users::SessionStore::new()),
        shutdown_tx: api_shutdown_tx,
        oauth_states: br_daemon::api::oauth::create_state_store(),
    });

    // Start token refresh service (refreshes OAuth tokens before expiry)
    br_daemon::services::token_refresh::start_token_refresh_service(state.clone());
    tracing::info!("Token refresh service started (interval: 5 minutes)");

    let app = br_daemon::api::create_router(state);

    tracing::info!("br-daemon listening on {}", addr);

    let listener = TcpListener::bind(addr).await?;

    // Graceful shutdown with tokio::select!
    tokio::select! {
        result = axum::serve(listener, app) => {
            if let Err(e) = result {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received Ctrl+C, initiating graceful shutdown");
        }
        _ = api_shutdown_rx.recv() => {
            tracing::info!("Received API shutdown request, initiating graceful shutdown");
        }
    }

    // Signal workers to stop
    tracing::info!("Sending shutdown signals to background workers");
    if shutdown_tx.send(()).await.is_err() {
        tracing::debug!("Polling loop shutdown channel already closed");
    }
    if reconciliation_shutdown_tx.send(()).await.is_err() {
        tracing::debug!("Reconciliation worker shutdown channel already closed");
    }

    // Give some time for cleanup
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    tracing::info!("Shutdown complete");

    Ok(())
}
