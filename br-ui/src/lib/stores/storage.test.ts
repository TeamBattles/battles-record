import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { StorageStats, ChannelStorageStats } from '$lib/api/types';

// Mock the API module before importing the store
vi.mock('$lib/api/client', () => ({
	api: {
		getStorageStats: vi.fn(),
		cleanupStorage: vi.fn()
	}
}));

describe('StorageStore', () => {
	// Create a testable version of the store logic
	class TestableStorageStore {
		stats: StorageStats | null = null;
		isLoading = false;
		error: string | null = null;

		cleanupPreview: {
			recordings_affected: number;
			bytes_to_free: number;
			dry_run: boolean;
		} | null = null;
		isCleaningUp = false;
		cleanupError: string | null = null;

		sortBy: 'channel' | 'size' | 'count' = 'size';
		sortOrder: 'asc' | 'desc' = 'desc';

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

		get hasSeparateLibrary(): boolean {
			if (!this.stats) return false;
			return this.stats.recordings_dir !== this.stats.library_dir;
		}

		get libraryOnDifferentDisk(): boolean {
			if (!this.stats) return false;
			return this.stats.library_disk_total_bytes !== undefined;
		}

		get librarySizeGB(): string {
			if (!this.stats) return '0.00';
			return (this.stats.library_size_bytes / (1024 * 1024 * 1024)).toFixed(2);
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

		formatBytes(bytes: number): string {
			if (bytes < 1024) return `${bytes} B`;
			if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
			if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
			return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
		}

		getChannelPercent(channelStats: ChannelStorageStats): number {
			if (!this.stats || this.stats.total_size_bytes === 0) return 0;
			return Math.round((channelStats.size_bytes / this.stats.total_size_bytes) * 100);
		}

		setSort(by: 'channel' | 'size' | 'count') {
			if (this.sortBy === by) {
				this.sortOrder = this.sortOrder === 'asc' ? 'desc' : 'asc';
			} else {
				this.sortBy = by;
				this.sortOrder = 'desc';
			}
		}
	}

	let store: TestableStorageStore;

	const mockStats: StorageStats = {
		total_recordings: 10,
		total_size_bytes: 50_000_000_000, // 50 GB
		disk_free_bytes: 200_000_000_000, // 200 GB
		disk_total_bytes: 500_000_000_000, // 500 GB
		per_channel: [
			{ channel: 'streamer_a', platform: 'twitch', count: 5, size_bytes: 30_000_000_000 },
			{ channel: 'streamer_b', platform: 'youtube', count: 3, size_bytes: 15_000_000_000 },
			{ channel: 'streamer_c', platform: 'kick', count: 2, size_bytes: 5_000_000_000 }
		],
		recordings_dir: '/recordings',
		library_dir: '/library',
		library_size_bytes: 45_000_000_000
	};

	beforeEach(() => {
		store = new TestableStorageStore();
	});

	describe('computed properties with no stats', () => {
		it('totalSizeGB returns 0 when no stats', () => {
			expect(store.totalSizeGB).toBe('0');
		});

		it('diskUsedPercent returns 0 when no stats', () => {
			expect(store.diskUsedPercent).toBe(0);
		});

		it('diskFreeGB returns 0 when no stats', () => {
			expect(store.diskFreeGB).toBe('0');
		});

		it('diskTotalGB returns 0 when no stats', () => {
			expect(store.diskTotalGB).toBe('0');
		});

		it('recordingsUsagePercent returns 0 when no stats', () => {
			expect(store.recordingsUsagePercent).toBe('0');
		});

		it('hasSeparateLibrary returns false when no stats', () => {
			expect(store.hasSeparateLibrary).toBe(false);
		});

		it('libraryOnDifferentDisk returns false when no stats', () => {
			expect(store.libraryOnDifferentDisk).toBe(false);
		});

		it('librarySizeGB returns 0.00 when no stats', () => {
			expect(store.librarySizeGB).toBe('0.00');
		});

		it('sortedChannelStats returns empty array when no stats', () => {
			expect(store.sortedChannelStats).toEqual([]);
		});
	});

	describe('computed properties with stats', () => {
		beforeEach(() => {
			store.stats = mockStats;
		});

		it('totalSizeGB calculates correctly', () => {
			// 50 GB = 50_000_000_000 bytes / (1024^3) = 46.57 GB
			expect(store.totalSizeGB).toBe('46.57');
		});

		it('diskUsedPercent calculates correctly', () => {
			// Used = 500 - 200 = 300 GB
			// Percent = 300 / 500 * 100 = 60%
			expect(store.diskUsedPercent).toBe(60);
		});

		it('diskFreeGB calculates correctly', () => {
			// 200 GB = 200_000_000_000 / (1024^3) = 186.3 GB
			expect(store.diskFreeGB).toBe('186.3');
		});

		it('diskTotalGB calculates correctly', () => {
			// 500 GB = 500_000_000_000 / (1024^3) = 465.7 GB
			expect(store.diskTotalGB).toBe('465.7');
		});

		it('recordingsUsagePercent calculates correctly', () => {
			// 50 GB / 500 GB = 10%
			expect(store.recordingsUsagePercent).toBe('10.0');
		});

		it('hasSeparateLibrary returns true when dirs differ', () => {
			expect(store.hasSeparateLibrary).toBe(true);
		});

		it('hasSeparateLibrary returns false when dirs are same', () => {
			store.stats = { ...mockStats, library_dir: '/recordings' };
			expect(store.hasSeparateLibrary).toBe(false);
		});

		it('libraryOnDifferentDisk returns false when no library disk stats', () => {
			expect(store.libraryOnDifferentDisk).toBe(false);
		});

		it('libraryOnDifferentDisk returns true when library disk stats present', () => {
			store.stats = { ...mockStats, library_disk_total_bytes: 1_000_000_000_000 };
			expect(store.libraryOnDifferentDisk).toBe(true);
		});

		it('librarySizeGB calculates correctly', () => {
			// 45 GB = 45_000_000_000 / (1024^3) = 41.91 GB
			expect(store.librarySizeGB).toBe('41.91');
		});
	});

	describe('sortedChannelStats', () => {
		beforeEach(() => {
			store.stats = mockStats;
		});

		it('sorts by size descending by default', () => {
			const sorted = store.sortedChannelStats;
			expect(sorted[0].channel).toBe('streamer_a'); // 30 GB
			expect(sorted[1].channel).toBe('streamer_b'); // 15 GB
			expect(sorted[2].channel).toBe('streamer_c'); // 5 GB
		});

		it('sorts by size ascending when toggled', () => {
			store.sortBy = 'size';
			store.sortOrder = 'asc';
			const sorted = store.sortedChannelStats;
			expect(sorted[0].channel).toBe('streamer_c'); // 5 GB
			expect(sorted[1].channel).toBe('streamer_b'); // 15 GB
			expect(sorted[2].channel).toBe('streamer_a'); // 30 GB
		});

		it('sorts by channel name', () => {
			store.sortBy = 'channel';
			store.sortOrder = 'asc';
			const sorted = store.sortedChannelStats;
			expect(sorted[0].channel).toBe('streamer_a');
			expect(sorted[1].channel).toBe('streamer_b');
			expect(sorted[2].channel).toBe('streamer_c');
		});

		it('sorts by count descending', () => {
			store.sortBy = 'count';
			store.sortOrder = 'desc';
			const sorted = store.sortedChannelStats;
			expect(sorted[0].count).toBe(5);
			expect(sorted[1].count).toBe(3);
			expect(sorted[2].count).toBe(2);
		});
	});

	describe('setSort', () => {
		it('toggles order when clicking same column', () => {
			store.sortBy = 'size';
			store.sortOrder = 'desc';

			store.setSort('size');
			expect(store.sortOrder).toBe('asc');

			store.setSort('size');
			expect(store.sortOrder).toBe('desc');
		});

		it('changes column and resets to desc', () => {
			store.sortBy = 'size';
			store.sortOrder = 'asc';

			store.setSort('channel');
			expect(store.sortBy).toBe('channel');
			expect(store.sortOrder).toBe('desc');
		});
	});

	describe('formatBytes', () => {
		it('formats bytes', () => {
			expect(store.formatBytes(500)).toBe('500 B');
		});

		it('formats kilobytes', () => {
			expect(store.formatBytes(1536)).toBe('1.5 KB');
		});

		it('formats megabytes', () => {
			expect(store.formatBytes(10_485_760)).toBe('10.0 MB');
		});

		it('formats gigabytes', () => {
			expect(store.formatBytes(5_368_709_120)).toBe('5.00 GB');
		});

		it('handles zero', () => {
			expect(store.formatBytes(0)).toBe('0 B');
		});
	});

	describe('getChannelPercent', () => {
		beforeEach(() => {
			store.stats = mockStats;
		});

		it('calculates percentage correctly', () => {
			const channelStats = {
				channel: 'test',
				platform: 'twitch',
				count: 1,
				size_bytes: 10_000_000_000
			};
			// 10 GB / 50 GB = 20%
			expect(store.getChannelPercent(channelStats)).toBe(20);
		});

		it('returns 0 when no stats', () => {
			store.stats = null;
			const channelStats = {
				channel: 'test',
				platform: 'twitch',
				count: 1,
				size_bytes: 10_000_000_000
			};
			expect(store.getChannelPercent(channelStats)).toBe(0);
		});

		it('returns 0 when total size is 0', () => {
			store.stats = { ...mockStats, total_size_bytes: 0 };
			const channelStats = {
				channel: 'test',
				platform: 'twitch',
				count: 1,
				size_bytes: 10_000_000_000
			};
			expect(store.getChannelPercent(channelStats)).toBe(0);
		});
	});

	describe('edge cases', () => {
		it('handles disk with 0 total bytes', () => {
			store.stats = { ...mockStats, disk_total_bytes: 0 };
			expect(store.diskUsedPercent).toBe(0);
			expect(store.recordingsUsagePercent).toBe('0');
		});

		it('recordingsUsagePercent formats small percentages correctly', () => {
			store.stats = {
				...mockStats,
				total_size_bytes: 1_000_000_000, // 1 GB
				disk_total_bytes: 1_000_000_000_000 // 1 TB
			};
			// 1 GB / 1 TB = 0.1%
			expect(store.recordingsUsagePercent).toBe('0.10');
		});
	});
});
