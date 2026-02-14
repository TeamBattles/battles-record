/**
 * Backend API types - raw formats returned by the daemon
 * These are transformed to frontend types before use in the UI
 */

import type { Channel, ChannelStatus, QuotaStatus } from './types';

/**
 * Backend channel format
 * Status is a string enum, not an object like the frontend Channel type
 */
export interface BackendChannel {
	id: string;
	name: string;
	platform: 'twitch' | 'youtube' | 'kick';
	enabled: boolean;
	quality: string;
	status: 'offline' | 'live' | 'recording' | 'error';
	current_stream?: {
		title: string;
		game?: string;
		viewer_count?: number;
		started_at: string;
	};
	// Quota fields
	quota_gb?: number;
	retention_days?: number;
	quota_status?: QuotaStatus;
	quota_used_bytes?: number;
	quota_percent?: number;
	// Schedule fields
	schedule_enabled?: boolean;
	timezone?: string;
	schedule_rules?: { days: number[]; start_time: string; end_time: string }[];
	// Filter fields
	filters?: {
		title_includes?: string[];
		title_excludes?: string[];
		game_includes?: string[];
		game_excludes?: string[];
		min_viewers?: number;
	};
	// Image fields (custom images stored in config)
	custom_profile_image?: string;
	custom_banner_image?: string;
}

/**
 * Transform backend channel to frontend format
 * Converts the string status enum to the object-based ChannelStatus
 */
export function transformChannel(backend: BackendChannel): Channel {
	const status: ChannelStatus = {
		is_live: backend.status === 'live' || backend.status === 'recording',
		is_recording: backend.status === 'recording',
		current_stream: backend.current_stream
	};

	return {
		id: backend.id,
		name: backend.name,
		platform: backend.platform,
		enabled: backend.enabled,
		quality: backend.quality,
		status,
		// Quota fields
		quota_gb: backend.quota_gb,
		retention_days: backend.retention_days,
		quota_status: backend.quota_status,
		quota_used_bytes: backend.quota_used_bytes,
		quota_percent: backend.quota_percent,
		// Schedule fields
		schedule_enabled: backend.schedule_enabled,
		timezone: backend.timezone,
		schedule_rules: backend.schedule_rules,
		// Filter fields
		filters: backend.filters
	};
}
