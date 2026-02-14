// Reconnection settings
export const RECONNECT_DELAYS = [1000, 2000, 4000, 8000, 16000, 30000] as const;
export const MAX_RECONNECT_TIME = 120000; // 2 minutes

// Activity limits
export const MAX_ACTIVITY_EVENTS = 1000;

// Token settings
export const TOKEN_EXPIRY_WARNING_DAYS = 7;
export const TOKEN_DEFAULT_LIFETIME_DAYS = 30;

// Platform list
export const PLATFORMS = [
	{ id: 'twitch', label: 'Twitch' },
	{ id: 'youtube', label: 'YouTube' },
	{ id: 'kick', label: 'Kick' }
] as const;

export type Platform = (typeof PLATFORMS)[number]['id'];

// Platform profile URL bases
export const PLATFORM_PROFILE_URLS: Record<Platform, string> = {
	twitch: 'https://twitch.tv/',
	youtube: 'https://youtube.com/@',
	kick: 'https://kick.com/'
};

// Quality options
export const QUALITY_OPTIONS = [
	'Best Available',
	'1080p60',
	'1080p',
	'720p60',
	'720p',
	'480p',
	'360p',
	'Audio Only'
] as const;

// Timezone options
export const TIMEZONE_OPTIONS = [
	{ value: 'UTC', label: 'UTC' },
	{ value: 'America/New_York', label: 'Eastern Time (ET)' },
	{ value: 'America/Chicago', label: 'Central Time (CT)' },
	{ value: 'America/Denver', label: 'Mountain Time (MT)' },
	{ value: 'America/Los_Angeles', label: 'Pacific Time (PT)' },
	{ value: 'Europe/London', label: 'London (GMT/BST)' },
	{ value: 'Europe/Paris', label: 'Central European (CET)' },
	{ value: 'Europe/Berlin', label: 'Berlin (CET)' },
	{ value: 'Asia/Tokyo', label: 'Japan (JST)' },
	{ value: 'Asia/Shanghai', label: 'China (CST)' },
	{ value: 'Australia/Sydney', label: 'Sydney (AEST)' }
] as const;

// Output format options
export const OUTPUT_FORMATS = [
	{ value: 'mp4_reencode', label: 'MP4 (Re-encode)' },
	{ value: 'mp4_copy', label: 'MP4 (Stream Copy)' },
	{ value: 'ts_concat', label: 'TS (Concatenate)' }
] as const;

// Status colors for recordings
export const RECORDING_STATUS_COLORS: Record<string, string> = {
	recording: 'bg-orange-500',
	stopping: 'bg-amber-500',
	pending_processing: 'bg-blue-500',
	processing: 'bg-blue-500',
	processed: 'bg-emerald-500',
	processing_failed: 'bg-orange-500',
	failed: 'bg-red-500',
	completed: 'bg-emerald-500'
} as const;

// Status colors for channels
export const CHANNEL_STATUS_COLORS = {
	live: 'bg-emerald-500',
	recording: 'bg-orange-500',
	offline: 'bg-zinc-500'
} as const;

// Status text colors
export const RECORDING_STATUS_TEXT_COLORS: Record<string, string> = {
	recording: 'text-orange-400',
	stopping: 'text-amber-400',
	pending_processing: 'text-blue-400',
	processing: 'text-blue-400',
	processed: 'text-emerald-400',
	processing_failed: 'text-orange-400',
	failed: 'text-red-400',
	completed: 'text-emerald-400'
} as const;

// CRF slider settings
export const CRF_RANGE = { min: 0, max: 51, default: 23 } as const;

// Encoding presets (FFmpeg x264/x265)
export const ENCODING_PRESETS = [
	'ultrafast',
	'superfast',
	'veryfast',
	'faster',
	'fast',
	'medium',
	'slow',
	'slower',
	'veryslow'
] as const;

// Video codecs
export const VIDEO_CODECS = ['libx264', 'libx265', 'copy'] as const;

// Audio codecs
export const AUDIO_CODECS = ['aac', 'libmp3lame', 'copy'] as const;

// Days of week for schedule
export const DAYS_OF_WEEK = [
	{ value: 0, label: 'Sun', fullLabel: 'Sunday' },
	{ value: 1, label: 'Mon', fullLabel: 'Monday' },
	{ value: 2, label: 'Tue', fullLabel: 'Tuesday' },
	{ value: 3, label: 'Wed', fullLabel: 'Wednesday' },
	{ value: 4, label: 'Thu', fullLabel: 'Thursday' },
	{ value: 5, label: 'Fri', fullLabel: 'Friday' },
	{ value: 6, label: 'Sat', fullLabel: 'Saturday' }
] as const;

// Pagination defaults
export const DEFAULT_PAGE_SIZE = 20;
export const PAGE_SIZE_OPTIONS = [10, 20, 50, 100] as const;

// UI animation durations (ms)
export const ANIMATION_DURATION = {
	fast: 150,
	normal: 200,
	slow: 300
} as const;

// Polling intervals (ms)
export const POLLING_INTERVALS = {
	status: 5000, // Status updates
	channels: 10000, // Channel list refresh
	recordings: 15000 // Recordings list refresh
} as const;

// Download status colors (map to StatusDot status strings)
export const DOWNLOAD_STATUS_COLORS: Record<string, string> = {
	queued: 'warning',
	extracting_info: 'info',
	waiting_for_format: 'info',
	downloading: 'recording',
	processing: 'recording',
	paused: 'warning',
	complete: 'success',
	cancelled: 'offline',
	failed: 'error'
};

export const DOWNLOAD_STATUS_LABELS: Record<string, string> = {
	queued: 'Queued',
	extracting_info: 'Extracting Info',
	waiting_for_format: 'Waiting for Format',
	downloading: 'Downloading',
	processing: 'Processing',
	paused: 'Paused',
	complete: 'Complete',
	cancelled: 'Cancelled',
	failed: 'Failed'
};
