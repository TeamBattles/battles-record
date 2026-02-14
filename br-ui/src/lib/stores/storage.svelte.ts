import { api } from '$lib/api/client';
import type {
	StorageStats,
	CleanupRequest,
	CleanupResponse,
	ChannelStorageStats,
	DownloadStorageStats,
	DownloadCleanupRequest,
	DownloadCleanupResponse
} from '$lib/api/types';

class StorageStore {
	stats = $state<StorageStats | null>(null);
	downloadStats = $state<DownloadStorageStats | null>(null);
	isLoading = $state(false);
	error = $state<string | null>(null);

	// Cleanup state
	cleanupPreview = $state<CleanupResponse | null>(null);
	downloadCleanupPreview = $state<DownloadCleanupResponse | null>(null);
	isCleaningUp = $state(false);
	cleanupError = $state<string | null>(null);

	// Sort state
	sortBy = $state<'channel' | 'size' | 'count'>('size');
	sortOrder = $state<'asc' | 'desc'>('desc');

	// Computed stats
	get totalSizeGB(): string {
		if (!this.stats) return '0';
		return (this.stats.total_size_bytes / (1024 * 1024 * 1024)).toFixed(2);
	}

	get diskUsedPercent(): number {
		if (!this.stats || this.stats.disk_total_bytes === 0) return 0;
		const usedBytes = this.stats.disk_total_bytes - this.stats.disk_free_bytes;
		return Math.round((usedBytes / this.stats.disk_total_bytes) * 100);
	}

	get diskFreeGB(): string {
		if (!this.stats) return '0';
		return (this.stats.disk_free_bytes / (1024 * 1024 * 1024)).toFixed(1);
	}

	get diskTotalGB(): string {
		if (!this.stats) return '0';
		return (this.stats.disk_total_bytes / (1024 * 1024 * 1024)).toFixed(1);
	}

	get recordingsUsagePercent(): string {
		if (!this.stats || this.stats.disk_total_bytes === 0) return '0';
		const percent = (this.stats.total_size_bytes / this.stats.disk_total_bytes) * 100;
		return percent < 1 ? percent.toFixed(2) : percent.toFixed(1);
	}

	// Check if recordings_dir and library_dir are different
	get hasSeparateLibrary(): boolean {
		if (!this.stats) return false;
		return this.stats.recordings_dir !== this.stats.library_dir;
	}

	// Check if library is on a different disk (we have library disk stats)
	get libraryOnDifferentDisk(): boolean {
		if (!this.stats) return false;
		return this.stats.library_disk_total_bytes !== undefined;
	}

	// Library disk stats
	get libraryDiskUsedPercent(): number {
		if (!this.stats?.library_disk_total_bytes) return 0;
		const usedBytes =
			this.stats.library_disk_total_bytes - (this.stats.library_disk_free_bytes ?? 0);
		return Math.round((usedBytes / this.stats.library_disk_total_bytes) * 100);
	}

	get libraryDiskFreeGB(): string {
		if (!this.stats?.library_disk_free_bytes) return '0';
		return (this.stats.library_disk_free_bytes / (1024 * 1024 * 1024)).toFixed(1);
	}

	get libraryDiskTotalGB(): string {
		if (!this.stats?.library_disk_total_bytes) return '0';
		return (this.stats.library_disk_total_bytes / (1024 * 1024 * 1024)).toFixed(1);
	}

	// Library size stats
	get librarySizeGB(): string {
		if (!this.stats) return '0.00';
		return (this.stats.library_size_bytes / (1024 * 1024 * 1024)).toFixed(2);
	}

	get libraryUsagePercent(): string {
		if (!this.stats) return '0.00';
		// Use library disk total if on different disk, otherwise use main disk total
		const diskTotal = this.stats.library_disk_total_bytes ?? this.stats.disk_total_bytes;
		if (diskTotal === 0) return '0.00';
		const percent = (this.stats.library_size_bytes / diskTotal) * 100;
		return percent < 1 ? percent.toFixed(2) : percent.toFixed(1);
	}

	// Get the appropriate disk total for the library (its own disk or shared with recordings)
	get libraryDiskTotalForUsage(): string {
		if (!this.stats) return '0';
		const diskTotal = this.stats.library_disk_total_bytes ?? this.stats.disk_total_bytes;
		return (diskTotal / (1024 * 1024 * 1024)).toFixed(1);
	}

	get oldestRecordingDays(): number | null {
		// This would need to be calculated from recordings data
		// For now return null - could be computed if we had recordings list
		return null;
	}

	// Download stats
	get totalDownloadsSizeGB(): string {
		if (!this.downloadStats) return '0';
		return (this.downloadStats.total_size_bytes / (1024 * 1024 * 1024)).toFixed(2);
	}

	get sortedDownloadChannelStats(): ChannelStorageStats[] {
		if (!this.downloadStats) return [];
		const sorted = [...this.downloadStats.per_channel];
		sorted.sort((a, b) => {
			let cmp = 0;
			switch (this.sortBy) {
				case 'channel': cmp = a.channel.localeCompare(b.channel); break;
				case 'size': cmp = a.size_bytes - b.size_bytes; break;
				case 'count': cmp = a.count - b.count; break;
			}
			return this.sortOrder === 'desc' ? -cmp : cmp;
		});
		return sorted;
	}

	getDownloadChannelPercent(channelStats: ChannelStorageStats): number {
		if (!this.downloadStats || this.downloadStats.total_size_bytes === 0) return 0;
		return Math.round((channelStats.size_bytes / this.downloadStats.total_size_bytes) * 100);
	}

	get sortedChannelStats(): ChannelStorageStats[] {
		if (!this.stats) return [];

		const sorted = [...this.stats.per_channel];
		sorted.sort((a, b) => {
			let comparison = 0;
			switch (this.sortBy) {
				case 'channel':
					comparison = a.channel.localeCompare(b.channel);
					break;
				case 'size':
					comparison = a.size_bytes - b.size_bytes;
					break;
				case 'count':
					comparison = a.count - b.count;
					break;
			}
			return this.sortOrder === 'desc' ? -comparison : comparison;
		});
		return sorted;
	}

	// Track which server's data we have for stale-while-revalidate
	private _loadedServerId: string | null = null;

	async load(serverId?: string) {
		const isServerSwitch = serverId != null && serverId !== this._loadedServerId;
		const hasData = this.stats !== null;

		if (!hasData || isServerSwitch) {
			this.isLoading = true;
			if (isServerSwitch) this.stats = null;
		}
		this.error = null;
		try {
			const [stats, dlStats] = await Promise.all([
				api.getStorageStats(),
				api.getDownloadStorageStats()
			]);
			this.stats = stats;
			this.downloadStats = dlStats;
			if (serverId) this._loadedServerId = serverId;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to load storage stats';
		} finally {
			this.isLoading = false;
		}
	}

	async previewCleanup(request: Omit<CleanupRequest, 'dry_run'>): Promise<CleanupResponse | null> {
		this.isCleaningUp = true;
		this.cleanupError = null;
		this.cleanupPreview = null;
		try {
			const response = await api.cleanupStorage({ ...request, dry_run: true });
			this.cleanupPreview = response;
			return response;
		} catch (e) {
			this.cleanupError = e instanceof Error ? e.message : 'Failed to preview cleanup';
			return null;
		} finally {
			this.isCleaningUp = false;
		}
	}

	async executeCleanup(request: Omit<CleanupRequest, 'dry_run'>): Promise<CleanupResponse | null> {
		this.isCleaningUp = true;
		this.cleanupError = null;
		try {
			const response = await api.cleanupStorage({ ...request, dry_run: false });
			// Refresh stats after cleanup
			await this.load();
			this.cleanupPreview = null;
			return response;
		} catch (e) {
			this.cleanupError = e instanceof Error ? e.message : 'Failed to execute cleanup';
			return null;
		} finally {
			this.isCleaningUp = false;
		}
	}

	async previewDownloadCleanup(request: Omit<DownloadCleanupRequest, 'dry_run'>) {
		this.isCleaningUp = true;
		this.cleanupError = null;
		this.downloadCleanupPreview = null;
		try {
			this.downloadCleanupPreview = await api.cleanupDownloads({ ...request, dry_run: true });
		} catch (e) {
			this.cleanupError = e instanceof Error ? e.message : 'Failed to preview cleanup';
		} finally {
			this.isCleaningUp = false;
		}
	}

	async executeDownloadCleanup(request: Omit<DownloadCleanupRequest, 'dry_run'>) {
		this.isCleaningUp = true;
		this.cleanupError = null;
		try {
			await api.cleanupDownloads({ ...request, dry_run: false });
			await this.load();
			this.downloadCleanupPreview = null;
		} catch (e) {
			this.cleanupError = e instanceof Error ? e.message : 'Failed to execute cleanup';
		} finally {
			this.isCleaningUp = false;
		}
	}

	clearCleanupPreview() {
		this.cleanupPreview = null;
		this.downloadCleanupPreview = null;
		this.cleanupError = null;
	}

	setSort(by: 'channel' | 'size' | 'count') {
		if (this.sortBy === by) {
			this.sortOrder = this.sortOrder === 'asc' ? 'desc' : 'asc';
		} else {
			this.sortBy = by;
			this.sortOrder = 'desc';
		}
	}

	// Helper to format bytes to human readable
	formatBytes(bytes: number): string {
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
		if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
		return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
	}

	// Calculate percentage of total storage for a channel
	getChannelPercent(channelStats: ChannelStorageStats): number {
		if (!this.stats || this.stats.total_size_bytes === 0) return 0;
		return Math.round((channelStats.size_bytes / this.stats.total_size_bytes) * 100);
	}
}

export const storageStore = new StorageStore();
