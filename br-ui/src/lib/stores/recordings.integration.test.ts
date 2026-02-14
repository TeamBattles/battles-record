/**
 * Recordings Store Integration Tests
 *
 * These tests verify the recordings store behavior through a wrapper component
 * to properly test Svelte 5 $state reactivity.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/svelte';
import { RecordingsStoreWrapper } from '../../tests/wrappers';
import { recordingsStore } from './recordings.svelte';
import type { Recording } from '$lib/api/types';
import type { WebSocketEvent } from '$lib/api/websocket';

// Mock the API client
vi.mock('$lib/api/client', () => ({
	api: {
		getRecordings: vi.fn(),
		deleteRecording: vi.fn(),
		reprocessRecording: vi.fn()
	}
}));

// Store the subscribe handler for simulating events
// We need to capture the handler that the RecordingsStore passes to wsClient.subscribe()
// The RecordingsStore subscribes in its constructor, so we capture it via the mock
const handlers: Set<(event: WebSocketEvent) => void> = new Set();

// Mock the WebSocket client
vi.mock('$lib/api/websocket', () => ({
	wsClient: {
		subscribe: vi.fn((handler: (event: WebSocketEvent) => void) => {
			// Need to use a workaround - store via globalThis
			const global = globalThis as unknown as {
				__wsHandlers?: Set<(event: WebSocketEvent) => void>;
			};
			global.__wsHandlers = global.__wsHandlers || new Set();
			global.__wsHandlers.add(handler);
			return () => {
				global.__wsHandlers?.delete(handler);
			};
		})
	}
}));

import { api } from '$lib/api/client';

// Test data
const mockRecordings: Recording[] = [
	{
		id: 'rec-1',
		channel_name: 'streamer_one',
		platform: 'twitch',
		started_at: '2024-01-15T10:00:00Z',
		ended_at: '2024-01-15T12:00:00Z',
		duration_secs: 7200,
		status: 'processed',
		path: '/recordings/streamer_one/2024-01-15',
		size_bytes: 5_000_000_000,
		title: 'Morning Stream',
		game: 'Minecraft'
	},
	{
		id: 'rec-2',
		channel_name: 'streamer_two',
		platform: 'youtube',
		started_at: '2024-01-14T18:00:00Z',
		ended_at: '2024-01-14T20:00:00Z',
		duration_secs: 7200,
		status: 'pending_processing',
		path: '/recordings/streamer_two/2024-01-14',
		size_bytes: 8_000_000_000,
		title: 'Evening Games'
	},
	{
		id: 'rec-3',
		channel_name: 'streamer_three',
		platform: 'kick',
		started_at: '2024-01-13T14:00:00Z',
		status: 'recording',
		path: '/recordings/streamer_three/2024-01-13',
		size_bytes: 2_000_000_000
	}
];

// Helper to reset store state
function resetStore() {
	recordingsStore.recordings = [];
	recordingsStore.isLoading = false;
	recordingsStore.error = null;
	recordingsStore.processingProgress = new Map();
	recordingsStore.platformFilter = null;
	recordingsStore.statusFilter = null;
	recordingsStore.searchQuery = '';
	recordingsStore.sortBy = 'date';
	recordingsStore.sortOrder = 'desc';
}

// Helper to emit WebSocket events to all registered handlers
function emitEvent(event: WebSocketEvent) {
	const handlers = (globalThis as { __wsHandlers?: Set<(event: WebSocketEvent) => void> })
		.__wsHandlers;
	if (handlers && handlers.size > 0) {
		handlers.forEach((handler) => handler(event));
	}
}

describe('RecordingsStore Integration', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		resetStore();
	});

	afterEach(() => {
		cleanup();
	});

	describe('initial state', () => {
		it('renders with no recordings initially', () => {
			render(RecordingsStoreWrapper);

			expect(screen.getByTestId('recording-count')).toHaveTextContent('0');
			expect(screen.getByTestId('filtered-count')).toHaveTextContent('0');
			expect(screen.getByTestId('is-loading')).toHaveTextContent('false');
		});

		it('shows no error initially', () => {
			render(RecordingsStoreWrapper);

			expect(screen.getByTestId('error')).toHaveTextContent('');
		});

		it('has default filter values', () => {
			render(RecordingsStoreWrapper);

			expect(screen.getByTestId('platform-filter')).toHaveTextContent('');
			expect(screen.getByTestId('status-filter')).toHaveTextContent('');
			expect(screen.getByTestId('search-query')).toHaveTextContent('');
		});

		it('has default sort values', () => {
			render(RecordingsStoreWrapper);

			expect(screen.getByTestId('sort-by')).toHaveTextContent('date');
			expect(screen.getByTestId('sort-order')).toHaveTextContent('desc');
		});
	});

	describe('load()', () => {
		it('sets isLoading during fetch', async () => {
			let resolveLoad: (value: Recording[]) => void;
			(api.getRecordings as ReturnType<typeof vi.fn>).mockReturnValue(
				new Promise((resolve) => {
					resolveLoad = resolve;
				})
			);

			render(RecordingsStoreWrapper);

			const loadPromise = recordingsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('is-loading')).toHaveTextContent('true');
			});

			resolveLoad!(mockRecordings);
			await loadPromise;

			await waitFor(() => {
				expect(screen.getByTestId('is-loading')).toHaveTextContent('false');
			});
		});

		it('populates recordings on success', async () => {
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue(mockRecordings);

			render(RecordingsStoreWrapper);
			await recordingsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('recording-count')).toHaveTextContent('3');
			});
		});

		it('handles error state', async () => {
			(api.getRecordings as ReturnType<typeof vi.fn>).mockRejectedValue(new Error('Network error'));

			render(RecordingsStoreWrapper);
			await recordingsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('error')).toHaveTextContent('Network error');
				expect(screen.getByTestId('recording-count')).toHaveTextContent('0');
			});
		});

		it('clears previous recordings on reload', async () => {
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue(mockRecordings);

			render(RecordingsStoreWrapper);
			await recordingsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('recording-count')).toHaveTextContent('3');
			});

			// Reload with different data
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue([mockRecordings[0]]);
			await recordingsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('recording-count')).toHaveTextContent('1');
			});
		});
	});

	describe('deleteRecording()', () => {
		beforeEach(async () => {
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue(mockRecordings);
			render(RecordingsStoreWrapper);
			await recordingsStore.load();
		});

		it('removes recording from list on success', async () => {
			(api.deleteRecording as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);

			await waitFor(() => {
				expect(screen.getByTestId('recording-count')).toHaveTextContent('3');
			});

			const result = await recordingsStore.deleteRecording('rec-1');

			expect(result).toBe(true);
			await waitFor(() => {
				expect(screen.getByTestId('recording-count')).toHaveTextContent('2');
				expect(screen.queryByTestId('recording-rec-1-channel')).not.toBeInTheDocument();
			});
		});

		it('handles delete error gracefully', async () => {
			(api.deleteRecording as ReturnType<typeof vi.fn>).mockRejectedValue(
				new Error('Delete failed')
			);

			const result = await recordingsStore.deleteRecording('rec-1');

			expect(result).toBe(false);
			await waitFor(() => {
				expect(screen.getByTestId('error')).toHaveTextContent('Delete failed');
				expect(screen.getByTestId('recording-count')).toHaveTextContent('3');
			});
		});
	});

	describe('processRecording()', () => {
		beforeEach(async () => {
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue(mockRecordings);
			render(RecordingsStoreWrapper);
			await recordingsStore.load();
		});

		it('updates status to pending_processing on success', async () => {
			(api.reprocessRecording as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);

			const result = await recordingsStore.processRecording('rec-1');

			expect(result).toBe(true);
			await waitFor(() => {
				expect(screen.getByTestId('recording-rec-1-status')).toHaveTextContent(
					'pending_processing'
				);
			});
		});

		it('handles process error gracefully', async () => {
			(api.reprocessRecording as ReturnType<typeof vi.fn>).mockRejectedValue(
				new Error('Process failed')
			);

			const result = await recordingsStore.processRecording('rec-1');

			expect(result).toBe(false);
			await waitFor(() => {
				// Error message comes from the thrown error since it's an instance of Error
				expect(screen.getByTestId('error')).toHaveTextContent('Process failed');
			});
		});
	});

	describe('filtering', () => {
		beforeEach(async () => {
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue(mockRecordings);
			render(RecordingsStoreWrapper);
			await recordingsStore.load();
		});

		it('platform filter works', async () => {
			await waitFor(() => {
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('3');
			});

			recordingsStore.setFilter('twitch');

			await waitFor(() => {
				expect(screen.getByTestId('platform-filter')).toHaveTextContent('twitch');
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('1');
				expect(screen.getByTestId('filtered-recording-rec-1-channel')).toHaveTextContent(
					'streamer_one'
				);
			});
		});

		it('status filter works', async () => {
			recordingsStore.setStatusFilter('processed');

			await waitFor(() => {
				expect(screen.getByTestId('status-filter')).toHaveTextContent('processed');
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('1');
			});
		});

		it('search filter matches channel_name and title', async () => {
			recordingsStore.setSearch('Morning');

			await waitFor(() => {
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('1');
				expect(screen.getByTestId('filtered-recording-rec-1-channel')).toHaveTextContent(
					'streamer_one'
				);
			});

			// Also matches channel name
			recordingsStore.setSearch('streamer_two');

			await waitFor(() => {
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('1');
				expect(screen.getByTestId('filtered-recording-rec-2-channel')).toHaveTextContent(
					'streamer_two'
				);
			});
		});

		it('combined filters use AND logic', async () => {
			// Set platform to twitch
			recordingsStore.setFilter('twitch');

			await waitFor(() => {
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('1');
			});

			// Add status filter - no twitch recordings with 'recording' status
			recordingsStore.setStatusFilter('recording');

			await waitFor(() => {
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('0');
			});
		});

		it('empty filters return all', async () => {
			recordingsStore.setFilter('twitch');
			recordingsStore.setStatusFilter('processed');

			await waitFor(() => {
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('1');
			});

			recordingsStore.setFilter(null);
			recordingsStore.setStatusFilter(null);

			await waitFor(() => {
				expect(screen.getByTestId('filtered-count')).toHaveTextContent('3');
			});
		});
	});

	describe('sorting', () => {
		beforeEach(async () => {
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue(mockRecordings);
			render(RecordingsStoreWrapper);
			await recordingsStore.load();
		});

		it('sorts by date descending by default', async () => {
			await waitFor(() => {
				// Most recent first: rec-1 (Jan 15), rec-2 (Jan 14), rec-3 (Jan 13)
				const filtered = screen.getAllByTestId(/^filtered-recording-rec-\d+-channel$/);
				expect(filtered[0]).toHaveTextContent('streamer_one');
			});
		});

		it('sorts by date ascending', async () => {
			recordingsStore.setSort('date', 'asc');

			await waitFor(() => {
				expect(screen.getByTestId('sort-order')).toHaveTextContent('asc');
				// Oldest first: rec-3 (Jan 13), rec-2 (Jan 14), rec-1 (Jan 15)
				const filtered = screen.getAllByTestId(/^filtered-recording-rec-\d+-channel$/);
				expect(filtered[0]).toHaveTextContent('streamer_three');
			});
		});

		it('sorts by size descending', async () => {
			recordingsStore.setSort('size', 'desc');

			await waitFor(() => {
				expect(screen.getByTestId('sort-by')).toHaveTextContent('size');
				// Largest first: rec-2 (8GB), rec-1 (5GB), rec-3 (2GB)
				const filtered = screen.getAllByTestId(/^filtered-recording-rec-\d+-channel$/);
				expect(filtered[0]).toHaveTextContent('streamer_two');
			});
		});

		it('sorts by duration descending', async () => {
			recordingsStore.setSort('duration', 'desc');

			await waitFor(() => {
				expect(screen.getByTestId('sort-by')).toHaveTextContent('duration');
				// Longest first: rec-1 and rec-2 (7200s), rec-3 (undefined -> 0)
				const filtered = screen.getAllByTestId(/^filtered-recording-rec-\d+-channel$/);
				// rec-1 and rec-2 both have 7200s, order depends on stable sort
				expect(['streamer_one', 'streamer_two']).toContain(filtered[0].textContent);
			});
		});
	});
});

describe('RecordingsStore WebSocket Events', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		resetStore();
	});

	afterEach(() => {
		cleanup();
	});

	describe('recording_started event', () => {
		it('adds new recording to the list', async () => {
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue([]);

			render(RecordingsStoreWrapper);
			await recordingsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('recording-count')).toHaveTextContent('0');
			});

			emitEvent({
				type: 'recording_started',
				recording_id: 'new-rec',
				channel_id: 'ch-1',
				channel_name: 'new_streamer',
				platform: 'twitch',
				quality: 'best'
			});

			await waitFor(() => {
				expect(screen.getByTestId('recording-count')).toHaveTextContent('1');
				expect(screen.getByTestId('recording-new-rec-channel')).toHaveTextContent('new_streamer');
				expect(screen.getByTestId('recording-new-rec-status')).toHaveTextContent('recording');
			});
		});

		it('does not add duplicate recording', async () => {
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue([mockRecordings[2]]); // rec-3 is recording

			render(RecordingsStoreWrapper);
			await recordingsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('recording-count')).toHaveTextContent('1');
			});

			emitEvent({
				type: 'recording_started',
				recording_id: 'rec-3',
				channel_id: 'ch-3',
				channel_name: 'streamer_three',
				platform: 'kick',
				quality: 'best'
			});

			await waitFor(() => {
				expect(screen.getByTestId('recording-count')).toHaveTextContent('1');
			});
		});
	});

	describe('recording_ended event', () => {
		it('updates status and sets ended_at', async () => {
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue([mockRecordings[2]]); // recording status

			render(RecordingsStoreWrapper);
			await recordingsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('recording-rec-3-status')).toHaveTextContent('recording');
			});

			emitEvent({
				type: 'recording_ended',
				recording_id: 'rec-3',
				duration_secs: 3600,
				size_bytes: 5_000_000_000,
				segment_count: 100,
				reason: 'stream_ended'
			});

			await waitFor(() => {
				expect(screen.getByTestId('recording-rec-3-status')).toHaveTextContent(
					'pending_processing'
				);
				expect(screen.getByTestId('recording-rec-3-duration')).toHaveTextContent('3600');
				expect(screen.getByTestId('recording-rec-3-size')).toHaveTextContent('5000000000');
			});
		});
	});

	describe('segment_downloaded event', () => {
		it('updates size_bytes incrementally', async () => {
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue([mockRecordings[2]]); // recording status

			render(RecordingsStoreWrapper);
			await recordingsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('recording-rec-3-size')).toHaveTextContent('2000000000');
			});

			emitEvent({
				type: 'segment_downloaded',
				recording_id: 'rec-3',
				sequence: 50,
				size_bytes: 50_000_000,
				total_segments: 50,
				total_bytes: 2_500_000_000
			});

			await waitFor(() => {
				expect(screen.getByTestId('recording-rec-3-size')).toHaveTextContent('2500000000');
			});
		});
	});

	describe('processing_started event', () => {
		it('sets status to processing', async () => {
			const pendingRecording: Recording = {
				...mockRecordings[1],
				status: 'pending_processing'
			};
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue([pendingRecording]);

			render(RecordingsStoreWrapper);
			await recordingsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('recording-rec-2-status')).toHaveTextContent(
					'pending_processing'
				);
			});

			emitEvent({
				type: 'processing_started',
				recording_id: 'rec-2'
			});

			await waitFor(() => {
				expect(screen.getByTestId('recording-rec-2-status')).toHaveTextContent('processing');
			});
		});

		it('initializes progress to 0', async () => {
			const pendingRecording: Recording = {
				...mockRecordings[1],
				status: 'pending_processing'
			};
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue([pendingRecording]);

			render(RecordingsStoreWrapper);
			await recordingsStore.load();

			emitEvent({
				type: 'processing_started',
				recording_id: 'rec-2'
			});

			await waitFor(() => {
				expect(screen.getByTestId('recording-rec-2-progress')).toHaveTextContent('0');
			});
		});
	});

	describe('processing_progress event', () => {
		it('updates processingProgress Map', async () => {
			const processingRecording: Recording = {
				...mockRecordings[1],
				status: 'processing'
			};
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue([processingRecording]);

			render(RecordingsStoreWrapper);
			await recordingsStore.load();

			// Initialize progress
			emitEvent({
				type: 'processing_started',
				recording_id: 'rec-2'
			});

			await waitFor(() => {
				expect(screen.getByTestId('recording-rec-2-progress')).toHaveTextContent('0');
			});

			// Update progress
			emitEvent({
				type: 'processing_progress',
				recording_id: 'rec-2',
				percent: 50
			});

			await waitFor(() => {
				expect(screen.getByTestId('recording-rec-2-progress')).toHaveTextContent('50');
			});
		});
	});

	describe('processing_complete event', () => {
		it('sets status to processed', async () => {
			const processingRecording: Recording = {
				...mockRecordings[1],
				status: 'processing'
			};
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue([processingRecording]);

			render(RecordingsStoreWrapper);
			await recordingsStore.load();

			// Initialize processing
			emitEvent({
				type: 'processing_started',
				recording_id: 'rec-2'
			});

			emitEvent({
				type: 'processing_complete',
				recording_id: 'rec-2',
				output_file: '/library/streamer_two/video.mp4',
				size_bytes: 7_500_000_000
			});

			await waitFor(() => {
				expect(screen.getByTestId('recording-rec-2-status')).toHaveTextContent('processed');
				expect(screen.getByTestId('recording-rec-2-size')).toHaveTextContent('7500000000');
			});
		});

		it('clears progress from map', async () => {
			const processingRecording: Recording = {
				...mockRecordings[1],
				status: 'processing'
			};
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue([processingRecording]);

			render(RecordingsStoreWrapper);
			await recordingsStore.load();

			emitEvent({
				type: 'processing_started',
				recording_id: 'rec-2'
			});

			await waitFor(() => {
				expect(screen.getByTestId('recording-rec-2-progress')).toBeInTheDocument();
			});

			emitEvent({
				type: 'processing_complete',
				recording_id: 'rec-2',
				output_file: '/library/streamer_two/video.mp4',
				size_bytes: 7_500_000_000
			});

			await waitFor(() => {
				expect(screen.queryByTestId('recording-rec-2-progress')).not.toBeInTheDocument();
			});
		});
	});

	describe('processing_failed event', () => {
		it('sets status to processing_failed', async () => {
			const processingRecording: Recording = {
				...mockRecordings[1],
				status: 'processing'
			};
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue([processingRecording]);

			render(RecordingsStoreWrapper);
			await recordingsStore.load();

			emitEvent({
				type: 'processing_started',
				recording_id: 'rec-2'
			});

			emitEvent({
				type: 'processing_failed',
				recording_id: 'rec-2',
				error: 'FFmpeg error: invalid codec'
			});

			await waitFor(() => {
				expect(screen.getByTestId('recording-rec-2-status')).toHaveTextContent('processing_failed');
			});
		});

		it('clears progress from map', async () => {
			const processingRecording: Recording = {
				...mockRecordings[1],
				status: 'processing'
			};
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue([processingRecording]);

			render(RecordingsStoreWrapper);
			await recordingsStore.load();

			emitEvent({
				type: 'processing_started',
				recording_id: 'rec-2'
			});

			await waitFor(() => {
				expect(screen.getByTestId('recording-rec-2-progress')).toBeInTheDocument();
			});

			emitEvent({
				type: 'processing_failed',
				recording_id: 'rec-2',
				error: 'FFmpeg error'
			});

			await waitFor(() => {
				expect(screen.queryByTestId('recording-rec-2-progress')).not.toBeInTheDocument();
			});
		});
	});

	describe('event for unknown recording_id', () => {
		it('ignores recording_ended for unknown recording', async () => {
			(api.getRecordings as ReturnType<typeof vi.fn>).mockResolvedValue([mockRecordings[0]]);

			render(RecordingsStoreWrapper);
			await recordingsStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('recording-count')).toHaveTextContent('1');
			});

			// Should not throw or affect existing recordings
			emitEvent({
				type: 'recording_ended',
				recording_id: 'unknown-id',
				duration_secs: 1000,
				size_bytes: 1000,
				segment_count: 10,
				reason: 'stream_ended'
			});

			await waitFor(() => {
				expect(screen.getByTestId('recording-count')).toHaveTextContent('1');
				expect(screen.getByTestId('recording-rec-1-status')).toHaveTextContent('processed');
			});
		});
	});
});
