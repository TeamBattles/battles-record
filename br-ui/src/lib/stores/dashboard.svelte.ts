import { api, wsClient } from '$lib/api';
import type { DaemonStatus, Channel, Recording, WebSocketEvent } from '$lib/api';

class DashboardStore {
	status = $state<DaemonStatus | null>(null);
	channels = $state<Channel[]>([]);
	activeRecordings = $state<Recording[]>([]);
	isLoading = $state(false);
	error = $state<string | null>(null);

	private unsubscribe: (() => void) | null = null;

	async load() {
		this.isLoading = true;
		this.error = null;

		try {
			const [status, channels, recordings] = await Promise.all([
				api.getStatus(),
				api.getChannels(),
				api.getRecordings()
			]);

			this.status = status;
			this.channels = channels;
			this.activeRecordings = recordings.filter((r) => r.status === 'recording');
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to load dashboard';
		} finally {
			this.isLoading = false;
		}
	}

	subscribe() {
		this.unsubscribe = wsClient.subscribe(this.handleEvent.bind(this));
	}

	unsubscribeEvents() {
		this.unsubscribe?.();
		this.unsubscribe = null;
	}

	private handleEvent(event: WebSocketEvent) {
		// Refresh dashboard data when recordings change
		if (event.type === 'recording_started' || event.type === 'recording_ended') {
			this.load();
		}
	}
}

export const dashboardStore = new DashboardStore();
