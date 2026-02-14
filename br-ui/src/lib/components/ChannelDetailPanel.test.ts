/**
 * ChannelDetailPanel Component Tests
 *
 * Tests for the channel detail panel that allows editing
 * channel settings across multiple tabs.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup, fireEvent, waitFor } from '@testing-library/svelte';
import ChannelDetailPanel from './ChannelDetailPanel.svelte';
import type { Channel } from '$lib/api/types';

// Mock the channels store
vi.mock('$lib/stores/channels.svelte', () => ({
	channelsStore: {
		stopRecording: vi.fn(),
		checkChannel: vi.fn()
	}
}));

import { channelsStore } from '$lib/stores/channels.svelte';

const mockChannel: Channel = {
	id: 'ch-1',
	name: 'test_streamer',
	platform: 'twitch',
	enabled: true,
	quality: 'best',
	status: { is_live: false, is_recording: false },
	schedule_enabled: false,
	timezone: 'UTC',
	schedule_rules: [],
	filters: {
		title_includes: [],
		title_excludes: [],
		game_includes: [],
		game_excludes: [],
		min_viewers: 0
	},
	quota_gb: undefined,
	retention_days: undefined
};

const mockLiveChannel: Channel = {
	...mockChannel,
	status: {
		is_live: true,
		is_recording: false,
		current_stream: {
			title: 'Test Stream Title',
			game: 'Just Chatting',
			viewer_count: 5000,
			started_at: new Date(Date.now() - 3600000).toISOString() // 1 hour ago
		}
	}
};

const mockRecordingChannel: Channel = {
	...mockChannel,
	status: {
		is_live: true,
		is_recording: true,
		current_stream: {
			title: 'Recording Stream',
			game: 'Gaming',
			viewer_count: 10000,
			started_at: new Date(Date.now() - 7200000).toISOString() // 2 hours ago
		}
	}
};

describe('ChannelDetailPanel', () => {
	const mockOnclose = vi.fn();
	const mockOnsave = vi.fn();

	beforeEach(() => {
		vi.clearAllMocks();
		mockOnsave.mockResolvedValue(undefined);
	});

	afterEach(() => {
		cleanup();
	});

	function renderPanel(channel: Channel = mockChannel) {
		return render(ChannelDetailPanel, {
			props: {
				channel,
				onclose: mockOnclose,
				onsave: mockOnsave
			}
		});
	}

	describe('header & channel info', () => {
		it('shows channel name', () => {
			renderPanel();

			expect(screen.getByText('test_streamer')).toBeInTheDocument();
		});

		it('shows Offline status text when not live', () => {
			renderPanel();

			// The status text in the header area
			const statusElements = screen.getAllByText('Offline');
			expect(statusElements.length).toBeGreaterThan(0);
		});

		it('shows Live status text when live', () => {
			renderPanel(mockLiveChannel);

			// Find the "Live" status text (not the button)
			const liveElements = screen.getAllByText('Live');
			expect(liveElements.length).toBeGreaterThan(0);
		});

		it('shows Recording status text when recording', () => {
			renderPanel(mockRecordingChannel);

			// The status label shows "Recording" (there's also Stop Recording button)
			const recordingElements = screen.getAllByText(/Recording/);
			expect(recordingElements.length).toBeGreaterThan(0);
		});

		it('shows current stream info when live', () => {
			renderPanel(mockLiveChannel);

			expect(screen.getByText('Test Stream Title')).toBeInTheDocument();
			expect(screen.getByText('Just Chatting')).toBeInTheDocument();
			expect(screen.getByText('5,000')).toBeInTheDocument();
		});
	});

	describe('quick actions', () => {
		it('shows Stop Recording button when recording', () => {
			renderPanel(mockRecordingChannel);

			expect(screen.getByRole('button', { name: /stop recording/i })).toBeInTheDocument();
		});

		it('shows Start Recording button when live but not recording', () => {
			renderPanel(mockLiveChannel);

			expect(screen.getByRole('button', { name: /start recording/i })).toBeInTheDocument();
		});

		it('does not show Start/Stop when offline', () => {
			renderPanel();

			expect(screen.queryByRole('button', { name: /stop recording/i })).not.toBeInTheDocument();
			expect(screen.queryByRole('button', { name: /start recording/i })).not.toBeInTheDocument();
		});

		it('Check Now button is always visible', () => {
			renderPanel();

			expect(screen.getByRole('button', { name: /check now/i })).toBeInTheDocument();
		});

		it('Stop Recording calls channelsStore.stopRecording', async () => {
			(channelsStore.stopRecording as ReturnType<typeof vi.fn>).mockResolvedValue(true);

			renderPanel(mockRecordingChannel);

			const stopButton = screen.getByRole('button', { name: /stop recording/i });
			await fireEvent.click(stopButton);

			expect(channelsStore.stopRecording).toHaveBeenCalledWith('ch-1');
		});

		it('Stop Recording closes panel on success', async () => {
			(channelsStore.stopRecording as ReturnType<typeof vi.fn>).mockResolvedValue(true);

			renderPanel(mockRecordingChannel);

			const stopButton = screen.getByRole('button', { name: /stop recording/i });
			await fireEvent.click(stopButton);

			await waitFor(() => {
				expect(mockOnclose).toHaveBeenCalled();
			});
		});

		it('Check Now calls channelsStore.checkChannel', async () => {
			(channelsStore.checkChannel as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);

			renderPanel();

			const checkButton = screen.getByRole('button', { name: /check now/i });
			await fireEvent.click(checkButton);

			expect(channelsStore.checkChannel).toHaveBeenCalledWith('ch-1');
		});
	});

	describe('tab system', () => {
		it('shows all four tabs', () => {
			renderPanel();

			expect(screen.getByRole('button', { name: 'General' })).toBeInTheDocument();
			expect(screen.getByRole('button', { name: 'Schedule' })).toBeInTheDocument();
			expect(screen.getByRole('button', { name: 'Filters' })).toBeInTheDocument();
			expect(screen.getByRole('button', { name: 'Storage' })).toBeInTheDocument();
		});

		it('General tab is active by default', () => {
			renderPanel();

			const generalTab = screen.getByRole('button', { name: 'General' });
			expect(generalTab).toHaveClass('text-zinc-100');
		});

		it('switching tabs updates content', async () => {
			renderPanel();

			// Initially shows General content
			expect(screen.getByText('Quality')).toBeInTheDocument();

			// Switch to Storage tab
			const storageTab = screen.getByRole('button', { name: 'Storage' });
			await fireEvent.click(storageTab);

			// Should show Storage content
			expect(screen.getByText('Quota (GB)')).toBeInTheDocument();
			expect(screen.getByText('Retention (days)')).toBeInTheDocument();
		});

		it('switching to Filters tab shows filter fields', async () => {
			renderPanel();

			const filtersTab = screen.getByRole('button', { name: 'Filters' });
			await fireEvent.click(filtersTab);

			expect(screen.getByText('Title Includes')).toBeInTheDocument();
			expect(screen.getByText('Title Excludes')).toBeInTheDocument();
			expect(screen.getByText('Games Include')).toBeInTheDocument();
			expect(screen.getByText('Games Exclude')).toBeInTheDocument();
			expect(screen.getByText('Minimum Viewers')).toBeInTheDocument();
		});

		it('switching to Schedule tab shows schedule fields', async () => {
			renderPanel();

			const scheduleTab = screen.getByRole('button', { name: 'Schedule' });
			await fireEvent.click(scheduleTab);

			expect(screen.getByText('Enable Schedule')).toBeInTheDocument();
		});
	});

	describe('general tab', () => {
		it('shows platform as read-only', () => {
			renderPanel();

			expect(screen.getByText('twitch')).toBeInTheDocument();
		});

		it('shows quality dropdown with current value', () => {
			renderPanel();

			const qualitySelect = screen.getByRole('combobox');
			expect(qualitySelect).toHaveValue('best');
		});

		it('shows enabled checkbox', () => {
			renderPanel();

			const enabledCheckbox = screen.getByRole('checkbox');
			expect(enabledCheckbox).toBeChecked();
		});
	});

	describe('schedule tab', () => {
		it('shows timezone dropdown when schedule enabled', async () => {
			const channelWithSchedule = { ...mockChannel, schedule_enabled: true };
			renderPanel(channelWithSchedule);

			const scheduleTab = screen.getByRole('button', { name: 'Schedule' });
			await fireEvent.click(scheduleTab);

			expect(screen.getByText('Timezone')).toBeInTheDocument();
		});

		it('hides timezone when schedule disabled', async () => {
			renderPanel();

			const scheduleTab = screen.getByRole('button', { name: 'Schedule' });
			await fireEvent.click(scheduleTab);

			expect(screen.queryByText('Timezone')).not.toBeInTheDocument();
		});

		it('enabling schedule shows schedule fields', async () => {
			renderPanel();

			const scheduleTab = screen.getByRole('button', { name: 'Schedule' });
			await fireEvent.click(scheduleTab);

			// Enable schedule
			const scheduleCheckbox = screen.getByRole('checkbox');
			await fireEvent.click(scheduleCheckbox);

			await waitFor(() => {
				expect(screen.getByText('Timezone')).toBeInTheDocument();
				expect(screen.getByText('Recording Windows')).toBeInTheDocument();
			});
		});
	});

	describe('filters tab', () => {
		it('displays existing filters as comma-separated', async () => {
			const channelWithFilters = {
				...mockChannel,
				filters: {
					title_includes: ['keyword1', 'keyword2'],
					title_excludes: ['exclude1'],
					game_includes: ['Game1', 'Game2'],
					game_excludes: [],
					min_viewers: 100
				}
			};
			renderPanel(channelWithFilters);

			const filtersTab = screen.getByRole('button', { name: 'Filters' });
			await fireEvent.click(filtersTab);

			// Find inputs by their placeholder
			const titleIncludesInput = screen.getByDisplayValue('keyword1, keyword2');
			expect(titleIncludesInput).toBeInTheDocument();

			const titleExcludesInput = screen.getByDisplayValue('exclude1');
			expect(titleExcludesInput).toBeInTheDocument();

			const gameIncludesInput = screen.getByDisplayValue('Game1, Game2');
			expect(gameIncludesInput).toBeInTheDocument();
		});

		it('parses comma-separated input to array on change', async () => {
			renderPanel();

			const filtersTab = screen.getByRole('button', { name: 'Filters' });
			await fireEvent.click(filtersTab);

			// Find title includes input
			const inputs = screen.getAllByPlaceholderText('comma-separated keywords');
			const titleIncludesInput = inputs[0];

			// Simulate change event with comma-separated values
			await fireEvent.change(titleIncludesInput, { target: { value: 'word1, word2, word3' } });

			// Save and check the data
			const saveButton = screen.getByRole('button', { name: /save/i });
			await fireEvent.click(saveButton);

			await waitFor(() => {
				expect(mockOnsave).toHaveBeenCalledWith(
					expect.objectContaining({
						filters: expect.objectContaining({
							title_includes: ['word1', 'word2', 'word3']
						})
					})
				);
			});
		});

		it('trims whitespace from filter values', async () => {
			renderPanel();

			const filtersTab = screen.getByRole('button', { name: 'Filters' });
			await fireEvent.click(filtersTab);

			const inputs = screen.getAllByPlaceholderText('comma-separated keywords');
			const titleIncludesInput = inputs[0];

			// Input with extra whitespace
			await fireEvent.change(titleIncludesInput, { target: { value: '  word1  ,  word2  ' } });

			const saveButton = screen.getByRole('button', { name: /save/i });
			await fireEvent.click(saveButton);

			await waitFor(() => {
				expect(mockOnsave).toHaveBeenCalledWith(
					expect.objectContaining({
						filters: expect.objectContaining({
							title_includes: ['word1', 'word2']
						})
					})
				);
			});
		});

		it('filters out empty strings', async () => {
			renderPanel();

			const filtersTab = screen.getByRole('button', { name: 'Filters' });
			await fireEvent.click(filtersTab);

			const inputs = screen.getAllByPlaceholderText('comma-separated keywords');
			const titleIncludesInput = inputs[0];

			// Input with empty entries
			await fireEvent.change(titleIncludesInput, { target: { value: 'word1,,word2,,' } });

			const saveButton = screen.getByRole('button', { name: /save/i });
			await fireEvent.click(saveButton);

			await waitFor(() => {
				expect(mockOnsave).toHaveBeenCalledWith(
					expect.objectContaining({
						filters: expect.objectContaining({
							title_includes: ['word1', 'word2']
						})
					})
				);
			});
		});
	});

	describe('storage tab', () => {
		it('shows quota and retention inputs', async () => {
			renderPanel();

			const storageTab = screen.getByRole('button', { name: 'Storage' });
			await fireEvent.click(storageTab);

			expect(screen.getByText('Quota (GB)')).toBeInTheDocument();
			expect(screen.getByText('Retention (days)')).toBeInTheDocument();
		});

		it('shows "0 = unlimited" hint for quota', async () => {
			renderPanel();

			const storageTab = screen.getByRole('button', { name: 'Storage' });
			await fireEvent.click(storageTab);

			expect(screen.getByText('0 = unlimited')).toBeInTheDocument();
		});

		it('shows "0 = keep forever" hint for retention', async () => {
			renderPanel();

			const storageTab = screen.getByRole('button', { name: 'Storage' });
			await fireEvent.click(storageTab);

			expect(screen.getByText('0 = keep forever')).toBeInTheDocument();
		});

		it('converts 0 quota to null on save', async () => {
			renderPanel();

			const storageTab = screen.getByRole('button', { name: 'Storage' });
			await fireEvent.click(storageTab);

			// Quota should be 0 (default)
			const saveButton = screen.getByRole('button', { name: /save/i });
			await fireEvent.click(saveButton);

			await waitFor(() => {
				expect(mockOnsave).toHaveBeenCalledWith(
					expect.objectContaining({
						quota_gb: undefined,
						retention_days: undefined
					})
				);
			});
		});

		it('sends quota value when non-zero', async () => {
			const channelWithQuota = { ...mockChannel, quota_gb: 50 };
			renderPanel(channelWithQuota);

			const storageTab = screen.getByRole('button', { name: 'Storage' });
			await fireEvent.click(storageTab);

			const saveButton = screen.getByRole('button', { name: /save/i });
			await fireEvent.click(saveButton);

			await waitFor(() => {
				expect(mockOnsave).toHaveBeenCalledWith(
					expect.objectContaining({
						quota_gb: 50
					})
				);
			});
		});
	});

	describe('save/cancel behavior', () => {
		it('Save button calls onsave with correct data', async () => {
			renderPanel();

			const saveButton = screen.getByRole('button', { name: /save/i });
			await fireEvent.click(saveButton);

			await waitFor(() => {
				expect(mockOnsave).toHaveBeenCalledWith({
					quality: 'best',
					enabled: true,
					schedule_enabled: false,
					timezone: 'UTC',
					schedule_rules: [],
					filters: {
						title_includes: [],
						title_excludes: [],
						game_includes: [],
						game_excludes: [],
						min_viewers: 0
					},
					quota_gb: undefined,
					retention_days: undefined
				});
			});
		});

		it('Cancel button calls onclose', async () => {
			renderPanel();

			const cancelButton = screen.getByRole('button', { name: /cancel/i });
			await fireEvent.click(cancelButton);

			expect(mockOnclose).toHaveBeenCalled();
		});

		it('shows error message on save failure', async () => {
			mockOnsave.mockRejectedValue(new Error('Save failed'));

			renderPanel();

			const saveButton = screen.getByRole('button', { name: /save/i });
			await fireEvent.click(saveButton);

			await waitFor(() => {
				expect(screen.getByText('Save failed')).toBeInTheDocument();
			});
		});

		it('Save button shows loading state during save', async () => {
			// Make onsave slow
			mockOnsave.mockImplementation(() => new Promise((resolve) => setTimeout(resolve, 100)));

			renderPanel();

			const saveButton = screen.getByRole('button', { name: /save/i });
			await fireEvent.click(saveButton);

			// Should show Saving... text
			expect(screen.getByText('Saving...')).toBeInTheDocument();
		});

		it('Save button is disabled during save', async () => {
			mockOnsave.mockImplementation(() => new Promise((resolve) => setTimeout(resolve, 100)));

			renderPanel();

			const saveButton = screen.getByRole('button', { name: /save/i });
			await fireEvent.click(saveButton);

			expect(saveButton).toBeDisabled();
		});
	});

	describe('close button', () => {
		it('X button calls onclose', async () => {
			renderPanel();

			// Find the X button in the header
			const closeButtons = screen.getAllByRole('button');
			// The first button with just an icon (no text) is the X button
			const xButton = closeButtons.find(
				(btn) => !btn.textContent?.trim() || btn.querySelector('svg')
			);

			if (xButton) {
				await fireEvent.click(xButton);
				expect(mockOnclose).toHaveBeenCalled();
			}
		});
	});
});
