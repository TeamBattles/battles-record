/**
 * ChannelQuotaModal Component Tests
 *
 * Tests for the modal that handles setting storage quota
 * and retention period for channels.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup, fireEvent, waitFor } from '@testing-library/svelte';
import ChannelQuotaModal from './ChannelQuotaModal.svelte';
import type { Channel } from '$lib/api/types';

// Mock the channels store
vi.mock('$lib/stores/channels.svelte', () => ({
	channelsStore: {
		updateChannel: vi.fn(),
		error: null
	}
}));

// Mock the toast store
vi.mock('$lib/stores/toast.svelte', () => ({
	toastStore: {
		success: vi.fn(),
		error: vi.fn()
	}
}));

import { channelsStore } from '$lib/stores/channels.svelte';
import { toastStore } from '$lib/stores/toast.svelte';

const mockChannel: Channel = {
	id: 'ch-1',
	name: 'test_streamer',
	platform: 'twitch',
	enabled: true,
	quality: 'best',
	status: { is_live: false, is_recording: false },
	quota_gb: undefined,
	retention_days: undefined
};

describe('ChannelQuotaModal', () => {
	const mockOnclose = vi.fn();

	beforeEach(() => {
		vi.clearAllMocks();
	});

	afterEach(() => {
		cleanup();
	});

	function renderModal(channel: Channel = mockChannel) {
		return render(ChannelQuotaModal, {
			props: {
				channel,
				onclose: mockOnclose
			}
		});
	}

	describe('modal display', () => {
		it('shows channel name and platform', () => {
			renderModal();

			expect(screen.getByText('test_streamer')).toBeInTheDocument();
			expect(screen.getByText('twitch')).toBeInTheDocument();
		});

		it('shows Save and Cancel buttons', () => {
			renderModal();

			expect(screen.getByRole('button', { name: /save/i })).toBeInTheDocument();
			expect(screen.getByRole('button', { name: /cancel/i })).toBeInTheDocument();
		});
	});

	describe('preset selection - storage quota', () => {
		it('clicking Unlimited sets undefined', async () => {
			// Start with a quota set
			const channelWithQuota = { ...mockChannel, quota_gb: 50 };
			renderModal(channelWithQuota);

			// 50 GB should be initially selected
			const fiftyButton = screen.getByRole('button', { name: '50 GB' });
			expect(fiftyButton).toHaveClass('border-emerald-500');

			// Click Unlimited
			const unlimitedButton = screen.getAllByRole('button', { name: 'Unlimited' })[0];
			await fireEvent.click(unlimitedButton);

			// Unlimited should now be selected
			expect(unlimitedButton).toHaveClass('border-emerald-500');
		});

		it('clicking number preset sets specific value', async () => {
			renderModal();

			const tenGbButton = screen.getByRole('button', { name: '10 GB' });
			await fireEvent.click(tenGbButton);

			// 10 GB should be selected
			expect(tenGbButton).toHaveClass('border-emerald-500');
		});

		it('active preset shows emerald styling', () => {
			const channelWithQuota = { ...mockChannel, quota_gb: 25 };
			renderModal(channelWithQuota);

			const twentyFiveButton = screen.getByRole('button', { name: '25 GB' });
			expect(twentyFiveButton).toHaveClass('border-emerald-500');
			expect(twentyFiveButton).toHaveClass('bg-emerald-500/10');
		});

		it('inactive preset shows default styling', () => {
			const channelWithQuota = { ...mockChannel, quota_gb: 25 };
			renderModal(channelWithQuota);

			const tenGbButton = screen.getByRole('button', { name: '10 GB' });
			expect(tenGbButton).not.toHaveClass('border-emerald-500');
			expect(tenGbButton).toHaveClass('border-border');
		});
	});

	describe('preset selection - retention period', () => {
		it('clicking Unlimited sets undefined for retention', async () => {
			const channelWithRetention = { ...mockChannel, retention_days: 30 };
			renderModal(channelWithRetention);

			// Click Unlimited for retention (second Unlimited button)
			const unlimitedButtons = screen.getAllByRole('button', { name: 'Unlimited' });
			await fireEvent.click(unlimitedButtons[1]); // Second one is for retention

			expect(unlimitedButtons[1]).toHaveClass('border-emerald-500');
		});

		it('clicking days preset sets specific value', async () => {
			renderModal();

			const sevenDaysButton = screen.getByRole('button', { name: '7 days' });
			await fireEvent.click(sevenDaysButton);

			expect(sevenDaysButton).toHaveClass('border-emerald-500');
		});
	});

	describe('save behavior', () => {
		it('calls updateChannel with correct values', async () => {
			(channelsStore.updateChannel as ReturnType<typeof vi.fn>).mockResolvedValue(true);

			renderModal();

			// Select 25 GB quota
			const twentyFiveButton = screen.getByRole('button', { name: '25 GB' });
			await fireEvent.click(twentyFiveButton);

			// Select 14 days retention
			const fourteenDaysButton = screen.getByRole('button', { name: '14 days' });
			await fireEvent.click(fourteenDaysButton);

			// Save
			const saveButton = screen.getByRole('button', { name: /save/i });
			await fireEvent.click(saveButton);

			expect(channelsStore.updateChannel).toHaveBeenCalledWith('ch-1', {
				quota_gb: 25,
				retention_days: 14
			});
		});

		it('sends null for unlimited quota', async () => {
			(channelsStore.updateChannel as ReturnType<typeof vi.fn>).mockResolvedValue(true);

			// Start with quota set
			const channelWithQuota = { ...mockChannel, quota_gb: 50 };
			renderModal(channelWithQuota);

			// Click Unlimited
			const unlimitedButton = screen.getAllByRole('button', { name: 'Unlimited' })[0];
			await fireEvent.click(unlimitedButton);

			// Save
			const saveButton = screen.getByRole('button', { name: /save/i });
			await fireEvent.click(saveButton);

			expect(channelsStore.updateChannel).toHaveBeenCalledWith('ch-1', {
				quota_gb: undefined,
				retention_days: undefined
			});
		});

		it('shows success toast on success', async () => {
			(channelsStore.updateChannel as ReturnType<typeof vi.fn>).mockResolvedValue(true);

			renderModal();

			const saveButton = screen.getByRole('button', { name: /save/i });
			await fireEvent.click(saveButton);

			await waitFor(() => {
				expect(toastStore.success).toHaveBeenCalledWith('Quota settings updated for test_streamer');
			});
		});

		it('shows error toast on failure', async () => {
			(channelsStore.updateChannel as ReturnType<typeof vi.fn>).mockResolvedValue(false);
			channelsStore.error = 'Update failed';

			renderModal();

			const saveButton = screen.getByRole('button', { name: /save/i });
			await fireEvent.click(saveButton);

			await waitFor(() => {
				expect(toastStore.error).toHaveBeenCalled();
			});
		});

		it('closes modal on success', async () => {
			(channelsStore.updateChannel as ReturnType<typeof vi.fn>).mockResolvedValue(true);

			renderModal();

			const saveButton = screen.getByRole('button', { name: /save/i });
			await fireEvent.click(saveButton);

			await waitFor(() => {
				expect(mockOnclose).toHaveBeenCalled();
			});
		});

		it('does not close modal on failure', async () => {
			(channelsStore.updateChannel as ReturnType<typeof vi.fn>).mockResolvedValue(false);

			renderModal();

			const saveButton = screen.getByRole('button', { name: /save/i });
			await fireEvent.click(saveButton);

			await waitFor(() => {
				expect(mockOnclose).not.toHaveBeenCalled();
			});
		});
	});

	describe('cancel behavior', () => {
		it('Cancel closes modal', async () => {
			renderModal();

			const cancelButton = screen.getByRole('button', { name: /cancel/i });
			await fireEvent.click(cancelButton);

			expect(mockOnclose).toHaveBeenCalled();
		});
	});
});
