import type { WebSocketEvent } from '$lib/api/websocket';
import { formatDuration, formatBytes, MAX_ACTIVITY_EVENTS } from '$lib/utils';

export type EventCategory = 'recording' | 'channel' | 'processing' | 'system';

export interface ActivityEvent {
	id: string;
	timestamp: Date;
	type: string;
	category: EventCategory;
	channelName?: string;
	platform?: string;
	message: string;
	data: Record<string, unknown>;
}

class ActivityStore {
	events = $state<ActivityEvent[]>([]);

	// Filters
	categoryFilter = $state<EventCategory | 'all'>('all');
	channelFilter = $state<string | 'all'>('all');
	searchQuery = $state('');

	// UI state
	autoScroll = $state(true);
	selectedEventId = $state<string | null>(null);

	get filteredEvents(): ActivityEvent[] {
		return this.events.filter((event) => {
			// Category filter
			if (this.categoryFilter !== 'all' && event.category !== this.categoryFilter) {
				return false;
			}

			// Channel filter
			if (this.channelFilter !== 'all' && event.channelName !== this.channelFilter) {
				return false;
			}

			// Search filter
			if (this.searchQuery) {
				const query = this.searchQuery.toLowerCase();
				const matchesMessage = event.message.toLowerCase().includes(query);
				const matchesType = event.type.toLowerCase().includes(query);
				const matchesChannel = event.channelName?.toLowerCase().includes(query);
				if (!matchesMessage && !matchesType && !matchesChannel) {
					return false;
				}
			}

			return true;
		});
	}

	get selectedEvent(): ActivityEvent | null {
		return this.events.find((e) => e.id === this.selectedEventId) ?? null;
	}

	get uniqueChannels(): string[] {
		const channels = new Set<string>();
		for (const event of this.events) {
			if (event.channelName) {
				channels.add(event.channelName);
			}
		}
		return Array.from(channels).sort();
	}

	get eventCount(): number {
		return this.events.length;
	}

	get filteredCount(): number {
		return this.filteredEvents.length;
	}

	/**
	 * Process a WebSocket event and add it to the activity log
	 */
	handleWebSocketEvent(wsEvent: WebSocketEvent & Record<string, unknown>) {
		const activityEvent = this.mapWebSocketEvent(wsEvent);
		if (activityEvent) {
			this.addEvent(activityEvent);
		}
	}

	private mapWebSocketEvent(
		wsEvent: WebSocketEvent & Record<string, unknown>
	): ActivityEvent | null {
		const id = crypto.randomUUID();
		const timestamp = new Date();
		const type = wsEvent.type;

		switch (type) {
			case 'recording_started':
				return {
					id,
					timestamp,
					type,
					category: 'recording',
					channelName: wsEvent.channel_name as string | undefined,
					platform: wsEvent.platform as string | undefined,
					message: `Recording started for ${wsEvent.channel_name}`,
					data: { ...wsEvent }
				};

			case 'recording_ended':
				return {
					id,
					timestamp,
					type,
					category: 'recording',
					channelName: wsEvent.channel_name as string | undefined,
					message: `Recording ended (${formatDuration(wsEvent.duration_secs as number | undefined)})`,
					data: { ...wsEvent }
				};

			case 'channel_status':
				return {
					id,
					timestamp,
					type,
					category: 'channel',
					channelName: wsEvent.name as string | undefined,
					platform: wsEvent.platform as string | undefined,
					message: `${wsEvent.name} is now ${wsEvent.status}`,
					data: { ...wsEvent }
				};

			case 'error':
				return {
					id,
					timestamp,
					type: 'channel_error',
					category: 'channel',
					channelName: wsEvent.name as string | undefined,
					message: (wsEvent.message as string) || 'Unknown error',
					data: { ...wsEvent }
				};

			case 'processing_started':
				return {
					id,
					timestamp,
					type,
					category: 'processing',
					message: `Processing started for recording ${(wsEvent.recording_id as string)?.slice(0, 8)}...`,
					data: { ...wsEvent }
				};

			case 'processing_progress':
				// Skip progress events to avoid spam
				return null;

			case 'processing_complete':
				return {
					id,
					timestamp,
					type,
					category: 'processing',
					message: `Processing complete: ${wsEvent.output_file}`,
					data: { ...wsEvent }
				};

			case 'processing_failed':
				return {
					id,
					timestamp,
					type,
					category: 'processing',
					message: `Processing failed: ${wsEvent.error}`,
					data: { ...wsEvent }
				};

			case 'disk_warning':
				return {
					id,
					timestamp,
					type,
					category: 'system',
					message: `Disk warning: ${wsEvent.usage_percent}% used (${formatBytes(wsEvent.free_bytes ?? 0)} free)`,
					data: { ...wsEvent }
				};

			case 'config_reloaded':
				return {
					id,
					timestamp,
					type,
					category: 'system',
					message: `Config reloaded: ${(wsEvent.sections as string[] | undefined)?.join(', ') || 'all sections'}`,
					data: { ...wsEvent }
				};

			case 'schedule_skip':
				return {
					id,
					timestamp,
					type,
					category: 'channel',
					channelName: wsEvent.channel_name as string | undefined,
					platform: wsEvent.platform as string | undefined,
					message: `Recording skipped for ${wsEvent.channel_name} (outside schedule)`,
					data: { ...wsEvent }
				};

			case 'filter_skip':
				return {
					id,
					timestamp,
					type,
					category: 'channel',
					channelName: wsEvent.channel_name as string | undefined,
					platform: wsEvent.platform as string | undefined,
					message: `Recording skipped for ${wsEvent.channel_name} (filter rules)`,
					data: { ...wsEvent }
				};

			case 'quota_skip':
				return {
					id,
					timestamp,
					type,
					category: 'channel',
					channelName: wsEvent.channel_name as string | undefined,
					platform: wsEvent.platform as string | undefined,
					message: `Recording skipped for ${wsEvent.channel_name} (quota exceeded)`,
					data: { ...wsEvent }
				};

			case 'quota_status_changed':
				return {
					id,
					timestamp,
					type,
					category: 'system',
					channelName: wsEvent.channel_name as string | undefined,
					message: `Quota status for ${wsEvent.channel_name}: ${wsEvent.quota_status} (${wsEvent.quota_percent}% used)`,
					data: { ...wsEvent }
				};

			case 'connected':
				return {
					id,
					timestamp,
					type,
					category: 'system',
					message: `Connected to daemon (${(wsEvent.channels as unknown[])?.length || 0} channels, ${(wsEvent.active_recordings as unknown[])?.length || 0} active recordings)`,
					data: { ...wsEvent }
				};

			case 'segment_downloaded':
				// Skip segment events to avoid spam
				return null;

			default:
				// Log unknown event types for debugging
				return {
					id,
					timestamp,
					type,
					category: 'system',
					message: `Unknown event: ${type}`,
					data: { ...wsEvent }
				};
		}
	}

	private addEvent(event: ActivityEvent) {
		// Add to beginning for reverse chronological order
		this.events = [event, ...this.events].slice(0, MAX_ACTIVITY_EVENTS);
	}

	selectEvent(id: string | null) {
		this.selectedEventId = id;
	}

	setCategoryFilter(category: EventCategory | 'all') {
		this.categoryFilter = category;
	}

	setChannelFilter(channel: string | 'all') {
		this.channelFilter = channel;
	}

	setSearch(query: string) {
		this.searchQuery = query;
	}

	setAutoScroll(enabled: boolean) {
		this.autoScroll = enabled;
	}

	clear() {
		this.events = [];
		this.selectedEventId = null;
	}

	exportToJson(): string {
		const exportData = {
			exportedAt: new Date().toISOString(),
			eventCount: this.events.length,
			events: this.events.map((e) => ({
				...e,
				timestamp: e.timestamp.toISOString()
			}))
		};
		return JSON.stringify(exportData, null, 2);
	}

	downloadExport() {
		const json = this.exportToJson();
		const blob = new Blob([json], { type: 'application/json' });
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a');
		a.href = url;
		a.download = `activity-log-${new Date().toISOString().split('T')[0]}.json`;
		document.body.appendChild(a);
		a.click();
		document.body.removeChild(a);
		URL.revokeObjectURL(url);
	}
}

export const activityStore = new ActivityStore();
