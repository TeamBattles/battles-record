import { api, wsClient } from '$lib/api';
import type { DaemonStatus, Channel, Recording, WebSocketEvent } from '$lib/api';
import { versionStore } from './version.svelte';

class DashboardStore {
	status = $state<DaemonStatus | null>(null);
	channels = $state<Channel[]>([]);
	activeRecordings = $state<Recording[]>([]);
	isLoading = $state(false);
	error = $state<string | null>(null);

	// Track which server's data we have for stale-while-revalidate
	private _loadedServerId: string | null = null;

	private unsubscribe: (() => void) | null = null;

	async load(serverId?: string) {
		const isServerSwitch = serverId != null && serverId !== this._loadedServerId;
		const hasData = this.status !== null;

		if (!hasData || isServerSwitch) {
			this.isLoading = true;
			if (isServerSwitch) {
				this.status = null;
				this.channels = [];
				this.activeRecordings = [];
			}
		}
		this.error = null;

		try {
			const [status, channels, recordings] = await Promise.all([
				api.getStatus(),
				api.getChannels(),
				api.getRecordings()
			]);

			this.status = status;
			versionStore.checkCompatibility(status.min_client_version, status.max_client_version);
			versionStore.setDaemonUpdateInfo(status.update);
			this.channels = channels;
			this.activeRecordings = recordings.filter((r) => r.status === 'recording');
			if (serverId) this._loadedServerId = serverId;
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
		// Refresh dashboard data when recordings or channels change
		if (
			event.type === 'recording_started' ||
			event.type === 'recording_ended' ||
			event.type === 'channel_added' ||
			event.type === 'channel_removed'
		) {
			this.load();
		}
	}
}

export const dashboardStore = new DashboardStore();
