/**
 * Connection Store Integration Tests
 *
 * Tests for the connection store which manages server connections,
 * authentication, and reconnection logic.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { connectionStore, type ConnectionState } from './connection.svelte';
import type { SavedServer } from './settings.svelte';

// Mock Tauri invoke
vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn()
}));

// Mock the API module
vi.mock('$lib/api', () => ({
	api: {
		getStatus: vi.fn(),
		checkHealth: vi.fn(),
		login: vi.fn(),
		setBaseUrl: vi.fn(),
		setToken: vi.fn(),
		getBaseUrl: vi.fn(() => 'http://localhost:8080'),
		getToken: vi.fn(() => null),
		getTokenExpiry: vi.fn(() => Date.now() + 86400000), // 24 hours from now
		refreshToken: vi.fn().mockResolvedValue(true),
		onAuthFailure: null as ((code: string) => void) | null,
		onTokenRefreshed: null as ((token: string, expiry: number) => void) | null
	},
	wsClient: {
		setBaseUrl: vi.fn(),
		setToken: vi.fn(),
		connect: vi.fn(),
		disconnect: vi.fn(),
		onAuthFailure: null as (() => void) | null
	},
	AuthenticationError: class AuthenticationError extends Error {
		constructor(
			message: string,
			public code: string,
			public status: number
		) {
			super(message);
			this.name = 'AuthenticationError';
		}
	}
}));

// Mock settings store
vi.mock('./settings.svelte', () => ({
	settingsStore: {
		getServer: vi.fn(),
		updateServer: vi.fn(),
		upsertLocalServer: vi.fn(),
		settings: {
			localDaemonDataDir: null,
			localDaemonLibraryDir: null,
			localDaemonDownloadsDir: null
		}
	}
}));

import { invoke } from '@tauri-apps/api/core';
import { api, wsClient } from '$lib/api';
import { settingsStore } from './settings.svelte';

// Test data
const mockLocalServer: SavedServer = {
	id: 'local',
	name: 'Local',
	type: 'local',
	url: 'http://localhost:8080'
};

const mockRemoteServer: SavedServer = {
	id: 'remote-1',
	name: 'Remote Server',
	type: 'remote',
	url: 'https://remote.example.com',
	token: 'valid-token',
	tokenExpiry: Date.now() + 86400000, // 24 hours from now
	username: 'testuser'
};

const mockExpiredTokenServer: SavedServer = {
	id: 'remote-expired',
	name: 'Expired Server',
	type: 'remote',
	url: 'https://expired.example.com',
	token: 'expired-token',
	tokenExpiry: Date.now() - 1000 // Expired
};

// Helper to reset store state
function resetStore() {
	connectionStore.activeServerId = null;
	connectionStore.connectionState = 'disconnected';
	connectionStore.error = null;
	connectionStore.username = null;
}

describe('ConnectionStore', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		vi.useFakeTimers();
		resetStore();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	describe('initial state', () => {
		it('starts disconnected with no active server', () => {
			expect(connectionStore.connectionState).toBe('disconnected');
			expect(connectionStore.activeServerId).toBeNull();
			expect(connectionStore.error).toBeNull();
		});

		it('isConnected returns false initially', () => {
			expect(connectionStore.isConnected).toBe(false);
		});

		it('isReconnecting returns false initially', () => {
			expect(connectionStore.isReconnecting).toBe(false);
		});
	});

	describe('connectToServer()', () => {
		it('sets connectionState to connecting during connection', async () => {
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockLocalServer);
			(api.checkHealth as ReturnType<typeof vi.fn>).mockResolvedValue({ status: 'ok' });

			const connectPromise = connectionStore.connectToServer('local');

			// State should transition to connecting immediately
			expect(connectionStore.connectionState).toBe('connecting');

			await connectPromise;
		});

		it('sets to connected on success for local server', async () => {
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockLocalServer);
			(api.checkHealth as ReturnType<typeof vi.fn>).mockResolvedValue({ status: 'ok' });

			const result = await connectionStore.connectToServer('local');

			expect(result).toBe(true);
			expect(connectionStore.connectionState).toBe('connected');
			expect(connectionStore.activeServerId).toBe('local');
			expect(connectionStore.username).toBe('admin'); // Local = admin
		});

		it('sets to connected on success for remote server', async () => {
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockRemoteServer);
			(api.getStatus as ReturnType<typeof vi.fn>).mockResolvedValue({ version: '1.0.0' });

			const result = await connectionStore.connectToServer('remote-1');

			expect(result).toBe(true);
			expect(connectionStore.connectionState).toBe('connected');
			expect(connectionStore.username).toBe('testuser');
			// setToken now takes 2 args: token and expiry
			expect(api.setToken).toHaveBeenCalledWith('valid-token', expect.any(Number));
		});

		it('restores previous state on failure', async () => {
			// First connect to local
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockLocalServer);
			(api.checkHealth as ReturnType<typeof vi.fn>).mockResolvedValue({ status: 'ok' });
			await connectionStore.connectToServer('local');

			expect(connectionStore.connectionState).toBe('connected');

			// Now try to connect to remote and fail
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockRemoteServer);
			(api.getStatus as ReturnType<typeof vi.fn>).mockRejectedValue(
				new Error('Connection refused')
			);

			const result = await connectionStore.connectToServer('remote-1');

			expect(result).toBe(false);
			// Should restore previous state (still connected to local, conceptually)
			expect(connectionStore.connectionState).toBe('connected');
			expect(connectionStore.error).toBe('Connection refused');
		});

		it('sets API and WebSocket URLs', async () => {
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockLocalServer);
			(api.checkHealth as ReturnType<typeof vi.fn>).mockResolvedValue({ status: 'ok' });

			await connectionStore.connectToServer('local');

			expect(api.setBaseUrl).toHaveBeenCalledWith('http://localhost:8080');
			expect(wsClient.setBaseUrl).toHaveBeenCalledWith('ws://localhost:8080');
			expect(wsClient.connect).toHaveBeenCalled();
		});

		it('handles token expiry for remote servers by attempting refresh', async () => {
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockExpiredTokenServer);
			// When token is expired, connectToServer tries to refresh first
			// If refresh fails, it shows the session expired modal
			(api.refreshToken as ReturnType<typeof vi.fn>).mockResolvedValue(false);

			const result = await connectionStore.connectToServer('remote-expired');

			expect(result).toBe(false);
			// New behavior: shows session expired modal instead of just setting error
			expect(connectionStore.showSessionExpiredModal).toBe(true);
			expect(connectionStore.authState).toBe('expired');
		});

		it('returns false when server not found', async () => {
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(undefined);

			const result = await connectionStore.connectToServer('nonexistent');

			expect(result).toBe(false);
			expect(connectionStore.error).toBe('Server not found');
		});
	});

	describe('authenticateRemote()', () => {
		it('authenticates and connects to remote server', async () => {
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockRemoteServer);
			(api.login as ReturnType<typeof vi.fn>).mockResolvedValue({
				token: 'new-token',
				role: 'admin',
				expires_at: new Date(Date.now() + 86400000).toISOString()
			});
			(api.getStatus as ReturnType<typeof vi.fn>).mockResolvedValue({ version: '1.0.0' });

			const result = await connectionStore.authenticateRemote('remote-1', 'user', 'pass');

			expect(result).toBe(true);
			expect(api.login).toHaveBeenCalledWith('user', 'pass');
			expect(settingsStore.updateServer).toHaveBeenCalledWith(
				'remote-1',
				expect.objectContaining({
					token: 'new-token',
					username: 'user'
				})
			);
		});

		it('handles authentication failure', async () => {
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockRemoteServer);
			(api.login as ReturnType<typeof vi.fn>).mockRejectedValue(new Error('Invalid credentials'));

			const result = await connectionStore.authenticateRemote('remote-1', 'user', 'wrongpass');

			expect(result).toBe(false);
			expect(connectionStore.error).toBe('Invalid credentials');
		});

		it('returns false for non-remote servers', async () => {
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockLocalServer);

			const result = await connectionStore.authenticateRemote('local', 'user', 'pass');

			expect(result).toBe(false);
			expect(connectionStore.error).toBe('Invalid server');
		});
	});

	describe('disconnect()', () => {
		it('clears all state', async () => {
			// First connect
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockLocalServer);
			(api.checkHealth as ReturnType<typeof vi.fn>).mockResolvedValue({ status: 'ok' });
			await connectionStore.connectToServer('local');

			connectionStore.disconnect();

			expect(connectionStore.connectionState).toBe('disconnected');
			expect(connectionStore.activeServerId).toBeNull();
			expect(connectionStore.error).toBeNull();
			expect(connectionStore.username).toBeNull();
			expect(wsClient.disconnect).toHaveBeenCalled();
		});
	});

	describe('reconnection logic', () => {
		beforeEach(async () => {
			// Start with a connected state
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockLocalServer);
			(api.checkHealth as ReturnType<typeof vi.fn>).mockResolvedValue({ status: 'ok' });
			await connectionStore.connectToServer('local');
			vi.clearAllMocks();
		});

		it('handleConnectionLost() triggers reconnect', () => {
			connectionStore.handleConnectionLost();

			expect(connectionStore.connectionState).toBe('reconnecting');
		});

		it('scheduleReconnect() uses exponential backoff', async () => {
			// Simulate connection loss
			connectionStore.handleConnectionLost();

			expect(connectionStore.connectionState).toBe('reconnecting');

			// First attempt should be after 1000ms
			(api.checkHealth as ReturnType<typeof vi.fn>).mockRejectedValue(new Error('Still down'));
			vi.advanceTimersByTime(1000);
			await vi.runAllTicks();

			// Still reconnecting after failure
			expect(connectionStore.connectionState).toBe('reconnecting');
		});

		it('attemptReconnect() checks health first', async () => {
			connectionStore.handleConnectionLost();

			vi.advanceTimersByTime(1000);
			await vi.runAllTicks();

			expect(api.checkHealth).toHaveBeenCalled();
		});

		it('stops after max reconnect time exceeded', async () => {
			connectionStore.handleConnectionLost();

			// Keep failing for more than MAX_RECONNECT_TIME (2 minutes)
			(api.checkHealth as ReturnType<typeof vi.fn>).mockRejectedValue(new Error('Down'));

			// Need to advance time in increments matching the backoff delays
			// and run ticks after each to process the reconnect attempts
			// Delays: 1000, 2000, 4000, 8000, 16000, 30000 (then capped)
			for (let i = 0; i < 10; i++) {
				vi.advanceTimersByTime(30000);
				await vi.runAllTicks();
			}

			// After enough time and attempts, should give up
			expect(connectionStore.connectionState).toBe('disconnected');
			expect(connectionStore.error).toContain('Connection lost');
		});

		it('retryNow() resets and attempts reconnect immediately', async () => {
			connectionStore.handleConnectionLost();

			// Wait a bit
			vi.advanceTimersByTime(500);

			// Clear mocks and make next attempt succeed
			vi.clearAllMocks();
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockLocalServer);
			(api.checkHealth as ReturnType<typeof vi.fn>).mockResolvedValue({ status: 'ok' });

			connectionStore.retryNow();
			await vi.runAllTicks();

			expect(api.checkHealth).toHaveBeenCalled();
		});

		it('does not double-schedule reconnect', () => {
			connectionStore.handleConnectionLost();

			// Try to trigger again
			connectionStore.handleConnectionLost();

			// Should only have one timer (hard to test directly, but no crash)
			expect(connectionStore.connectionState).toBe('reconnecting');
		});
	});

	describe('state getters', () => {
		it('isConnected returns true when connected', async () => {
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockLocalServer);
			(api.checkHealth as ReturnType<typeof vi.fn>).mockResolvedValue({ status: 'ok' });
			await connectionStore.connectToServer('local');

			expect(connectionStore.isConnected).toBe(true);
		});

		it('isReconnecting returns true during reconnect', async () => {
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockLocalServer);
			(api.checkHealth as ReturnType<typeof vi.fn>).mockResolvedValue({ status: 'ok' });
			await connectionStore.connectToServer('local');

			connectionStore.handleConnectionLost();

			expect(connectionStore.isReconnecting).toBe(true);
		});

		it('shouldShowReconnectBanner returns true after multiple failures', async () => {
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockLocalServer);
			(api.checkHealth as ReturnType<typeof vi.fn>).mockResolvedValue({ status: 'ok' });
			await connectionStore.connectToServer('local');

			// Start reconnecting
			connectionStore.handleConnectionLost();

			// Initially should not show banner
			expect(connectionStore.shouldShowReconnectBanner).toBe(false);

			// Simulate multiple failed attempts
			(api.checkHealth as ReturnType<typeof vi.fn>).mockRejectedValue(new Error('Down'));

			// Advance through several reconnect cycles (1s, 2s, 4s, 8s) - need 4 attempts for counter >= 3
			// Each cycle: wait for delay, then attempt runs, then schedules next
			for (let i = 0; i < 5; i++) {
				vi.advanceTimersByTime(10000); // Advance enough to trigger next attempt
				await vi.runAllTicks();
			}

			// After 3+ attempts, should show banner (reconnectAttempt >= 3)
			expect(connectionStore.isReconnecting).toBe(true);
			expect(connectionStore.shouldShowReconnectBanner).toBe(true);
		});
	});

	describe('local daemon management', () => {
		it('connectToLocal() starts daemon via Tauri', async () => {
			(invoke as ReturnType<typeof vi.fn>).mockResolvedValueOnce(null); // get_daemon_port returns null (not running)
			(invoke as ReturnType<typeof vi.fn>).mockResolvedValueOnce(8080); // start_local_daemon returns port
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockLocalServer);
			(api.checkHealth as ReturnType<typeof vi.fn>).mockResolvedValue({ status: 'ok' });

			const result = await connectionStore.connectToLocal();

			expect(invoke).toHaveBeenCalledWith('get_daemon_port');
			expect(invoke).toHaveBeenCalledWith('start_local_daemon', expect.any(Object));
			expect(result).toBe(true);
		});

		it('connectToLocal() uses existing daemon if running', async () => {
			(invoke as ReturnType<typeof vi.fn>).mockResolvedValueOnce(8080); // Already running
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockLocalServer);
			(api.checkHealth as ReturnType<typeof vi.fn>).mockResolvedValue({ status: 'ok' });

			await connectionStore.connectToLocal();

			expect(invoke).toHaveBeenCalledWith('get_daemon_port');
			expect(invoke).not.toHaveBeenCalledWith('start_local_daemon', expect.any(Object));
		});

		it('stopLocalDaemon() calls invoke', async () => {
			(invoke as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);

			await connectionStore.stopLocalDaemon();

			expect(invoke).toHaveBeenCalledWith('stop_local_daemon');
		});

		it('isDaemonRunning() returns correct state', async () => {
			(invoke as ReturnType<typeof vi.fn>).mockResolvedValue(true);

			const result = await connectionStore.isDaemonRunning();

			expect(result).toBe(true);
			expect(invoke).toHaveBeenCalledWith('is_daemon_running');
		});

		it('isDaemonRunning() returns false on error', async () => {
			(invoke as ReturnType<typeof vi.fn>).mockRejectedValue(new Error('Tauri not available'));

			const result = await connectionStore.isDaemonRunning();

			expect(result).toBe(false);
		});
	});

	describe('switchServer()', () => {
		it('switches from one server to another', async () => {
			// Connect to local first
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockLocalServer);
			(api.checkHealth as ReturnType<typeof vi.fn>).mockResolvedValue({ status: 'ok' });
			await connectionStore.connectToServer('local');

			expect(connectionStore.activeServerId).toBe('local');

			// Switch to remote
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockRemoteServer);
			(api.getStatus as ReturnType<typeof vi.fn>).mockResolvedValue({ version: '1.0.0' });

			const result = await connectionStore.switchServer('remote-1');

			expect(result).toBe(true);
			expect(connectionStore.activeServerId).toBe('remote-1');
		});
	});

	describe('activeServer getter', () => {
		it('returns null when no active server', () => {
			expect(connectionStore.activeServer).toBeNull();
		});

		it('returns active server from settings', async () => {
			(settingsStore.getServer as ReturnType<typeof vi.fn>).mockReturnValue(mockLocalServer);
			(api.checkHealth as ReturnType<typeof vi.fn>).mockResolvedValue({ status: 'ok' });
			await connectionStore.connectToServer('local');

			expect(connectionStore.activeServer).toEqual(mockLocalServer);
		});
	});
});
