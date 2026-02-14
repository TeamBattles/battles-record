/**
 * Toast Store Integration Tests
 *
 * These tests verify the toast store behavior through a wrapper component
 * to properly test Svelte 5 $state reactivity.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/svelte';
import { ToastStoreWrapper } from '../../tests/wrappers';
import { toastStore } from './toast.svelte';

describe('ToastStore Integration', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		// Clear all toasts before each test
		toastStore.toasts = [];
	});

	afterEach(() => {
		vi.useRealTimers();
		cleanup();
	});

	describe('add()', () => {
		it('adds a toast and updates count', async () => {
			render(ToastStoreWrapper);

			expect(screen.getByTestId('toast-count')).toHaveTextContent('0');

			toastStore.add('success', 'Test message', 5000);

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('1');
			});
		});

		it('adds toast with correct properties', async () => {
			render(ToastStoreWrapper);

			const id = toastStore.add('error', 'Error occurred', 8000);

			await waitFor(() => {
				expect(screen.getByTestId('toast-0-type')).toHaveTextContent('error');
				expect(screen.getByTestId('toast-0-message')).toHaveTextContent('Error occurred');
				expect(screen.getByTestId('toast-0-duration')).toHaveTextContent('8000');
				expect(screen.getByTestId('toast-0-id')).toHaveTextContent(id);
			});
		});

		it('adds multiple toasts', async () => {
			render(ToastStoreWrapper);

			toastStore.add('success', 'Message 1');
			toastStore.add('error', 'Message 2');
			toastStore.add('info', 'Message 3');

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('3');
			});
		});

		it('returns unique IDs', () => {
			const id1 = toastStore.add('success', 'Message 1');
			const id2 = toastStore.add('success', 'Message 2');
			const id3 = toastStore.add('success', 'Message 3');

			expect(id1).not.toBe(id2);
			expect(id2).not.toBe(id3);
			expect(id1).not.toBe(id3);
		});

		it('uses default duration of 5000ms', async () => {
			render(ToastStoreWrapper);

			toastStore.add('success', 'Test');

			await waitFor(() => {
				expect(screen.getByTestId('toast-0-duration')).toHaveTextContent('5000');
			});
		});
	});

	describe('dismiss()', () => {
		it('removes toast by ID', async () => {
			render(ToastStoreWrapper);

			const id = toastStore.add('success', 'Test');

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('1');
			});

			toastStore.dismiss(id);

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('0');
			});
		});

		it('removes only the specified toast', async () => {
			render(ToastStoreWrapper);

			const id1 = toastStore.add('success', 'Message 1');
			toastStore.add('error', 'Message 2');
			toastStore.add('info', 'Message 3');

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('3');
			});

			toastStore.dismiss(id1);

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('2');
				// Message 1 should be gone, Message 2 should be at index 0
				expect(screen.getByTestId('toast-0-message')).toHaveTextContent('Message 2');
			});
		});

		it('handles dismissing non-existent ID gracefully', async () => {
			render(ToastStoreWrapper);

			toastStore.add('success', 'Test');

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('1');
			});

			toastStore.dismiss('non-existent-id');

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('1');
			});
		});
	});

	describe('auto-dismiss', () => {
		it('auto-dismisses toast after duration', async () => {
			render(ToastStoreWrapper);

			toastStore.add('success', 'Test', 3000);

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('1');
			});

			vi.advanceTimersByTime(3000);

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('0');
			});
		});

		it('does not auto-dismiss when duration is 0', async () => {
			render(ToastStoreWrapper);

			toastStore.add('success', 'Permanent toast', 0);

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('1');
			});

			vi.advanceTimersByTime(10000);

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('1');
			});
		});

		it('auto-dismisses multiple toasts at different times', async () => {
			render(ToastStoreWrapper);

			toastStore.add('success', 'Fast', 1000);
			toastStore.add('error', 'Medium', 3000);
			toastStore.add('info', 'Slow', 5000);

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('3');
			});

			vi.advanceTimersByTime(1000);

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('2');
			});

			vi.advanceTimersByTime(2000);

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('1');
			});

			vi.advanceTimersByTime(2000);

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('0');
			});
		});
	});

	describe('convenience methods', () => {
		it('success() creates success toast with 5000ms duration', async () => {
			render(ToastStoreWrapper);

			toastStore.success('Success message');

			await waitFor(() => {
				expect(screen.getByTestId('toast-0-type')).toHaveTextContent('success');
				expect(screen.getByTestId('toast-0-message')).toHaveTextContent('Success message');
				expect(screen.getByTestId('toast-0-duration')).toHaveTextContent('5000');
			});
		});

		it('error() creates error toast with 8000ms duration', async () => {
			render(ToastStoreWrapper);

			toastStore.error('Error message');

			await waitFor(() => {
				expect(screen.getByTestId('toast-0-type')).toHaveTextContent('error');
				expect(screen.getByTestId('toast-0-message')).toHaveTextContent('Error message');
				expect(screen.getByTestId('toast-0-duration')).toHaveTextContent('8000');
			});
		});

		it('info() creates info toast with 5000ms duration', async () => {
			render(ToastStoreWrapper);

			toastStore.info('Info message');

			await waitFor(() => {
				expect(screen.getByTestId('toast-0-type')).toHaveTextContent('info');
				expect(screen.getByTestId('toast-0-message')).toHaveTextContent('Info message');
				expect(screen.getByTestId('toast-0-duration')).toHaveTextContent('5000');
			});
		});

		it('warning() creates warning toast with 6000ms duration', async () => {
			render(ToastStoreWrapper);

			toastStore.warning('Warning message');

			await waitFor(() => {
				expect(screen.getByTestId('toast-0-type')).toHaveTextContent('warning');
				expect(screen.getByTestId('toast-0-message')).toHaveTextContent('Warning message');
				expect(screen.getByTestId('toast-0-duration')).toHaveTextContent('6000');
			});
		});
	});

	describe('toast lifecycle', () => {
		it('manual dismiss before auto-dismiss works', async () => {
			render(ToastStoreWrapper);

			const id = toastStore.add('success', 'Test', 5000);

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('1');
			});

			// Advance partially
			vi.advanceTimersByTime(2000);

			// Dismiss manually
			toastStore.dismiss(id);

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('0');
			});

			// Advance past original timer
			vi.advanceTimersByTime(5000);

			// Should still be 0 (no errors from the timer firing on dismissed toast)
			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('0');
			});
		});

		it('can dismiss toast using returned ID', async () => {
			render(ToastStoreWrapper);

			const id = toastStore.success('Test');

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('1');
			});

			toastStore.dismiss(id);

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('0');
			});
		});
	});

	describe('reactivity', () => {
		it('updates UI when toast is added', async () => {
			render(ToastStoreWrapper);

			expect(screen.getByTestId('toast-count')).toHaveTextContent('0');

			toastStore.success('New toast');

			// UI should update reactively
			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('1');
				expect(screen.getByTestId('toast-0-message')).toHaveTextContent('New toast');
			});
		});

		it('updates UI when toast is removed', async () => {
			render(ToastStoreWrapper);

			const id = toastStore.success('Will be removed');

			await waitFor(() => {
				expect(screen.getByTestId('toast-0-message')).toHaveTextContent('Will be removed');
			});

			toastStore.dismiss(id);

			await waitFor(() => {
				expect(screen.queryByTestId('toast-0-message')).not.toBeInTheDocument();
			});
		});

		it('maintains correct order as toasts are added and removed', async () => {
			render(ToastStoreWrapper);

			const id1 = toastStore.add('success', 'First', 0);
			toastStore.add('error', 'Second', 0);
			toastStore.add('info', 'Third', 0);

			await waitFor(() => {
				expect(screen.getByTestId('toast-0-message')).toHaveTextContent('First');
				expect(screen.getByTestId('toast-1-message')).toHaveTextContent('Second');
				expect(screen.getByTestId('toast-2-message')).toHaveTextContent('Third');
			});

			// Remove first
			toastStore.dismiss(id1);

			await waitFor(() => {
				expect(screen.getByTestId('toast-count')).toHaveTextContent('2');
				expect(screen.getByTestId('toast-0-message')).toHaveTextContent('Second');
				expect(screen.getByTestId('toast-1-message')).toHaveTextContent('Third');
			});
		});
	});
});
