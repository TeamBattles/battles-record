/**
 * RecordingCard Component Tests
 *
 * Tests for the RecordingCard component using @testing-library/svelte.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/svelte';
import RecordingCard from './RecordingCard.svelte';
import type { Recording } from '$lib/api/types';

// Mock the recordings store
vi.mock('$lib/stores/recordings.svelte', () => ({
	recordingsStore: {
		getProcessingProgress: vi.fn()
	}
}));

import { recordingsStore } from '$lib/stores/recordings.svelte';

// Base recording fixture
const baseRecording: Recording = {
	id: 'test-recording-123',
	channel_name: 'test_streamer',
	platform: 'twitch',
	status: 'completed',
	started_at: '2024-01-15T10:30:00Z',
	ended_at: '2024-01-15T12:30:00Z',
	size_bytes: 1_500_000_000, // 1.5 GB
	duration_secs: 7200, // 2 hours
	path: '/recordings/test_streamer/2024-01-15',
	title: 'Playing Minecraft with friends!',
	game: 'Minecraft'
};

describe('RecordingCard', () => {
	const mockOnDelete = vi.fn();
	const mockOnProcess = vi.fn();
	const mockOnOpenFolder = vi.fn();

	beforeEach(() => {
		vi.clearAllMocks();
		(recordingsStore.getProcessingProgress as ReturnType<typeof vi.fn>).mockReturnValue(undefined);
	});

	afterEach(() => {
		cleanup();
	});

	describe('basic rendering', () => {
		it('renders channel name', () => {
			render(RecordingCard, {
				props: {
					recording: baseRecording,
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByText('test_streamer')).toBeInTheDocument();
		});

		it('renders recording title', () => {
			render(RecordingCard, {
				props: {
					recording: baseRecording,
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByText('Playing Minecraft with friends!')).toBeInTheDocument();
		});

		it('renders formatted duration', () => {
			render(RecordingCard, {
				props: {
					recording: baseRecording,
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			// 7200 seconds = 2h 0m
			expect(screen.getByText('2h 0m')).toBeInTheDocument();
		});

		it('renders formatted size', () => {
			render(RecordingCard, {
				props: {
					recording: baseRecording,
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			// 1.5 GB
			expect(screen.getByText('1.40 GB')).toBeInTheDocument();
		});

		it('renders date', () => {
			render(RecordingCard, {
				props: {
					recording: baseRecording,
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			// Should contain date formatted as "Jan 15"
			const dateElement = screen.getByText(/Jan 15/i);
			expect(dateElement).toBeInTheDocument();
		});

		it('shows dash when no duration', () => {
			const noDurationRecording = { ...baseRecording, duration_secs: undefined };
			render(RecordingCard, {
				props: {
					recording: noDurationRecording,
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByText('-')).toBeInTheDocument();
		});
	});

	describe('status display', () => {
		it('shows completed status', () => {
			render(RecordingCard, {
				props: {
					recording: { ...baseRecording, status: 'completed' },
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByText('completed')).toBeInTheDocument();
		});

		it('shows recording status', () => {
			render(RecordingCard, {
				props: {
					recording: { ...baseRecording, status: 'recording' },
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByText('recording')).toBeInTheDocument();
		});

		it('shows pending_processing as pending', () => {
			render(RecordingCard, {
				props: {
					recording: { ...baseRecording, status: 'pending_processing' },
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByText('pending')).toBeInTheDocument();
		});

		it('shows processing status', () => {
			render(RecordingCard, {
				props: {
					recording: { ...baseRecording, status: 'processing' },
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByText('processing')).toBeInTheDocument();
		});

		it('shows processed status', () => {
			render(RecordingCard, {
				props: {
					recording: { ...baseRecording, status: 'processed' },
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByText('processed')).toBeInTheDocument();
		});
	});

	describe('button actions', () => {
		it('calls onDelete when delete button clicked', async () => {
			render(RecordingCard, {
				props: {
					recording: baseRecording,
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			const deleteButton = screen.getByRole('button', { name: /delete/i });
			await fireEvent.click(deleteButton);

			expect(mockOnDelete).toHaveBeenCalledTimes(1);
		});

		it('calls onProcess when process button clicked', async () => {
			render(RecordingCard, {
				props: {
					recording: { ...baseRecording, status: 'completed' },
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			const processButton = screen.getByRole('button', { name: /process/i });
			await fireEvent.click(processButton);

			expect(mockOnProcess).toHaveBeenCalledTimes(1);
		});

		it('calls onOpenFolder when open button clicked', async () => {
			render(RecordingCard, {
				props: {
					recording: baseRecording,
					onDelete: mockOnDelete,
					onProcess: mockOnProcess,
					onOpenFolder: mockOnOpenFolder,
					showOpenFolder: true
				}
			});

			const openButton = screen.getByRole('button', { name: /open/i });
			await fireEvent.click(openButton);

			expect(mockOnOpenFolder).toHaveBeenCalledTimes(1);
		});

		it('does not show open folder button when showOpenFolder is false', () => {
			render(RecordingCard, {
				props: {
					recording: baseRecording,
					onDelete: mockOnDelete,
					onProcess: mockOnProcess,
					onOpenFolder: mockOnOpenFolder,
					showOpenFolder: false
				}
			});

			expect(screen.queryByRole('button', { name: /open/i })).not.toBeInTheDocument();
		});

		it('shows process button for completed recordings', () => {
			render(RecordingCard, {
				props: {
					recording: { ...baseRecording, status: 'completed' },
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByRole('button', { name: /process/i })).toBeInTheDocument();
		});

		it('shows process button for failed recordings', () => {
			render(RecordingCard, {
				props: {
					recording: { ...baseRecording, status: 'failed' },
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByRole('button', { name: /process/i })).toBeInTheDocument();
		});

		it('shows reprocess button for processed recordings', () => {
			render(RecordingCard, {
				props: {
					recording: { ...baseRecording, status: 'processed' },
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByRole('button', { name: /reprocess/i })).toBeInTheDocument();
		});

		it('shows retry button for processing_failed recordings', () => {
			render(RecordingCard, {
				props: {
					recording: {
						...baseRecording,
						status: 'processing_failed',
						processing_attempts: 1,
						failure_reason: 'FFmpeg error'
					},
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument();
		});

		it('hides retry button after 5 attempts', () => {
			render(RecordingCard, {
				props: {
					recording: {
						...baseRecording,
						status: 'processing_failed',
						processing_attempts: 5,
						failure_reason: 'FFmpeg error'
					},
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.queryByRole('button', { name: /retry/i })).not.toBeInTheDocument();
		});

		it('does not show process button for recording status', () => {
			render(RecordingCard, {
				props: {
					recording: { ...baseRecording, status: 'recording' },
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.queryByRole('button', { name: /process/i })).not.toBeInTheDocument();
		});

		it('does not show process button for processing status', () => {
			render(RecordingCard, {
				props: {
					recording: { ...baseRecording, status: 'processing' },
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.queryByRole('button', { name: /process/i })).not.toBeInTheDocument();
		});
	});

	describe('format helpers', () => {
		it('formats minutes-only duration', () => {
			const shortRecording = { ...baseRecording, duration_secs: 1800 }; // 30 min
			render(RecordingCard, {
				props: {
					recording: shortRecording,
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByText('30m 0s')).toBeInTheDocument();
		});

		it('formats hours and minutes', () => {
			const longRecording = { ...baseRecording, duration_secs: 5400 }; // 1h 30m
			render(RecordingCard, {
				props: {
					recording: longRecording,
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByText('1h 30m')).toBeInTheDocument();
		});

		it('formats KB size', () => {
			const smallRecording = { ...baseRecording, size_bytes: 512_000 }; // 500 KB
			render(RecordingCard, {
				props: {
					recording: smallRecording,
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByText('500.0 KB')).toBeInTheDocument();
		});

		it('formats MB size', () => {
			const mediumRecording = { ...baseRecording, size_bytes: 100_000_000 }; // ~95 MB
			render(RecordingCard, {
				props: {
					recording: mediumRecording,
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByText('95.4 MB')).toBeInTheDocument();
		});
	});

	describe('platform display', () => {
		it('renders for twitch platform', () => {
			render(RecordingCard, {
				props: {
					recording: { ...baseRecording, platform: 'twitch' },
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			// Platform icon is rendered (checking for channel name presence is enough)
			expect(screen.getByText('test_streamer')).toBeInTheDocument();
		});

		it('renders for youtube platform', () => {
			render(RecordingCard, {
				props: {
					recording: { ...baseRecording, platform: 'youtube' },
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByText('test_streamer')).toBeInTheDocument();
		});

		it('renders for kick platform', () => {
			render(RecordingCard, {
				props: {
					recording: { ...baseRecording, platform: 'kick' },
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			expect(screen.getByText('test_streamer')).toBeInTheDocument();
		});
	});

	describe('no title handling', () => {
		it('does not render title paragraph when title is undefined', () => {
			const noTitleRecording = { ...baseRecording, title: undefined };
			render(RecordingCard, {
				props: {
					recording: noTitleRecording,
					onDelete: mockOnDelete,
					onProcess: mockOnProcess
				}
			});

			// Title should not be in the document
			expect(screen.queryByText('Playing Minecraft with friends!')).not.toBeInTheDocument();
		});
	});
});
