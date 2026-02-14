/**
 * Activity Store Integration Tests
 *
 * Tests for the activity store which maps WebSocket events to
 * a human-readable activity log.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { activityStore, type EventCategory, type ActivityEvent } from './activity.svelte';
import type { WebSocketEvent } from '$lib/api/websocket';

// Helper to reset store state
function resetStore() {
	activityStore.events = [];
	activityStore.categoryFilter = 'all';
	activityStore.channelFilter = 'all';
	activityStore.searchQuery = '';
	activityStore.autoScroll = true;
	activityStore.selectedEventId = null;
}

// Helper to create a WebSocket event and have it processed
function processEvent(event: WebSocketEvent & Record<string, unknown>) {
	activityStore.handleWebSocketEvent(event);
}

describe('ActivityStore', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		resetStore();
	});

	afterEach(() => {
		resetStore();
	});

	describe('initial state', () => {
		it('starts with empty events', () => {
			expect(activityStore.events).toHaveLength(0);
			expect(activityStore.eventCount).toBe(0);
		});

		it('has default filter values', () => {
			expect(activityStore.categoryFilter).toBe('all');
			expect(activityStore.channelFilter).toBe('all');
			expect(activityStore.searchQuery).toBe('');
		});

		it('has autoScroll enabled by default', () => {
			expect(activityStore.autoScroll).toBe(true);
		});
	});

	describe('event mapping', () => {
		it('maps recording_started to recording category', () => {
			processEvent({
				type: 'recording_started',
				recording_id: 'rec-1',
				channel_id: 'ch-1',
				channel_name: 'test_streamer',
				platform: 'twitch',
				quality: 'best'
			});

			expect(activityStore.events).toHaveLength(1);
			const event = activityStore.events[0];
			expect(event.type).toBe('recording_started');
			expect(event.category).toBe('recording');
			expect(event.channelName).toBe('test_streamer');
			expect(event.platform).toBe('twitch');
			expect(event.message).toContain('Recording started');
		});

		it('maps recording_ended to recording category', () => {
			processEvent({
				type: 'recording_ended',
				recording_id: 'rec-1',
				channel_name: 'test_streamer',
				duration_secs: 3600,
				size_bytes: 1000000,
				segment_count: 100,
				reason: 'stream_ended'
			});

			const event = activityStore.events[0];
			expect(event.type).toBe('recording_ended');
			expect(event.category).toBe('recording');
			expect(event.message).toContain('Recording ended');
			expect(event.message).toContain('1h 0m');
		});

		it('maps channel_status to channel category', () => {
			processEvent({
				type: 'channel_status',
				channel_id: 'ch-1',
				name: 'test_streamer',
				platform: 'twitch',
				status: 'live'
			});

			const event = activityStore.events[0];
			expect(event.type).toBe('channel_status');
			expect(event.category).toBe('channel');
			expect(event.channelName).toBe('test_streamer');
			expect(event.message).toContain('is now live');
		});

		it('maps processing_started to processing category', () => {
			processEvent({
				type: 'processing_started',
				recording_id: 'rec-12345678-abcd'
			});

			const event = activityStore.events[0];
			expect(event.type).toBe('processing_started');
			expect(event.category).toBe('processing');
			expect(event.message).toContain('Processing started');
		});

		it('maps processing_complete to processing category', () => {
			processEvent({
				type: 'processing_complete',
				recording_id: 'rec-1',
				output_file: '/library/video.mp4',
				size_bytes: 5000000
			});

			const event = activityStore.events[0];
			expect(event.type).toBe('processing_complete');
			expect(event.category).toBe('processing');
			expect(event.message).toContain('Processing complete');
		});

		it('maps processing_failed to processing category', () => {
			processEvent({
				type: 'processing_failed',
				recording_id: 'rec-1',
				error: 'FFmpeg error'
			});

			const event = activityStore.events[0];
			expect(event.type).toBe('processing_failed');
			expect(event.category).toBe('processing');
			expect(event.message).toContain('Processing failed');
			expect(event.message).toContain('FFmpeg error');
		});

		it('maps error to channel category', () => {
			processEvent({
				type: 'error',
				name: 'test_channel',
				message: 'Something went wrong'
			});

			const event = activityStore.events[0];
			expect(event.type).toBe('channel_error');
			expect(event.category).toBe('channel');
			expect(event.message).toBe('Something went wrong');
		});

		it('maps disk_warning to system category', () => {
			processEvent({
				type: 'disk_warning',
				usage_percent: 95,
				free_bytes: 10000000000
			});

			const event = activityStore.events[0];
			expect(event.type).toBe('disk_warning');
			expect(event.category).toBe('system');
			expect(event.message).toContain('95%');
		});

		it('maps unknown event type to system category', () => {
			// Force an unknown event type by using type assertion
			processEvent({
				type: 'unknown_event_type',
				message: 'test message'
			} as unknown as WebSocketEvent & Record<string, unknown>);

			const event = activityStore.events[0];
			expect(event.type).toBe('unknown_event_type');
			expect(event.category).toBe('system');
			expect(event.message).toContain('Unknown event');
		});

		it('handles missing optional fields gracefully', () => {
			processEvent({
				type: 'recording_ended',
				recording_id: 'rec-1',
				duration_secs: 0, // This could be undefined
				size_bytes: 0,
				segment_count: 0,
				reason: 'stream_ended'
			});

			// Should not throw, and should have event
			expect(activityStore.events).toHaveLength(1);
		});

		it('skips processing_progress events to avoid spam', () => {
			processEvent({
				type: 'processing_progress',
				recording_id: 'rec-1',
				percent: 50
			});

			// Should not add event
			expect(activityStore.events).toHaveLength(0);
		});

		it('skips segment_downloaded events to avoid spam', () => {
			processEvent({
				type: 'segment_downloaded',
				recording_id: 'rec-1',
				sequence: 1,
				size_bytes: 5000000,
				total_segments: 10,
				total_bytes: 50000000
			});

			// Should not add event
			expect(activityStore.events).toHaveLength(0);
		});
	});

	describe('event management', () => {
		it('addEvent() prepends to array (newest first)', () => {
			processEvent({
				type: 'recording_started',
				recording_id: 'rec-1',
				channel_id: 'ch-1',
				channel_name: 'first_channel',
				platform: 'twitch',
				quality: 'best'
			});

			processEvent({
				type: 'recording_started',
				recording_id: 'rec-2',
				channel_id: 'ch-2',
				channel_name: 'second_channel',
				platform: 'twitch',
				quality: 'best'
			});

			expect(activityStore.events[0].channelName).toBe('second_channel');
			expect(activityStore.events[1].channelName).toBe('first_channel');
		});

		it('maintains max 1000 events', () => {
			// Add 1005 events
			for (let i = 0; i < 1005; i++) {
				processEvent({
					type: 'recording_started',
					recording_id: `rec-${i}`,
					channel_id: `ch-${i}`,
					channel_name: `channel_${i}`,
					platform: 'twitch',
					quality: 'best'
				});
			}

			expect(activityStore.events.length).toBeLessThanOrEqual(1000);
		});

		it('clear() empties array', () => {
			processEvent({
				type: 'recording_started',
				recording_id: 'rec-1',
				channel_id: 'ch-1',
				channel_name: 'test',
				platform: 'twitch',
				quality: 'best'
			});

			expect(activityStore.events).toHaveLength(1);

			activityStore.clear();

			expect(activityStore.events).toHaveLength(0);
		});

		it('events have unique IDs', () => {
			processEvent({
				type: 'recording_started',
				recording_id: 'rec-1',
				channel_id: 'ch-1',
				channel_name: 'test1',
				platform: 'twitch',
				quality: 'best'
			});

			processEvent({
				type: 'recording_started',
				recording_id: 'rec-2',
				channel_id: 'ch-2',
				channel_name: 'test2',
				platform: 'twitch',
				quality: 'best'
			});

			const ids = activityStore.events.map((e) => e.id);
			const uniqueIds = new Set(ids);
			expect(uniqueIds.size).toBe(ids.length);
		});
	});

	describe('filtering', () => {
		beforeEach(() => {
			// Add various events
			processEvent({
				type: 'recording_started',
				recording_id: 'rec-1',
				channel_id: 'ch-1',
				channel_name: 'twitch_streamer',
				platform: 'twitch',
				quality: 'best'
			});

			processEvent({
				type: 'channel_status',
				channel_id: 'ch-2',
				name: 'youtube_channel',
				platform: 'youtube',
				status: 'live'
			});

			processEvent({
				type: 'disk_warning',
				usage_percent: 95,
				free_bytes: 10000000000
			});
		});

		it('category filter works', () => {
			expect(activityStore.filteredEvents).toHaveLength(3);

			activityStore.setCategoryFilter('recording');

			expect(activityStore.filteredEvents).toHaveLength(1);
			expect(activityStore.filteredEvents[0].category).toBe('recording');
		});

		it('channel filter works', () => {
			activityStore.setChannelFilter('twitch_streamer');

			expect(activityStore.filteredEvents).toHaveLength(1);
			expect(activityStore.filteredEvents[0].channelName).toBe('twitch_streamer');
		});

		it('search filter matches message text', () => {
			activityStore.setSearch('Disk');

			expect(activityStore.filteredEvents).toHaveLength(1);
			expect(activityStore.filteredEvents[0].type).toBe('disk_warning');
		});

		it('search filter matches channel name', () => {
			activityStore.setSearch('youtube');

			expect(activityStore.filteredEvents).toHaveLength(1);
			expect(activityStore.filteredEvents[0].channelName).toBe('youtube_channel');
		});

		it('combined filters use AND logic', () => {
			activityStore.setCategoryFilter('channel');
			activityStore.setSearch('youtube');

			expect(activityStore.filteredEvents).toHaveLength(1);
			expect(activityStore.filteredEvents[0].channelName).toBe('youtube_channel');

			// Add recording filter - should find nothing
			activityStore.setCategoryFilter('recording');
			activityStore.setSearch('youtube');

			expect(activityStore.filteredEvents).toHaveLength(0);
		});
	});

	describe('selection', () => {
		it('selectEvent() sets selectedEventId', () => {
			processEvent({
				type: 'recording_started',
				recording_id: 'rec-1',
				channel_id: 'ch-1',
				channel_name: 'test',
				platform: 'twitch',
				quality: 'best'
			});

			const eventId = activityStore.events[0].id;
			activityStore.selectEvent(eventId);

			expect(activityStore.selectedEventId).toBe(eventId);
		});

		it('selectedEvent getter returns correct event', () => {
			processEvent({
				type: 'recording_started',
				recording_id: 'rec-1',
				channel_id: 'ch-1',
				channel_name: 'test',
				platform: 'twitch',
				quality: 'best'
			});

			const eventId = activityStore.events[0].id;
			activityStore.selectEvent(eventId);

			expect(activityStore.selectedEvent?.channelName).toBe('test');
		});

		it('selectEvent(null) clears selection', () => {
			processEvent({
				type: 'recording_started',
				recording_id: 'rec-1',
				channel_id: 'ch-1',
				channel_name: 'test',
				platform: 'twitch',
				quality: 'best'
			});

			activityStore.selectEvent(activityStore.events[0].id);
			expect(activityStore.selectedEventId).not.toBeNull();

			activityStore.selectEvent(null);
			expect(activityStore.selectedEventId).toBeNull();
		});
	});

	describe('unique channels', () => {
		it('returns unique channels from events', () => {
			processEvent({
				type: 'recording_started',
				recording_id: 'rec-1',
				channel_id: 'ch-1',
				channel_name: 'channel_a',
				platform: 'twitch',
				quality: 'best'
			});

			processEvent({
				type: 'recording_started',
				recording_id: 'rec-2',
				channel_id: 'ch-2',
				channel_name: 'channel_b',
				platform: 'twitch',
				quality: 'best'
			});

			processEvent({
				type: 'recording_ended',
				recording_id: 'rec-1',
				channel_name: 'channel_a',
				duration_secs: 100,
				size_bytes: 1000,
				segment_count: 10,
				reason: 'ended'
			});

			const channels = activityStore.uniqueChannels;
			expect(channels).toHaveLength(2);
			expect(channels).toContain('channel_a');
			expect(channels).toContain('channel_b');
		});
	});

	describe('export', () => {
		it('exportToJson() returns valid JSON', () => {
			processEvent({
				type: 'recording_started',
				recording_id: 'rec-1',
				channel_id: 'ch-1',
				channel_name: 'test',
				platform: 'twitch',
				quality: 'best'
			});

			const json = activityStore.exportToJson();
			const parsed = JSON.parse(json);

			expect(parsed.exportedAt).toBeDefined();
			expect(parsed.eventCount).toBe(1);
			expect(parsed.events).toHaveLength(1);
			expect(parsed.events[0].channelName).toBe('test');
		});
	});

	describe('UI state', () => {
		it('setAutoScroll() updates autoScroll', () => {
			expect(activityStore.autoScroll).toBe(true);

			activityStore.setAutoScroll(false);

			expect(activityStore.autoScroll).toBe(false);
		});
	});
});
