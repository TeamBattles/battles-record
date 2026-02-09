/**
 * AddChannelModal Component Tests
 *
 * Tests for the modal that handles adding new channels with
 * URL extraction and validation logic.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup, fireEvent, waitFor } from '@testing-library/svelte';
import AddChannelModal from './AddChannelModal.svelte';

describe('AddChannelModal', () => {
	const mockOnclose = vi.fn();
	const mockOncreate = vi.fn();

	beforeEach(() => {
		vi.clearAllMocks();
	});

	afterEach(() => {
		cleanup();
	});

	function renderModal() {
		return render(AddChannelModal, {
			props: {
				onclose: mockOnclose,
				oncreate: mockOncreate
			}
		});
	}

	describe('initial state', () => {
		it('renders with Twitch selected by default', () => {
			renderModal();

			// Twitch button should have the selected style
			const twitchButton = screen.getByRole('button', { name: /twitch/i });
			expect(twitchButton).toHaveClass('bg-zinc-700');
		});

		it('shows correct placeholder for Twitch', () => {
			renderModal();

			const input = screen.getByPlaceholderText(/username or URL/i);
			expect(input).toBeInTheDocument();
		});

		it('has Create button disabled initially', () => {
			renderModal();

			const createButton = screen.getByRole('button', { name: /create channel/i });
			expect(createButton).toBeDisabled();
		});
	});

	describe('URL extraction', () => {
		it('extracts username from Twitch URL', async () => {
			renderModal();

			const input = screen.getByRole('textbox');
			await fireEvent.input(input, { target: { value: 'https://twitch.tv/shroud' } });

			// Should show "Will use: shroud"
			await waitFor(() => {
				expect(screen.getByText(/Will use: shroud/i)).toBeInTheDocument();
			});
		});

		it('extracts @handle from YouTube URL', async () => {
			renderModal();

			// Switch to YouTube
			const youtubeButton = screen.getByRole('button', { name: /youtube/i });
			await fireEvent.click(youtubeButton);

			const input = screen.getByRole('textbox');
			await fireEvent.input(input, { target: { value: 'https://youtube.com/@PewDiePie' } });

			await waitFor(() => {
				expect(screen.getByText(/Will use: @PewDiePie/i)).toBeInTheDocument();
			});
		});

		it('extracts channel ID from YouTube channel URL', async () => {
			renderModal();

			const youtubeButton = screen.getByRole('button', { name: /youtube/i });
			await fireEvent.click(youtubeButton);

			const input = screen.getByRole('textbox');
			await fireEvent.input(input, {
				target: { value: 'https://youtube.com/channel/UC-lHJZR3Gqxm24_Vd_AJ5Yw' }
			});

			await waitFor(() => {
				expect(screen.getByText(/Will use: UC-lHJZR3Gqxm24_Vd_AJ5Yw/i)).toBeInTheDocument();
			});
		});

		it('extracts username from Kick URL', async () => {
			renderModal();

			const kickButton = screen.getByRole('button', { name: /kick/i });
			await fireEvent.click(kickButton);

			const input = screen.getByRole('textbox');
			await fireEvent.input(input, { target: { value: 'https://kick.com/xqc' } });

			await waitFor(() => {
				expect(screen.getByText(/Will use: xqc/i)).toBeInTheDocument();
			});
		});

		it('does not show "Will use" when input matches extracted', async () => {
			renderModal();

			const input = screen.getByRole('textbox');
			await fireEvent.input(input, { target: { value: 'shroud' } });

			// Should not show "Will use" since input equals extracted
			expect(screen.queryByText(/Will use:/i)).not.toBeInTheDocument();
		});
	});

	describe('validation', () => {
		it('shows error for Twitch username under 4 chars', async () => {
			renderModal();

			const input = screen.getByRole('textbox');
			await fireEvent.input(input, { target: { value: 'abc' } });

			await waitFor(() => {
				expect(screen.getByText(/must be at least 4 characters/i)).toBeInTheDocument();
			});

			// Create button should be disabled
			const createButton = screen.getByRole('button', { name: /create channel/i });
			expect(createButton).toBeDisabled();
		});

		it('shows error for Twitch username over 25 chars', async () => {
			renderModal();

			const input = screen.getByRole('textbox');
			await fireEvent.input(input, { target: { value: 'a'.repeat(26) } });

			await waitFor(() => {
				expect(screen.getByText(/must be at most 25 characters/i)).toBeInTheDocument();
			});
		});

		it('shows error for Twitch username with special chars', async () => {
			renderModal();

			const input = screen.getByRole('textbox');
			await fireEvent.input(input, { target: { value: 'user@name!' } });

			await waitFor(() => {
				expect(
					screen.getByText(/can only contain letters, numbers, and underscores/i)
				).toBeInTheDocument();
			});
		});

		it('validates YouTube @handle format', async () => {
			renderModal();

			const youtubeButton = screen.getByRole('button', { name: /youtube/i });
			await fireEvent.click(youtubeButton);

			const input = screen.getByRole('textbox');
			await fireEvent.input(input, { target: { value: '@ab' } }); // Too short (2 chars after @)

			await waitFor(() => {
				expect(screen.getByText(/must be at least 3 characters/i)).toBeInTheDocument();
			});
		});

		it('shows validation error with amber border', async () => {
			renderModal();

			const input = screen.getByRole('textbox');
			await fireEvent.input(input, { target: { value: 'ab' } }); // Too short for Twitch

			await waitFor(() => {
				expect(input).toHaveClass('border-amber-500/50');
			});
		});
	});

	describe('form behavior', () => {
		it('enables Create button with valid input', async () => {
			renderModal();

			const input = screen.getByRole('textbox');
			await fireEvent.input(input, { target: { value: 'validuser' } });

			const createButton = screen.getByRole('button', { name: /create channel/i });
			await waitFor(() => {
				expect(createButton).not.toBeDisabled();
			});
		});

		it('updates placeholder when platform changes', async () => {
			renderModal();

			// Initially shows Twitch placeholder
			expect(screen.getByPlaceholderText(/username or URL/i)).toBeInTheDocument();

			// Switch to YouTube
			const youtubeButton = screen.getByRole('button', { name: /youtube/i });
			await fireEvent.click(youtubeButton);

			expect(screen.getByPlaceholderText(/channel URL or handle/i)).toBeInTheDocument();
		});

		it('shows info box when schedule is enabled', async () => {
			renderModal();

			const scheduleCheckbox = screen.getAllByRole('checkbox')[0]; // First checkbox is schedule
			await fireEvent.click(scheduleCheckbox);

			expect(screen.getByText(/Configure schedule after creating/i)).toBeInTheDocument();
		});

		it('shows info box when filters are enabled', async () => {
			renderModal();

			const filtersCheckbox = screen.getAllByRole('checkbox')[1]; // Second checkbox is filters
			await fireEvent.click(filtersCheckbox);

			expect(screen.getByText(/Configure filters after creating/i)).toBeInTheDocument();
		});

		it('calls oncreate with correct data on submit', async () => {
			renderModal();

			const input = screen.getByRole('textbox');
			await fireEvent.input(input, { target: { value: 'teststreamer' } });

			// Enable schedule and filters
			const checkboxes = screen.getAllByRole('checkbox');
			await fireEvent.click(checkboxes[0]); // schedule
			await fireEvent.click(checkboxes[1]); // filters

			const createButton = screen.getByRole('button', { name: /create channel/i });
			await fireEvent.click(createButton);

			expect(mockOncreate).toHaveBeenCalledWith({
				platform: 'twitch',
				name: 'teststreamer',
				quality: 'best',
				scheduleEnabled: true,
				filtersEnabled: true
			});
		});
	});

	describe('user interactions', () => {
		it('Cancel button calls onclose', async () => {
			renderModal();

			const cancelButton = screen.getByRole('button', { name: /cancel/i });
			await fireEvent.click(cancelButton);

			expect(mockOnclose).toHaveBeenCalled();
		});

		it('platform buttons are mutually exclusive', async () => {
			renderModal();

			// Initially Twitch is selected
			const twitchButton = screen.getByRole('button', { name: /twitch/i });
			const youtubeButton = screen.getByRole('button', { name: /youtube/i });
			const kickButton = screen.getByRole('button', { name: /kick/i });

			expect(twitchButton).toHaveClass('bg-zinc-700');
			expect(youtubeButton).not.toHaveClass('bg-zinc-700');

			// Click YouTube
			await fireEvent.click(youtubeButton);

			expect(twitchButton).not.toHaveClass('bg-zinc-700');
			expect(youtubeButton).toHaveClass('bg-zinc-700');

			// Click Kick
			await fireEvent.click(kickButton);

			expect(youtubeButton).not.toHaveClass('bg-zinc-700');
			expect(kickButton).toHaveClass('bg-zinc-700');
		});
	});

	describe('quality selection', () => {
		it('has "Best Available" selected by default', async () => {
			renderModal();

			const qualitySelect = screen.getByRole('combobox');
			expect(qualitySelect).toHaveValue('best');
		});

		it('allows changing quality', async () => {
			renderModal();

			const qualitySelect = screen.getByRole('combobox');
			await fireEvent.change(qualitySelect, { target: { value: '720p' } });

			expect(qualitySelect).toHaveValue('720p');
		});

		it('includes quality in submit data', async () => {
			renderModal();

			// Enter valid username first
			const input = screen.getByRole('textbox');
			await fireEvent.input(input, { target: { value: 'testuser' } });

			// Submit with default quality
			const createButton = screen.getByRole('button', { name: /create channel/i });
			await fireEvent.click(createButton);

			// Default quality should be 'best'
			expect(mockOncreate).toHaveBeenCalledWith(
				expect.objectContaining({
					quality: 'best'
				})
			);
		});
	});
});
