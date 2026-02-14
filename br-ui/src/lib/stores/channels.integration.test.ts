/**
 * Channels Store Integration Tests
 *
 * These tests verify the channels store behavior through a wrapper component
 * to properly test Svelte 5 $state reactivity.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/svelte';
import { ChannelsStoreWrapper } from '../../tests/wrappers';
import { channelsStore } from './channels.svelte';
import type { Channel } from '$lib/api/types';
import type { BackendChannel } from '$lib/api/backend-types';
import type { WebSocketEvent } from '$lib/api/websocket';

// Mock the API client
vi.mock('$lib/api', () => ({
	api: {
		getChannels: vi.fn(),
		getChannel: vi.fn(),
		createChannel: vi.fn(),
		deleteChannel: vi.fn(),
		updateChannel: vi.fn(),
		checkChannel: vi.fn(),
		stopRecording: vi.fn()
	}
}));

// Mock the WebSocket client
vi.mock('$lib/api/websocket', () => ({
	wsClient: {
		subscribe: vi.fn(() => vi.fn())
	}
}));

// Mock toast store
vi.mock('./toast.svelte', () => ({
	toastStore: {
		success: vi.fn(),
		error: vi.fn(),
		info: vi.fn(),
		warning: vi.fn()
	}
}));

import { api } from '$lib/api';
import { wsClient } from '$lib/api/websocket';
import { toastStore } from './toast.svelte';

// Mock channel data
const mockChannels: Channel[] = [
	{
		id: 'ch-1',
		name: 'streamer_one',
		platform: 'twitch',
		enabled: true,
		quality: 'best',
		status: { is_live: false, is_recording: false }
	},
	{
		id: 'ch-2',
		name: 'streamer_two',
		platform: 'youtube',
		enabled: true,
		quality: '1080p',
		status: { is_live: true, is_recording: false }
	},
	{
		id: 'ch-3',
		name: 'streamer_three',
		platform: 'kick',
		enabled: false,
		quality: 'best',
		status: { is_live: false, is_recording: false }
	}
];

// Helper to reset store state
function resetStore() {
	channelsStore.channels = [];
	channelsStore.isLoading = false;
	channelsStore.error = null;
	channelsStore.platformFilter = 'all';
	channelsStore.searchQuery = '';
	channelsStore.selectedChannelId = null;
}

describe('ChannelsStore Integration', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		resetStore();
	});

	afterEach(() => {
		cleanup();
	});

	describe('initial state', () => {
		it('renders with no channels initially', () => {
			render(ChannelsStoreWrapper);

			expect(screen.getByTestId('channel-count')).toHaveTextContent('0');
			expect(screen.getByTestId('filtered-count')).toHaveTextContent('0');
			expect(screen.getByTestId('is-loading')).toHaveTextContent('false');
		});

		it('shows no error initially', () => {
			render(ChannelsStoreWrapper);

			expect(screen.getByTestId('error')).toHaveTextContent('');
		});

		it('has default filter values', () => {
			render(ChannelsStoreWrapper);

			expect(screen.getByTestId('platform-filter')).toHaveTextContent('all');
			expect(screen.getByTestId('search-query')).toHaveTextContent('');
		});
	});

	describe('load()', () => {
		it('sets isLoading=true during load', async () => {
			// Setup a delayed response
			let resolveLoad: (value: Channel[]) => void;
			(api.getChannels as ReturnType<typeof vi.fn>).mockReturnValue(
				new Promise((resolve) => {
					resolveLoad = resolve;
				})
			);

			render(ChannelsStoreWrapper);

			// Start loading
			const loadPromise = channelsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('is-loading')).toHaveTextContent('true');
			});

			// Resolve the load
			resolveLoad!(mockChannels);
			await loadPromise;

			await waitFor(() => {
				expect(screen.getByTestId('is-loading')).toHaveTextContent('false');
			});
		});

		it('populates channels on success', async () => {
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValue(mockChannels);

			render(ChannelsStoreWrapper);
			await channelsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('channel-count')).toHaveTextContent('3');
			});
		});

		it('sets error on failure', async () => {
			(api.getChannels as ReturnType<typeof vi.fn>).mockRejectedValue(new Error('Network error'));

			render(ChannelsStoreWrapper);
			await channelsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('error')).toHaveTextContent('Network error');
				expect(screen.getByTestId('channel-count')).toHaveTextContent('0');
			});
		});

		it('clears error on successful reload', async () => {
			// First fail
			(api.getChannels as ReturnType<typeof vi.fn>).mockRejectedValueOnce(new Error('Failed'));

			render(ChannelsStoreWrapper);
			await channelsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('error')).toHaveTextContent('Failed');
			});

			// Then succeed
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValueOnce(mockChannels);
			await channelsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('error')).toHaveTextContent('');
				expect(screen.getByTestId('channel-count')).toHaveTextContent('3');
			});
		});

		it('stores channels in correct format', async () => {
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValue(mockChannels);

			render(ChannelsStoreWrapper);
			await channelsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('channel-ch-1-name')).toHaveTextContent('streamer_one');
				expect(screen.getByTestId('channel-ch-1-platform')).toHaveTextContent('twitch');
				expect(screen.getByTestId('channel-ch-2-name')).toHaveTextContent('streamer_two');
				expect(screen.getByTestId('channel-ch-2-platform')).toHaveTextContent('youtube');
			});
		});
	});

	describe('createChannel()', () => {
		it('adds channel to list on success', async () => {
			const newChannel: Channel = {
				id: 'ch-new',
				name: 'new_streamer',
				platform: 'twitch',
				enabled: true,
				quality: 'best',
				status: { is_live: false, is_recording: false }
			};

			(api.createChannel as ReturnType<typeof vi.fn>).mockResolvedValue(newChannel);
			(api.checkChannel as ReturnType<typeof vi.fn>).mockResolvedValue(newChannel);

			render(ChannelsStoreWrapper);

			const result = await channelsStore.createChannel({
				name: 'new_streamer',
				platform: 'twitch'
			});

			expect(result.success).toBe(true);
			await waitFor(() => {
				expect(screen.getByTestId('channel-count')).toHaveTextContent('1');
				expect(screen.getByTestId('channel-ch-new-name')).toHaveTextContent('new_streamer');
			});
		});

		it('triggers checkChannel in background after creation', async () => {
			const newChannel: Channel = {
				id: 'ch-new',
				name: 'new_streamer',
				platform: 'twitch',
				enabled: true,
				quality: 'best',
				status: { is_live: false, is_recording: false }
			};

			const checkedChannel: Channel = {
				...newChannel,
				status: { is_live: true, is_recording: false }
			};

			(api.createChannel as ReturnType<typeof vi.fn>).mockResolvedValue(newChannel);
			(api.checkChannel as ReturnType<typeof vi.fn>).mockResolvedValue(checkedChannel);

			render(ChannelsStoreWrapper);

			await channelsStore.createChannel({ name: 'new_streamer', platform: 'twitch' });

			// Wait for background check
			await waitFor(() => {
				expect(api.checkChannel).toHaveBeenCalledWith('ch-new');
			});
		});

		it('handles API error gracefully', async () => {
			(api.createChannel as ReturnType<typeof vi.fn>).mockRejectedValue(
				new Error('Creation failed')
			);

			render(ChannelsStoreWrapper);

			const result = await channelsStore.createChannel({
				name: 'new_streamer',
				platform: 'twitch'
			});

			expect(result.success).toBe(false);
			if (!result.success) {
				expect(result.error).toBe('Creation failed');
			}
		});

		it('returns success object on success', async () => {
			const newChannel: Channel = {
				id: 'ch-new',
				name: 'new_streamer',
				platform: 'twitch',
				enabled: true,
				quality: 'best',
				status: { is_live: false, is_recording: false }
			};

			(api.createChannel as ReturnType<typeof vi.fn>).mockResolvedValue(newChannel);
			(api.checkChannel as ReturnType<typeof vi.fn>).mockResolvedValue(newChannel);

			render(ChannelsStoreWrapper);

			const result = await channelsStore.createChannel({
				name: 'new_streamer',
				platform: 'twitch'
			});

			expect(result).toEqual({ success: true });
		});
	});

	describe('deleteChannel()', () => {
		beforeEach(async () => {
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValue(mockChannels);
			render(ChannelsStoreWrapper);
			await channelsStore.load();
		});

		it('removes channel from list on success', async () => {
			(api.deleteChannel as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);

			await waitFor(() => {
				expect(screen.getByTestId('channel-count')).toHaveTextContent('3');
			});

			const result = await channelsStore.deleteChannel('ch-1');

			expect(result).toBe(true);
			await waitFor(() => {
				expect(screen.getByTestId('channel-count')).toHaveTextContent('2');
				expect(screen.queryByTestId('channel-ch-1-name')).not.toBeInTheDocument();
			});
		});

		it('clears selection if deleted channel was selected', async () => {
			(api.deleteChannel as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);

			channelsStore.selectChannel('ch-1');

			await waitFor(() => {
				expect(screen.getByTestId('selected-channel-id')).toHaveTextContent('ch-1');
			});

			await channelsStore.deleteChannel('ch-1');

			await waitFor(() => {
				expect(screen.getByTestId('selected-channel-id')).toHaveTextContent('');
			});
		});

		it('handles delete error gracefully', async () => {
			(api.deleteChannel as ReturnType<typeof vi.fn>).mockRejectedValue(new Error('Delete failed'));

			const result = await channelsStore.deleteChannel('ch-1');

			expect(result).toBe(false);
			await waitFor(() => {
				expect(screen.getByTestId('error')).toHaveTextContent('Delete failed');
				expect(screen.getByTestId('channel-count')).toHaveTextContent('3');
			});
		});
	});

	describe('updateChannel()', () => {
		beforeEach(async () => {
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValue(mockChannels);
			render(ChannelsStoreWrapper);
			await channelsStore.load();
		});

		it('updates channel in list on success', async () => {
			const updatedChannel: Channel = {
				...mockChannels[0],
				quality: '720p'
			};
			(api.updateChannel as ReturnType<typeof vi.fn>).mockResolvedValue(updatedChannel);

			const result = await channelsStore.updateChannel('ch-1', { quality: '720p' });

			expect(result).toBe(true);
			await waitFor(() => {
				expect(screen.getByTestId('channel-ch-1-quality')).toHaveTextContent('720p');
			});
		});

		it('handles update error gracefully', async () => {
			(api.updateChannel as ReturnType<typeof vi.fn>).mockRejectedValue(new Error('Update failed'));

			const result = await channelsStore.updateChannel('ch-1', { quality: '720p' });

			expect(result).toBe(false);
			await waitFor(() => {
				expect(screen.getByTestId('error')).toHaveTextContent('Update failed');
			});
		});
	});

	describe('stopRecording()', () => {
		beforeEach(async () => {
			const recordingChannel: Channel = {
				...mockChannels[0],
				status: { is_live: true, is_recording: true }
			};
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValue([recordingChannel]);
			render(ChannelsStoreWrapper);
			await channelsStore.load();
		});

		it('updates channel status on success', async () => {
			const stoppedChannel: Channel = {
				...mockChannels[0],
				status: { is_live: true, is_recording: false }
			};
			(api.stopRecording as ReturnType<typeof vi.fn>).mockResolvedValue(stoppedChannel);

			await waitFor(() => {
				expect(screen.getByTestId('channel-ch-1-is-recording')).toHaveTextContent('true');
			});

			const result = await channelsStore.stopRecording('ch-1');

			expect(result).toBe(true);
			await waitFor(() => {
				expect(screen.getByTestId('channel-ch-1-is-recording')).toHaveTextContent('false');
			});
			expect(toastStore.success).toHaveBeenCalledWith('Recording stopped, channel paused');
		});

		it('shows error toast on failure', async () => {
			(api.stopRecording as ReturnType<typeof vi.fn>).mockRejectedValue(new Error('Stop failed'));

			const result = await channelsStore.stopRecording('ch-1');

			expect(result).toBe(false);
			expect(toastStore.error).toHaveBeenCalledWith('Stop failed');
		});
	});

	describe('filtering', () => {
		beforeEach(async () => {
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValue(mockChannels);
			render(ChannelsStoreWrapper);
			await channelsStore.load();
		});

		it('platform filter shows only matching channels', async () => {
			await waitFor(() => {
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('3');
			});

			channelsStore.setFilter('twitch');

			await waitFor(() => {
				expect(screen.getByTestId('platform-filter')).toHaveTextContent('twitch');
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('1');
				expect(screen.getByTestId('filtered-channel-ch-1-name')).toHaveTextContent('streamer_one');
			});
		});

		it('platform filter "all" shows all channels', async () => {
			channelsStore.setFilter('twitch');

			await waitFor(() => {
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('1');
			});

			channelsStore.setFilter('all');

			await waitFor(() => {
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('3');
			});
		});

		it('search filter matches channel name case-insensitively', async () => {
			channelsStore.setSearch('STREAMER_ONE');

			await waitFor(() => {
				expect(screen.getByTestId('search-query')).toHaveTextContent('STREAMER_ONE');
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('1');
				expect(screen.getByTestId('filtered-channel-ch-1-name')).toHaveTextContent('streamer_one');
			});
		});

		it('combined filters use AND logic', async () => {
			// Two twitch channels would be: streamer_one (twitch)
			// Set both platform and search
			channelsStore.setFilter('twitch');
			channelsStore.setSearch('one');

			await waitFor(() => {
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('1');
				expect(screen.getByTestId('filtered-channel-ch-1-name')).toHaveTextContent('streamer_one');
			});

			// Search for something that doesn't match
			channelsStore.setSearch('nonexistent');

			await waitFor(() => {
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('0');
			});
		});

		it('empty search shows all channels', async () => {
			channelsStore.setSearch('one');

			await waitFor(() => {
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('1');
			});

			channelsStore.setSearch('');

			await waitFor(() => {
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('3');
			});
		});
	});

	describe('selection', () => {
		beforeEach(async () => {
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValue(mockChannels);
			render(ChannelsStoreWrapper);
			await channelsStore.load();
		});

		it('selectChannel() sets selectedChannelId', async () => {
			channelsStore.selectChannel('ch-2');

			await waitFor(() => {
				expect(screen.getByTestId('selected-channel-id')).toHaveTextContent('ch-2');
			});
		});

		it('selectedChannel getter returns correct channel', async () => {
			channelsStore.selectChannel('ch-2');

			await waitFor(() => {
				expect(screen.getByTestId('selected-channel-name')).toHaveTextContent('streamer_two');
				expect(screen.getByTestId('selected-channel-platform')).toHaveTextContent('youtube');
			});
		});

		it('selectChannel(null) clears selection', async () => {
			channelsStore.selectChannel('ch-2');

			await waitFor(() => {
				expect(screen.getByTestId('selected-channel-id')).toHaveTextContent('ch-2');
			});

			channelsStore.selectChannel(null);

			await waitFor(() => {
				expect(screen.getByTestId('selected-channel-id')).toHaveTextContent('');
			});
		});
	});
});

describe('ChannelsStore WebSocket Events', () => {
	let mockSubscribeHandler: ((event: WebSocketEvent) => void) | null = null;

	beforeEach(() => {
		vi.clearAllMocks();
		resetStore();
		// Also unsubscribe from any previous subscriptions
		channelsStore.unsubscribeEvents();

		// Capture the subscribe handler when channelsStore.subscribe() is called
		(wsClient.subscribe as ReturnType<typeof vi.fn>).mockImplementation((handler) => {
			mockSubscribeHandler = handler;
			return () => {
				mockSubscribeHandler = null;
			};
		});
	});

	afterEach(() => {
		cleanup();
		channelsStore.unsubscribeEvents();
		mockSubscribeHandler = null;
	});

	// Helper to emit WebSocket events directly to the captured handler
	function emitEvent(event: WebSocketEvent) {
		if (mockSubscribeHandler) {
			mockSubscribeHandler(event);
		} else {
			throw new Error('No subscribe handler captured - did you call channelsStore.subscribe()?');
		}
	}

	describe('channel_status event', () => {
		it('updates channel is_live and is_recording on status change', async () => {
			// Load initial channels
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValue(mockChannels);
			(api.getChannel as ReturnType<typeof vi.fn>).mockResolvedValue({
				...mockChannels[0],
				status: { is_live: true, is_recording: true }
			});

			render(ChannelsStoreWrapper);
			await channelsStore.load();
			channelsStore.subscribe();

			await waitFor(() => {
				expect(screen.getByTestId('channel-ch-1-is-live')).toHaveTextContent('false');
				expect(screen.getByTestId('channel-ch-1-is-recording')).toHaveTextContent('false');
			});

			// Simulate channel going live and recording
			emitEvent({
				type: 'channel_status',
				channel_id: 'ch-1',
				name: 'streamer_one',
				platform: 'twitch',
				status: 'recording'
			});

			await waitFor(() => {
				expect(screen.getByTestId('channel-ch-1-is-live')).toHaveTextContent('true');
				expect(screen.getByTestId('channel-ch-1-is-recording')).toHaveTextContent('true');
			});
		});

		it('updates channel to offline status', async () => {
			const liveChannel: Channel = {
				...mockChannels[0],
				status: { is_live: true, is_recording: false }
			};
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValue([liveChannel]);
			(api.getChannel as ReturnType<typeof vi.fn>).mockResolvedValue({
				...mockChannels[0],
				status: { is_live: false, is_recording: false }
			});

			render(ChannelsStoreWrapper);
			await channelsStore.load();
			channelsStore.subscribe();

			await waitFor(() => {
				expect(screen.getByTestId('channel-ch-1-is-live')).toHaveTextContent('true');
			});

			emitEvent({
				type: 'channel_status',
				channel_id: 'ch-1',
				name: 'streamer_one',
				platform: 'twitch',
				status: 'offline'
			});

			await waitFor(() => {
				expect(screen.getByTestId('channel-ch-1-is-live')).toHaveTextContent('false');
			});
		});

		it('ignores status for unknown channel_id', async () => {
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValue(mockChannels);

			render(ChannelsStoreWrapper);
			await channelsStore.load();
			channelsStore.subscribe();

			await waitFor(() => {
				expect(screen.getByTestId('channel-count')).toHaveTextContent('3');
			});

			// Should not throw or create new channel
			emitEvent({
				type: 'channel_status',
				channel_id: 'unknown-id',
				name: 'unknown',
				platform: 'twitch',
				status: 'live'
			});

			await waitFor(() => {
				expect(screen.getByTestId('channel-count')).toHaveTextContent('3');
			});
		});
	});

	describe('recording_started event', () => {
		it('sets is_recording=true for matching channel', async () => {
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValue(mockChannels);

			render(ChannelsStoreWrapper);
			await channelsStore.load();
			channelsStore.subscribe();

			await waitFor(() => {
				expect(screen.getByTestId('channel-ch-1-is-recording')).toHaveTextContent('false');
			});

			emitEvent({
				type: 'recording_started',
				recording_id: 'rec-1',
				channel_id: 'ch-1',
				channel_name: 'streamer_one',
				platform: 'twitch',
				quality: 'best'
			});

			await waitFor(() => {
				expect(screen.getByTestId('channel-ch-1-is-recording')).toHaveTextContent('true');
				expect(screen.getByTestId('channel-ch-1-is-live')).toHaveTextContent('true');
			});

			expect(toastStore.success).toHaveBeenCalledWith('Recording started: streamer_one');
		});
	});

	describe('recording_ended event', () => {
		it('shows info toast with duration', async () => {
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValue(mockChannels);

			render(ChannelsStoreWrapper);
			await channelsStore.load();
			channelsStore.subscribe();

			emitEvent({
				type: 'recording_ended',
				recording_id: 'rec-1',
				duration_secs: 3600,
				size_bytes: 1000000,
				segment_count: 100,
				reason: 'stream_ended'
			});

			await waitFor(() => {
				expect(toastStore.info).toHaveBeenCalledWith('Recording ended (60m)');
			});
		});

		it('triggers reload after recording ends', async () => {
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValue(mockChannels);

			render(ChannelsStoreWrapper);
			await channelsStore.load();
			channelsStore.subscribe();

			vi.clearAllMocks();

			emitEvent({
				type: 'recording_ended',
				recording_id: 'rec-1',
				duration_secs: 300,
				size_bytes: 500000,
				segment_count: 50,
				reason: 'stream_ended'
			});

			await waitFor(() => {
				expect(api.getChannels).toHaveBeenCalled();
			});
		});
	});

	describe('quota_status_changed event', () => {
		it('updates channel quota fields', async () => {
			const channelWithQuota: Channel = {
				...mockChannels[0],
				quota_gb: 10,
				quota_status: 'ok',
				quota_used_bytes: 1000000000,
				quota_percent: 10
			};
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValue([channelWithQuota]);

			render(ChannelsStoreWrapper);
			await channelsStore.load();
			channelsStore.subscribe();

			await waitFor(() => {
				expect(screen.getByTestId('channel-ch-1-quota-status')).toHaveTextContent('ok');
				expect(screen.getByTestId('channel-ch-1-quota-percent')).toHaveTextContent('10');
			});

			emitEvent({
				type: 'quota_status_changed',
				channel_id: 'ch-1',
				channel_name: 'streamer_one',
				quota_status: 'warning',
				quota_used_bytes: 8000000000,
				quota_percent: 80
			});

			await waitFor(() => {
				expect(screen.getByTestId('channel-ch-1-quota-status')).toHaveTextContent('warning');
				expect(screen.getByTestId('channel-ch-1-quota-percent')).toHaveTextContent('80');
			});
		});
	});

	describe('connected event', () => {
		it('updates channels from connected event', async () => {
			render(ChannelsStoreWrapper);
			channelsStore.subscribe();

			const backendChannels: BackendChannel[] = [
				{
					id: 'ch-1',
					name: 'streamer_one',
					platform: 'twitch',
					enabled: true,
					quality: 'best',
					status: 'live',
					quota_status: 'unlimited',
					quota_used_bytes: 0,
					quota_percent: 0
				},
				{
					id: 'ch-2',
					name: 'streamer_two',
					platform: 'youtube',
					enabled: true,
					quality: '1080p',
					status: 'recording',
					quota_status: 'unlimited',
					quota_used_bytes: 0,
					quota_percent: 0
				}
			];

			emitEvent({
				type: 'connected',
				channels: backendChannels,
				active_recordings: []
			});

			await waitFor(() => {
				expect(screen.getByTestId('channel-count')).toHaveTextContent('2');
				expect(screen.getByTestId('channel-ch-1-is-live')).toHaveTextContent('true');
				expect(screen.getByTestId('channel-ch-2-is-recording')).toHaveTextContent('true');
			});
		});
	});

	describe('error event', () => {
		it('shows error toast', async () => {
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValue(mockChannels);

			render(ChannelsStoreWrapper);
			await channelsStore.load();
			channelsStore.subscribe();

			emitEvent({
				type: 'error',
				message: 'Something went wrong'
			});

			await waitFor(() => {
				expect(toastStore.error).toHaveBeenCalledWith('Something went wrong');
			});
		});
	});

	describe('quota_skip event', () => {
		it('shows warning toast with quota details', async () => {
			(api.getChannels as ReturnType<typeof vi.fn>).mockResolvedValue(mockChannels);

			render(ChannelsStoreWrapper);
			await channelsStore.load();
			channelsStore.subscribe();

			emitEvent({
				type: 'quota_skip',
				channel_id: 'ch-1',
				channel_name: 'streamer_one',
				platform: 'twitch',
				quota_used_bytes: 10737418240, // 10 GB
				quota_limit_bytes: 10737418240,
				message: 'Quota exceeded'
			});

			await waitFor(() => {
				expect(toastStore.warning).toHaveBeenCalled();
			});
		});
	});
});
