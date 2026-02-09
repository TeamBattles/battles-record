/**
 * Storage Store Integration Tests
 *
 * These tests verify the storage store behavior through a wrapper component
 * to properly test Svelte 5 $state reactivity.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/svelte';
import { StorageStoreWrapper } from '../../tests/wrappers';
import { storageStore } from './storage.svelte';
import type { StorageStats } from '$lib/api/types';

// Mock the API
vi.mock('$lib/api/client', () => ({
	api: {
		getStorageStats: vi.fn(),
		cleanupStorage: vi.fn()
	}
}));

import { api } from '$lib/api/client';

// Test data
const mockStats: StorageStats = {
	total_recordings: 10,
	total_size_bytes: 50_000_000_000, // 50 GB in bytes (not actual GB)
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

describe('StorageStore Integration', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		// Reset store state before each test
		storageStore.stats = null;
		storageStore.isLoading = false;
		storageStore.error = null;
		storageStore.cleanupPreview = null;
		storageStore.isCleaningUp = false;
		storageStore.cleanupError = null;
		storageStore.sortBy = 'size';
		storageStore.sortOrder = 'desc';
	});

	afterEach(() => {
		cleanup();
	});

	describe('initial state', () => {
		it('renders with no stats initially', () => {
			render(StorageStoreWrapper);

			expect(screen.getByTestId('has-stats')).toHaveTextContent('false');
			expect(screen.getByTestId('total-size-gb')).toHaveTextContent('0');
			expect(screen.getByTestId('disk-used-percent')).toHaveTextContent('0');
			expect(screen.getByTestId('channel-stats-count')).toHaveTextContent('0');
		});

		it('shows loading state as false initially', () => {
			render(StorageStoreWrapper);

			expect(screen.getByTestId('is-loading')).toHaveTextContent('false');
		});

		it('shows no error initially', () => {
			render(StorageStoreWrapper);

			expect(screen.getByTestId('error')).toHaveTextContent('');
		});
	});

	describe('load()', () => {
		it('updates isLoading during load', async () => {
			// Setup a delayed response
			let resolveLoad: (value: StorageStats) => void;
			(api.getStorageStats as ReturnType<typeof vi.fn>).mockReturnValue(
				new Promise((resolve) => {
					resolveLoad = resolve;
				})
			);

			render(StorageStoreWrapper);

			// Start loading
			const loadPromise = storageStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('is-loading')).toHaveTextContent('true');
			});

			// Resolve the load
			resolveLoad!(mockStats);
			await loadPromise;

			await waitFor(() => {
				expect(screen.getByTestId('is-loading')).toHaveTextContent('false');
			});
		});

		it('populates stats after successful load', async () => {
			(api.getStorageStats as ReturnType<typeof vi.fn>).mockResolvedValue(mockStats);

			render(StorageStoreWrapper);
			await storageStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('has-stats')).toHaveTextContent('true');
				expect(screen.getByTestId('total-recordings')).toHaveTextContent('10');
			});
		});

		it('shows error on failed load', async () => {
			(api.getStorageStats as ReturnType<typeof vi.fn>).mockRejectedValue(
				new Error('Network error')
			);

			render(StorageStoreWrapper);
			await storageStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('error')).toHaveTextContent('Network error');
				expect(screen.getByTestId('has-stats')).toHaveTextContent('false');
			});
		});

		it('clears error on successful reload', async () => {
			// First fail
			(api.getStorageStats as ReturnType<typeof vi.fn>).mockRejectedValueOnce(new Error('Failed'));

			render(StorageStoreWrapper);
			await storageStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('error')).toHaveTextContent('Failed');
			});

			// Then succeed
			(api.getStorageStats as ReturnType<typeof vi.fn>).mockResolvedValueOnce(mockStats);
			await storageStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('error')).toHaveTextContent('');
				expect(screen.getByTestId('has-stats')).toHaveTextContent('true');
			});
		});
	});

	describe('computed properties', () => {
		beforeEach(async () => {
			(api.getStorageStats as ReturnType<typeof vi.fn>).mockResolvedValue(mockStats);
			render(StorageStoreWrapper);
			await storageStore.load();
		});

		it('computes totalSizeGB correctly', async () => {
			await waitFor(() => {
				// 50_000_000_000 / (1024^3) = 46.57 GB
				expect(screen.getByTestId('total-size-gb')).toHaveTextContent('46.57');
			});
		});

		it('computes diskUsedPercent correctly', async () => {
			await waitFor(() => {
				// Used = 500 - 200 = 300 GB, Percent = 300/500 * 100 = 60%
				expect(screen.getByTestId('disk-used-percent')).toHaveTextContent('60');
			});
		});

		it('computes diskFreeGB correctly', async () => {
			await waitFor(() => {
				// 200_000_000_000 / (1024^3) = 186.3 GB
				expect(screen.getByTestId('disk-free-gb')).toHaveTextContent('186.3');
			});
		});

		it('computes diskTotalGB correctly', async () => {
			await waitFor(() => {
				// 500_000_000_000 / (1024^3) = 465.7 GB
				expect(screen.getByTestId('disk-total-gb')).toHaveTextContent('465.7');
			});
		});

		it('detects separate library directory', async () => {
			await waitFor(() => {
				expect(screen.getByTestId('has-separate-library')).toHaveTextContent('true');
			});
		});

		it('computes librarySizeGB correctly', async () => {
			await waitFor(() => {
				// 45_000_000_000 / (1024^3) = 41.91 GB
				expect(screen.getByTestId('library-size-gb')).toHaveTextContent('41.91');
			});
		});
	});

	describe('channel stats sorting', () => {
		beforeEach(async () => {
			(api.getStorageStats as ReturnType<typeof vi.fn>).mockResolvedValue(mockStats);
			render(StorageStoreWrapper);
			await storageStore.load();
		});

		it('sorts by size descending by default', async () => {
			await waitFor(() => {
				expect(screen.getByTestId('sort-by')).toHaveTextContent('size');
				expect(screen.getByTestId('sort-order')).toHaveTextContent('desc');
				// streamer_a has 30GB, should be first
				expect(screen.getByTestId('channel-stat-0-name')).toHaveTextContent('streamer_a');
			});
		});

		it('toggles sort order when clicking same column', async () => {
			await waitFor(() => {
				expect(screen.getByTestId('sort-order')).toHaveTextContent('desc');
			});

			storageStore.setSort('size');

			await waitFor(() => {
				expect(screen.getByTestId('sort-order')).toHaveTextContent('asc');
				// streamer_c has 5GB, should be first when ascending
				expect(screen.getByTestId('channel-stat-0-name')).toHaveTextContent('streamer_c');
			});
		});

		it('changes column and resets to desc', async () => {
			storageStore.sortOrder = 'asc'; // Start with asc

			storageStore.setSort('channel');

			await waitFor(() => {
				expect(screen.getByTestId('sort-by')).toHaveTextContent('channel');
				expect(screen.getByTestId('sort-order')).toHaveTextContent('desc');
			});
		});

		it('sorts by channel name alphabetically', async () => {
			storageStore.setSort('channel');
			storageStore.setSort('channel'); // Toggle to ascending

			await waitFor(() => {
				expect(screen.getByTestId('channel-stat-0-name')).toHaveTextContent('streamer_a');
				expect(screen.getByTestId('channel-stat-1-name')).toHaveTextContent('streamer_b');
				expect(screen.getByTestId('channel-stat-2-name')).toHaveTextContent('streamer_c');
			});
		});

		it('sorts by count', async () => {
			storageStore.setSort('count');

			await waitFor(() => {
				// Descending by count: streamer_a(5), streamer_b(3), streamer_c(2)
				expect(screen.getByTestId('channel-stat-0-name')).toHaveTextContent('streamer_a');
			});
		});
	});

	describe('channel percentages', () => {
		beforeEach(async () => {
			(api.getStorageStats as ReturnType<typeof vi.fn>).mockResolvedValue(mockStats);
			render(StorageStoreWrapper);
			await storageStore.load();
		});

		it('calculates channel percent correctly', async () => {
			await waitFor(() => {
				// streamer_a: 30GB / 50GB = 60%
				expect(screen.getByTestId('channel-stat-0-percent')).toHaveTextContent('60');
			});
		});
	});

	describe('cleanup operations', () => {
		beforeEach(async () => {
			(api.getStorageStats as ReturnType<typeof vi.fn>).mockResolvedValue(mockStats);
			render(StorageStoreWrapper);
			await storageStore.load();
		});

		it('shows cleanup preview after preview request', async () => {
			const mockPreview = { recordings_affected: 5, bytes_to_free: 10_000_000_000, dry_run: true };
			(api.cleanupStorage as ReturnType<typeof vi.fn>).mockResolvedValue(mockPreview);

			await storageStore.previewCleanup({ older_than_days: 30 });

			await waitFor(() => {
				expect(screen.getByTestId('cleanup-preview-recordings')).toHaveTextContent('5');
				expect(screen.getByTestId('cleanup-preview-bytes')).toHaveTextContent('10000000000');
			});
		});

		it('shows cleanup error on failure', async () => {
			(api.cleanupStorage as ReturnType<typeof vi.fn>).mockRejectedValue(
				new Error('Cleanup failed')
			);

			await storageStore.previewCleanup({ older_than_days: 30 });

			await waitFor(() => {
				expect(screen.getByTestId('cleanup-error')).toHaveTextContent('Cleanup failed');
			});
		});

		it('clears preview after clearCleanupPreview', async () => {
			const mockPreview = { recordings_affected: 5, bytes_to_free: 10_000_000_000, dry_run: true };
			(api.cleanupStorage as ReturnType<typeof vi.fn>).mockResolvedValue(mockPreview);

			await storageStore.previewCleanup({ older_than_days: 30 });

			await waitFor(() => {
				expect(screen.getByTestId('cleanup-preview-recordings')).toBeInTheDocument();
			});

			storageStore.clearCleanupPreview();

			await waitFor(() => {
				expect(screen.queryByTestId('cleanup-preview-recordings')).not.toBeInTheDocument();
			});
		});
	});

	describe('edge cases', () => {
		it('handles disk with 0 total bytes', async () => {
			const zeroStats = { ...mockStats, disk_total_bytes: 0 };
			(api.getStorageStats as ReturnType<typeof vi.fn>).mockResolvedValue(zeroStats);

			render(StorageStoreWrapper);
			await storageStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('disk-used-percent')).toHaveTextContent('0');
				expect(screen.getByTestId('recordings-usage-percent')).toHaveTextContent('0');
			});
		});

		it('handles empty per_channel array', async () => {
			const emptyChannels = { ...mockStats, per_channel: [] };
			(api.getStorageStats as ReturnType<typeof vi.fn>).mockResolvedValue(emptyChannels);

			render(StorageStoreWrapper);
			await storageStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('channel-stats-count')).toHaveTextContent('0');
			});
		});

		it('handles same recordings_dir and library_dir', async () => {
			const sameDir = { ...mockStats, library_dir: '/recordings' };
			(api.getStorageStats as ReturnType<typeof vi.fn>).mockResolvedValue(sameDir);

			render(StorageStoreWrapper);
			await storageStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('has-separate-library')).toHaveTextContent('false');
			});
		});

		it('detects library on different disk', async () => {
			const diffDisk = { ...mockStats, library_disk_total_bytes: 1_000_000_000_000 };
			(api.getStorageStats as ReturnType<typeof vi.fn>).mockResolvedValue(diffDisk);

			render(StorageStoreWrapper);
			await storageStore.load();

			await waitFor(() => {
				expect(screen.getByTestId('library-on-different-disk')).toHaveTextContent('true');
			});
		});
	});
});

describe('StorageStore Unit Tests', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		storageStore.stats = null;
	});

	describe('formatBytes', () => {
		it('formats bytes', () => {
			expect(storageStore.formatBytes(500)).toBe('500 B');
		});

		it('formats kilobytes', () => {
			expect(storageStore.formatBytes(1536)).toBe('1.5 KB');
		});

		it('formats megabytes', () => {
			expect(storageStore.formatBytes(10_485_760)).toBe('10.0 MB');
		});

		it('formats gigabytes', () => {
			expect(storageStore.formatBytes(5_368_709_120)).toBe('5.00 GB');
		});

		it('handles zero', () => {
			expect(storageStore.formatBytes(0)).toBe('0 B');
		});
	});

	describe('getChannelPercent', () => {
		it('returns 0 when no stats', () => {
			const channelStats = {
				channel: 'test',
				platform: 'twitch',
				count: 1,
				size_bytes: 10_000_000_000
			};
			expect(storageStore.getChannelPercent(channelStats)).toBe(0);
		});

		it('returns 0 when total size is 0', () => {
			storageStore.stats = { ...mockStats, total_size_bytes: 0 };
			const channelStats = {
				channel: 'test',
				platform: 'twitch',
				count: 1,
				size_bytes: 10_000_000_000
			};
			expect(storageStore.getChannelPercent(channelStats)).toBe(0);
		});

		it('calculates percentage correctly', () => {
			storageStore.stats = mockStats;
			const channelStats = {
				channel: 'test',
				platform: 'twitch',
				count: 1,
				size_bytes: 10_000_000_000
			};
			// 10 GB / 50 GB = 20%
			expect(storageStore.getChannelPercent(channelStats)).toBe(20);
		});
	});
});
