import { vi } from 'vitest';
import type {
	Channel,
	Recording,
	DaemonStatus,
	AuthTokens,
	StorageStats,
	CleanupResponse,
	PostProcessingConfig,
	PlatformAuth,
	User
} from '$lib/api/types';
import { mockChannels, mockRecordings } from './fixtures/channels';

export function createMockApiClient() {
	return {
		setToken: vi.fn(),
		getToken: vi.fn(() => 'test-token'),
		getTokenExpiry: vi.fn(() => Date.now() + 86400000), // 24 hours from now
		setBaseUrl: vi.fn(),
		getBaseUrl: vi.fn(() => 'http://localhost:8080'),
		refreshToken: vi.fn().mockResolvedValue(true),
		onAuthFailure: null as ((code: string) => void) | null,
		onTokenRefreshed: null as ((token: string, expiry: number) => void) | null,

		// Auth
		login: vi.fn().mockResolvedValue({
			token: 'mock-jwt-token',
			role: 'admin',
			expires_at: '2024-12-31T23:59:59Z'
		} satisfies AuthTokens),

		checkHealth: vi.fn().mockResolvedValue({
			status: 'ok',
			version: '1.0.0'
		}),

		getStatus: vi.fn().mockResolvedValue({
			version: '1.0.0',
			uptime_secs: 3600,
			disk: {
				recordings_path: '/data/recordings',
				total_bytes: 100_000_000_000,
				used_bytes: 10_000_000_000,
				usage_percent: 10.0
			},
			channels: {
				total: 3,
				enabled: 2,
				recording: 1,
				live_not_recording: 1
			},
			processing_queue: {
				active: null,
				active_progress_percent: null,
				queued: 0
			}
		} satisfies DaemonStatus),

		// Channels
		getChannels: vi.fn().mockResolvedValue([...mockChannels]),
		getChannel: vi.fn().mockImplementation((id: string) => {
			const channel = mockChannels.find((c) => c.id === id);
			if (!channel) {
				return Promise.reject(new Error('Channel not found'));
			}
			return Promise.resolve({ ...channel });
		}),
		createChannel: vi.fn().mockImplementation((data: Partial<Channel>) =>
			Promise.resolve({
				id: `ch-${Date.now()}`,
				name: data.name || 'new_channel',
				platform: data.platform || 'twitch',
				enabled: data.enabled ?? true,
				quality: data.quality || 'best',
				status: { is_live: false, is_recording: false },
				...data
			} as Channel)
		),
		updateChannel: vi.fn().mockImplementation((id: string, data: Partial<Channel>) => {
			const channel = mockChannels.find((c) => c.id === id);
			if (!channel) {
				return Promise.reject(new Error('Channel not found'));
			}
			return Promise.resolve({ ...channel, ...data });
		}),
		deleteChannel: vi.fn().mockResolvedValue(undefined),
		checkChannel: vi.fn().mockImplementation((id: string) => {
			const channel = mockChannels.find((c) => c.id === id);
			if (!channel) {
				return Promise.reject(new Error('Channel not found'));
			}
			return Promise.resolve({ ...channel });
		}),
		stopRecording: vi.fn().mockImplementation((id: string) => {
			const channel = mockChannels.find((c) => c.id === id);
			if (!channel) {
				return Promise.reject(new Error('Channel not found'));
			}
			return Promise.resolve({
				...channel,
				status: { ...channel.status, is_recording: false }
			});
		}),

		// Recordings
		getRecordings: vi.fn().mockResolvedValue([...mockRecordings]),
		deleteRecording: vi.fn().mockResolvedValue(undefined),
		processRecording: vi.fn().mockResolvedValue(undefined),
		reprocessRecording: vi.fn().mockResolvedValue(undefined),

		// Platform Auth
		getPlatformAuth: vi.fn().mockResolvedValue([
			{ platform: 'twitch', status: 'connected', username: 'test_user' },
			{ platform: 'youtube', status: 'not_connected' },
			{ platform: 'kick', status: 'not_connected' }
		] satisfies PlatformAuth[]),
		getPlatformAuthStatus: vi.fn(),
		setPlatformAuth: vi.fn(),
		deletePlatformAuth: vi.fn(),
		testPlatformAuth: vi.fn(),

		// Users
		getUsers: vi.fn().mockResolvedValue([
			{ id: 1, username: 'admin', role: 'admin', is_online: true },
			{ id: 2, username: 'viewer', role: 'viewer', is_online: false }
		] satisfies User[]),
		createUser: vi.fn(),
		updateUser: vi.fn(),
		deleteUser: vi.fn(),
		getUserSessions: vi.fn().mockResolvedValue([]),
		revokeAllUserSessions: vi.fn(),
		revokeUserSession: vi.fn(),

		// Storage
		getStorageStats: vi.fn().mockResolvedValue({
			total_recordings: 10,
			total_size_bytes: 50_000_000_000,
			disk_free_bytes: 200_000_000_000,
			disk_total_bytes: 500_000_000_000,
			per_channel: [
				{ channel: 'test_streamer', platform: 'twitch', count: 5, size_bytes: 25_000_000_000 },
				{ channel: 'kick_streamer', platform: 'kick', count: 5, size_bytes: 25_000_000_000 }
			],
			recordings_dir: '/recordings',
			library_dir: '/library',
			library_size_bytes: 45_000_000_000
		} satisfies StorageStats),
		cleanupStorage: vi.fn().mockResolvedValue({
			recordings_affected: 2,
			bytes_to_free: 5_000_000_000,
			dry_run: true
		} satisfies CleanupResponse),

		// Post-Processing
		getPostProcessingConfig: vi.fn().mockResolvedValue({
			enabled: true,
			check_interval_minutes: 5,
			output_format: 'mp4_reencode',
			segment_handling: 'delete',
			encoding: {
				crf: 23,
				preset: 'medium',
				video_codec: 'libx264',
				audio_codec: 'aac',
				audio_bitrate: '192k'
			},
			max_concurrent: 2
		} satisfies PostProcessingConfig),
		updatePostProcessingConfig: vi.fn()
	};
}

export type MockApiClient = ReturnType<typeof createMockApiClient>;
