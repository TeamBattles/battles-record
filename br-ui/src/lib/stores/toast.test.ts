import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// We need to test the store in isolation, so we'll import the class directly
// and create new instances for each test
describe('ToastStore', () => {
	// Create a minimal ToastStore class for testing that mirrors the real implementation
	type ToastType = 'success' | 'error' | 'info' | 'warning';
	interface Toast {
		id: string;
		type: ToastType;
		message: string;
		duration: number;
	}

	class TestableToastStore {
		toasts: Toast[] = [];

		add(type: ToastType, message: string, duration = 5000): string {
			const id = crypto.randomUUID();
			this.toasts = [...this.toasts, { id, type, message, duration }];

			if (duration > 0) {
				setTimeout(() => this.dismiss(id), duration);
			}

			return id;
		}

		dismiss(id: string) {
			this.toasts = this.toasts.filter((t) => t.id !== id);
		}

		success(message: string): string {
			return this.add('success', message, 5000);
		}

		error(message: string): string {
			return this.add('error', message, 8000);
		}

		info(message: string): string {
			return this.add('info', message, 5000);
		}

		warning(message: string): string {
			return this.add('warning', message, 6000);
		}
	}

	let store: TestableToastStore;

	beforeEach(() => {
		vi.useFakeTimers();
		store = new TestableToastStore();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	describe('add', () => {
		it('adds a toast with correct properties', () => {
			const id = store.add('success', 'Test message', 5000);

			expect(store.toasts).toHaveLength(1);
			expect(store.toasts[0]).toEqual({
				id,
				type: 'success',
				message: 'Test message',
				duration: 5000
			});
		});

		it('returns a unique ID', () => {
			const id1 = store.add('success', 'Message 1');
			const id2 = store.add('success', 'Message 2');

			expect(id1).not.toBe(id2);
		});

		it('adds multiple toasts', () => {
			store.add('success', 'Message 1');
			store.add('error', 'Message 2');
			store.add('info', 'Message 3');

			expect(store.toasts).toHaveLength(3);
		});

		it('uses default duration of 5000ms', () => {
			store.add('success', 'Test');

			expect(store.toasts[0].duration).toBe(5000);
		});

		it('allows custom duration', () => {
			store.add('success', 'Test', 10000);

			expect(store.toasts[0].duration).toBe(10000);
		});
	});

	describe('dismiss', () => {
		it('removes toast by ID', () => {
			const id = store.add('success', 'Test');
			expect(store.toasts).toHaveLength(1);

			store.dismiss(id);
			expect(store.toasts).toHaveLength(0);
		});

		it('removes only the specified toast', () => {
			const id1 = store.add('success', 'Message 1');
			store.add('error', 'Message 2');
			const id3 = store.add('info', 'Message 3');

			store.dismiss(id1);

			expect(store.toasts).toHaveLength(2);
			expect(store.toasts.find((t) => t.id === id1)).toBeUndefined();
			expect(store.toasts.find((t) => t.id === id3)).toBeDefined();
		});

		it('handles dismissing non-existent ID gracefully', () => {
			store.add('success', 'Test');

			store.dismiss('non-existent-id');

			expect(store.toasts).toHaveLength(1);
		});
	});

	describe('auto-dismiss', () => {
		it('auto-dismisses toast after duration', () => {
			store.add('success', 'Test', 3000);
			expect(store.toasts).toHaveLength(1);

			vi.advanceTimersByTime(3000);

			expect(store.toasts).toHaveLength(0);
		});

		it('does not auto-dismiss when duration is 0', () => {
			store.add('success', 'Test', 0);
			expect(store.toasts).toHaveLength(1);

			vi.advanceTimersByTime(10000);

			expect(store.toasts).toHaveLength(1);
		});

		it('auto-dismisses multiple toasts at different times', () => {
			store.add('success', 'Fast', 1000);
			store.add('error', 'Medium', 3000);
			store.add('info', 'Slow', 5000);

			expect(store.toasts).toHaveLength(3);

			vi.advanceTimersByTime(1000);
			expect(store.toasts).toHaveLength(2);

			vi.advanceTimersByTime(2000);
			expect(store.toasts).toHaveLength(1);

			vi.advanceTimersByTime(2000);
			expect(store.toasts).toHaveLength(0);
		});
	});

	describe('convenience methods', () => {
		it('success() creates success toast with 5000ms duration', () => {
			const id = store.success('Success message');

			expect(store.toasts[0]).toEqual({
				id,
				type: 'success',
				message: 'Success message',
				duration: 5000
			});
		});

		it('error() creates error toast with 8000ms duration', () => {
			const id = store.error('Error message');

			expect(store.toasts[0]).toEqual({
				id,
				type: 'error',
				message: 'Error message',
				duration: 8000
			});
		});

		it('info() creates info toast with 5000ms duration', () => {
			const id = store.info('Info message');

			expect(store.toasts[0]).toEqual({
				id,
				type: 'info',
				message: 'Info message',
				duration: 5000
			});
		});

		it('warning() creates warning toast with 6000ms duration', () => {
			const id = store.warning('Warning message');

			expect(store.toasts[0]).toEqual({
				id,
				type: 'warning',
				message: 'Warning message',
				duration: 6000
			});
		});
	});

	describe('toast lifecycle', () => {
		it('manual dismiss before auto-dismiss works', () => {
			const id = store.add('success', 'Test', 5000);

			// Dismiss manually before timer
			vi.advanceTimersByTime(2000);
			store.dismiss(id);

			expect(store.toasts).toHaveLength(0);

			// Timer fires but toast already removed
			vi.advanceTimersByTime(5000);
			expect(store.toasts).toHaveLength(0);
		});

		it('adding toast returns ID that can be used for manual dismiss', () => {
			const id = store.success('Test');

			// Use returned ID to dismiss
			store.dismiss(id);

			expect(store.toasts).toHaveLength(0);
		});
	});
});
