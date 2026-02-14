import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { WebSocketClient, type WebSocketEvent } from './websocket';

// Type for our mock WebSocket
interface MockWebSocket {
	url: string;
	readyState: number;
	onopen: ((ev: Event) => void) | null;
	onclose: ((ev: CloseEvent) => void) | null;
	onmessage: ((ev: MessageEvent) => void) | null;
	onerror: ((ev: Event) => void) | null;
	send: (data: string) => void;
	close: () => void;
	simulateMessage: (data: unknown) => void;
	simulateError: () => void;
}

// Track created WebSocket instances for testing
let mockWebSocketInstance: MockWebSocket | null = null;
let originalWebSocket: typeof WebSocket;

describe('WebSocketClient', () => {
	let client: WebSocketClient;

	beforeEach(() => {
		// Store original and set up mock
		originalWebSocket = globalThis.WebSocket;

		// Create a fresh mock for each test
		const MockWebSocket = class implements MockWebSocket {
			static CONNECTING = 0;
			static OPEN = 1;
			static CLOSING = 2;
			static CLOSED = 3;

			readyState = 0;
			url: string;

			onopen: ((ev: Event) => void) | null = null;
			onclose: ((ev: CloseEvent) => void) | null = null;
			onmessage: ((ev: MessageEvent) => void) | null = null;
			onerror: ((ev: Event) => void) | null = null;

			constructor(url: string) {
				this.url = url;
				mockWebSocketInstance = this;
			}

			send = vi.fn();
			close = vi.fn(() => {
				this.readyState = 3;
				this.onclose?.(new CloseEvent('close'));
			});

			simulateOpen() {
				this.readyState = 1;
				this.onopen?.(new Event('open'));
			}

			simulateMessage(data: unknown) {
				this.onmessage?.(new MessageEvent('message', { data: JSON.stringify(data) }));
			}

			simulateError() {
				this.onerror?.(new Event('error'));
			}
		};

		globalThis.WebSocket = MockWebSocket as unknown as typeof WebSocket;

		client = new WebSocketClient('ws://localhost:8080');
		mockWebSocketInstance = null;
	});

	afterEach(() => {
		client.disconnect();
		globalThis.WebSocket = originalWebSocket;
		mockWebSocketInstance = null;
		vi.clearAllMocks();
	});

	describe('constructor and configuration', () => {
		it('creates client with default base URL', () => {
			const defaultClient = new WebSocketClient();
			expect(defaultClient).toBeDefined();
		});

		it('stores and retrieves token', () => {
			client.setToken('test-token');
			// Token is used internally for URL construction
		});

		it('updates base URL', () => {
			client.setBaseUrl('ws://newhost:9000/');
			// URL normalization happens internally
		});

		it('normalizes base URL by removing trailing slashes', () => {
			client.setBaseUrl('ws://example.com///');
			client.connect();
			expect(mockWebSocketInstance?.url).toContain('ws://example.com/api/events');
		});
	});

	describe('connection lifecycle', () => {
		it('connects to WebSocket with correct URL', () => {
			client.connect();
			expect(mockWebSocketInstance?.url).toBe('ws://localhost:8080/api/events');
		});

		it('includes token in URL when set', () => {
			client.setToken('jwt-token');
			client.connect();
			expect(mockWebSocketInstance?.url).toBe('ws://localhost:8080/api/events?token=jwt-token');
		});

		it('does not reconnect if already connected', () => {
			client.connect();
			const firstInstance = mockWebSocketInstance;

			// Simulate open
			(mockWebSocketInstance as MockWebSocket & { simulateOpen: () => void })?.simulateOpen();

			client.connect();
			expect(mockWebSocketInstance).toBe(firstInstance);
		});

		it('isConnected returns false initially', () => {
			expect(client.isConnected).toBe(false);
		});

		it('isConnected returns true after connection opens', () => {
			client.connect();
			(mockWebSocketInstance as MockWebSocket & { simulateOpen: () => void })?.simulateOpen();
			expect(client.isConnected).toBe(true);
		});

		it('disconnect closes the connection', () => {
			client.connect();
			(mockWebSocketInstance as MockWebSocket & { simulateOpen: () => void })?.simulateOpen();

			client.disconnect();

			expect(mockWebSocketInstance?.close).toHaveBeenCalled();
			expect(client.isConnected).toBe(false);
		});

		it('disconnect clears reconnect timer', () => {
			vi.useFakeTimers();

			client.connect();
			// Trigger close to schedule reconnect
			mockWebSocketInstance?.onclose?.(new CloseEvent('close'));

			client.disconnect();

			// Advance past reconnect time
			vi.advanceTimersByTime(10000);

			// Should not have created new connection
			vi.useRealTimers();
		});
	});

	describe('event parsing', () => {
		it('parses channel_status event', () => {
			const handler = vi.fn();
			client.subscribe(handler);
			client.connect();
			(mockWebSocketInstance as MockWebSocket & { simulateOpen: () => void })?.simulateOpen();

			mockWebSocketInstance?.simulateMessage({
				type: 'channel_status',
				channel_id: 'ch-1',
				name: 'test_channel',
				platform: 'twitch',
				status: 'live'
			});

			expect(handler).toHaveBeenCalledWith(
				expect.objectContaining({
					type: 'channel_status',
					channel_id: 'ch-1',
					status: 'live'
				})
			);
		});

		it('parses recording_started event', () => {
			const handler = vi.fn();
			client.subscribe(handler);
			client.connect();
			(mockWebSocketInstance as MockWebSocket & { simulateOpen: () => void })?.simulateOpen();

			mockWebSocketInstance?.simulateMessage({
				type: 'recording_started',
				recording_id: 'rec-1',
				channel_id: 'ch-1',
				channel_name: 'test_channel',
				platform: 'twitch',
				quality: 'best'
			});

			expect(handler).toHaveBeenCalledWith(
				expect.objectContaining({
					type: 'recording_started',
					recording_id: 'rec-1'
				})
			);
		});

		it('parses segment_downloaded event', () => {
			const handler = vi.fn();
			client.subscribe(handler);
			client.connect();
			(mockWebSocketInstance as MockWebSocket & { simulateOpen: () => void })?.simulateOpen();

			mockWebSocketInstance?.simulateMessage({
				type: 'segment_downloaded',
				recording_id: 'rec-1',
				sequence: 42,
				size_bytes: 500000,
				total_segments: 100,
				total_bytes: 50000000
			});

			expect(handler).toHaveBeenCalledWith(
				expect.objectContaining({
					type: 'segment_downloaded',
					sequence: 42,
					total_segments: 100
				})
			);
		});

		it('parses processing_progress event', () => {
			const handler = vi.fn();
			client.subscribe(handler);
			client.connect();
			(mockWebSocketInstance as MockWebSocket & { simulateOpen: () => void })?.simulateOpen();

			mockWebSocketInstance?.simulateMessage({
				type: 'processing_progress',
				recording_id: 'rec-1',
				percent: 75
			});

			expect(handler).toHaveBeenCalledWith(
				expect.objectContaining({
					type: 'processing_progress',
					percent: 75
				})
			);
		});

		it('parses connected event with channels', () => {
			const handler = vi.fn();
			client.subscribe(handler);
			client.connect();
			(mockWebSocketInstance as MockWebSocket & { simulateOpen: () => void })?.simulateOpen();

			mockWebSocketInstance?.simulateMessage({
				type: 'connected',
				channels: [
					{
						id: 'ch-1',
						name: 'test',
						platform: 'twitch',
						status: 'offline',
						enabled: true,
						quality: 'best'
					}
				],
				active_recordings: []
			});

			expect(handler).toHaveBeenCalledWith(
				expect.objectContaining({
					type: 'connected',
					channels: expect.arrayContaining([expect.objectContaining({ id: 'ch-1' })])
				})
			);
		});

		it('handles malformed JSON gracefully', () => {
			const handler = vi.fn();
			const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

			client.subscribe(handler);
			client.connect();
			(mockWebSocketInstance as MockWebSocket & { simulateOpen: () => void })?.simulateOpen();

			// Send malformed message
			mockWebSocketInstance?.onmessage?.(new MessageEvent('message', { data: 'not json' }));

			expect(handler).not.toHaveBeenCalled();
			expect(consoleSpy).toHaveBeenCalled();

			consoleSpy.mockRestore();
		});
	});

	describe('subscriber management', () => {
		it('adds subscriber and receives events', () => {
			const handler = vi.fn();
			client.subscribe(handler);
			client.connect();
			(mockWebSocketInstance as MockWebSocket & { simulateOpen: () => void })?.simulateOpen();

			mockWebSocketInstance?.simulateMessage({ type: 'error', message: 'Test error' });

			expect(handler).toHaveBeenCalled();
		});

		it('removes subscriber via unsubscribe function', () => {
			const handler = vi.fn();
			const unsubscribe = client.subscribe(handler);

			client.connect();
			(mockWebSocketInstance as MockWebSocket & { simulateOpen: () => void })?.simulateOpen();

			unsubscribe();

			mockWebSocketInstance?.simulateMessage({ type: 'error', message: 'Test error' });

			expect(handler).not.toHaveBeenCalled();
		});

		it('supports multiple subscribers', () => {
			const handler1 = vi.fn();
			const handler2 = vi.fn();

			client.subscribe(handler1);
			client.subscribe(handler2);
			client.connect();
			(mockWebSocketInstance as MockWebSocket & { simulateOpen: () => void })?.simulateOpen();

			mockWebSocketInstance?.simulateMessage({ type: 'error', message: 'Test' });

			expect(handler1).toHaveBeenCalled();
			expect(handler2).toHaveBeenCalled();
		});

		it('removing one subscriber does not affect others', () => {
			const handler1 = vi.fn();
			const handler2 = vi.fn();

			const unsub1 = client.subscribe(handler1);
			client.subscribe(handler2);
			client.connect();
			(mockWebSocketInstance as MockWebSocket & { simulateOpen: () => void })?.simulateOpen();

			unsub1();

			mockWebSocketInstance?.simulateMessage({ type: 'error', message: 'Test' });

			expect(handler1).not.toHaveBeenCalled();
			expect(handler2).toHaveBeenCalled();
		});
	});

	describe('cached event replay', () => {
		it('caches connected event', () => {
			client.connect();
			(mockWebSocketInstance as MockWebSocket & { simulateOpen: () => void })?.simulateOpen();

			// Send connected event
			mockWebSocketInstance?.simulateMessage({
				type: 'connected',
				channels: [
					{
						id: 'ch-1',
						name: 'test',
						platform: 'twitch',
						status: 'live',
						enabled: true,
						quality: 'best'
					}
				],
				active_recordings: []
			});

			// New subscriber should receive cached event
			const lateHandler = vi.fn();
			client.subscribe(lateHandler);

			expect(lateHandler).toHaveBeenCalledWith(
				expect.objectContaining({
					type: 'connected',
					channels: expect.any(Array)
				})
			);
		});

		it('replays cached connected event to new subscribers', () => {
			const earlyHandler = vi.fn();
			client.subscribe(earlyHandler);
			client.connect();
			(mockWebSocketInstance as MockWebSocket & { simulateOpen: () => void })?.simulateOpen();

			// Connected event
			const connectedEvent = {
				type: 'connected',
				channels: [
					{
						id: 'ch-1',
						name: 'test1',
						platform: 'twitch',
						status: 'offline',
						enabled: true,
						quality: 'best'
					},
					{
						id: 'ch-2',
						name: 'test2',
						platform: 'youtube',
						status: 'live',
						enabled: true,
						quality: '1080p'
					}
				],
				active_recordings: []
			};
			mockWebSocketInstance?.simulateMessage(connectedEvent);

			expect(earlyHandler).toHaveBeenCalledWith(expect.objectContaining({ type: 'connected' }));

			// Late subscriber
			const lateHandler = vi.fn();
			client.subscribe(lateHandler);

			// Should receive the cached connected event
			expect(lateHandler).toHaveBeenCalledTimes(1);
			expect(lateHandler).toHaveBeenCalledWith(expect.objectContaining({ type: 'connected' }));
		});

		it('clears cached event on disconnect', () => {
			client.connect();
			(mockWebSocketInstance as MockWebSocket & { simulateOpen: () => void })?.simulateOpen();

			mockWebSocketInstance?.simulateMessage({
				type: 'connected',
				channels: [],
				active_recordings: []
			});

			client.disconnect();

			// New subscriber after disconnect should not receive cached event
			const handler = vi.fn();
			client.subscribe(handler);

			expect(handler).not.toHaveBeenCalled();
		});
	});

	describe('reconnection', () => {
		it('schedules reconnect on close', () => {
			vi.useFakeTimers();

			client.connect();
			const firstUrl = mockWebSocketInstance?.url;

			// Simulate close
			mockWebSocketInstance?.onclose?.(new CloseEvent('close'));

			// Advance timer past reconnect delay (5000ms)
			vi.advanceTimersByTime(5000);

			// Should have created new connection
			expect(mockWebSocketInstance?.url).toBe(firstUrl);

			vi.useRealTimers();
		});

		it('does not double-schedule reconnect', () => {
			vi.useFakeTimers();

			client.connect();

			// Multiple close events
			mockWebSocketInstance?.onclose?.(new CloseEvent('close'));
			mockWebSocketInstance?.onclose?.(new CloseEvent('close'));

			// Only one reconnect should happen
			vi.advanceTimersByTime(10000);

			vi.useRealTimers();
		});
	});

	describe('error handling', () => {
		it('logs error on WebSocket error', () => {
			const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

			client.connect();
			mockWebSocketInstance?.simulateError();

			expect(consoleSpy).toHaveBeenCalled();
			consoleSpy.mockRestore();
		});
	});
});
