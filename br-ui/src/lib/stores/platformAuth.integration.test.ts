/**
 * PlatformAuth Store Integration Tests
 *
 * Tests for the platform authentication store which manages
 * credentials for Twitch, YouTube, and Kick.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { platformAuthStore } from './platformAuth.svelte';
import type { PlatformAuth, Platform } from '$lib/api/types';

// Mock the API
vi.mock('$lib/api', () => ({
	api: {
		getPlatformAuth: vi.fn(),
		setPlatformAuth: vi.fn(),
		deletePlatformAuth: vi.fn(),
		testPlatformAuth: vi.fn()
	}
}));

// Mock toast store
vi.mock('./toast.svelte', () => ({
	toastStore: {
		success: vi.fn(),
		error: vi.fn()
	}
}));

import { api } from '$lib/api';
import { toastStore } from './toast.svelte';

// Test data
const mockPlatformAuth: PlatformAuth[] = [
	{
		platform: 'twitch',
		status: 'connected',
		username: 'twitch_user',
		expires_at: new Date(Date.now() + 86400000 * 30).toISOString(), // 30 days
		last_validated: new Date().toISOString()
	},
	{
		platform: 'youtube',
		status: 'expired',
		username: 'youtube_user',
		expires_at: new Date(Date.now() - 86400000).toISOString() // Expired yesterday
	},
	{
		platform: 'kick',
		status: 'not_connected'
	}
];

// Helper to reset store state
function resetStore() {
	platformAuthStore.platforms = [];
	platformAuthStore.isLoading = false;
	platformAuthStore.error = null;
	platformAuthStore.testingPlatform = null;
}

describe('PlatformAuthStore', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		resetStore();
	});

	afterEach(() => {
		resetStore();
	});

	describe('initial state', () => {
		it('starts with empty platforms', () => {
			expect(platformAuthStore.platforms).toHaveLength(0);
			expect(platformAuthStore.connectedCount).toBe(0);
		});

		it('has no error initially', () => {
			expect(platformAuthStore.error).toBeNull();
		});

		it('is not loading initially', () => {
			expect(platformAuthStore.isLoading).toBe(false);
		});
	});

	describe('load()', () => {
		it('fetches platform auth list', async () => {
			(api.getPlatformAuth as ReturnType<typeof vi.fn>).mockResolvedValue(mockPlatformAuth);

			await platformAuthStore.load();

			expect(api.getPlatformAuth).toHaveBeenCalled();
			expect(platformAuthStore.platforms).toHaveLength(3);
		});

		it('sets isLoading during fetch', async () => {
			let resolveLoad: (value: PlatformAuth[]) => void;
			(api.getPlatformAuth as ReturnType<typeof vi.fn>).mockReturnValue(
				new Promise((resolve) => {
					resolveLoad = resolve;
				})
			);

			const loadPromise = platformAuthStore.load();
			expect(platformAuthStore.isLoading).toBe(true);

			resolveLoad!(mockPlatformAuth);
			await loadPromise;

			expect(platformAuthStore.isLoading).toBe(false);
		});

		it('handles error', async () => {
			(api.getPlatformAuth as ReturnType<typeof vi.fn>).mockRejectedValue(
				new Error('Network error')
			);

			await platformAuthStore.load();

			expect(platformAuthStore.error).toBe('Network error');
			expect(platformAuthStore.platforms).toHaveLength(0);
		});
	});

	describe('platform getters', () => {
		beforeEach(async () => {
			(api.getPlatformAuth as ReturnType<typeof vi.fn>).mockResolvedValue(mockPlatformAuth);
			await platformAuthStore.load();
		});

		it('twitch getter returns twitch platform', () => {
			expect(platformAuthStore.twitch?.platform).toBe('twitch');
			expect(platformAuthStore.twitch?.status).toBe('connected');
		});

		it('youtube getter returns youtube platform', () => {
			expect(platformAuthStore.youtube?.platform).toBe('youtube');
			expect(platformAuthStore.youtube?.status).toBe('expired');
		});

		it('kick getter returns kick platform', () => {
			expect(platformAuthStore.kick?.platform).toBe('kick');
			expect(platformAuthStore.kick?.status).toBe('not_connected');
		});

		it('connectedCount returns correct count', () => {
			expect(platformAuthStore.connectedCount).toBe(1); // Only twitch is connected
		});
	});

	describe('setCredentials()', () => {
		beforeEach(async () => {
			(api.getPlatformAuth as ReturnType<typeof vi.fn>).mockResolvedValue(mockPlatformAuth);
			await platformAuthStore.load();
		});

		it('updates existing platform', async () => {
			const newAuth = {
				platform: 'youtube' as Platform,
				status: 'connected' as const,
				username: 'new_youtube_user',
				expires_at: new Date(Date.now() + 86400000 * 30).toISOString()
			};
			(api.setPlatformAuth as ReturnType<typeof vi.fn>).mockResolvedValue(newAuth);

			const result = await platformAuthStore.setCredentials('youtube', {
				access_token: 'new-token',
				refresh_token: 'new-refresh'
			});

			expect(result).toBe(true);
			expect(platformAuthStore.youtube?.status).toBe('connected');
			expect(platformAuthStore.youtube?.username).toBe('new_youtube_user');
		});

		it('adds new platform if not exists', async () => {
			// Start fresh with only kick not connected
			platformAuthStore.platforms = [{ platform: 'kick', status: 'not_connected' }];

			const newAuth = {
				platform: 'twitch' as Platform,
				status: 'connected' as const,
				username: 'new_twitch_user'
			};
			(api.setPlatformAuth as ReturnType<typeof vi.fn>).mockResolvedValue(newAuth);

			await platformAuthStore.setCredentials('twitch', { access_token: 'token' });

			expect(platformAuthStore.platforms).toHaveLength(2);
			expect(platformAuthStore.twitch?.username).toBe('new_twitch_user');
		});

		it('shows success toast on success', async () => {
			(api.setPlatformAuth as ReturnType<typeof vi.fn>).mockResolvedValue({
				platform: 'twitch',
				status: 'connected',
				username: 'user'
			});

			await platformAuthStore.setCredentials('twitch', { access_token: 'token' });

			expect(toastStore.success).toHaveBeenCalledWith('Twitch connected successfully');
		});

		it('shows error toast on failure', async () => {
			(api.setPlatformAuth as ReturnType<typeof vi.fn>).mockRejectedValue(
				new Error('Invalid token')
			);

			const result = await platformAuthStore.setCredentials('twitch', {
				access_token: 'bad-token'
			});

			expect(result).toBe(false);
			expect(toastStore.error).toHaveBeenCalledWith('Invalid token');
		});
	});

	describe('disconnect()', () => {
		beforeEach(async () => {
			(api.getPlatformAuth as ReturnType<typeof vi.fn>).mockResolvedValue(mockPlatformAuth);
			await platformAuthStore.load();
		});

		it('removes platform credentials', async () => {
			(api.deletePlatformAuth as ReturnType<typeof vi.fn>).mockResolvedValue({
				platform: 'twitch',
				deleted: true
			});

			const result = await platformAuthStore.disconnect('twitch');

			expect(result).toBe(true);
			expect(platformAuthStore.twitch?.status).toBe('not_connected');
			expect(platformAuthStore.twitch?.username).toBeUndefined();
		});

		it('shows success toast', async () => {
			(api.deletePlatformAuth as ReturnType<typeof vi.fn>).mockResolvedValue({
				platform: 'twitch',
				deleted: true
			});

			await platformAuthStore.disconnect('twitch');

			expect(toastStore.success).toHaveBeenCalledWith('Twitch disconnected');
		});

		it('shows error toast on failure', async () => {
			(api.deletePlatformAuth as ReturnType<typeof vi.fn>).mockRejectedValue(
				new Error('Disconnect failed')
			);

			const result = await platformAuthStore.disconnect('twitch');

			expect(result).toBe(false);
			expect(toastStore.error).toHaveBeenCalledWith('Disconnect failed');
		});
	});

	describe('testConnection()', () => {
		beforeEach(async () => {
			(api.getPlatformAuth as ReturnType<typeof vi.fn>).mockResolvedValue(mockPlatformAuth);
			await platformAuthStore.load();
		});

		it('sets testingPlatform during test', async () => {
			let resolveTest: (value: { platform: Platform; success: boolean; message: string }) => void;
			(api.testPlatformAuth as ReturnType<typeof vi.fn>).mockReturnValue(
				new Promise((resolve) => {
					resolveTest = resolve;
				})
			);

			const testPromise = platformAuthStore.testConnection('twitch');
			expect(platformAuthStore.testingPlatform).toBe('twitch');

			resolveTest!({ platform: 'twitch', success: true, message: 'OK' });
			await testPromise;

			expect(platformAuthStore.testingPlatform).toBeNull();
		});

		it('reloads on successful test', async () => {
			(api.testPlatformAuth as ReturnType<typeof vi.fn>).mockResolvedValue({
				platform: 'twitch',
				success: true,
				message: 'Connection successful'
			});
			vi.clearAllMocks();
			(api.getPlatformAuth as ReturnType<typeof vi.fn>).mockResolvedValue(mockPlatformAuth);

			await platformAuthStore.testConnection('twitch');

			expect(api.getPlatformAuth).toHaveBeenCalled();
			expect(toastStore.success).toHaveBeenCalledWith('Connection successful');
		});

		it('shows error toast on failed test', async () => {
			(api.testPlatformAuth as ReturnType<typeof vi.fn>).mockResolvedValue({
				platform: 'twitch',
				success: false,
				message: 'Token invalid'
			});

			await platformAuthStore.testConnection('twitch');

			expect(toastStore.error).toHaveBeenCalledWith('Token invalid');
		});

		it('handles API error', async () => {
			(api.testPlatformAuth as ReturnType<typeof vi.fn>).mockRejectedValue(
				new Error('Network error')
			);

			const result = await platformAuthStore.testConnection('twitch');

			expect(result).toBeNull();
			expect(toastStore.error).toHaveBeenCalledWith('Network error');
		});
	});

	describe('getExpiryInfo()', () => {
		it('returns empty for not_connected', () => {
			const auth: PlatformAuth = { platform: 'kick', status: 'not_connected' };
			const info = platformAuthStore.getExpiryInfo(auth);

			expect(info.text).toBe('');
			expect(info.isExpired).toBe(false);
			expect(info.isExpiringSoon).toBe(false);
		});

		it('returns expired for expired status', () => {
			const auth: PlatformAuth = {
				platform: 'twitch',
				status: 'expired',
				expires_at: new Date(Date.now() - 86400000).toISOString()
			};
			const info = platformAuthStore.getExpiryInfo(auth);

			expect(info.text).toBe('Token expired');
			expect(info.isExpired).toBe(true);
		});

		it('returns days remaining for far expiry', () => {
			const auth: PlatformAuth = {
				platform: 'twitch',
				status: 'connected',
				expires_at: new Date(Date.now() + 86400000 * 45).toISOString() // 45 days
			};
			const info = platformAuthStore.getExpiryInfo(auth);

			expect(info.text).toContain('days');
			expect(info.isExpired).toBe(false);
			expect(info.isExpiringSoon).toBe(false);
		});

		it('returns expiring soon for less than 7 days', () => {
			const auth: PlatformAuth = {
				platform: 'twitch',
				status: 'connected',
				expires_at: new Date(Date.now() + 86400000 * 5).toISOString() // 5 days
			};
			const info = platformAuthStore.getExpiryInfo(auth);

			expect(info.isExpiringSoon).toBe(true);
		});

		it('returns hours remaining for less than a day', () => {
			const auth: PlatformAuth = {
				platform: 'twitch',
				status: 'connected',
				expires_at: new Date(Date.now() + 3600000 * 5).toISOString() // 5 hours
			};
			const info = platformAuthStore.getExpiryInfo(auth);

			expect(info.text).toContain('hours');
			expect(info.isExpiringSoon).toBe(true);
		});

		it('returns minutes remaining for less than an hour', () => {
			const auth: PlatformAuth = {
				platform: 'twitch',
				status: 'connected',
				expires_at: new Date(Date.now() + 60000 * 30).toISOString() // 30 minutes
			};
			const info = platformAuthStore.getExpiryInfo(auth);

			expect(info.text).toContain('minutes');
			expect(info.isExpiringSoon).toBe(true);
		});

		it('handles no expiry (permanent token)', () => {
			const auth: PlatformAuth = {
				platform: 'twitch',
				status: 'connected'
				// No expires_at
			};
			const info = platformAuthStore.getExpiryInfo(auth);

			expect(info.text).toContain('permanent');
			expect(info.isExpired).toBe(false);
		});

		it('handles null auth', () => {
			const info = platformAuthStore.getExpiryInfo(null);

			expect(info.text).toBe('');
			expect(info.isExpired).toBe(false);
		});
	});
});
