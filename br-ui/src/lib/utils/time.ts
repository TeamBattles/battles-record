/**
 * Parse ISO date string safely
 */
export function parseISODate(dateString: string): Date {
	const date = new Date(dateString);
	if (isNaN(date.getTime())) {
		throw new Error(`Invalid date string: ${dateString}`);
	}
	return date;
}

/**
 * Calculate token expiry timestamp (ms since epoch)
 */
export function calculateTokenExpiry(expiresAt?: string): number {
	if (expiresAt) {
		return new Date(expiresAt).getTime();
	}
	// Default to 30 days from now
	return Date.now() + 30 * 24 * 60 * 60 * 1000;
}

/**
 * Get time remaining until token expires
 */
export function getTokenTimeRemaining(expiresAt: string): {
	days: number;
	hours: number;
	minutes: number;
	expired: boolean;
} {
	const now = Date.now();
	const expiry = new Date(expiresAt).getTime();
	const remaining = expiry - now;

	if (remaining <= 0) {
		return { days: 0, hours: 0, minutes: 0, expired: true };
	}

	const days = Math.floor(remaining / (24 * 60 * 60 * 1000));
	const hours = Math.floor((remaining % (24 * 60 * 60 * 1000)) / (60 * 60 * 1000));
	const minutes = Math.floor((remaining % (60 * 60 * 1000)) / (60 * 1000));

	return { days, hours, minutes, expired: false };
}

/**
 * Check if token is expired
 */
export function isTokenExpired(expiresAt?: string): boolean {
	if (!expiresAt) return true;
	return new Date(expiresAt).getTime() <= Date.now();
}

/**
 * Check if token expires within threshold (default: 7 days)
 */
export function isTokenExpiringSoon(expiresAt?: string, thresholdDays: number = 7): boolean {
	if (!expiresAt) return true;
	const threshold = thresholdDays * 24 * 60 * 60 * 1000;
	return new Date(expiresAt).getTime() - Date.now() <= threshold;
}

/**
 * Add days to a date
 */
export function addDays(date: Date, days: number): Date {
	const result = new Date(date);
	result.setDate(result.getDate() + days);
	return result;
}

/**
 * Get relative time string (e.g., "2 hours ago", "in 3 days")
 */
export function getRelativeTime(dateStr: string): string {
	const date = new Date(dateStr);
	const now = Date.now();
	const diff = now - date.getTime();
	const absDiff = Math.abs(diff);

	const minutes = Math.floor(absDiff / (60 * 1000));
	const hours = Math.floor(absDiff / (60 * 60 * 1000));
	const days = Math.floor(absDiff / (24 * 60 * 60 * 1000));

	const suffix = diff > 0 ? 'ago' : 'from now';

	if (minutes < 1) return 'just now';
	if (minutes < 60) return `${minutes}m ${suffix}`;
	if (hours < 24) return `${hours}h ${suffix}`;
	if (days < 7) return `${days}d ${suffix}`;

	return new Date(dateStr).toLocaleDateString();
}
