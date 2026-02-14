import '@testing-library/jest-dom/vitest';
import { vi, beforeEach, afterEach } from 'vitest';

// Mock crypto.randomUUID for toast tests
if (!globalThis.crypto) {
	globalThis.crypto = {
		randomUUID: () => `test-uuid-${Math.random().toString(36).substring(7)}`
	} as Crypto;
}

// Store cleanup tracking - tests should reset stores after use
const storeCleanupCallbacks: (() => void)[] = [];

/**
 * Register a cleanup callback for store reset
 * Use in tests that modify store state
 */
export function registerStoreCleanup(callback: () => void): void {
	storeCleanupCallbacks.push(callback);
}

/**
 * Run all registered store cleanup callbacks
 */
export function cleanupStores(): void {
	storeCleanupCallbacks.forEach((cb) => cb());
	storeCleanupCallbacks.length = 0;
}

// Mock WebSocket for WebSocket client tests
class MockWebSocket {
	static CONNECTING = 0;
	static OPEN = 1;
	static CLOSING = 2;
	static CLOSED = 3;

	readyState = MockWebSocket.CONNECTING;
	url: string;

	onopen: ((ev: Event) => void) | null = null;
	onclose: ((ev: CloseEvent) => void) | null = null;
	onmessage: ((ev: MessageEvent) => void) | null = null;
	onerror: ((ev: Event) => void) | null = null;

	constructor(url: string) {
		this.url = url;
		// Simulate connection after a tick
		setTimeout(() => {
			this.readyState = MockWebSocket.OPEN;
			this.onopen?.(new Event('open'));
		}, 0);
	}

	send(_data: string): void {
		// Mock send - override in tests as needed
	}

	close(): void {
		this.readyState = MockWebSocket.CLOSED;
		this.onclose?.(new CloseEvent('close'));
	}

	// Helper for tests to simulate receiving messages
	simulateMessage(data: unknown): void {
		this.onmessage?.(new MessageEvent('message', { data: JSON.stringify(data) }));
	}

	// Helper for tests to simulate errors
	simulateError(): void {
		this.onerror?.(new Event('error'));
	}
}

globalThis.WebSocket = MockWebSocket as unknown as typeof WebSocket;

// Mock fetch for API client tests
globalThis.fetch = vi.fn();

// Reset mocks between tests
beforeEach(() => {
	vi.clearAllMocks();
});

// Cleanup stores after tests
afterEach(() => {
	cleanupStores();
});
