/**
 * Format bytes to human-readable string (KB, MB, GB)
 */
export function formatBytes(bytes: number, decimals: number = 1): string {
	if (bytes === 0) return '0 B';
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(decimals)} KB`;
	if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(decimals)} MB`;
	return `${(bytes / (1024 * 1024 * 1024)).toFixed(decimals + 1)} GB`;
}

/**
 * Format duration in seconds to human-readable string
 */
export function formatDuration(secs?: number): string {
	if (!secs || secs < 0) return '-';

	const hours = Math.floor(secs / 3600);
	const minutes = Math.floor((secs % 3600) / 60);
	const seconds = Math.floor(secs % 60);

	if (hours > 0) return `${hours}h ${minutes}m`;
	if (minutes > 0) return `${minutes}m ${seconds}s`;
	return `${seconds}s`;
}

/**
 * Format ISO date string to localized short format
 */
export function formatDate(dateStr: string): string {
	return new Date(dateStr).toLocaleDateString(undefined, {
		month: 'short',
		day: 'numeric',
		hour: '2-digit',
		minute: '2-digit'
	});
}

/**
 * Format ISO date string to full datetime
 */
export function formatDatetime(dateStr: string): string {
	return new Date(dateStr).toLocaleString(undefined, {
		year: 'numeric',
		month: 'short',
		day: 'numeric',
		hour: '2-digit',
		minute: '2-digit',
		second: '2-digit'
	});
}

/**
 * Format percentage value
 */
export function formatPercent(value: number, decimals: number = 1): string {
	return `${(value * 100).toFixed(decimals)}%`;
}

/**
 * Format a number with commas for thousands
 */
export function formatNumber(value: number): string {
	return value.toLocaleString();
}

/**
 * Format ETA seconds to human-readable string (e.g. "3m 12s")
 */
export function formatEta(seconds?: number): string {
	if (!seconds || seconds <= 0) return '';
	const m = Math.floor(seconds / 60);
	const s = Math.floor(seconds % 60);
	if (m > 0) return `${m}m ${s}s`;
	return `${s}s`;
}

/**
 * Format a platform identifier to its display name
 */
export function formatPlatformName(platform: string): string {
	const map: Record<string, string> = {
		youtube: 'YouTube',
		twitch: 'Twitch',
		kick: 'Kick',
		instagram: 'Instagram',
		twitter: 'Twitter',
		tiktok: 'TikTok'
	};
	return map[platform.toLowerCase()] ?? platform;
}
