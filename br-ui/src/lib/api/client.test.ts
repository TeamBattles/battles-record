import { describe, it, expect, vi, beforeEach } from 'vitest';
import { ApiClient } from './client';

// Helper to mock fetch responses
function mockFetchResponse(data: unknown, ok = true, status = 200) {
	return vi.mocked(fetch).mockResolvedValueOnce({
		ok,
		status,
		statusText: ok ? 'OK' : 'Error',
		json: () => Promise.resolve(data)
	} as Response);
}

describe('ApiClient', () => {
	let client: ApiClient;

	beforeEach(() => {
		client = new ApiClient('http://localhost:8080');
		vi.clearAllMocks();
	});

	describe('constructor and configuration', () => {
		it('creates client with default base URL', () => {
			const defaultClient = new ApiClient();
			expect(defaultClient.getBaseUrl()).toBe('http://localhost:8080');
		});

		it('normalizes base URL by removing trailing slashes', () => {
			const client1 = new ApiClient('http://localhost:8080/');
			expect(client1.getBaseUrl()).toBe('http://localhost:8080');

			const client2 = new ApiClient('http://localhost:8080///');
			expect(client2.getBaseUrl()).toBe('http://localhost:8080');
		});

		it('stores and retrieves token', () => {
			expect(client.getToken()).toBeNull();

			client.setToken('test-token');
			expect(client.getToken()).toBe('test-token');

			client.setToken(null);
			expect(client.getToken()).toBeNull();
		});

		it('updates base URL', () => {
			client.setBaseUrl('http://newhost:9000/');
			expect(client.getBaseUrl()).toBe('http://newhost:9000');
		});
	});

	describe('fetch wrapper', () => {
		it('adds Content-Type header', async () => {
			mockFetchResponse({ data: {} });
			client.setToken('jwt-token');

			await client.getStatus();

			expect(fetch).toHaveBeenCalledWith(
				'http://localhost:8080/api/status',
				expect.objectContaining({
					headers: {
						'Content-Type': 'application/json',
						Authorization: 'Bearer jwt-token'
					}
				})
			);
		});

		it('includes Authorization header when token is set', async () => {
			mockFetchResponse({ data: {} });
			client.setToken('my-jwt-token');

			await client.getStatus();

			expect(fetch).toHaveBeenCalledWith(
				expect.any(String),
				expect.objectContaining({
					headers: expect.objectContaining({
						Authorization: 'Bearer my-jwt-token'
					})
				})
			);
		});

		it('does not include Authorization header when no token', async () => {
			mockFetchResponse({ data: {} });

			await client.getStatus();

			const call = vi.mocked(fetch).mock.calls[0];
			const headers = call[1]?.headers as Record<string, string>;
			expect(headers.Authorization).toBeUndefined();
		});

		it('throws error on non-OK response', async () => {
			mockFetchResponse({ error: 'Not found' }, false, 404);

			// Error extraction now prioritizes errorBody.error
			await expect(client.getStatus()).rejects.toThrow('Not found');
		});

		it('throws error on server error', async () => {
			mockFetchResponse({ error: 'Server error' }, false, 500);

			// Error extraction now prioritizes errorBody.error
			await expect(client.getStatus()).rejects.toThrow('Server error');
		});
	});

	describe('transformChannel', () => {
		it('transforms backend offline channel to frontend format', async () => {
			mockFetchResponse({
				data: {
					channels: [
						{
							id: 'ch-1',
							name: 'test_channel',
							platform: 'twitch',
							enabled: true,
							quality: 'best',
							status: 'offline'
						}
					]
				}
			});

			const channels = await client.getChannels();

			expect(channels).toHaveLength(1);
			expect(channels[0]).toEqual({
				id: 'ch-1',
				name: 'test_channel',
				platform: 'twitch',
				enabled: true,
				quality: 'best',
				status: {
					is_live: false,
					is_recording: false,
					current_stream: undefined
				},
				quota_gb: undefined,
				retention_days: undefined,
				quota_status: undefined,
				quota_used_bytes: undefined,
				quota_percent: undefined,
				schedule_enabled: undefined,
				timezone: undefined,
				schedule_rules: undefined,
				filters: undefined
			});
		});

		it('transforms backend live channel to frontend format', async () => {
			mockFetchResponse({
				data: {
					channels: [
						{
							id: 'ch-2',
							name: 'live_channel',
							platform: 'youtube',
							enabled: true,
							quality: '1080p',
							status: 'live',
							current_stream: {
								title: 'Live Stream',
								game: 'Gaming',
								viewer_count: 1000,
								started_at: '2024-01-15T10:00:00Z'
							}
						}
					]
				}
			});

			const channels = await client.getChannels();

			expect(channels[0].status).toEqual({
				is_live: true,
				is_recording: false,
				current_stream: {
					title: 'Live Stream',
					game: 'Gaming',
					viewer_count: 1000,
					started_at: '2024-01-15T10:00:00Z'
				}
			});
		});

		it('transforms backend recording channel to frontend format', async () => {
			mockFetchResponse({
				data: {
					channels: [
						{
							id: 'ch-3',
							name: 'recording_channel',
							platform: 'kick',
							enabled: true,
							quality: '720p',
							status: 'recording',
							current_stream: {
								title: 'Recording Stream',
								started_at: '2024-01-15T09:00:00Z'
							}
						}
					]
				}
			});

			const channels = await client.getChannels();

			expect(channels[0].status).toEqual({
				is_live: true,
				is_recording: true,
				current_stream: {
					title: 'Recording Stream',
					started_at: '2024-01-15T09:00:00Z'
				}
			});
		});

		it('preserves quota fields in transformation', async () => {
			mockFetchResponse({
				data: {
					channels: [
						{
							id: 'ch-4',
							name: 'quota_channel',
							platform: 'twitch',
							enabled: true,
							quality: 'best',
							status: 'offline',
							quota_gb: 10,
							retention_days: 30,
							quota_status: 'warning',
							quota_used_bytes: 8_500_000_000,
							quota_percent: 85
						}
					]
				}
			});

			const channels = await client.getChannels();

			expect(channels[0].quota_gb).toBe(10);
			expect(channels[0].retention_days).toBe(30);
			expect(channels[0].quota_status).toBe('warning');
			expect(channels[0].quota_used_bytes).toBe(8_500_000_000);
			expect(channels[0].quota_percent).toBe(85);
		});
	});

	describe('auth operations', () => {
		it('login sends correct request and returns tokens', async () => {
			mockFetchResponse({
				data: {
					token: 'jwt-token',
					role: 'admin',
					expires_at: '2024-12-31T23:59:59Z'
				}
			});

			const result = await client.login('admin', 'password123');

			expect(fetch).toHaveBeenCalledWith(
				'http://localhost:8080/api/auth/login',
				expect.objectContaining({
					method: 'POST',
					body: JSON.stringify({ username: 'admin', password: 'password123' })
				})
			);
			expect(result).toEqual({
				token: 'jwt-token',
				role: 'admin',
				expires_at: '2024-12-31T23:59:59Z'
			});
		});

		it('checkHealth does not require auth', async () => {
			vi.mocked(fetch).mockResolvedValueOnce({
				ok: true,
				json: () => Promise.resolve({ status: 'ok', version: '1.0.0' })
			} as Response);

			const result = await client.checkHealth();

			expect(result).toEqual({ status: 'ok', version: '1.0.0' });
			// Should not include Authorization header
			const call = vi.mocked(fetch).mock.calls[0];
			expect(call[0]).toBe('http://localhost:8080/health');
		});

		it('checkHealth throws on failure', async () => {
			vi.mocked(fetch).mockResolvedValueOnce({
				ok: false,
				status: 503
			} as Response);

			await expect(client.checkHealth()).rejects.toThrow('Health check failed: 503');
		});
	});

	describe('channel operations', () => {
		beforeEach(() => {
			client.setToken('test-token');
		});

		it('getChannels returns transformed channels', async () => {
			mockFetchResponse({
				data: {
					channels: [
						{
							id: 'ch-1',
							name: 'channel1',
							platform: 'twitch',
							enabled: true,
							quality: 'best',
							status: 'offline'
						},
						{
							id: 'ch-2',
							name: 'channel2',
							platform: 'youtube',
							enabled: false,
							quality: '720p',
							status: 'live'
						}
					]
				}
			});

			const channels = await client.getChannels();

			expect(channels).toHaveLength(2);
			expect(channels[0].name).toBe('channel1');
			expect(channels[1].name).toBe('channel2');
		});

		it('getChannel returns single transformed channel', async () => {
			mockFetchResponse({
				data: {
					id: 'ch-1',
					name: 'test_channel',
					platform: 'twitch',
					enabled: true,
					quality: 'best',
					status: 'offline'
				}
			});

			const channel = await client.getChannel('ch-1');

			expect(fetch).toHaveBeenCalledWith(
				'http://localhost:8080/api/channels/ch-1',
				expect.any(Object)
			);
			expect(channel.id).toBe('ch-1');
		});

		it('createChannel sends POST and returns new channel', async () => {
			mockFetchResponse({
				data: {
					id: 'ch-new',
					channel: {
						id: 'ch-new',
						name: 'new_channel',
						platform: 'twitch',
						enabled: true,
						quality: 'best',
						status: 'offline'
					}
				}
			});

			const channel = await client.createChannel({
				name: 'new_channel',
				platform: 'twitch',
				quality: 'best'
			});

			expect(fetch).toHaveBeenCalledWith(
				'http://localhost:8080/api/channels',
				expect.objectContaining({
					method: 'POST',
					body: JSON.stringify({ name: 'new_channel', platform: 'twitch', quality: 'best' })
				})
			);
			expect(channel.name).toBe('new_channel');
		});

		it('updateChannel sends PUT and returns updated channel', async () => {
			mockFetchResponse({
				data: {
					id: 'ch-1',
					name: 'updated_channel',
					platform: 'twitch',
					enabled: false,
					quality: '720p',
					status: 'offline'
				}
			});

			const channel = await client.updateChannel('ch-1', { enabled: false, quality: '720p' });

			expect(fetch).toHaveBeenCalledWith(
				'http://localhost:8080/api/channels/ch-1',
				expect.objectContaining({
					method: 'PUT',
					body: JSON.stringify({ enabled: false, quality: '720p' })
				})
			);
			expect(channel.enabled).toBe(false);
		});

		it('deleteChannel sends DELETE request', async () => {
			mockFetchResponse({});

			await client.deleteChannel('ch-1');

			expect(fetch).toHaveBeenCalledWith(
				'http://localhost:8080/api/channels/ch-1',
				expect.objectContaining({ method: 'DELETE' })
			);
		});

		it('checkChannel sends POST and returns updated channel', async () => {
			mockFetchResponse({
				data: {
					channel: {
						id: 'ch-1',
						name: 'test_channel',
						platform: 'twitch',
						enabled: true,
						quality: 'best',
						status: 'live'
					},
					message: 'Channel is live'
				}
			});

			const channel = await client.checkChannel('ch-1');

			expect(fetch).toHaveBeenCalledWith(
				'http://localhost:8080/api/channels/ch-1/check',
				expect.objectContaining({ method: 'POST' })
			);
			expect(channel.status.is_live).toBe(true);
		});

		it('stopRecording sends POST and returns updated channel', async () => {
			mockFetchResponse({
				data: {
					channel: {
						id: 'ch-1',
						name: 'test_channel',
						platform: 'twitch',
						enabled: false,
						quality: 'best',
						status: 'live'
					},
					message: 'Recording stopped'
				}
			});

			const channel = await client.stopRecording('ch-1');

			expect(fetch).toHaveBeenCalledWith(
				'http://localhost:8080/api/channels/ch-1/stop-recording',
				expect.objectContaining({ method: 'POST' })
			);
			expect(channel.status.is_recording).toBe(false);
		});
	});

	describe('recording operations', () => {
		beforeEach(() => {
			client.setToken('test-token');
		});

		it('getRecordings returns recordings list', async () => {
			mockFetchResponse({
				data: {
					recordings: [
						{
							id: 'rec-1',
							channel_name: 'test_channel',
							platform: 'twitch',
							started_at: '2024-01-15T10:00:00Z',
							status: 'processed',
							path: '/recordings/test',
							size_bytes: 1000000
						}
					]
				}
			});

			const recordings = await client.getRecordings();

			expect(recordings).toHaveLength(1);
			expect(recordings[0].id).toBe('rec-1');
		});

		it('deleteRecording sends DELETE request', async () => {
			mockFetchResponse({});

			await client.deleteRecording('rec-1');

			expect(fetch).toHaveBeenCalledWith(
				'http://localhost:8080/api/recordings/rec-1',
				expect.objectContaining({ method: 'DELETE' })
			);
		});

		it('processRecording sends POST request', async () => {
			mockFetchResponse({});

			await client.processRecording('rec-1');

			expect(fetch).toHaveBeenCalledWith(
				'http://localhost:8080/api/recordings/rec-1/process',
				expect.objectContaining({
					method: 'POST',
					body: JSON.stringify({})
				})
			);
		});

		it('reprocessRecording sends POST request', async () => {
			mockFetchResponse({});

			await client.reprocessRecording('rec-1');

			expect(fetch).toHaveBeenCalledWith(
				'http://localhost:8080/api/recordings/rec-1/reprocess',
				expect.objectContaining({ method: 'POST' })
			);
		});
	});

	describe('storage operations', () => {
		beforeEach(() => {
			client.setToken('test-token');
		});

		it('getStorageStats returns storage stats', async () => {
			mockFetchResponse({
				data: {
					total_recordings: 10,
					total_size_bytes: 50_000_000_000,
					disk_free_bytes: 200_000_000_000,
					disk_total_bytes: 500_000_000_000,
					per_channel: [],
					recordings_dir: '/recordings',
					library_dir: '/library',
					library_size_bytes: 45_000_000_000
				}
			});

			const stats = await client.getStorageStats();

			expect(stats.total_recordings).toBe(10);
			expect(stats.total_size_bytes).toBe(50_000_000_000);
		});

		it('cleanupStorage sends POST with cleanup request', async () => {
			mockFetchResponse({
				data: {
					recordings_affected: 5,
					bytes_to_free: 10_000_000_000,
					dry_run: true
				}
			});

			const result = await client.cleanupStorage({ older_than_days: 30, dry_run: true });

			expect(fetch).toHaveBeenCalledWith(
				'http://localhost:8080/api/storage/cleanup',
				expect.objectContaining({
					method: 'POST',
					body: JSON.stringify({ older_than_days: 30, dry_run: true })
				})
			);
			expect(result.recordings_affected).toBe(5);
		});
	});

	describe('post-processing config', () => {
		beforeEach(() => {
			client.setToken('test-token');
		});

		it('getPostProcessingConfig returns config', async () => {
			mockFetchResponse({
				data: {
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
				}
			});

			const config = await client.getPostProcessingConfig();

			expect(config.enabled).toBe(true);
			expect(config.encoding.crf).toBe(23);
		});

		it('updatePostProcessingConfig sends PUT request', async () => {
			mockFetchResponse({
				data: {
					enabled: false,
					check_interval_minutes: 10,
					output_format: 'mp4_copy',
					segment_handling: 'keep',
					encoding: {
						crf: 20,
						preset: 'fast',
						video_codec: 'libx264',
						audio_codec: 'aac',
						audio_bitrate: '192k'
					},
					max_concurrent: 1
				}
			});

			const config = await client.updatePostProcessingConfig({ enabled: false, max_concurrent: 1 });

			expect(fetch).toHaveBeenCalledWith(
				'http://localhost:8080/api/config/post-processing',
				expect.objectContaining({
					method: 'PUT',
					body: JSON.stringify({ enabled: false, max_concurrent: 1 })
				})
			);
			expect(config.enabled).toBe(false);
		});
	});
});
