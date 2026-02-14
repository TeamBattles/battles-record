import { api } from '$lib/api/client';
import { wsClient, type WebSocketEvent } from '$lib/api/websocket';
import type { Recording, RecordingStatus } from '$lib/api/types';

class RecordingsStore {
	recordings = $state<Recording[]>([]);
	isLoading = $state(false);
	error = $state<string | null>(null);

	// Processing progress tracking (recording_id -> percent)
	processingProgress = $state<Map<string, number>>(new Map());

	// Filters
	platformFilter = $state<string | null>(null);
	statusFilter = $state<RecordingStatus | null>(null);
	searchQuery = $state('');

	constructor() {
		// Subscribe to WebSocket events for processing updates
		wsClient.subscribe((event: WebSocketEvent) => {
			this.handleWebSocketEvent(event);
		});
	}

	private handleWebSocketEvent(event: WebSocketEvent) {
		switch (event.type) {
			case 'recording_started': {
				console.log('[RecordingsStore] Recording started:', event.recording_id, event.channel_name);
				// Add new recording to the list if not already present
				const exists = this.recordings.some((r) => r.id === event.recording_id);
				if (!exists) {
					const newRecording: Recording = {
						id: event.recording_id,
						channel_name: event.channel_name,
						platform: event.platform,
						started_at: new Date().toISOString(),
						status: 'recording',
						path: '',
						size_bytes: 0,
						duration_secs: 0
					};
					this.recordings = [newRecording, ...this.recordings];
				}
				break;
			}
			case 'recording_ended': {
				console.log('[RecordingsStore] Recording ended:', event.recording_id);
				const idx = this.recordings.findIndex((r) => r.id === event.recording_id);
				if (idx !== -1) {
					this.recordings[idx] = {
						...this.recordings[idx],
						status: 'pending_processing',
						duration_secs: event.duration_secs,
						size_bytes: event.size_bytes,
						ended_at: new Date().toISOString()
					};
				}
				break;
			}
			case 'segment_downloaded': {
				// Update active recording stats in real-time
				const idx = this.recordings.findIndex((r) => r.id === event.recording_id);
				if (idx !== -1 && this.recordings[idx].status === 'recording') {
					// Estimate duration: ~2 seconds per segment
					const estimatedDuration = event.total_segments * 2;
					this.recordings[idx] = {
						...this.recordings[idx],
						size_bytes: event.total_bytes,
						duration_secs: estimatedDuration
					};
				}
				break;
			}
			case 'processing_started': {
				console.log('[RecordingsStore] Processing started:', event.recording_id);
				// Update recording status and initialize progress
				const idx = this.recordings.findIndex((r) => r.id === event.recording_id);
				if (idx !== -1) {
					this.recordings[idx] = { ...this.recordings[idx], status: 'processing' };
				}
				this.processingProgress = new Map(this.processingProgress).set(event.recording_id, 0);
				break;
			}
			case 'processing_progress': {
				console.log(
					'[RecordingsStore] Processing progress:',
					event.recording_id,
					event.percent + '%'
				);
				// Update progress percentage
				this.processingProgress = new Map(this.processingProgress).set(
					event.recording_id,
					event.percent
				);
				break;
			}
			case 'processing_complete': {
				console.log(
					'[RecordingsStore] Processing complete:',
					event.recording_id,
					event.size_bytes,
					'bytes'
				);
				// Update recording status and clear progress
				const idx = this.recordings.findIndex((r) => r.id === event.recording_id);
				if (idx !== -1) {
					this.recordings[idx] = {
						...this.recordings[idx],
						status: 'processed',
						size_bytes: event.size_bytes,
						output_file: event.output_file
					};
				}
				const newProgress = new Map(this.processingProgress);
				newProgress.delete(event.recording_id);
				this.processingProgress = newProgress;
				break;
			}
			case 'processing_failed': {
				console.log('[RecordingsStore] Processing failed:', event.recording_id, event.error);
				// Update recording status and clear progress
				const idx = this.recordings.findIndex((r) => r.id === event.recording_id);
				if (idx !== -1) {
					this.recordings[idx] = { ...this.recordings[idx], status: 'processing_failed' };
				}
				const newProgress = new Map(this.processingProgress);
				newProgress.delete(event.recording_id);
				this.processingProgress = newProgress;
				break;
			}
		}
	}

	getProcessingProgress(recordingId: string): number | undefined {
		return this.processingProgress.get(recordingId);
	}

	// Sorting
	sortBy = $state<'date' | 'size' | 'duration'>('date');
	sortOrder = $state<'asc' | 'desc'>('desc');

	get filteredRecordings(): Recording[] {
		let result = this.recordings;

		// Platform filter
		if (this.platformFilter) {
			result = result.filter((r) => r.platform === this.platformFilter);
		}

		// Status filter
		if (this.statusFilter) {
			result = result.filter((r) => r.status === this.statusFilter);
		}

		// Search
		if (this.searchQuery) {
			const q = this.searchQuery.toLowerCase();
			result = result.filter(
				(r) =>
					r.channel_name.toLowerCase().includes(q) ||
					r.title?.toLowerCase().includes(q) ||
					r.game?.toLowerCase().includes(q)
			);
		}

		// Sort
		result = [...result].sort((a, b) => {
			let comparison = 0;
			switch (this.sortBy) {
				case 'date':
					comparison = new Date(a.started_at).getTime() - new Date(b.started_at).getTime();
					break;
				case 'size':
					comparison = a.size_bytes - b.size_bytes;
					break;
				case 'duration':
					comparison = (a.duration_secs ?? 0) - (b.duration_secs ?? 0);
					break;
			}
			return this.sortOrder === 'desc' ? -comparison : comparison;
		});

		return result;
	}

	async load() {
		this.isLoading = true;
		this.error = null;
		try {
			this.recordings = await api.getRecordings();
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to load recordings';
		} finally {
			this.isLoading = false;
		}
	}

	async deleteRecording(id: string): Promise<boolean> {
		try {
			await api.deleteRecording(id);
			this.recordings = this.recordings.filter((r) => r.id !== id);
			return true;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to delete recording';
			return false;
		}
	}

	async processRecording(id: string): Promise<boolean> {
		try {
			await api.reprocessRecording(id);
			// Update local status to pending_processing (will be picked up by reconciliation worker)
			const index = this.recordings.findIndex((r) => r.id === id);
			if (index !== -1) {
				this.recordings[index] = { ...this.recordings[index], status: 'pending_processing' };
			}
			return true;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to trigger reprocessing';
			return false;
		}
	}

	setFilter(platform: string | null) {
		this.platformFilter = platform;
	}

	setStatusFilter(status: RecordingStatus | null) {
		this.statusFilter = status;
	}

	setSearch(query: string) {
		this.searchQuery = query;
	}

	setSort(by: 'date' | 'size' | 'duration', order: 'asc' | 'desc') {
		this.sortBy = by;
		this.sortOrder = order;
	}
}

export const recordingsStore = new RecordingsStore();
