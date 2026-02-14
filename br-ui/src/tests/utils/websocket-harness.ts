/**
 * WebSocket Test Harness
 *
 * Provides utilities for simulating WebSocket events in tests.
 * This allows testing store reactions to real-time events without
 * needing an actual WebSocket connection.
 */

import type { BackendChannel } from '$lib/api/backend-types';
import type { WebSocketEvent } from '$lib/api/websocket';
import type { QuotaStatus } from '$lib/api/types';

/**
 * Creates mock backend channel data
 */
export function createMockBackendChannel(overrides: Partial<BackendChannel> = {}): BackendChannel {
	return {
		id: crypto.randomUUID(),
		name: 'test_channel',
		platform: 'twitch',
		enabled: true,
		quality: 'best',
		status: 'offline',
		current_stream: undefined,
		quota_gb: undefined,
		retention_days: undefined,
		quota_status: 'unlimited',
		quota_used_bytes: 0,
		quota_percent: 0,
		...overrides
	};
}

/**
 * Creates a connected event with channels
 */
export function createConnectedEvent(channels: BackendChannel[] = []): WebSocketEvent {
	return {
		type: 'connected',
		channels,
		active_recordings: []
	};
}

/**
 * Creates a channel status changed event
 */
export function createChannelStatusEvent(
	channelId: string,
	name: string,
	platform: string,
	status: 'live' | 'offline' | 'recording',
	stream?: { title: string; game?: string; viewers: number }
): WebSocketEvent {
	return {
		type: 'channel_status',
		channel_id: channelId,
		name,
		platform,
		status,
		stream
	};
}

/**
 * Creates a recording started event
 */
export function createRecordingStartedEvent(
	channelName: string,
	recordingId: string = crypto.randomUUID(),
	channelId: string = crypto.randomUUID(),
	platform: string = 'twitch',
	quality: string = 'best'
): WebSocketEvent {
	return {
		type: 'recording_started',
		recording_id: recordingId,
		channel_id: channelId,
		channel_name: channelName,
		platform,
		quality
	};
}

/**
 * Creates a recording ended event
 */
export function createRecordingEndedEvent(
	recordingId: string = crypto.randomUUID(),
	durationSecs: number = 3600,
	sizeBytes: number = 1_000_000_000,
	segmentCount: number = 100,
	reason: string = 'stream_ended'
): WebSocketEvent {
	return {
		type: 'recording_ended',
		recording_id: recordingId,
		duration_secs: durationSecs,
		size_bytes: sizeBytes,
		segment_count: segmentCount,
		reason
	};
}

/**
 * Creates a segment downloaded event
 */
export function createSegmentDownloadedEvent(
	recordingId: string,
	sequence: number,
	sizeBytes: number,
	totalSegments: number,
	totalBytes: number
): WebSocketEvent {
	return {
		type: 'segment_downloaded',
		recording_id: recordingId,
		sequence,
		size_bytes: sizeBytes,
		total_segments: totalSegments,
		total_bytes: totalBytes
	};
}

/**
 * Creates a processing started event
 */
export function createProcessingStartedEvent(recordingId: string): WebSocketEvent {
	return {
		type: 'processing_started',
		recording_id: recordingId
	};
}

/**
 * Creates a processing progress event
 */
export function createProcessingProgressEvent(
	recordingId: string,
	percent: number
): WebSocketEvent {
	return {
		type: 'processing_progress',
		recording_id: recordingId,
		percent
	};
}

/**
 * Creates a processing complete event
 */
export function createProcessingCompleteEvent(
	recordingId: string,
	outputFile: string = '/library/test/video.mp4',
	sizeBytes: number = 500_000_000
): WebSocketEvent {
	return {
		type: 'processing_complete',
		recording_id: recordingId,
		output_file: outputFile,
		size_bytes: sizeBytes
	};
}

/**
 * Creates a processing failed event
 */
export function createProcessingFailedEvent(
	recordingId: string,
	error: string = 'FFmpeg error'
): WebSocketEvent {
	return {
		type: 'processing_failed',
		recording_id: recordingId,
		error
	};
}

/**
 * Creates a quota status changed event
 */
export function createQuotaStatusChangedEvent(
	channelId: string,
	channelName: string,
	quotaStatus: QuotaStatus,
	quotaUsedBytes: number,
	quotaPercent: number
): WebSocketEvent {
	return {
		type: 'quota_status_changed',
		channel_id: channelId,
		channel_name: channelName,
		quota_status: quotaStatus,
		quota_used_bytes: quotaUsedBytes,
		quota_percent: quotaPercent
	};
}

/**
 * Creates a quota skip event (recording blocked due to quota)
 */
export function createQuotaSkipEvent(
	channelId: string,
	channelName: string,
	platform: string,
	quotaUsedBytes: number,
	quotaLimitBytes: number,
	message: string = 'Quota exceeded'
): WebSocketEvent {
	return {
		type: 'quota_skip',
		channel_id: channelId,
		channel_name: channelName,
		platform,
		quota_used_bytes: quotaUsedBytes,
		quota_limit_bytes: quotaLimitBytes,
		message
	};
}

/**
 * Creates an error event
 */
export function createErrorEvent(message: string): WebSocketEvent {
	return {
		type: 'error',
		message
	};
}

/**
 * WebSocket event emitter for testing
 *
 * This class provides a simple way to simulate WebSocket events in tests.
 * Subscribe handlers and then emit events to trigger them.
 */
export class WebSocketTestHarness {
	private handlers: Set<(event: WebSocketEvent) => void> = new Set();

	/**
	 * Subscribe to events (returns unsubscribe function)
	 */
	subscribe(handler: (event: WebSocketEvent) => void): () => void {
		this.handlers.add(handler);
		return () => this.handlers.delete(handler);
	}

	/**
	 * Emit an event to all subscribers
	 */
	emit(event: WebSocketEvent): void {
		this.handlers.forEach((handler) => handler(event));
	}

	/**
	 * Clear all subscribers
	 */
	clear(): void {
		this.handlers.clear();
	}

	/**
	 * Get the number of subscribers
	 */
	get subscriberCount(): number {
		return this.handlers.size;
	}

	// Convenience methods for common events

	/**
	 * Simulate a connection with initial channels
	 */
	emitConnected(channels: BackendChannel[] = []): void {
		this.emit(createConnectedEvent(channels));
	}

	/**
	 * Simulate a channel going live
	 */
	emitChannelLive(
		channelId: string,
		name: string = 'test_channel',
		platform: string = 'twitch'
	): void {
		this.emit(createChannelStatusEvent(channelId, name, platform, 'live'));
	}

	/**
	 * Simulate a channel going offline
	 */
	emitChannelOffline(
		channelId: string,
		name: string = 'test_channel',
		platform: string = 'twitch'
	): void {
		this.emit(createChannelStatusEvent(channelId, name, platform, 'offline'));
	}

	/**
	 * Simulate a channel starting recording
	 */
	emitChannelRecording(
		channelId: string,
		name: string = 'test_channel',
		platform: string = 'twitch'
	): void {
		this.emit(createChannelStatusEvent(channelId, name, platform, 'recording'));
	}

	/**
	 * Simulate a recording session (started -> segments -> ended)
	 */
	async emitRecordingSession(
		channelName: string,
		options: {
			channelId?: string;
			platform?: string;
			durationSecs?: number;
			segmentCount?: number;
			totalBytes?: number;
			delayMs?: number;
		} = {}
	): Promise<void> {
		const {
			channelId = crypto.randomUUID(),
			platform = 'twitch',
			durationSecs = 300,
			segmentCount = 30,
			totalBytes = 50_000_000,
			delayMs = 0
		} = options;

		const recordingId = crypto.randomUUID();

		this.emit(createRecordingStartedEvent(channelName, recordingId, channelId, platform));

		if (delayMs > 0) {
			await sleep(delayMs);
		}

		// Emit progress segments
		const segmentsPerEmit = Math.ceil(segmentCount / 5);
		for (let i = 1; i <= segmentCount; i += segmentsPerEmit) {
			const currentSegment = Math.min(i + segmentsPerEmit - 1, segmentCount);
			const currentBytes = Math.floor((currentSegment / segmentCount) * totalBytes);
			this.emit(
				createSegmentDownloadedEvent(
					recordingId,
					currentSegment,
					Math.floor(currentBytes / currentSegment),
					currentSegment,
					currentBytes
				)
			);
			if (delayMs > 0) {
				await sleep(delayMs / 5);
			}
		}

		this.emit(createRecordingEndedEvent(recordingId, durationSecs, totalBytes, segmentCount));
	}

	/**
	 * Simulate a processing session (started -> progress -> complete/failed)
	 */
	async emitProcessingSession(
		recordingId: string,
		options: {
			success?: boolean;
			progressSteps?: number;
			delayMs?: number;
			outputFile?: string;
			sizeBytes?: number;
			errorMessage?: string;
		} = {}
	): Promise<void> {
		const {
			success = true,
			progressSteps = 5,
			delayMs = 0,
			outputFile = '/library/test/video.mp4',
			sizeBytes = 500_000_000,
			errorMessage = 'Processing failed'
		} = options;

		this.emit(createProcessingStartedEvent(recordingId));

		// Emit progress updates
		for (let i = 1; i <= progressSteps; i++) {
			const percent = Math.floor((i / progressSteps) * 100);
			this.emit(createProcessingProgressEvent(recordingId, percent));
			if (delayMs > 0) {
				await sleep(delayMs);
			}
		}

		if (success) {
			this.emit(createProcessingCompleteEvent(recordingId, outputFile, sizeBytes));
		} else {
			this.emit(createProcessingFailedEvent(recordingId, errorMessage));
		}
	}
}

/**
 * Helper function for delays in tests
 */
function sleep(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Create a fresh harness instance
 */
export function createWebSocketHarness(): WebSocketTestHarness {
	return new WebSocketTestHarness();
}
