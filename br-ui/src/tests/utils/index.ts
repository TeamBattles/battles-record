/**
 * Test Utilities
 *
 * Common utilities for frontend testing.
 */

export * from './websocket-harness';

// Re-export testing library utilities for convenience
export { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/svelte';

/**
 * Wait for a specified number of milliseconds
 * Useful for waiting for async operations in tests
 */
export function delay(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Wait for the next tick (microtask)
 */
export function nextTick(): Promise<void> {
	return new Promise((resolve) => queueMicrotask(resolve));
}

/**
 * Create a mock API response
 */
export function mockApiResponse<T>(data: T): Promise<T> {
	return Promise.resolve(data);
}

/**
 * Create a mock API error
 */
export function mockApiError(message: string): Promise<never> {
	return Promise.reject(new Error(message));
}

/**
 * Mock fetch to return a specific response
 */
export function mockFetchResponse(response: unknown, status = 200): void {
	(globalThis.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
		ok: status >= 200 && status < 300,
		status,
		json: () => Promise.resolve(response)
	} as Response);
}

/**
 * Mock fetch to return an error
 */
export function mockFetchError(message: string): void {
	(globalThis.fetch as ReturnType<typeof vi.fn>).mockRejectedValueOnce(new Error(message));
}

// Import vi for mockFetchResponse/Error
import { vi } from 'vitest';
