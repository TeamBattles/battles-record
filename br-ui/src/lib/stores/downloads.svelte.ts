import { api } from '$lib/api/client';
import { wsClient, type WebSocketEvent } from '$lib/api/websocket';
import type { DownloadSummary, DownloadStatus } from '$lib/api/types';
import { extractErrorMessage } from '$lib/utils/errors';

class DownloadsStore {
	downloads = $state<DownloadSummary[]>([]);
	isLoading = $state(false);
	error = $state<string | null>(null);

	// Filters
	statusFilter = $state<DownloadStatus | 'all'>('all');
	searchQuery = $state('');

	private _loadedServerId: string | null = null;

	constructor() {
		wsClient.subscribe((event: WebSocketEvent) => this.handleWebSocketEvent(event));
	}

	private handleWebSocketEvent(event: WebSocketEvent) {
		switch (event.type) {
			case 'connected':
				if (event.active_downloads) {
					this.downloads = event.active_downloads;
				}
				break;
			case 'download_queued':
				this.downloads = [
					...this.downloads,
					{
						id: event.download_id,
						url: event.url,
						title: event.title,
						thumbnail: event.thumbnail,
						platform_name: event.platform_name,
						channel_name: event.channel_name,
						source_platform: event.source_platform,
						status: (event.status as DownloadStatus) || 'queued',
						percent: 0,
						downloaded_bytes: 0,
						format: event.format,
						requested_by: event.requested_by,
						created_at: event.created_at,
						update_available: false
					}
				];
				break;
			case 'download_progress':
				this.downloads = this.downloads.map((d) =>
					d.id === event.download_id
						? {
								...d,
								status: 'downloading' as DownloadStatus,
								percent: event.percent,
								speed: event.speed,
								eta: event.eta,
								downloaded_bytes: event.downloaded_bytes,
								total_bytes: event.total_bytes
							}
						: d
				);
				break;
			case 'download_complete':
				this.downloads = this.downloads.map((d) =>
					d.id === event.download_id
						? { ...d, status: 'complete' as DownloadStatus, percent: 100 }
						: d
				);
				break;
			case 'download_failed':
				this.downloads = this.downloads.map((d) =>
					d.id === event.download_id
						? {
								...d,
								status: 'failed' as DownloadStatus,
								error: event.error,
								update_available: event.update_available
							}
						: d
				);
				break;
			case 'download_paused':
				this.downloads = this.downloads.map((d) =>
					d.id === event.download_id ? { ...d, status: 'paused' as DownloadStatus } : d
				);
				break;
			case 'download_resumed':
				this.downloads = this.downloads.map((d) =>
					d.id === event.download_id
						? { ...d, status: 'downloading' as DownloadStatus }
						: d
				);
				break;
			case 'download_cancelled':
				this.downloads = this.downloads.map((d) =>
					d.id === event.download_id
						? { ...d, status: 'cancelled' as DownloadStatus }
						: d
				);
				break;
		}
	}

	async load(serverId?: string) {
		const isServerSwitch = serverId != null && serverId !== this._loadedServerId;
		const hasData = this.downloads.length > 0;

		if (!hasData || isServerSwitch) {
			this.isLoading = true;
			if (isServerSwitch) this.downloads = [];
		}
		this.error = null;

		try {
			this.downloads = await api.getDownloads();
			if (serverId) this._loadedServerId = serverId;
		} catch (e) {
			this.error = extractErrorMessage(e, 'Failed to load downloads');
		} finally {
			this.isLoading = false;
		}
	}

	get filteredDownloads(): DownloadSummary[] {
		let result = this.downloads;

		if (this.statusFilter !== 'all') {
			result = result.filter((d) => d.status === this.statusFilter);
		}
		if (this.searchQuery.trim()) {
			const query = this.searchQuery.toLowerCase();
			result = result.filter(
				(d) =>
					d.title?.toLowerCase().includes(query) ||
					d.channel_name.toLowerCase().includes(query) ||
					d.url.toLowerCase().includes(query)
			);
		}

		return result;
	}

	get activeCount(): number {
		return this.downloads.filter(
			(d) =>
				d.status === 'downloading' ||
				d.status === 'extracting_info' ||
				d.status === 'processing'
		).length;
	}

	get queuedCount(): number {
		return this.downloads.filter((d) => d.status === 'queued').length;
	}

	async pause(id: string) {
		try {
			await api.pauseDownload(id);
		} catch (e) {
			this.error = extractErrorMessage(e, 'Failed to pause download');
		}
	}

	async resume(id: string) {
		try {
			await api.resumeDownload(id);
		} catch (e) {
			this.error = extractErrorMessage(e, 'Failed to resume download');
		}
	}

	async cancel(id: string) {
		try {
			await api.cancelDownload(id);
		} catch (e) {
			this.error = extractErrorMessage(e, 'Failed to cancel download');
		}
	}

	async prioritize(id: string) {
		try {
			await api.prioritizeDownload(id);
		} catch (e) {
			this.error = extractErrorMessage(e, 'Failed to prioritize download');
		}
	}

	async remove(id: string) {
		this.downloads = this.downloads.filter((d) => d.id !== id);
		try {
			await api.deleteDownload(id);
		} catch {
			// Already gone from server or other failure - local list is already updated
		}
	}
}

export const downloadsStore = new DownloadsStore();
