use crate::config::{ChannelConfig, Config};
use crate::platforms::{
    is_bun_available, KickPlatform, StreamPlatform, StreamUrl, TwitchPlatform, YoutubePlatform,
};
use crate::recording::{RecordingEngine, RecordingEvent, RecordingState};
use crate::scheduler::{DecisionReason, FilterMatcher, ScheduleChecker, StreamMetadata};
use crate::storage::StorageManager;
use crate::types::{
    Channel, ChannelStatus, FiltersResponse, Platform, Quality, QuotaStatus, ScheduleRuleResponse,
    StreamInfo,
};
use chrono::{DateTime, Utc};
use parking_lot::{Mutex, RwLock};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/** Channel update fields. */
pub struct ChannelUpdate {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub quality: Option<String>,
    pub quota_gb: Option<Option<u32>>,
    pub retention_days: Option<Option<u32>>,
    // Schedule fields
    pub schedule_enabled: Option<bool>,
    pub timezone: Option<String>,
    pub schedule_rules: Option<Vec<crate::config::ScheduleRule>>,
    // Filter fields (Option<Option<...>> to support clearing: None = don't change, Some(None) = clear, Some(Some(x)) = set)
    pub filters: Option<Option<crate::config::FiltersConfig>>,
}

/** Events emitted by the ChannelManager. */
#[derive(Debug, Clone)]
pub enum ManagerEvent {
    /** Channel status changed (e.g., offline -> live). */
    StatusChanged {
        channel_id: Uuid,
        channel_name: String,
        platform: Platform,
        old_status: ChannelStatus,
        new_status: ChannelStatus,
    },
    /** Recording started for a channel. */
    RecordingStarted {
        channel_id: Uuid,
        channel_name: String,
        platform: Platform,
        recording_id: Uuid,
        output_dir: PathBuf,
    },
    /** Recording progress update. */
    RecordingProgress {
        channel_id: Uuid,
        recording_id: Uuid,
        segments_downloaded: u32,
        bytes_downloaded: u64,
    },
    /** Recording ended. */
    RecordingEnded {
        channel_id: Uuid,
        channel_name: String,
        recording_id: Uuid,
        total_segments: u32,
        total_bytes: u64,
    },
    /** An error occurred. */
    Error {
        channel_id: Option<Uuid>,
        channel_name: Option<String>,
        message: String,
    },
    /** Post-processing started. */
    ProcessingStarted { recording_id: Uuid },
    /** Post-processing progress. */
    ProcessingProgress { recording_id: Uuid, percent: u8 },
    /** Post-processing completed successfully. */
    ProcessingComplete {
        recording_id: Uuid,
        output_file: String,
        size_bytes: u64,
    },
    /** Post-processing failed. */
    ProcessingFailed { recording_id: Uuid, error: String },
    /** Recording skipped due to schedule rules. */
    ScheduleSkip {
        channel_id: Uuid,
        channel_name: String,
        platform: String,
    },
    /** Recording skipped due to filter rules. */
    FilterSkip {
        channel_id: Uuid,
        channel_name: String,
        platform: String,
        reason: DecisionReason,
    },
    /** Recording skipped due to quota exceeded. */
    QuotaSkip {
        channel_id: Uuid,
        channel_name: String,
        platform: String,
        quota_used_bytes: u64,
        quota_limit_bytes: u64,
    },
    /** Quota status changed for a channel. */
    QuotaStatusChanged {
        channel_id: Uuid,
        channel_name: String,
        quota_status: QuotaStatus,
        quota_used_bytes: u64,
        quota_percent: u8,
    },
    /** Platform authentication was updated (OAuth connected or token refreshed). */
    PlatformAuthUpdated {
        platform: Platform,
        status: String,
        username: Option<String>,
        expires_at: Option<DateTime<Utc>>,
    },
    /** Platform authentication expired (refresh failed). */
    PlatformAuthExpired { platform: Platform, reason: String },
}

/** Handle to an active recording. */
#[allow(dead_code)]
struct RecordingHandle {
    recording_id: Uuid,
    /** ID of the recording in the StorageManager index (if registration succeeded). */
    storage_recording_id: Option<Uuid>,
    output_dir: PathBuf,
    shutdown_tx: mpsc::Sender<()>,
    started_at: DateTime<Utc>,
}

/** A channel being managed. */
struct ManagedChannel {
    id: Uuid,
    config: ChannelConfig,
    status: ChannelStatus,
    current_stream: Option<StreamInfo>,
    recording: Option<RecordingHandle>,
    last_checked: Option<DateTime<Utc>>,
    last_error: Option<String>,
    quota_status: QuotaStatus,
    quota_used_bytes: u64,
    quota_percent: u8,
}

impl ManagedChannel {
    fn new(config: ChannelConfig) -> Self {
        Self {
            id: Uuid::new_v4(),
            config,
            status: ChannelStatus::Offline,
            current_stream: None,
            recording: None,
            last_checked: None,
            last_error: None,
            quota_status: QuotaStatus::Unlimited,
            quota_used_bytes: 0,
            quota_percent: 0,
        }
    }

    fn to_channel(&self) -> Channel {
        // Convert schedule rules from config format (day names) to API format (day numbers)
        let schedule_rules = self
            .config
            .schedule
            .as_ref()
            .map(|s| {
                s.rules
                    .iter()
                    .map(|r| ScheduleRuleResponse {
                        days: r
                            .days
                            .iter()
                            .filter_map(|d| day_name_to_number(d))
                            .collect(),
                        start_time: r.start_time.clone().unwrap_or_default(),
                        end_time: r.end_time.clone().unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Convert filters from config format to API format
        let filters = self.config.filters.as_ref().map(|f| FiltersResponse {
            title_includes: f.title_contains.clone(),
            title_excludes: f.title_excludes.clone(),
            game_includes: f.game_contains.clone(),
            game_excludes: f.game_excludes.clone(),
            min_viewers: f.min_viewers,
        });

        // Resolve profile image URL: custom > platform
        let profile_image_url = self
            .config
            .custom_profile_image
            .as_ref()
            .map(|_| format!("/api/channels/{}/images/profile", self.id))
            .or_else(|| self.config.platform_profile_url.clone());

        // Resolve banner image URL: custom > platform
        let banner_image_url = self
            .config
            .custom_banner_image
            .as_ref()
            .map(|_| format!("/api/channels/{}/images/banner", self.id))
            .or_else(|| self.config.platform_banner_url.clone());

        Channel {
            id: self.id,
            name: self.config.name.clone(),
            platform: self.config.platform,
            enabled: self.config.enabled,
            quality: self.config.quality.clone(),
            status: self.status,
            current_stream: self.current_stream.clone(),
            quota_gb: self.config.quota_gb,
            retention_days: self.config.retention_days,
            quota_status: self.quota_status,
            quota_used_bytes: self.quota_used_bytes,
            quota_percent: self.quota_percent,
            schedule_enabled: self
                .config
                .schedule
                .as_ref()
                .map(|s| s.enabled)
                .unwrap_or(false),
            timezone: self
                .config
                .schedule
                .as_ref()
                .and_then(|s| s.timezone.clone()),
            schedule_rules,
            filters,
            profile_image_url,
            banner_image_url,
        }
    }
}

/** Convert day name to number (Sunday=0, Monday=1, ..., Saturday=6). */
fn day_name_to_number(day: &str) -> Option<u8> {
    match day.to_lowercase().as_str() {
        "sunday" => Some(0),
        "monday" => Some(1),
        "tuesday" => Some(2),
        "wednesday" => Some(3),
        "thursday" => Some(4),
        "friday" => Some(5),
        "saturday" => Some(6),
        _ => None,
    }
}

/** Manages channels and their recording lifecycle. */
pub struct ChannelManager {
    channels: RwLock<HashMap<Uuid, ManagedChannel>>,
    /** Channels currently in the process of starting a recording (prevents race conditions). */
    starting_recordings: Mutex<HashSet<Uuid>>,
    recordings_dir: PathBuf,
    poll_interval: u64,
    event_tx: broadcast::Sender<ManagerEvent>,
    storage_manager: Arc<StorageManager>,
    schedule_checker: ScheduleChecker,
    filter_matcher: FilterMatcher,
    /** Track last poll time to prevent excessive polling on multiple client connects. */
    last_poll_time: RwLock<Option<std::time::Instant>>,
    /** Shared config for platform auth tokens. */
    config: Arc<RwLock<Config>>,
}

impl ChannelManager {
    /**
     * Create a new ChannelManager.
     *
     * Returns the manager and a receiver for manager events.
     */
    pub fn new(
        recordings_dir: PathBuf,
        poll_interval: u64,
        storage_manager: Arc<StorageManager>,
        config: Arc<RwLock<Config>>,
    ) -> (Self, broadcast::Receiver<ManagerEvent>) {
        let (event_tx, event_rx) = broadcast::channel(256);

        let manager = Self {
            channels: RwLock::new(HashMap::new()),
            starting_recordings: Mutex::new(HashSet::new()),
            recordings_dir,
            poll_interval,
            event_tx,
            storage_manager,
            schedule_checker: ScheduleChecker::new(),
            filter_matcher: FilterMatcher::new(),
            last_poll_time: RwLock::new(None),
            config,
        };

        (manager, event_rx)
    }

    /** Create a platform instance with authentication if available. */
    fn create_platform(&self, platform: Platform) -> Box<dyn StreamPlatform> {
        match platform {
            Platform::Twitch => {
                // Twitch GQL API is undocumented/internal and doesn't accept OAuth tokens
                // from third-party apps. It only works with the public client ID.
                // For checking stream status (public data), we use unauthenticated requests.
                // OAuth tokens from our app would only work with Twitch's official Helix API.
                debug!("Creating TwitchPlatform with public client ID (GQL API doesn't accept third-party OAuth)");
                Box::new(TwitchPlatform::new())
            }
            Platform::YouTube => {
                // Check Bun availability before creating platform
                if !is_bun_available() {
                    warn!("Bun runtime not found - YouTube functionality will fail");
                }

                let config = self.config.read();
                if let Some(ref creds) = config.platform_auth.youtube {
                    let is_valid = creds
                        .expires_at
                        .map(|expires| expires > chrono::Utc::now())
                        .unwrap_or(true);

                    if is_valid {
                        debug!("Creating YoutubePlatform with auth token");
                        return Box::new(YoutubePlatform::with_auth(creds.access_token.clone()));
                    } else {
                        warn!("YouTube auth token is expired, creating unauthenticated platform");
                    }
                }
                Box::new(YoutubePlatform::new())
            }
            Platform::Kick => {
                let config = self.config.read();
                if let Some(ref creds) = config.platform_auth.kick {
                    // Check if token is expired (if expiry is set)
                    let is_valid = creds
                        .expires_at
                        .map(|expires| expires > chrono::Utc::now())
                        .unwrap_or(true); // No expiry means valid

                    if is_valid {
                        debug!("Creating KickPlatform with auth token");
                        return Box::new(KickPlatform::with_auth(creds.access_token.clone()));
                    } else {
                        warn!("Kick auth token is expired, creating unauthenticated platform");
                    }
                }
                Box::new(KickPlatform::new())
            }
        }
    }

    /** Get the event sender for subscribing to events. */
    pub fn subscribe(&self) -> broadcast::Receiver<ManagerEvent> {
        self.event_tx.subscribe()
    }

    /** Add a channel to be managed. */
    pub fn add_channel(&self, config: ChannelConfig) -> Uuid {
        let mut channels = self.channels.write();

        // Check if channel with same name and platform already exists
        for channel in channels.values() {
            if channel.config.name == config.name && channel.config.platform == config.platform {
                info!(
                    "Channel {} on {} already exists with id {}",
                    config.name, config.platform, channel.id
                );
                return channel.id;
            }
        }

        let managed = ManagedChannel::new(config.clone());
        let id = managed.id;
        info!(
            "Adding channel {} on {} with id {}",
            config.name, config.platform, id
        );
        channels.insert(id, managed);
        id
    }

    /**
     * Remove a channel.
     *
     * Returns the removed channel and optionally a shutdown sender if recording was active.
     * The caller should send on the shutdown sender to stop the recording.
     */
    pub fn remove_channel(&self, id: Uuid) -> Option<(Channel, Option<mpsc::Sender<()>>)> {
        let mut channels = self.channels.write();

        if let Some(mut managed) = channels.remove(&id) {
            // Extract any active recording handle
            let shutdown_tx = managed.recording.take().map(|h| {
                info!(
                    "Removed channel {} ({}) - recording will be stopped",
                    managed.config.name, id
                );
                h.shutdown_tx
            });
            Some((managed.to_channel(), shutdown_tx))
        } else {
            None
        }
    }

    /** Get all managed channels. */
    pub fn get_channels(&self) -> Vec<Channel> {
        let channels = self.channels.read();
        channels.values().map(|m| m.to_channel()).collect()
    }

    /** Get all channel configs (for persisting to disk). */
    pub fn get_channel_configs(&self) -> Vec<crate::config::ChannelConfig> {
        let channels = self.channels.read();
        channels.values().map(|m| m.config.clone()).collect()
    }

    /** Get a specific channel by ID. */
    pub fn get_channel(&self, id: Uuid) -> Option<Channel> {
        let channels = self.channels.read();
        channels.get(&id).map(|m| m.to_channel())
    }

    /** Get a specific channel's config by ID (for image management). */
    pub fn get_channel_config(&self, id: Uuid) -> Option<crate::config::ChannelConfig> {
        let channels = self.channels.read();
        channels.get(&id).map(|m| m.config.clone())
    }

    /** Get a channel's config by name and platform. */
    pub fn get_channel_config_by_name(
        &self,
        name: &str,
        platform: crate::types::Platform,
    ) -> Option<crate::config::ChannelConfig> {
        let channels = self.channels.read();
        channels
            .values()
            .find(|m| m.config.name.eq_ignore_ascii_case(name) && m.config.platform == platform)
            .map(|m| m.config.clone())
    }

    /**
     * Update a channel's custom image path.
     *
     * image_type should be "profile" or "banner"
     * path is the relative path within images_dir, or None to clear.
     */
    pub fn update_channel_image(
        &self,
        id: Uuid,
        image_type: &str,
        path: Option<String>,
    ) -> Option<()> {
        let mut channels = self.channels.write();
        if let Some(managed) = channels.get_mut(&id) {
            match image_type {
                "profile" => {
                    managed.config.custom_profile_image = path;
                }
                "banner" => {
                    managed.config.custom_banner_image = path;
                }
                _ => return None,
            }
            Some(())
        } else {
            None
        }
    }

    /** Update a channel's platform image URLs (cached from platform API). */
    pub fn update_platform_images(
        &self,
        id: Uuid,
        profile_url: Option<String>,
        banner_url: Option<String>,
    ) -> Option<()> {
        let mut channels = self.channels.write();
        if let Some(managed) = channels.get_mut(&id) {
            managed.config.platform_profile_url = profile_url;
            managed.config.platform_banner_url = banner_url;
            Some(())
        } else {
            None
        }
    }

    /** Update a channel's enabled status. */
    pub fn set_channel_enabled(&self, id: Uuid, enabled: bool) -> Option<Channel> {
        let mut channels = self.channels.write();
        if let Some(managed) = channels.get_mut(&id) {
            managed.config.enabled = enabled;
            Some(managed.to_channel())
        } else {
            None
        }
    }

    /**
     * Update a channel's configuration.
     *
     * Returns the updated channel and optionally a shutdown sender if recording
     * needs to be stopped (when disabling an actively recording channel).
     */
    pub fn update_channel(
        &self,
        id: Uuid,
        updates: ChannelUpdate,
    ) -> Option<(Channel, Option<mpsc::Sender<()>>)> {
        let mut channels = self.channels.write();
        if let Some(managed) = channels.get_mut(&id) {
            let mut shutdown_tx = None;

            // If disabling a channel that's recording, extract the shutdown sender
            if updates.enabled == Some(false) && managed.recording.is_some() {
                shutdown_tx = managed.recording.take().map(|h| h.shutdown_tx);
                managed.status = ChannelStatus::Offline;
                managed.current_stream = None;
                info!(
                    "Stopping recording for channel {} (disabled via update)",
                    managed.config.name
                );
            }

            if let Some(enabled) = updates.enabled {
                managed.config.enabled = enabled;
            }
            if let Some(quality) = updates.quality {
                managed.config.quality = quality;
            }
            if let Some(name) = updates.name {
                managed.config.name = name;
            }
            if let Some(quota_gb) = updates.quota_gb {
                managed.config.quota_gb = quota_gb;
            }
            if let Some(retention_days) = updates.retention_days {
                managed.config.retention_days = retention_days;
            }

            // Update schedule
            if updates.schedule_enabled.is_some()
                || updates.timezone.is_some()
                || updates.schedule_rules.is_some()
            {
                let schedule = managed.config.schedule.get_or_insert_with(Default::default);
                if let Some(enabled) = updates.schedule_enabled {
                    schedule.enabled = enabled;
                }
                if let Some(tz) = updates.timezone {
                    schedule.timezone = Some(tz);
                }
                if let Some(rules) = updates.schedule_rules {
                    schedule.rules = rules;
                }
            }

            // Update filters (None = don't change, Some(None) = clear, Some(Some(x)) = set)
            if let Some(filters) = updates.filters {
                managed.config.filters = filters;
            }

            Some((managed.to_channel(), shutdown_tx))
        } else {
            None
        }
    }

    /**
     * Stop recording for a channel and disable it (pause).
     *
     * This stops any active recording and sets enabled=false to prevent
     * auto-recording until the user re-enables the channel.
     * The channel status remains "Live" if the streamer is still live.
     */
    pub async fn stop_recording(&self, id: Uuid) -> anyhow::Result<Option<Channel>> {
        let shutdown_tx = {
            let mut channels = self.channels.write();
            let managed = match channels.get_mut(&id) {
                Some(m) => m,
                None => return Ok(None),
            };

            // Disable the channel (pause) - prevents auto-recording
            managed.config.enabled = false;

            // Change status from Recording to Live (streamer is still live, just not recording)
            // Keep current_stream info so UI shows what they're streaming
            if managed.status == ChannelStatus::Recording {
                managed.status = ChannelStatus::Live;
            }

            info!(
                "Stopping recording for channel {} and disabling",
                managed.config.name
            );

            // Extract the shutdown sender if recording is active
            managed.recording.take().map(|h| h.shutdown_tx)
        };

        // Send shutdown signal if recording was active
        if let Some(tx) = shutdown_tx {
            let _ = tx.send(()).await;
        }

        // Emit status change event (Recording -> Live, not Offline)
        let channel = self.get_channel(id);
        if let Some(ref ch) = channel {
            let _ = self.event_tx.send(ManagerEvent::StatusChanged {
                channel_id: id,
                channel_name: ch.name.clone(),
                platform: ch.platform,
                old_status: ChannelStatus::Recording,
                new_status: ChannelStatus::Live,
            });
        }

        Ok(channel)
    }

    /** Check a single channel and start recording if live. */
    pub async fn check_channel(&self, id: Uuid) -> anyhow::Result<ChannelStatus> {
        // Get channel info while holding the lock briefly
        let (config, current_status, has_recording) = {
            let channels = self.channels.read();
            let channel = channels
                .get(&id)
                .ok_or_else(|| anyhow::anyhow!("Channel not found"))?;

            if !channel.config.enabled {
                return Ok(channel.status);
            }

            (
                channel.config.clone(),
                channel.status,
                channel.recording.is_some(),
            )
        };

        // Create platform instance (with auth if available)
        let platform: Box<dyn StreamPlatform> = match config.platform {
            Platform::Twitch => self.create_platform(Platform::Twitch),
            Platform::YouTube => self.create_platform(Platform::YouTube),
            Platform::Kick => self.create_platform(Platform::Kick),
        };

        // Check if live
        let stream_info = match platform.check_live(&config.name).await {
            Ok(info) => {
                debug!(
                    channel = %config.name,
                    is_live = info.is_some(),
                    title = info.as_ref().map(|i| i.title.as_str()).unwrap_or("N/A"),
                    "check_live result"
                );
                info
            }
            Err(e) => {
                warn!(channel = %config.name, error = %e, "check_live FAILED");
                // Update last error
                {
                    let mut channels = self.channels.write();
                    if let Some(managed) = channels.get_mut(&id) {
                        managed.last_error = Some(e.to_string());
                        managed.last_checked = Some(Utc::now());
                    }
                }
                return Err(e.into());
            }
        };

        // Update last checked time
        {
            let mut channels = self.channels.write();
            if let Some(managed) = channels.get_mut(&id) {
                managed.last_checked = Some(Utc::now());
                managed.last_error = None;
            }
        }

        match stream_info {
            Some(info) => {
                // Stream is live
                debug!("Channel {} is live: {}", config.name, info.title);

                // Update status and stream info
                let old_status = {
                    let mut channels = self.channels.write();
                    if let Some(managed) = channels.get_mut(&id) {
                        let old = managed.status;
                        managed.current_stream = Some(info.clone());

                        if !has_recording {
                            managed.status = ChannelStatus::Live;
                        }
                        old
                    } else {
                        current_status
                    }
                };

                // Emit status change if needed
                if old_status != ChannelStatus::Live && old_status != ChannelStatus::Recording {
                    let _ = self.event_tx.send(ManagerEvent::StatusChanged {
                        channel_id: id,
                        channel_name: config.name.clone(),
                        platform: config.platform,
                        old_status,
                        new_status: ChannelStatus::Live,
                    });
                }

                // Start recording if not already recording
                if !has_recording {
                    // Check schedule
                    if let Some(schedule) = &config.schedule {
                        if !self.schedule_checker.is_within_schedule(schedule) {
                            debug!(
                                channel = %config.name,
                                "Skipping recording: outside schedule"
                            );
                            let _ = self.event_tx.send(ManagerEvent::ScheduleSkip {
                                channel_id: id,
                                channel_name: config.name.clone(),
                                platform: config.platform.to_string(),
                            });
                            return Ok(ChannelStatus::Live);
                        }
                    }

                    // Check filters
                    if let Some(filters) = &config.filters {
                        let metadata = StreamMetadata {
                            title: info.title.clone(),
                            game: info.game.clone(),
                            viewer_count: Some(info.viewer_count),
                        };
                        let decision = self.filter_matcher.matches(filters, &metadata);
                        if !decision.should_record {
                            debug!(
                                channel = %config.name,
                                reason = ?decision.reason,
                                "Skipping recording: filter not matched"
                            );
                            let _ = self.event_tx.send(ManagerEvent::FilterSkip {
                                channel_id: id,
                                channel_name: config.name.clone(),
                                platform: config.platform.to_string(),
                                reason: decision.reason,
                            });
                            return Ok(ChannelStatus::Live);
                        }
                    }

                    // Check quota
                    if let Some(quota_gb) = config.quota_gb {
                        let usage = self.storage_manager.get_channel_usage(&config.name).await;
                        let limit_bytes = quota_gb as u64 * 1024 * 1024 * 1024;

                        if usage >= limit_bytes {
                            debug!(
                                channel = %config.name,
                                usage_bytes = usage,
                                limit_bytes = limit_bytes,
                                "Skipping recording: quota exceeded"
                            );
                            let _ = self.event_tx.send(ManagerEvent::QuotaSkip {
                                channel_id: id,
                                channel_name: config.name.clone(),
                                platform: config.platform.to_string(),
                                quota_used_bytes: usage,
                                quota_limit_bytes: limit_bytes,
                            });
                            return Ok(ChannelStatus::Live);
                        }
                    }

                    self.start_recording(id, &config, platform.as_ref(), Some(&info))
                        .await?;
                }

                Ok(ChannelStatus::Recording)
            }
            None => {
                // Stream is offline
                debug!(channel = %config.name, "Stream is offline");
                let old_status = {
                    let mut channels = self.channels.write();
                    if let Some(managed) = channels.get_mut(&id) {
                        let old = managed.status;
                        // Only change to offline if we're not recording
                        // Recording will be stopped by the engine when stream ends
                        if managed.recording.is_none() {
                            debug!(channel = %config.name, old_status = ?old, "Setting status to Offline");
                            managed.status = ChannelStatus::Offline;
                            managed.current_stream = None;
                        } else {
                            debug!(channel = %config.name, "Channel has active recording, keeping current status");
                        }
                        old
                    } else {
                        current_status
                    }
                };

                if old_status != ChannelStatus::Offline && !has_recording {
                    let _ = self.event_tx.send(ManagerEvent::StatusChanged {
                        channel_id: id,
                        channel_name: config.name.clone(),
                        platform: config.platform,
                        old_status,
                        new_status: ChannelStatus::Offline,
                    });
                }

                Ok(ChannelStatus::Offline)
            }
        }
    }

    /** Start recording a channel. */
    async fn start_recording(
        &self,
        channel_id: Uuid,
        config: &ChannelConfig,
        platform: &dyn StreamPlatform,
        stream_info: Option<&StreamInfo>,
    ) -> anyhow::Result<()> {
        // Atomic guard: try to claim this channel for recording start
        // This prevents race conditions where multiple concurrent calls all try to start recording
        {
            let mut starting = self.starting_recordings.lock();
            if !starting.insert(channel_id) {
                debug!(
                    "Recording start already in progress for {}, skipping",
                    config.name
                );
                return Ok(());
            }
        }

        // Ensure we remove from starting_recordings when we're done (success or failure)
        struct StartGuard<'a> {
            channel_id: Uuid,
            starting: &'a Mutex<HashSet<Uuid>>,
        }
        impl Drop for StartGuard<'_> {
            fn drop(&mut self) {
                self.starting.lock().remove(&self.channel_id);
            }
        }
        let _guard = StartGuard {
            channel_id,
            starting: &self.starting_recordings,
        };

        // Check if already recording
        {
            let channels = self.channels.read();
            if let Some(managed) = channels.get(&channel_id) {
                if managed.recording.is_some() {
                    debug!(
                        "Recording already in progress for {}, skipping",
                        config.name
                    );
                    return Ok(());
                }
            }
        }

        let recording_id = Uuid::new_v4();

        // Parse quality preference
        let quality = if config.quality == "best" || config.quality == "source" {
            Quality::source()
        } else {
            Quality {
                name: config.quality.clone(),
                resolution: None,
                bandwidth: None,
            }
        };

        // Get stream URL
        let stream_url: StreamUrl = platform.get_stream_url(&config.name, &quality).await?;

        // Create output directory: recordings_dir/platform/channel_name/timestamp
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let output_dir = self
            .recordings_dir
            .join(config.platform.to_string())
            .join(&config.name)
            .join(timestamp.to_string());

        tokio::fs::create_dir_all(&output_dir).await?;

        info!("Starting recording for {} to {:?}", config.name, output_dir);

        // Register recording with StorageManager
        let storage_recording_id = match self
            .storage_manager
            .add_recording(
                &config.name,
                &config.platform.to_string(),
                output_dir.clone(),
                stream_info.map(|s| s.title.clone()),
                stream_info.and_then(|s| s.game.clone()),
                stream_info.and_then(|s| s.thumbnail_url.clone()),
            )
            .await
        {
            Ok(id) => {
                debug!(
                    "Registered recording {} with StorageManager as {}",
                    recording_id, id
                );
                Some(id)
            }
            Err(e) => {
                // Log the error but don't stop the recording
                error!(
                    "Failed to register recording with StorageManager: {}. Recording will proceed but won't be tracked.",
                    e
                );
                None
            }
        };

        // Create recording state
        let state =
            RecordingState::new(&config.name, &config.platform.to_string(), &config.quality);

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        // Get stale timeout from config
        let stale_timeout_secs = {
            let config = self.config.read();
            config.polling.stale_timeout_secs
        };

        // Create and start recording engine
        let (engine, mut event_rx) = RecordingEngine::new(
            stream_url,
            output_dir.clone(),
            state,
            shutdown_rx,
            stale_timeout_secs,
        );

        // Store recording handle
        {
            let mut channels = self.channels.write();
            if let Some(managed) = channels.get_mut(&channel_id) {
                // Final race condition check (belt and suspenders)
                // This should rarely trigger now due to the starting_recordings mutex
                if managed.recording.is_some() {
                    warn!(
                        "Recording already in progress for {}, aborting duplicate",
                        config.name
                    );
                    // Clean up StorageManager registration - delete since no data was captured
                    if let Some(sid) = storage_recording_id {
                        let sm = self.storage_manager.clone();
                        tokio::spawn(async move {
                            if let Err(e) = sm.delete_recording(&sid, true).await {
                                tracing::warn!(
                                    recording_id = %sid,
                                    error = %e,
                                    "Failed to clean up duplicate recording from storage"
                                );
                            }
                        });
                    }
                    return Ok(());
                }

                managed.status = ChannelStatus::Recording;
                managed.recording = Some(RecordingHandle {
                    recording_id,
                    storage_recording_id,
                    output_dir: output_dir.clone(),
                    shutdown_tx,
                    started_at: Utc::now(),
                });
            } else {
                // Channel was removed, clean up and abort - delete since no data was captured
                if let Some(sid) = storage_recording_id {
                    let sm = self.storage_manager.clone();
                    tokio::spawn(async move {
                        if let Err(e) = sm.delete_recording(&sid, true).await {
                            tracing::warn!(
                                recording_id = %sid,
                                error = %e,
                                "Failed to clean up orphaned recording from storage"
                            );
                        }
                    });
                }
                return Ok(());
            }
        }

        // Emit status change to Recording
        let _ = self.event_tx.send(ManagerEvent::StatusChanged {
            channel_id,
            channel_name: config.name.clone(),
            platform: config.platform,
            old_status: ChannelStatus::Live,
            new_status: ChannelStatus::Recording,
        });

        // Emit recording started event
        let _ = self.event_tx.send(ManagerEvent::RecordingStarted {
            channel_id,
            channel_name: config.name.clone(),
            platform: config.platform,
            recording_id,
            output_dir: output_dir.clone(),
        });

        // Spawn the recording engine
        let channel_name = config.name.clone();
        let event_tx = self.event_tx.clone();

        // We need to use a different approach to update state since we can't easily share &self
        // Instead, we'll spawn a task that forwards events
        let event_tx_clone = event_tx.clone();
        let channel_name_engine = channel_name.clone();

        tokio::spawn(async move {
            if let Err(e) = engine.run().await {
                warn!("Recording engine error for {}: {}", channel_name_engine, e);
                let _ = event_tx_clone.send(ManagerEvent::Error {
                    channel_id: Some(channel_id),
                    channel_name: Some(channel_name_engine.clone()),
                    message: e.to_string(),
                });
            }
        });

        // Spawn event forwarder task
        let event_tx_forward = event_tx.clone();
        let channel_name_forward = channel_name.clone();
        let storage_manager_forward = self.storage_manager.clone();

        tokio::spawn(async move {
            let mut total_segments = 0u32;
            let mut total_bytes = 0u64;
            while let Ok(event) = event_rx.recv().await {
                match event {
                    RecordingEvent::SegmentDownloaded {
                        sequence: _,
                        size_bytes,
                    } => {
                        total_segments += 1;
                        total_bytes += size_bytes;

                        // Send progress update every 10 segments
                        if total_segments.is_multiple_of(10) {
                            let _ = event_tx_forward.send(ManagerEvent::RecordingProgress {
                                channel_id,
                                recording_id,
                                segments_downloaded: total_segments,
                                bytes_downloaded: total_bytes,
                            });
                        }
                    }
                    RecordingEvent::StreamEnded => {
                        info!("Stream ended for {}", channel_name_forward);

                        // Update StorageManager with completed recording
                        if let Some(storage_id) = storage_recording_id {
                            // Calculate approximate duration from segment count (assume ~2 sec per segment)
                            let duration_secs = (total_segments as u64) * 2;

                            if total_segments == 0 {
                                // No data captured - delete the empty entry
                                if let Err(e) = storage_manager_forward
                                    .delete_recording(&storage_id, true)
                                    .await
                                {
                                    error!(
                                        "Failed to delete empty recording {} in StorageManager: {}",
                                        storage_id, e
                                    );
                                } else {
                                    debug!(
                                        "Deleted empty recording {} in StorageManager (no segments)",
                                        storage_id
                                    );
                                }
                            } else {
                                // Recording ended normally - mark as pending processing
                                // so it will be picked up by the reconciliation worker
                                if let Err(e) = storage_manager_forward
                                    .mark_pending_processing(
                                        &storage_id,
                                        duration_secs,
                                        total_bytes,
                                        total_segments,
                                    )
                                    .await
                                {
                                    error!(
                                        "Failed to mark recording {} as pending in StorageManager: {}",
                                        storage_id, e
                                    );
                                } else {
                                    debug!(
                                        "Marked recording {} as pending processing: {} bytes, {} segments",
                                        storage_id, total_bytes, total_segments
                                    );
                                }
                            }
                        }

                        let _ = event_tx_forward.send(ManagerEvent::RecordingEnded {
                            channel_id,
                            channel_name: channel_name_forward.clone(),
                            recording_id,
                            total_segments,
                            total_bytes,
                        });
                        break;
                    }
                    RecordingEvent::InitSegmentDownloaded { size_bytes } => {
                        // Init segment is downloaded once for fMP4/CMAF streams
                        total_bytes += size_bytes;
                        debug!("Init segment downloaded ({} bytes)", size_bytes);
                    }
                    RecordingEvent::Error { message } => {
                        let _ = event_tx_forward.send(ManagerEvent::Error {
                            channel_id: Some(channel_id),
                            channel_name: Some(channel_name_forward.clone()),
                            message,
                        });
                    }
                    RecordingEvent::PlaylistRefreshed { .. } => {}
                }
            }
        });

        // Note: The main polling loop will detect when recording is done and clean up
        // by checking if the shutdown channel is closed in cleanup_finished_recordings()
        let _ = (event_tx, channel_name); // suppress unused warnings

        Ok(())
    }

    /**
     * Run the polling loop.
     *
     * This method runs indefinitely, checking all channels at the configured interval.
     * It will stop when a shutdown signal is received.
     * Note: Initial poll should be done separately BEFORE starting the HTTP server
     * to ensure channels have correct status when clients first connect.
     */
    pub async fn run_polling_loop(&self, mut shutdown_rx: mpsc::Receiver<()>) {
        info!(
            "Starting channel polling loop with {}s interval",
            self.poll_interval
        );

        loop {
            tokio::select! {
                biased;

                _ = shutdown_rx.recv() => {
                    info!("Polling loop received shutdown signal");
                    break;
                }

                _ = tokio::time::sleep(tokio::time::Duration::from_secs(self.poll_interval)) => {
                    self.poll_all_channels().await;
                }
            }
        }

        // Stop all active recordings
        self.stop_all_recordings().await;

        info!("Polling loop stopped");
    }

    /** Poll all enabled channels. */
    pub async fn poll_all_channels(&self) {
        let channel_ids: Vec<Uuid> = {
            let channels = self.channels.read();
            channels
                .iter()
                .filter(|(_, m)| m.config.enabled)
                .map(|(id, _)| *id)
                .collect()
        };

        debug!("Polling {} channels", channel_ids.len());

        for id in channel_ids {
            // Get channel name for logging
            let channel_name = {
                let channels = self.channels.read();
                channels
                    .get(&id)
                    .map(|m| m.config.name.clone())
                    .unwrap_or_default()
            };
            match self.check_channel(id).await {
                Ok(_status) => {}
                Err(e) => {
                    warn!("Error checking channel {} ({}): {}", channel_name, id, e);

                    // Get channel name for the error event
                    let channel_name = {
                        let channels = self.channels.read();
                        channels.get(&id).map(|m| m.config.name.clone())
                    };

                    let _ = self.event_tx.send(ManagerEvent::Error {
                        channel_id: Some(id),
                        channel_name,
                        message: e.to_string(),
                    });
                }
            }

            // Small delay between channel checks to avoid rate limiting
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // Clean up finished recordings
        self.cleanup_finished_recordings().await;
    }

    /**
     * Poll all channels with debouncing to prevent excessive API calls.
     *
     * This is called when a WebSocket client connects. It ensures fresh status
     * without overwhelming platform APIs when multiple clients connect simultaneously.
     */
    pub async fn poll_all_channels_debounced(&self) {
        const MIN_POLL_INTERVAL_SECS: u64 = 10;

        let should_poll = {
            let last_poll = self.last_poll_time.read();
            match *last_poll {
                None => true,
                Some(time) => time.elapsed().as_secs() >= MIN_POLL_INTERVAL_SECS,
            }
        };

        if should_poll {
            info!("Running debounced poll (triggered by WebSocket connect)");
            self.poll_all_channels().await;
            *self.last_poll_time.write() = Some(std::time::Instant::now());
        } else {
            debug!(
                "Skipping poll - last poll was less than {}s ago",
                MIN_POLL_INTERVAL_SECS
            );
        }
    }

    /** Stop all active recordings. */
    async fn stop_all_recordings(&self) {
        let recording_handles: Vec<(Uuid, mpsc::Sender<()>)> = {
            let mut channels = self.channels.write();
            channels
                .iter_mut()
                .filter_map(|(id, m)| m.recording.take().map(|h| (*id, h.shutdown_tx)))
                .collect()
        };

        for (id, shutdown_tx) in recording_handles {
            info!("Stopping recording for channel {}", id);
            let _ = shutdown_tx.send(()).await;
        }
    }

    /** Clean up finished recordings. */
    async fn cleanup_finished_recordings(&self) {
        let mut channels = self.channels.write();

        for managed in channels.values_mut() {
            if let Some(ref handle) = managed.recording {
                // Check if the shutdown channel is closed (recording ended)
                if handle.shutdown_tx.is_closed() {
                    info!(
                        "Cleaning up finished recording for channel {}",
                        managed.config.name
                    );
                    managed.recording = None;
                    managed.status = ChannelStatus::Offline;
                    managed.current_stream = None;
                }
            }
        }
    }

    /** Get the number of active recordings. */
    pub fn active_recording_count(&self) -> u32 {
        let channels = self.channels.read();
        channels.values().filter(|m| m.recording.is_some()).count() as u32
    }

    /** Update quota status for a channel. */
    pub fn update_quota_status(
        &self,
        channel_id: Uuid,
        status: QuotaStatus,
        used_bytes: u64,
        percent: u8,
    ) -> bool {
        let mut channels = self.channels.write();
        if let Some(managed) = channels.get_mut(&channel_id) {
            managed.quota_status = status;
            managed.quota_used_bytes = used_bytes;
            managed.quota_percent = percent;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{QuotaConfig, RetentionConfig, StorageConfig};
    use tempfile::TempDir;

    fn create_test_config() -> Arc<RwLock<Config>> {
        Arc::new(RwLock::new(Config::default()))
    }

    fn create_test_storage_config(recordings_dir: PathBuf) -> StorageConfig {
        StorageConfig {
            recordings_dir: recordings_dir.clone(),
            library_dir: recordings_dir.join("library"),
            images_dir: recordings_dir.join("images"),
            disk_warning_threshold: 90,
            quotas: QuotaConfig {
                global_max_gb: Some(100),
                per_channel_max_gb: Some(10),
                warn_at_percent: 80,
            },
            retention: RetentionConfig {
                max_age_days: Some(30),
                keep_minimum: 2,
                cleanup_interval_hours: 6,
            },
        }
    }

    fn create_test_channel_config(name: &str, platform: Platform) -> ChannelConfig {
        ChannelConfig {
            name: name.to_string(),
            platform,
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
        }
    }

    #[tokio::test]
    async fn test_channel_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );
        assert!(manager.get_channels().is_empty());
    }

    #[tokio::test]
    async fn test_add_channel() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );

        let config = create_test_channel_config("testchannel", Platform::Twitch);

        let id = manager.add_channel(config.clone());

        // Adding same channel again should return the same ID
        let id2 = manager.add_channel(config);
        assert_eq!(id, id2);

        let channels = manager.get_channels();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "testchannel");
    }

    #[tokio::test]
    async fn test_get_channel() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );

        let config = create_test_channel_config("testchannel", Platform::Twitch);

        let id = manager.add_channel(config);

        let channel = manager.get_channel(id);
        assert!(channel.is_some());
        assert_eq!(channel.unwrap().name, "testchannel");

        let nonexistent = manager.get_channel(Uuid::new_v4());
        assert!(nonexistent.is_none());
    }

    #[tokio::test]
    async fn test_set_channel_enabled() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );

        let config = create_test_channel_config("testchannel", Platform::Twitch);

        let id = manager.add_channel(config);

        let channel = manager.set_channel_enabled(id, false);
        assert!(channel.is_some());
        assert!(!channel.unwrap().enabled);

        let channel = manager.get_channel(id);
        assert!(!channel.unwrap().enabled);
    }

    #[tokio::test]
    async fn test_remove_channel() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );

        let config = create_test_channel_config("removable_channel", Platform::Twitch);

        let id = manager.add_channel(config);
        assert_eq!(manager.get_channels().len(), 1);

        let removed = manager.remove_channel(id);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().0.name, "removable_channel");

        assert!(manager.get_channels().is_empty());
        assert!(manager.get_channel(id).is_none());
    }

    #[tokio::test]
    async fn test_remove_nonexistent_channel() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );

        let result = manager.remove_channel(Uuid::new_v4());
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_update_channel_quality() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );

        let config = create_test_channel_config("quality_channel", Platform::Twitch);

        let id = manager.add_channel(config);

        let update = ChannelUpdate {
            name: None,
            enabled: None,
            quality: Some("720p".to_string()),
            quota_gb: None,
            retention_days: None,
            schedule_enabled: None,
            timezone: None,
            schedule_rules: None,
            filters: None,
        };

        let result = manager.update_channel(id, update);
        assert!(result.is_some());
        let (channel, shutdown_tx) = result.unwrap();
        assert_eq!(channel.quality, "720p");
        assert!(shutdown_tx.is_none()); // No recording to stop
    }

    #[tokio::test]
    async fn test_update_channel_name() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );

        let mut config = create_test_channel_config("old_name", Platform::YouTube);
        config.quality = "1080p".to_string();

        let id = manager.add_channel(config);

        let update = ChannelUpdate {
            name: Some("new_name".to_string()),
            enabled: None,
            quality: None,
            quota_gb: None,
            retention_days: None,
            schedule_enabled: None,
            timezone: None,
            schedule_rules: None,
            filters: None,
        };

        let result = manager.update_channel(id, update);
        assert!(result.is_some());
        let (channel, _) = result.unwrap();
        assert_eq!(channel.name, "new_name");
    }

    #[tokio::test]
    async fn test_update_channel_quota() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );

        let config = create_test_channel_config("quota_channel", Platform::Kick);

        let id = manager.add_channel(config);

        let update = ChannelUpdate {
            name: None,
            enabled: None,
            quality: None,
            quota_gb: Some(Some(10)),
            retention_days: Some(Some(30)),
            schedule_enabled: None,
            timezone: None,
            schedule_rules: None,
            filters: None,
        };

        let result = manager.update_channel(id, update);
        assert!(result.is_some());
        let (channel, _) = result.unwrap();
        assert_eq!(channel.quota_gb, Some(10));
        assert_eq!(channel.retention_days, Some(30));
    }

    #[tokio::test]
    async fn test_update_channel_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );

        let update = ChannelUpdate {
            name: Some("whatever".to_string()),
            enabled: None,
            quality: None,
            quota_gb: None,
            retention_days: None,
            schedule_enabled: None,
            timezone: None,
            schedule_rules: None,
            filters: None,
        };

        let result = manager.update_channel(Uuid::new_v4(), update);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_update_quota_status() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );

        let mut config = create_test_channel_config("status_channel", Platform::Twitch);
        config.quota_gb = Some(10);

        let id = manager.add_channel(config);

        // Update quota status
        let updated = manager.update_quota_status(id, QuotaStatus::Warning, 8_000_000_000, 80);
        assert!(updated);

        // Verify the status was updated
        let channel = manager.get_channel(id).unwrap();
        assert_eq!(channel.quota_status, QuotaStatus::Warning);
        assert_eq!(channel.quota_used_bytes, 8_000_000_000);
        assert_eq!(channel.quota_percent, 80);
    }

    #[tokio::test]
    async fn test_update_quota_status_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );

        let result = manager.update_quota_status(Uuid::new_v4(), QuotaStatus::Ok, 0, 0);
        assert!(!result);
    }

    #[tokio::test]
    async fn test_active_recording_count() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );

        // Initially no recordings
        assert_eq!(manager.active_recording_count(), 0);

        // Add a channel (but no recording yet)
        let config = create_test_channel_config("count_channel", Platform::Twitch);

        manager.add_channel(config);

        // Still no active recordings (just a channel)
        assert_eq!(manager.active_recording_count(), 0);
    }

    #[tokio::test]
    async fn test_add_multiple_channels() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );

        let channels_data = [
            ("channel1", Platform::Twitch),
            ("channel2", Platform::YouTube),
            ("channel3", Platform::Kick),
        ];

        for (name, platform) in &channels_data {
            let config = create_test_channel_config(name, *platform);
            manager.add_channel(config);
        }

        let channels = manager.get_channels();
        assert_eq!(channels.len(), 3);

        // Verify each channel exists
        for (name, _) in &channels_data {
            let exists = channels.iter().any(|c| c.name == *name);
            assert!(exists, "Channel {} should exist", name);
        }
    }

    #[tokio::test]
    async fn test_duplicate_channel_same_platform() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );

        let config1 = create_test_channel_config("same_name", Platform::Twitch);

        let mut config2 = create_test_channel_config("same_name", Platform::Twitch);
        config2.enabled = false; // Different settings
        config2.quality = "720p".to_string();

        let id1 = manager.add_channel(config1);
        let id2 = manager.add_channel(config2);

        // Same name + platform = same channel
        assert_eq!(id1, id2);
        assert_eq!(manager.get_channels().len(), 1);
    }

    #[tokio::test]
    async fn test_same_name_different_platform() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );

        let config1 = create_test_channel_config("same_name", Platform::Twitch);
        let config2 = create_test_channel_config("same_name", Platform::YouTube);

        let id1 = manager.add_channel(config1);
        let id2 = manager.add_channel(config2);

        // Same name but different platform = different channels
        assert_ne!(id1, id2);
        assert_eq!(manager.get_channels().len(), 2);
    }

    #[tokio::test]
    async fn test_get_channel_configs() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = create_test_storage_config(temp_dir.path().to_path_buf());
        let storage_manager = Arc::new(StorageManager::new(storage_config).await.unwrap());

        let (manager, _rx) = ChannelManager::new(
            temp_dir.path().to_path_buf(),
            60,
            storage_manager,
            create_test_config(),
        );

        let mut config = create_test_channel_config("config_test", Platform::Twitch);
        config.quality = "1080p".to_string();
        config.quota_gb = Some(20);
        config.retention_days = Some(14);

        manager.add_channel(config.clone());

        let configs = manager.get_channel_configs();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].name, "config_test");
        assert_eq!(configs[0].quality, "1080p");
        assert_eq!(configs[0].quota_gb, Some(20));
    }
}
