// Re-export BackendChannel and transformChannel from the canonical location
export { type BackendChannel, transformChannel } from '$lib/api/backend-types';

/**
 * Normalize URL by removing trailing slashes
 */
export function normalizeUrl(url: string): string {
	return url.replace(/\/+$/, '');
}

/**
 * Convert HTTP URL to WebSocket URL
 */
export function convertToWsUrl(httpUrl: string): string {
	return httpUrl.replace(/^http/, 'ws');
}

/**
 * Unwrap API response to get data
 */
export function unwrapApiResponse<T>(response: { data: T }): T {
	return response.data;
}

/**
 * Build query string from params object
 */
export function buildQueryString(
	params: Record<string, string | number | boolean | undefined>
): string {
	const filtered = Object.entries(params)
		.filter(([, value]) => value !== undefined)
		.map(([key, value]) => `${encodeURIComponent(key)}=${encodeURIComponent(String(value))}`);

	return filtered.length > 0 ? `?${filtered.join('&')}` : '';
}
