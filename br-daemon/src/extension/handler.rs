use uuid::Uuid;

use crate::downloads::job::{CookieData, DownloadRequest};
use crate::downloads::DownloadError;
use crate::extension::connection::SharedState;
use crate::libraries::LibraryError;
use crate::extension::protocol::{
    download_failure_detail, CookieEntry, DaemonMessage, DownloadJobSummary as ProtoDownloadJobSummary,
    ExtensionMessage, FormatInfo as ProtoFormatInfo,
};

/// A failed download request as a coded `error`: the cause's fields travel as data (the existing
/// job id, the quota numbers) rather than inside a sentence.
fn download_error(id: &str, error: &DownloadError) -> DaemonMessage {
    let code = error.code();
    let mut existing = None;
    let mut quota = None;
    match error {
        DownloadError::DuplicateUrl { existing_id, .. } => existing = Some(existing_id.to_string()),
        DownloadError::QuotaExceeded { used_bytes, limit_bytes } => quota = Some((*used_bytes, *limit_bytes)),
        _ => {}
    }
    DaemonMessage::Error {
        id: Some(id.to_string()),
        code: code.to_string(),
        detail: download_failure_detail(code, &error.to_string()),
        existing_id: existing,
        used_bytes: quota.map(|(used, _)| used),
        limit_bytes: quota.map(|(_, limit)| limit),
    }
}

/// Install and update requests run in the background, so their failures are broadcast like
/// their successes. `library` is `all` when an install-everything request fails somewhere the
/// error doesn't pin to one library (the managed bin directory, for instance).
fn library_install_failed(library: &str, error: &LibraryError) -> crate::manager::ManagerEvent {
    crate::manager::ManagerEvent::LibraryInstallFailed {
        library: library.to_string(),
        code: error.code().to_string(),
        detail: Some(error.to_string()),
    }
}

/// Persist current channel list to the config file.
fn save_channels_to_config(state: &SharedState) {
    let channel_configs = state.channel_manager.get_channel_configs();
    let channels_file = {
        let config = state.config.read();
        config.daemon.channels_file.clone()
    };

    if let Some(channels_path) = channels_file {
        if let Err(e) = crate::config::save_channels_file(&channels_path, &channel_configs) {
            tracing::error!("Failed to save channels to {:?}: {}", channels_path, e);
        }
    } else {
        let mut config = state.config.write();
        config.channels = channel_configs;
        if let Err(e) = config.save(&state.config_path) {
            tracing::error!("Failed to save config: {}", e);
        }
    }
}

/// Parse a human-readable quality string from a yt-dlp format selector.
pub fn format_to_quality(format: Option<&str>) -> Option<String> {
    let f = format?;
    if let Some(start) = f.find("height<=") {
        let rest = &f[start + 8..];
        if let Some(end) = rest.find(']') {
            let height = &rest[..end];
            return Some(format!("{}p", height));
        }
    }
    if f.starts_with("bestaudio") {
        return Some("Audio".to_string());
    }
    if f.contains("bestvideo") || f == "best" {
        return Some("Best".to_string());
    }
    None
}

/// Extract the domain from a URL to use as a fallback channel name.
fn extract_channel_from_url(url: &str) -> String {
    let host = url_host(url);
    if host.is_empty() {
        "unknown".to_string()
    } else {
        host.to_string()
    }
}

fn url_host(url: &str) -> &str {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let host = without_scheme.split('/').next().unwrap_or("");
    let host = host.split('@').next_back().unwrap_or(host);
    host.split(':').next().unwrap_or(host)
}

/// Picks the platform a download is badged and filed under (it names the download folder).
/// Battles Replay sends its site detector's id, which has no pattern for Twitch or Kick VOD
/// pages (they arrive as "other") and names clips `twitch_clip` / `kick_clip`. Installed
/// extension builds keep doing that, so the URL decides whenever the value is not a platform.
fn resolve_source_platform(given: Option<&str>, url: &str) -> String {
    match given {
        Some("twitch_clip") => return "twitch".to_string(),
        Some("kick_clip") => return "kick".to_string(),
        Some(p) if !p.is_empty() && p != "other" && p != "unknown" => return p.to_string(),
        _ => {}
    }
    let host = url_host(url).to_ascii_lowercase();
    let on = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
    if on("twitch.tv") {
        "twitch".to_string()
    } else if on("kick.com") {
        "kick".to_string()
    } else if on("youtube.com") || on("youtu.be") {
        "youtube".to_string()
    } else {
        given
            .filter(|p| !p.is_empty())
            .unwrap_or("unknown")
            .to_string()
    }
}

/// Dispatch an extension message to the appropriate handler.
///
/// Returns `Some(response)` for messages that need a direct reply,
/// or `None` for fire-and-forget / background operations.
pub async fn handle_message(
    msg: &ExtensionMessage,
    state: &SharedState,
    client_id: Uuid,
) -> Option<DaemonMessage> {
    match msg {
        ExtensionMessage::Ping => Some(DaemonMessage::Pong),

        // -- Library operations --
        ExtensionMessage::GetLibraryStatus => {
            let lib_mgr = state.library_manager.lock().await;
            let status = lib_mgr.check_status().await;
            let libraries_installed = status.ytdlp.installed && status.ffmpeg.installed;
            Some(DaemonMessage::Hello {
                version: env!("CARGO_PKG_VERSION").to_string(),
                requires_pairing: false,
                identifier: None,
                libraries: status,
                libraries_installed,
            })
        }

        ExtensionMessage::InstallLibraries => {
            let manager = state.library_manager.clone();
            let event_tx = state.event_tx.clone();
            tokio::spawn(async move {
                let mut m = manager.lock().await;
                let missing = [
                    ("ytdlp", m.resolve_ytdlp().is_none()),
                    ("ffmpeg", m.resolve_ffmpeg().is_none()),
                ];
                for (library, _) in missing.iter().filter(|(_, missing)| *missing) {
                    let _ = event_tx.send(crate::manager::ManagerEvent::LibraryInstallStarted {
                        library: library.to_string(),
                    });
                }
                if let Err(e) = m.install_all(None).await {
                    tracing::error!(error = ?e, "Failed to install libraries");
                    let _ = event_tx.send(library_install_failed(e.library().unwrap_or("all"), &e));
                    return;
                }
                // Emit status for each library after install
                let status = m.check_status().await;
                let _ = event_tx.send(crate::manager::ManagerEvent::LibraryStatusChanged {
                    library: "ytdlp".to_string(),
                    installed: status.ytdlp.installed,
                    version: status.ytdlp.version.clone(),
                });
                let _ = event_tx.send(crate::manager::ManagerEvent::LibraryStatusChanged {
                    library: "ffmpeg".to_string(),
                    installed: status.ffmpeg.installed,
                    version: status.ffmpeg.version.clone(),
                });
            });
            None
        }

        ExtensionMessage::UpdateLibrary { library } => {
            let library = library.clone();
            let manager = state.library_manager.clone();
            let event_tx = state.event_tx.clone();
            tokio::spawn(async move {
                let mut m = manager.lock().await;
                if !matches!(library.as_str(), "ytdlp" | "ffmpeg") {
                    return;
                }
                let _ = event_tx.send(crate::manager::ManagerEvent::LibraryInstallStarted {
                    library: library.clone(),
                });
                let result = if library == "ytdlp" {
                    m.update_ytdlp(None).await
                } else {
                    m.update_ffmpeg(None).await
                };
                if let Err(e) = result {
                    tracing::error!(error = ?e, library = %library, "Failed to update library");
                    let _ = event_tx.send(library_install_failed(&library, &e));
                    return;
                }
                // Emit status after update
                let status = m.check_status().await;
                let info = match library.as_str() {
                    "ytdlp" => &status.ytdlp,
                    "ffmpeg" => &status.ffmpeg,
                    _ => return,
                };
                let _ = event_tx.send(crate::manager::ManagerEvent::LibraryStatusChanged {
                    library,
                    installed: info.installed,
                    version: info.version.clone(),
                });
            });
            None
        }

        ExtensionMessage::UninstallLibrary { id, library } => {
            let binary_name = match library.as_str() {
                "ytdlp" | "yt-dlp" => "yt-dlp",
                "ffmpeg" => "ffmpeg",
                "bun" => "bun",
                _ => {
                    return Some(DaemonMessage::error(Some(id.clone()), "unknown_library"));
                }
            };

            let mut mgr = state.library_manager.lock().await;
            if let Err(e) = mgr.uninstall_library(binary_name).await {
                return Some(DaemonMessage::error_with_detail(
                    Some(id.clone()),
                    "uninstall_failed",
                    e.to_string(),
                ));
            }

            // Use API-style name (e.g. "ytdlp" not "yt-dlp") for consistency with install/update events
            let event_name = match binary_name {
                "yt-dlp" => "ytdlp",
                other => other,
            };
            let _ = state.event_tx.send(crate::manager::ManagerEvent::LibraryStatusChanged {
                library: event_name.to_string(),
                installed: false,
                version: None,
            });

            Some(DaemonMessage::LibraryUninstalled {
                id: id.clone(),
                library: library.clone(),
            })
        }

        // -- Download operations --
        ExtensionMessage::ExtractInfo { id, url, cookies, .. } => {
            let cookie_count = cookies.as_ref().map_or(0, |c| c.len());
            tracing::debug!(url = %url, cookie_count, "ExtractInfo request received");
            let cookie_data: Option<Vec<CookieData>> = cookies.as_ref().map(|cs| {
                cs.iter().map(|c| cookie_entry_to_data(c)).collect()
            });
            match state
                .download_manager
                .extract_info(url, cookie_data.as_deref())
                .await
            {
                Ok(info) => {
                    // Find existing downloads matching this URL (checks file existence on disk)
                    let matching = state.download_manager.find_existing_for_url(url).await;
                    let existing: Vec<ProtoDownloadJobSummary> = matching
                        .into_iter()
                        .map(|j| ProtoDownloadJobSummary {
                            id: j.id.to_string(),
                            url: j.url,
                            title: j.title,
                            thumbnail: j.thumbnail,
                            platform_name: j.platform_name,
                            channel_name: j.channel_name,
                            source_platform: j.source_platform,
                            status: serde_json::to_value(j.status)
                                .ok()
                                .and_then(|v| v.as_str().map(String::from))
                                .unwrap_or_else(|| "unknown".to_string()),
                            percent: j.percent,
                            speed: j.speed,
                            eta: j.eta,
                            downloaded_bytes: j.downloaded_bytes,
                            total_bytes: j.total_bytes,
                            quality: j.quality.clone().or_else(|| format_to_quality(j.format.as_deref())),
                            format: j.format,
                            requested_by: j.requested_by.to_string(),
                            requested_by_name: j.requested_by_name.clone(),
                            created_at: j.created_at.to_rfc3339(),
                            completed_at: j.completed_at.map(|t| t.to_rfc3339()),
                            detail: j
                                .error_code
                                .as_deref()
                                .zip(j.error.as_deref())
                                .and_then(|(code, text)| download_failure_detail(code, text)),
                            error_code: j.error_code,
                            update_available: j.update_available,
                        })
                        .collect();

                    Some(DaemonMessage::InfoResult {
                        id: id.clone(),
                        title: info.title,
                        duration: info.duration,
                        thumbnail: info.thumbnail,
                        uploader: info.uploader,
                        platform_name: info.platform_name,
                        formats: info
                            .formats
                            .into_iter()
                            .map(|f| ProtoFormatInfo {
                                format_id: f.format_id,
                                ext: f.ext,
                                resolution: f.resolution,
                                filesize_approx: f.filesize_approx,
                                vcodec: f.vcodec,
                                acodec: f.acodec,
                            })
                            .collect(),
                        existing_downloads: existing,
                    })
                }
                Err(e) => Some(download_error(id, &e)),
            }
        }

        ExtensionMessage::Download {
            id,
            url,
            title,
            quality,
            channel_name,
            format,
            options,
            cookies,
            source_platform,
        } => {
            let cookie_data = cookies.as_ref().map(|cs| {
                cs.iter()
                    .map(|c| cookie_entry_to_data(c))
                    .collect::<Vec<_>>()
            });

            let dl_options = options.as_ref().map(|o| {
                crate::downloads::job::DownloadOptions {
                    embed_thumbnail: o.embed_thumbnail,
                    embed_metadata: o.embed_metadata,
                }
            });

            let resolved_channel = channel_name
                .clone()
                .unwrap_or_else(|| extract_channel_from_url(url));

            // Resolve client identifier for requested_by_name
            let client_name = {
                let pairing = state.pairing.read().await;
                pairing
                    .list_pairings()
                    .iter()
                    .find(|c| c.id == client_id)
                    .map(|c| c.identifier.clone())
            };

            let request = DownloadRequest {
                url: url.clone(),
                title: title.clone(),
                channel_name: resolved_channel,
                source_platform: resolve_source_platform(source_platform.as_deref(), url),
                format: format.clone(),
                quality: quality.clone(),
                options: dl_options,
                cookies: cookie_data,
                requested_by: client_id,
                requested_by_name: client_name,
                auto_start: false,
            };

            match state.download_manager.start_download(request).await {
                Ok(download_id) => Some(DaemonMessage::DownloadStarted {
                    id: id.clone(),
                    download_id: download_id.to_string(),
                }),
                Err(e) => Some(download_error(id, &e)),
            }
        }

        ExtensionMessage::Pause { id, download_id } => {
            match Uuid::parse_str(download_id) {
                Ok(uuid) => match state.download_manager.pause(uuid).await {
                    Ok(()) => Some(DaemonMessage::DownloadPaused {
                        id: id.clone(),
                        download_id: download_id.clone(),
                    }),
                    Err(e) => Some(download_error(id, &e)),
                },
                Err(_) => Some(DaemonMessage::error(Some(id.clone()), "invalid_id")),
            }
        }

        ExtensionMessage::Resume { id, download_id } => {
            match Uuid::parse_str(download_id) {
                Ok(uuid) => match state.download_manager.resume(uuid).await {
                    Ok(()) => Some(DaemonMessage::DownloadResumed {
                        id: id.clone(),
                        download_id: download_id.clone(),
                    }),
                    Err(e) => Some(download_error(id, &e)),
                },
                Err(_) => Some(DaemonMessage::error(Some(id.clone()), "invalid_id")),
            }
        }

        ExtensionMessage::Cancel { id, download_id } => {
            match Uuid::parse_str(download_id) {
                Ok(uuid) => match state.download_manager.cancel(uuid).await {
                    Ok(()) => Some(DaemonMessage::DownloadCancelled {
                        id: id.clone(),
                        download_id: download_id.clone(),
                    }),
                    Err(e) => Some(download_error(id, &e)),
                },
                Err(_) => Some(DaemonMessage::error(Some(id.clone()), "invalid_id")),
            }
        }

        ExtensionMessage::Prioritize { id, download_id } => {
            match Uuid::parse_str(download_id) {
                Ok(uuid) => match state.download_manager.prioritize(uuid).await {
                    Ok(()) => Some(DaemonMessage::DownloadPrioritized {
                        id: id.clone(),
                        download_id: download_id.clone(),
                    }),
                    Err(e) => Some(download_error(id, &e)),
                },
                Err(_) => Some(DaemonMessage::error(Some(id.clone()), "invalid_id")),
            }
        }

        ExtensionMessage::GetQueue => {
            let queue = state.download_manager.get_queue().await;
            let downloads: Vec<ProtoDownloadJobSummary> = queue
                .iter()
                .map(|j| ProtoDownloadJobSummary {
                    id: j.id.to_string(),
                    url: j.url.clone(),
                    title: j.title.clone(),
                    thumbnail: j.thumbnail.clone(),
                    platform_name: j.platform_name.clone(),
                    channel_name: j.channel_name.clone(),
                    source_platform: j.source_platform.clone(),
                    status: serde_json::to_value(j.status)
                        .ok()
                        .and_then(|v| v.as_str().map(String::from))
                        .unwrap_or_else(|| "unknown".to_string()),
                    percent: j.percent,
                    speed: j.speed.clone(),
                    eta: j.eta,
                    downloaded_bytes: j.downloaded_bytes,
                    total_bytes: j.total_bytes,
                    quality: j.quality.clone().or_else(|| format_to_quality(j.format.as_deref())),
                    format: j.format.clone(),
                    requested_by: j.requested_by.to_string(),
                    requested_by_name: j.requested_by_name.clone(),
                    created_at: j.created_at.to_rfc3339(),
                    completed_at: j.completed_at.map(|t| t.to_rfc3339()),
                    error_code: j.error_code.clone(),
                    detail: j
                        .error_code
                        .as_deref()
                        .zip(j.error.as_deref())
                        .and_then(|(code, text)| download_failure_detail(code, text)),
                    update_available: j.update_available,
                })
                .collect();
            Some(DaemonMessage::QueueState { downloads })
        }

        ExtensionMessage::MergeChannels { id, .. } => {
            Some(DaemonMessage::error(Some(id.clone()), "not_ready"))
        }

        ExtensionMessage::AddChannel { id, name, platform } => {
            // Parse platform string
            let parsed_platform = match platform.to_lowercase().as_str() {
                "twitch" => crate::types::Platform::Twitch,
                "youtube" => crate::types::Platform::YouTube,
                "kick" => crate::types::Platform::Kick,
                _ => {
                    return Some(DaemonMessage::error(Some(id.clone()), "invalid_platform"));
                }
            };

            let name = &crate::types::normalize_channel_name(name, parsed_platform);

            // The existing channel's id lets the extension treat the channel as added (and offer
            // Remove) instead of being stuck with a channel it has no id for. Stored names may
            // predate normalization, so both sides are normalized.
            let existing = state.channel_manager.get_channels();
            if let Some(duplicate) = existing.iter().find(|ch| {
                ch.platform == parsed_platform
                    && crate::types::normalize_channel_name(&ch.name, ch.platform) == *name
            }) {
                return Some(DaemonMessage::Error {
                    id: Some(id.clone()),
                    code: "channel_exists".to_string(),
                    detail: None,
                    existing_id: Some(duplicate.id.to_string()),
                    used_bytes: None,
                    limit_bytes: None,
                });
            }

            // Create channel config with defaults
            let channel_config = crate::config::ChannelConfig {
                name: name.clone(),
                platform: parsed_platform,
                enabled: true,
                quality: "best".to_string(),
                schedule: None,
                filters: None,
                post_processing: None,
                quota_gb: None,
                retention_days: None,
                custom_profile_image: None,
                custom_banner_image: None,
                platform_profile_url: None,
                platform_banner_url: None,
            };

            let channel_id = state.channel_manager.add_channel(channel_config);

            // Persist to config file
            save_channels_to_config(state);

            // Broadcast so the Tauri UI updates immediately
            let _ = state.event_tx.send(crate::manager::ManagerEvent::ChannelAdded {
                channel_id,
                channel_name: name.clone(),
                platform: parsed_platform,
            });

            // Spawn immediate live check so recording starts if the channel is live
            let cm = state.channel_manager.clone();
            let ch_name = name.clone();
            tokio::spawn(async move {
                if let Err(e) = cm.check_channel(channel_id).await {
                    tracing::warn!("Initial check for {} failed: {}", ch_name, e);
                }
            });

            Some(DaemonMessage::ChannelAdded {
                id: id.clone(),
                channel_id: channel_id.to_string(),
                name: name.clone(),
                platform: platform.clone(),
            })
        }

        ExtensionMessage::RemoveChannel { id, channel_id } => {
            let uuid = match Uuid::parse_str(&channel_id) {
                Ok(u) => u,
                Err(_) => {
                    return Some(DaemonMessage::error(Some(id.clone()), "invalid_id"));
                }
            };

            let result = state.channel_manager.remove_channel(uuid);
            let Some((channel, shutdown_tx)) = result else {
                return Some(DaemonMessage::error(Some(id.clone()), "channel_not_found"));
            };

            // Persist to config file
            save_channels_to_config(state);

            // Stop active recording if any
            if let Some(tx) = shutdown_tx {
                let _ = tx.send(()).await;
            }

            // Broadcast so other extension clients and the Tauri UI know
            let _ = state.event_tx.send(crate::manager::ManagerEvent::ChannelRemoved {
                channel_id: uuid,
                channel_name: channel.name.clone(),
                platform: channel.platform,
            });

            Some(DaemonMessage::ChannelRemoved {
                id: id.clone(),
                channel_id: channel_id.clone(),
                name: channel.name.clone(),
                platform: channel.platform.to_string().to_lowercase(),
            })
        }

        // Unpair is handled in the connection loop directly
        ExtensionMessage::Unpair => None,

        // Hello and Pair are handled during the handshake phase
        ExtensionMessage::Hello { .. } | ExtensionMessage::Pair { .. } => None,
    }
}

fn cookie_entry_to_data(entry: &CookieEntry) -> CookieData {
    CookieData {
        domain: entry.domain.clone(),
        path: entry.path.clone(),
        secure: entry.secure,
        expiration_date: entry.expiration_date,
        http_only: entry.http_only,
        name: entry.name.clone(),
        value: entry.value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloads::DownloadManager;
    use crate::extension::pairing::PairingManager;
    use crate::libraries::LibraryManager;
    use crate::manager::ManagerEvent;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::{broadcast, RwLock};

    fn test_client_id() -> Uuid {
        Uuid::nil()
    }

    #[test]
    fn download_errors_carry_their_structured_fields() {
        let existing = Uuid::new_v4();
        let duplicate = DownloadError::DuplicateUrl { url: "https://x".to_string(), existing_id: existing };
        let json = serde_json::to_value(download_error("r1", &duplicate)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"type": "error", "id": "r1", "code": "already_queued", "existing_id": existing.to_string()})
        );

        let quota = DownloadError::QuotaExceeded { used_bytes: 10, limit_bytes: 20 };
        let json = serde_json::to_value(download_error("r2", &quota)).unwrap();
        assert_eq!(json["code"], "quota_exceeded");
        assert_eq!(json["used_bytes"], 10);
        assert_eq!(json["limit_bytes"], 20);

        let io = DownloadError::Io(std::io::Error::other("disk full"));
        let json = serde_json::to_value(download_error("r3", &io)).unwrap();
        assert_eq!(json["code"], "storage_error");
        assert_eq!(json["detail"], "IO error: disk full");
    }

    async fn make_state() -> (SharedState, TempDir) {
        let tmp = TempDir::new().expect("failed to create temp dir");
        let config = crate::config::LibrariesConfig::default();
        let lib_mgr = LibraryManager::new(config, None);
        let lib_mgr = Arc::new(tokio::sync::Mutex::new(lib_mgr));

        let downloads_dir = tmp.path().join("downloads");
        let download_manager = Arc::new(
            DownloadManager::new(
                crate::config::DownloadsConfig::default(),
                downloads_dir,
                lib_mgr.clone(),
            )
            .await
            .expect("failed to create download manager"),
        );

        let mut storage_config = crate::config::StorageConfig::default();
        storage_config.recordings_dir = tmp.path().join("recordings");
        let full_config = Arc::new(parking_lot::RwLock::new(crate::config::Config::default()));
        let storage_manager = Arc::new(
            crate::storage::StorageManager::new(storage_config)
                .await
                .expect("create storage manager"),
        );
        let recordings_dir = tmp.path().join("recordings");
        let (channel_manager, _) = crate::manager::ChannelManager::new(
            recordings_dir,
            60,
            storage_manager,
            full_config,
        );
        let channel_manager = Arc::new(channel_manager);

        let (event_tx, _) = broadcast::channel::<ManagerEvent>(16);
        let config_path = tmp.path().join("config.toml");
        let app_config = Arc::new(parking_lot::RwLock::new(crate::config::Config::default()));
        let state = SharedState {
            pairing: Arc::new(RwLock::new(
                PairingManager::new(tmp.path()).expect("failed to init PairingManager"),
            )),
            connections: Arc::new(RwLock::new(HashMap::new())),
            library_manager: lib_mgr,
            download_manager,
            channel_manager,
            event_tx,
            message_senders: Arc::new(RwLock::new(HashMap::new())),
            config: app_config,
            config_path,
        };
        (state, tmp)
    }

    #[tokio::test]
    async fn test_ping_returns_pong() {
        let (state, _tmp) = make_state().await;
        let resp = handle_message(&ExtensionMessage::Ping, &state, test_client_id()).await;
        assert!(matches!(resp, Some(DaemonMessage::Pong)));
    }

    #[tokio::test]
    async fn test_get_library_status_returns_hello() {
        let (state, _tmp) = make_state().await;
        let resp = handle_message(&ExtensionMessage::GetLibraryStatus, &state, test_client_id()).await;
        match resp {
            Some(DaemonMessage::Hello {
                version,
                requires_pairing,
                ..
            }) => {
                assert_eq!(version, env!("CARGO_PKG_VERSION"));
                assert!(!requires_pairing);
            }
            other => panic!("expected Hello, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_install_libraries_returns_none() {
        let (state, _tmp) = make_state().await;
        let resp = handle_message(&ExtensionMessage::InstallLibraries, &state, test_client_id()).await;
        assert!(resp.is_none());
    }

    #[tokio::test]
    async fn test_update_library_returns_none() {
        let (state, _tmp) = make_state().await;
        let msg = ExtensionMessage::UpdateLibrary {
            library: "ytdlp".to_string(),
        };
        let resp = handle_message(&msg, &state, test_client_id()).await;
        assert!(resp.is_none());
    }

    #[tokio::test]
    async fn test_extract_info_returns_error_without_ytdlp() {
        let (state, _tmp) = make_state().await;
        let msg = ExtensionMessage::ExtractInfo {
            id: "req-1".to_string(),
            url: "https://example.com".to_string(),
            auto_start: None,
            cookies: None,
        };
        let resp = handle_message(&msg, &state, test_client_id()).await;
        match resp {
            Some(DaemonMessage::Error { id, code, .. }) => {
                assert_eq!(id, Some("req-1".to_string()));
                assert_eq!(code, "ytdlp_missing");
            }
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_download_returns_error_without_ytdlp() {
        let (state, _tmp) = make_state().await;
        let msg = ExtensionMessage::Download {
            id: "req-2".to_string(),
            url: "https://example.com/video".to_string(),
            title: None,
            quality: None,
            channel_name: Some("test".to_string()),
            format: None,
            options: None,
            cookies: None,
            source_platform: None,
        };
        let resp = handle_message(&msg, &state, test_client_id()).await;
        // Will start a download that fails (no yt-dlp), but start_download itself succeeds
        // because the error happens in the spawned task, not in start_download
        match resp {
            Some(DaemonMessage::DownloadStarted { id, download_id }) => {
                assert_eq!(id, "req-2");
                // download_id should be a valid UUID
                assert!(Uuid::parse_str(&download_id).is_ok());
            }
            Some(DaemonMessage::Error { .. }) => {
                // Also acceptable if quota is exceeded etc.
            }
            other => panic!("expected DownloadStarted or Error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_pause_invalid_uuid() {
        let (state, _tmp) = make_state().await;
        let msg = ExtensionMessage::Pause {
            id: "r1".into(),
            download_id: "not-a-uuid".into(),
        };
        let resp = handle_message(&msg, &state, test_client_id()).await;
        match resp {
            Some(DaemonMessage::Error { code, .. }) => {
                assert_eq!(code, "invalid_id");
            }
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_pause_nonexistent_download() {
        let (state, _tmp) = make_state().await;
        let msg = ExtensionMessage::Pause {
            id: "r1".into(),
            download_id: Uuid::new_v4().to_string(),
        };
        let resp = handle_message(&msg, &state, test_client_id()).await;
        match resp {
            Some(DaemonMessage::Error { code, .. }) => {
                assert_eq!(code, "not_found");
            }
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_resume_cancel_prioritize_invalid_uuid() {
        let (state, _tmp) = make_state().await;

        for msg in [
            ExtensionMessage::Resume {
                id: "r2".into(),
                download_id: "bad".into(),
            },
            ExtensionMessage::Cancel {
                id: "r3".into(),
                download_id: "bad".into(),
            },
            ExtensionMessage::Prioritize {
                id: "r4".into(),
                download_id: "bad".into(),
            },
        ] {
            let resp = handle_message(&msg, &state, test_client_id()).await;
            assert!(
                matches!(resp, Some(DaemonMessage::Error { code, .. }) if code == "invalid_id")
            );
        }
    }

    #[tokio::test]
    async fn test_get_queue_returns_empty() {
        let (state, _tmp) = make_state().await;
        let resp = handle_message(&ExtensionMessage::GetQueue, &state, test_client_id()).await;
        match resp {
            Some(DaemonMessage::QueueState { downloads }) => {
                assert!(downloads.is_empty());
            }
            other => panic!("expected QueueState, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_merge_channels_returns_not_ready() {
        let (state, _tmp) = make_state().await;
        let msg = ExtensionMessage::MergeChannels {
            id: "req-5".to_string(),
            platform: "twitch".to_string(),
            source: "old".to_string(),
            target: "new".to_string(),
            include_recordings: false,
        };
        let resp = handle_message(&msg, &state, test_client_id()).await;
        match resp {
            Some(DaemonMessage::Error { id, code, detail, .. }) => {
                assert_eq!(id, Some("req-5".to_string()));
                assert_eq!(code, "not_ready");
                assert!(detail.is_none());
            }
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_unpair_returns_none() {
        let (state, _tmp) = make_state().await;
        let resp = handle_message(&ExtensionMessage::Unpair, &state, test_client_id()).await;
        assert!(resp.is_none());
    }

    #[tokio::test]
    async fn test_hello_returns_none() {
        let (state, _tmp) = make_state().await;
        let msg = ExtensionMessage::Hello {
            extension_version: "1.0.0".to_string(),
            token: None,
        };
        let resp = handle_message(&msg, &state, test_client_id()).await;
        assert!(resp.is_none());
    }

    #[tokio::test]
    async fn test_add_channel_duplicate_carries_existing_id() {
        let (state, _tmp) = make_state().await;
        let config: crate::config::ChannelConfig =
            serde_json::from_value(serde_json::json!({"name": "Shroud", "platform": "twitch"})).unwrap();
        let existing = state.channel_manager.add_channel(config);

        let msg = ExtensionMessage::AddChannel {
            id: "req-1".to_string(),
            name: "shroud".to_string(),
            platform: "Twitch".to_string(),
        };
        let resp = handle_message(&msg, &state, test_client_id()).await;
        assert_eq!(
            serde_json::to_value(resp).unwrap(),
            serde_json::json!({"type": "error", "id": "req-1", "code": "channel_exists", "existing_id": existing.to_string()})
        );
    }

    #[tokio::test]
    async fn test_update_library_announces_install_start() {
        let (state, _tmp) = make_state().await;
        let mut events = state.event_tx.subscribe();
        let msg = ExtensionMessage::UpdateLibrary {
            library: "ytdlp".to_string(),
        };
        handle_message(&msg, &state, test_client_id()).await;

        let event = tokio::time::timeout(std::time::Duration::from_secs(2), events.recv())
            .await
            .expect("no event within 2s")
            .expect("event channel closed");
        assert!(
            matches!(event, ManagerEvent::LibraryInstallStarted { ref library } if library == "ytdlp"),
            "expected LibraryInstallStarted for ytdlp, got {event:?}"
        );
    }

    #[tokio::test]
    async fn test_pair_returns_none() {
        let (state, _tmp) = make_state().await;
        let msg = ExtensionMessage::Pair {
            code: "123456".to_string(),
            identifier: "test".to_string(),
        };
        let resp = handle_message(&msg, &state, test_client_id()).await;
        assert!(resp.is_none());
    }

    #[test]
    fn resolve_source_platform_reads_the_url_when_the_extension_cannot_tell() {
        let vod = "https://twitch.tv/videos/2938643974";
        assert_eq!(resolve_source_platform(Some("other"), vod), "twitch");
        assert_eq!(resolve_source_platform(None, vod), "twitch");
        assert_eq!(
            resolve_source_platform(Some("other"), "https://kick.com/xqc/videos/abc"),
            "kick"
        );
        assert_eq!(
            resolve_source_platform(Some("twitch_clip"), "https://clips.twitch.tv/Abc"),
            "twitch"
        );
        assert_eq!(
            resolve_source_platform(Some("youtube"), "https://youtu.be/x"),
            "youtube"
        );
        assert_eq!(
            resolve_source_platform(Some("other"), "https://example.com/v.mp4"),
            "other"
        );
        assert_eq!(
            resolve_source_platform(None, "https://example.com/v.mp4"),
            "unknown"
        );
        // A CDN playlist carries no platform in its host; the extension's value stands.
        assert_eq!(
            resolve_source_platform(
                Some("twitch"),
                "https://d1m7jfoe9zdc1j.cloudfront.net/x/index-dvr.m3u8"
            ),
            "twitch"
        );
    }

    #[tokio::test]
    async fn test_add_channel_normalizes_the_name_like_the_app() {
        let (state, _tmp) = make_state().await;
        let msg = ExtensionMessage::AddChannel {
            id: "req-1".to_string(),
            name: "@THATSGOODSPORTS".to_string(),
            platform: "youtube".to_string(),
        };
        handle_message(&msg, &state, test_client_id()).await;
        let names: Vec<String> = state
            .channel_manager
            .get_channels()
            .iter()
            .map(|c| c.name.clone())
            .collect();
        assert_eq!(names, vec!["@thatsgoodsports".to_string()]);

        // The app's form sends YouTube handles without the @.
        let again = ExtensionMessage::AddChannel {
            id: "req-2".to_string(),
            name: "ThatsGoodSports".to_string(),
            platform: "youtube".to_string(),
        };
        let resp = handle_message(&again, &state, test_client_id()).await;
        assert!(
            matches!(resp, Some(DaemonMessage::Error { ref code, .. }) if code == "channel_exists"),
            "expected channel_exists, got {resp:?}"
        );
    }
}
