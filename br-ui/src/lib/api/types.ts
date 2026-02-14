export interface ApiResponse<T> {
	data: T;
}

export type QuotaStatus = 'ok' | 'warning' | 'exceeded' | 'unlimited';

export interface Channel {
	id: string;
	name: string;
	platform: 'twitch' | 'youtube' | 'kick';
	enabled: boolean;
	quality: string;
	status: ChannelStatus;
	// Schedule fields
	schedule_enabled?: boolean;
	timezone?: string;
	schedule_rules?: ScheduleRule[];
	// Filter fields
	filters?: ChannelFilters;
	// Storage fields
	quota_gb?: number;
	retention_days?: number;
	// Quota status fields
	quota_status?: QuotaStatus;
	quota_used_bytes?: number;
	quota_percent?: number;
	// Profile image URLs (resolved: custom > platform)
	profile_image_url?: string;
	banner_image_url?: string;
}

export interface ScheduleRule {
	days: number[]; // 0-6, Sunday=0
	start_time: string; // "HH:MM"
	end_time: string; // "HH:MM"
}

export interface ChannelFilters {
	title_includes?: string[];
	title_excludes?: string[];
	game_includes?: string[];
	game_excludes?: string[];
	min_viewers?: number;
}

export interface ChannelStatus {
	is_live: boolean;
	is_recording: boolean;
	current_stream?: StreamInfo;
}

export interface StreamInfo {
	title: string;
	game?: string;
	viewer_count?: number;
	started_at: string;
}

export interface Recording {
	id: string;
	channel_name: string;
	platform: string;
	started_at: string;
	ended_at?: string;
	duration_secs?: number;
	status: RecordingStatus;
	path: string;
	size_bytes: number;
	title?: string;
	game?: string;
	output_file?: string;
	processing_attempts?: number;
	failure_reason?: string;
}

export type RecordingStatus =
	| 'recording'
	| 'stopping'
	| 'pending_processing'
	| 'processing'
	| 'processed'
	| 'processing_failed'
	| 'failed'
	| 'completed';

export interface DiskStatus {
	recordings_path: string;
	total_bytes: number;
	used_bytes: number;
	usage_percent: number;
}

export interface ChannelStats {
	total: number;
	enabled: number;
	recording: number;
	live_not_recording: number;
}

export interface ProcessingQueueStatus {
	active: string | null;
	active_progress_percent: number | null;
	queued: number;
}

export interface UpdateInfo {
	latest_version: string | null;
	update_available: boolean;
	release_url: string | null;
	release_notes: string | null;
}

export interface DaemonStatus {
	version: string;
	uptime_secs: number;
	min_client_version: string;
	max_client_version: string;
	update?: UpdateInfo;
	disk: DiskStatus;
	channels: ChannelStats;
	processing_queue: ProcessingQueueStatus;
}

export interface AuthTokens {
	token: string;
	role: 'admin' | 'viewer';
	expires_at: string;
}

/** Error codes returned by the backend auth endpoints */
export type AuthErrorCode =
	| 'TOKEN_EXPIRED'
	| 'TOKEN_INVALID'
	| 'TOKEN_MISSING'
	| 'UNAUTHORIZED'
	| 'FORBIDDEN';

/** Response from POST /api/auth/refresh */
export interface RefreshTokenResponse {
	token: string;
	role: 'admin' | 'viewer';
	expires_at: string;
}

// Platform Authentication Types
export type Platform = 'twitch' | 'youtube' | 'kick';

export type PlatformAuthStatus = 'connected' | 'expired' | 'not_connected';

export interface PlatformAuth {
	platform: Platform;
	status: PlatformAuthStatus;
	username?: string;
	expires_at?: string;
	last_validated?: string;
}

export interface PlatformAuthListResponse {
	platforms: PlatformAuth[];
}

export interface SetPlatformAuthRequest {
	access_token: string;
	refresh_token?: string;
	expires_at?: string;
	username?: string;
}

export interface SetPlatformAuthResponse {
	platform: Platform;
	status: PlatformAuthStatus;
	username?: string;
	expires_at?: string;
}

export interface TestConnectionResponse {
	platform: Platform;
	success: boolean;
	message: string;
	username?: string;
}

export interface DeletePlatformAuthResponse {
	platform: Platform;
	deleted: boolean;
}

export interface SetYouTubeCookiesRequest {
	cookie_content: string;
}

export interface SetYouTubeCookiesResponse {
	platform: Platform;
	status: PlatformAuthStatus;
	message: string;
}

// OAuth Types
export interface StartOAuthRequest {
	redirect_uri?: string;
	/** Custom client ID for advanced users (uses bundled ID if not provided) */
	client_id?: string;
	/** Custom client secret for advanced users (not needed for PKCE public clients) */
	client_secret?: string;
}

export interface StartOAuthResponse {
	auth_url: string;
	state: string;
}

export interface OAuthCallbackRequest {
	code: string;
	state: string;
}

export interface OAuthCallbackResponse {
	success: boolean;
	platform: Platform;
	status: PlatformAuthStatus;
	username?: string;
	expires_at?: string;
}

export interface OAuthAvailabilityResponse {
	twitch: boolean;
	youtube: boolean;
	kick: boolean;
}

// User Management Types
export type UserRole = 'admin' | 'viewer';

export interface User {
	id: number;
	username: string;
	role: UserRole;
	last_login?: string;
	is_online: boolean;
}

export interface Session {
	id: string;
	user_id: number;
	ip_address?: string;
	user_agent?: string;
	created_at: string;
	last_active: string;
}

export interface CreateUserRequest {
	username: string;
	password: string;
	role?: UserRole;
}

export interface UpdateUserRequest {
	role?: UserRole;
	password?: string;
}

// Storage Types
export interface StorageStats {
	total_recordings: number;
	total_size_bytes: number;
	disk_free_bytes: number;
	disk_total_bytes: number;
	per_channel: ChannelStorageStats[];
	recordings_dir: string;
	library_dir: string;
	/** Size of files in the library directory */
	library_size_bytes: number;
	/** Library disk stats (only present if library_dir is on a different disk) */
	library_disk_free_bytes?: number;
	library_disk_total_bytes?: number;
}

export interface ChannelStorageStats {
	channel: string;
	platform: string;
	count: number;
	size_bytes: number;
}

/** Location to clean up: recordings directory, library directory, or both */
export type CleanupLocation = 'recordings' | 'library' | 'both';

export interface CleanupRequest {
	older_than_days?: number;
	channel_id?: string;
	channel_name?: string;
	status?: RecordingStatus;
	/** Which location to clean up: "recordings", "library", or "both" (default) */
	location?: CleanupLocation;
	dry_run: boolean;
}

export interface CleanupResponse {
	recordings_affected: number;
	bytes_to_free: number;
	recordings?: Recording[];
	dry_run: boolean;
	/** Bytes freed from recordings directory (only in non-dry_run mode) */
	recordings_bytes_freed?: number;
	/** Bytes freed from library directory (only in non-dry_run mode) */
	library_bytes_freed?: number;
}

// Download Storage Types
export interface DownloadStorageStats {
	total_downloads: number;
	total_size_bytes: number;
	downloads_dir: string;
	per_channel: ChannelStorageStats[];
}

export interface DownloadCleanupRequest {
	older_than_days?: number;
	channel_name?: string;
	source_platform?: string;
	dry_run: boolean;
}

export interface DownloadCleanupResponse {
	affected: number;
	bytes_to_free: number;
	downloads?: DownloadSummary[];
	dry_run: boolean;
}

// Post-Processing Config Types
export interface EncodingConfig {
	crf: number;
	preset: string;
	video_codec: string;
	audio_codec: string;
	audio_bitrate: string;
}

export type SegmentHandling = 'delete' | 'concatenate' | 'keep';

export interface PostProcessingConfig {
	enabled: boolean;
	check_interval_minutes: number;
	output_format: 'mp4_reencode' | 'mp4_copy' | 'ts_concat';
	/** What to do with segment files after processing */
	segment_handling: SegmentHandling;
	encoding: EncodingConfig;
	ffmpeg_path?: string;
	max_concurrent: number;
}

// Storage Config Types
export interface StorageConfig {
	recordings_dir: string;
	library_dir: string;
	disk_warning_threshold: number;
}

// Jellyfin Config Types
export interface JellyfinConfig {
	enabled: boolean;
	fetch_profile_images: boolean;
	generate_thumbnails: boolean;
}

// Channel Image Types
export interface ChannelImages {
	platform_profile_url?: string;
	platform_banner_url?: string;
	custom_profile_url?: string;
	custom_banner_url?: string;
}

export interface ChannelProfile {
	channel_id: string;
	display_name: string;
	platform: Platform;
	description?: string;
	platform_profile_url?: string;
	platform_banner_url?: string;
	custom_profile_url?: string;
	custom_banner_url?: string;
}

export interface ImageUploadResponse {
	success: boolean;
	url: string;
}

export interface ImageDeleteResponse {
	deleted: boolean;
}

// YouTube Dependency Types
export interface DependencyInfo {
	available: boolean;
	version: string | null;
}

export interface DependenciesResponse {
	bun: DependencyInfo;
	ytdlp: DependencyInfo;
}

// ─── Downloads ───

export type DownloadStatus =
	| 'queued'
	| 'extracting_info'
	| 'waiting_for_format'
	| 'downloading'
	| 'processing'
	| 'paused'
	| 'complete'
	| 'cancelled'
	| 'failed';

export interface Download {
	id: string;
	url: string;
	title?: string;
	thumbnail?: string;
	duration?: number;
	uploader?: string;
	platform_name?: string;
	channel_name: string;
	source_platform: string;
	output_dir: string;
	output_file?: string;
	format?: string;
	available_formats?: FormatInfo[];
	status: DownloadStatus;
	percent: number;
	speed?: string;
	eta?: number;
	downloaded_bytes: number;
	total_bytes?: number;
	options: DownloadOptions;
	requested_by: string;
	created_at: string;
	completed_at?: string;
	error?: string;
}

export interface DownloadSummary {
	id: string;
	url: string;
	title?: string;
	thumbnail?: string;
	platform_name?: string;
	channel_name: string;
	source_platform: string;
	status: DownloadStatus;
	percent: number;
	speed?: string;
	eta?: number;
	downloaded_bytes: number;
	total_bytes?: number;
	format?: string;
	requested_by: string;
	created_at: string;
	completed_at?: string;
	error?: string;
	update_available: boolean;
}

export interface FormatInfo {
	format_id: string;
	ext: string;
	resolution?: string;
	filesize_approx?: number;
	vcodec?: string;
	acodec?: string;
}

export interface DownloadOptions {
	embed_thumbnail: boolean;
	embed_metadata: boolean;
}

export interface ExtractedInfo {
	title: string;
	duration?: number;
	thumbnail?: string;
	uploader?: string;
	platform_name?: string;
	formats: FormatInfo[];
	webpage_url?: string;
}

export interface CreateDownloadRequest {
	url: string;
	channel_name: string;
	source_platform: string;
	format?: string;
	options?: DownloadOptions;
}

// ─── Extensions ───

export interface ExtensionConnection {
	client_id: string;
	identifier: string;
	paired_at: string;
	last_connected: string;
	connected: boolean;
	connected_at?: string;
}

export interface ExtensionConfig {
	enabled: boolean;
	port: number;
	actual_port?: number;
	fallback_ports: number[];
}

export interface MessageLogEntry {
	timestamp: string;
	direction: 'sent' | 'received';
	message_type: string;
	payload?: string;
}

// ─── Libraries ───

export interface LibraryStatus {
	ytdlp: LibraryInfo;
	ffmpeg: LibraryInfo;
	bun: LibraryInfo;
}

export interface LibraryInfo {
	installed: boolean;
	version?: string;
	path?: string;
	update_available?: string;
}

// ─── Downloads Config ───

export interface DownloadsConfig {
	directory: string;
	max_concurrent: number;
	default_format: string;
	embed_thumbnail: boolean;
	embed_metadata: boolean;
	output_template: string;
	max_total_gb?: number;
	retention: DownloadRetentionConfig;
}

export interface DownloadRetentionConfig {
	max_age_days?: number;
	cleanup_interval_hours: number;
}
