/**
 * Channel name extraction and validation utilities.
 * Handles parsing URLs to extract usernames and validating channel names.
 */

type Platform = 'twitch' | 'youtube' | 'kick';

/**
 * Extracts channel name/username from a URL or returns input as-is if already a username.
 */
export function extractChannelName(platform: Platform, input: string): string {
	const trimmed = input.trim();
	if (!trimmed) return '';

	// Remove protocol
	let normalized = trimmed.replace(/^https?:\/\//, '');

	// Remove query params and fragments
	normalized = normalized.split('?')[0].split('#')[0];

	// Remove trailing slash
	normalized = normalized.replace(/\/$/, '');

	// Platform-specific extraction
	switch (platform) {
		case 'twitch':
			return extractTwitchUsername(normalized);
		case 'youtube':
			return extractYoutubeChannel(normalized);
		case 'kick':
			return extractKickUsername(normalized);
		default:
			return trimmed;
	}
}

function extractTwitchUsername(input: string): string {
	// Match [any-subdomains.]twitch.tv/{username}[/anything-else]
	// Strips all subdomains and extracts only the first path segment
	const twitchMatch = input.match(/^(?:[^/]*\.)?twitch\.tv\/([^/]+)/i);
	if (twitchMatch) {
		return twitchMatch[1];
	}
	// If it looks like a URL but not Twitch, return as-is (will fail validation)
	if (input.includes('/')) {
		return input;
	}
	return input;
}

function extractYoutubeChannel(input: string): string {
	// Match [any-subdomains.]youtube.com/@{handle}[/anything]
	const handleMatch = input.match(/^(?:[^/]*\.)?youtube\.com\/@([^/]+)/i);
	if (handleMatch) {
		return '@' + handleMatch[1];
	}

	// Match [any-subdomains.]youtube.com/channel/{id}[/anything]
	const channelMatch = input.match(/^(?:[^/]*\.)?youtube\.com\/channel\/([^/]+)/i);
	if (channelMatch) {
		return channelMatch[1];
	}

	// Match [any-subdomains.]youtube.com/c/{custom}[/anything]
	const customMatch = input.match(/^(?:[^/]*\.)?youtube\.com\/c\/([^/]+)/i);
	if (customMatch) {
		return customMatch[1];
	}

	// If it looks like a URL but not YouTube, return as-is
	if (input.includes('/')) {
		return input;
	}
	return input;
}

function extractKickUsername(input: string): string {
	// Match [any-subdomains.]kick.com/{username}[/anything]
	const kickMatch = input.match(/^(?:[^/]*\.)?kick\.com\/([^/]+)/i);
	if (kickMatch) {
		return kickMatch[1];
	}
	// If it looks like a URL but not Kick, return as-is
	if (input.includes('/')) {
		return input;
	}
	return input;
}

export interface ValidationResult {
	valid: boolean;
	warning?: string;
}

/**
 * Validates a channel name for the given platform.
 * Returns { valid: true } if valid, or { valid: false, warning: "message" } if invalid.
 */
export function validateChannelName(platform: Platform, name: string): ValidationResult {
	if (!name || name.trim().length === 0) {
		return { valid: false, warning: 'Channel name is required' };
	}

	const trimmed = name.trim();

	// Check if it looks like an unrecognized URL
	if (trimmed.includes('/') || trimmed.includes('.')) {
		return { valid: false, warning: 'Unrecognized URL format' };
	}

	switch (platform) {
		case 'twitch':
			return validateTwitchUsername(trimmed);
		case 'youtube':
			return validateYoutubeChannel(trimmed);
		case 'kick':
			return validateKickUsername(trimmed);
		default:
			return { valid: true };
	}
}

function validateTwitchUsername(name: string): ValidationResult {
	// Twitch usernames: 4-25 characters, alphanumeric and underscores only
	if (name.length < 4) {
		return { valid: false, warning: 'Twitch username must be at least 4 characters' };
	}
	if (name.length > 25) {
		return { valid: false, warning: 'Twitch username must be at most 25 characters' };
	}
	if (!/^[a-zA-Z0-9_]+$/.test(name)) {
		return {
			valid: false,
			warning: 'Twitch username can only contain letters, numbers, and underscores'
		};
	}
	return { valid: true };
}

function validateYoutubeChannel(name: string): ValidationResult {
	// YouTube handles start with @, 3-30 characters
	if (name.startsWith('@')) {
		const handle = name.slice(1);
		if (handle.length < 3) {
			return { valid: false, warning: 'YouTube handle must be at least 3 characters' };
		}
		if (handle.length > 30) {
			return { valid: false, warning: 'YouTube handle must be at most 30 characters' };
		}
		if (!/^[a-zA-Z0-9._-]+$/.test(handle)) {
			return {
				valid: false,
				warning:
					'YouTube handle can only contain letters, numbers, periods, hyphens, and underscores'
			};
		}
		return { valid: true };
	}

	// YouTube channel IDs are 24 characters starting with UC
	if (name.startsWith('UC') && name.length === 24) {
		return { valid: true };
	}

	// Custom channel names - be lenient since legacy formats vary
	if (name.length < 1) {
		return { valid: false, warning: 'Channel name is required' };
	}
	if (name.length > 100) {
		return { valid: false, warning: 'Channel name is too long' };
	}

	return { valid: true };
}

function validateKickUsername(name: string): ValidationResult {
	// Kick usernames: similar to Twitch, 3-25 characters, alphanumeric and underscores
	if (name.length < 3) {
		return { valid: false, warning: 'Kick username must be at least 3 characters' };
	}
	if (name.length > 25) {
		return { valid: false, warning: 'Kick username must be at most 25 characters' };
	}
	if (!/^[a-zA-Z0-9_]+$/.test(name)) {
		return {
			valid: false,
			warning: 'Kick username can only contain letters, numbers, and underscores'
		};
	}
	return { valid: true };
}
