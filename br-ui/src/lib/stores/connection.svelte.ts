import { invoke } from '@tauri-apps/api/core';
import { api, wsClient, AuthenticationError, type AuthErrorCode } from '$lib/api';
import { settingsStore, type SavedServer } from './settings.svelte';

export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'reconnecting';
export type AuthState = 'valid' | 'refreshing' | 'expired' | 'invalid';

const RECONNECT_DELAYS = [1000, 2000, 4000, 8000, 16000, 30000];
const MAX_RECONNECT_TIME = 120000; // 2 minutes

class ConnectionStore {
	activeServerId = $state<string | null>(null);
	connectionState = $state<ConnectionState>('disconnected');
	error = $state<string | null>(null);
	username = $state<string | null>(null);

	/** Current authentication state */
	authState = $state<AuthState>('valid');
	/** Whether to show the session expired modal */
	showSessionExpiredModal = $state(false);

	private reconnectAttempt = 0;
	private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
	private reconnectStartTime: number | null = null;

	constructor() {
		// Set up API client callbacks
		api.onAuthFailure = (code) => this.handleAuthFailure(code);
		api.onTokenRefreshed = (token, expiry) => this.handleTokenRefreshed(token, expiry);
		wsClient.onAuthFailure = () => this.handleAuthFailure('TOKEN_EXPIRED');
	}

	/**
	 * Handle authentication failure from API client or WebSocket
	 */
	private handleAuthFailure(code: AuthErrorCode) {
		// Only handle auth failures for remote servers
		const server = this.activeServer;
		if (!server || server.type !== 'remote') return;

		if (code === 'TOKEN_EXPIRED' || code === 'TOKEN_INVALID') {
			this.authState = 'expired';
			this.showSessionExpiredModal = true;
		} else if (code === 'FORBIDDEN') {
			this.error = 'Access denied. You may not have permission for this action.';
		}
	}

	/**
	 * Handle successful token refresh from API client
	 */
	private handleTokenRefreshed(token: string, expiry: number) {
		if (!this.activeServerId) return;

		// Update saved server with new token
		settingsStore.updateServer(this.activeServerId, {
			token,
			tokenExpiry: expiry
		});

		// Update WebSocket with new token
		wsClient.setToken(token);

		// Reset auth state
		this.authState = 'valid';
	}

	get activeServer(): SavedServer | null {
		if (!this.activeServerId) return null;
		return settingsStore.getServer(this.activeServerId) ?? null;
	}

	get isConnected(): boolean {
		return this.connectionState === 'connected';
	}

	get isReconnecting(): boolean {
		return this.connectionState === 'reconnecting';
	}

	get shouldShowReconnectBanner(): boolean {
		return this.connectionState === 'reconnecting' && this.reconnectAttempt >= 3;
	}

	async connectToServer(serverId: string): Promise<boolean> {
		const server = settingsStore.getServer(serverId);
		if (!server) {
			this.error = 'Server not found';
			return false;
		}

		// Store previous state to restore on failure
		const previousState = this.connectionState;
		const previousActiveId = this.activeServerId;
		const previousBaseUrl = api.getBaseUrl();
		const previousToken = api.getToken();
		const previousExpiry = api.getTokenExpiry();

		this.stopReconnect();
		this.connectionState = 'connecting';
		this.error = null;
		this.authState = 'valid';

		try {
			api.setBaseUrl(server.url);

			if (server.type === 'remote') {
				// Check if token is missing
				if (!server.token) {
					this.error = 'Authentication required';
					this.connectionState = previousState;
					api.setBaseUrl(previousBaseUrl);
					api.setToken(previousToken, previousExpiry ?? undefined);
					return false;
				}

				// Set token with expiry for proactive refresh
				api.setToken(server.token, server.tokenExpiry);

				// If token is expired, try refreshing first
				if (server.tokenExpiry && Date.now() > server.tokenExpiry) {
					this.authState = 'refreshing';
					const refreshed = await api.refreshToken();
					if (!refreshed) {
						// Refresh failed - show modal for re-auth
						this.authState = 'expired';
						this.showSessionExpiredModal = true;
						this.connectionState = previousState;
						api.setBaseUrl(previousBaseUrl);
						api.setToken(previousToken, previousExpiry ?? undefined);
						return false;
					}
					this.authState = 'valid';
				}

				// Test connection with authenticated endpoint
				await api.getStatus();
			} else {
				api.setToken(null);
				// Test connection with health endpoint (no auth required)
				await api.checkHealth();
			}

			// Setup WebSocket
			const wsUrl = server.url.replace(/^http/, 'ws');
			wsClient.setBaseUrl(wsUrl);
			wsClient.setToken(server.type === 'remote' ? api.getToken() : null);
			console.log('[CONN] About to call wsClient.connect()', { time: Date.now() });
			wsClient.connect();
			console.log('[CONN] wsClient.connect() returned, setting connectionState=connected', {
				time: Date.now()
			});

			this.activeServerId = serverId;
			this.connectionState = 'connected';
			this.reconnectAttempt = 0;

			// Restore username from saved settings (for remote servers) or use 'admin' for local
			// Local connections bypass auth and have admin privileges
			this.username = server.type === 'remote' ? (server.username ?? null) : 'admin';

			return true;
		} catch (e) {
			// Handle auth errors by showing the session expired modal
			if (e instanceof AuthenticationError) {
				if (e.code === 'TOKEN_EXPIRED' || e.code === 'TOKEN_INVALID') {
					this.authState = 'expired';
					this.showSessionExpiredModal = true;
				}
			}

			this.error = e instanceof Error ? e.message : 'Connection failed';
			// Restore previous state so we don't disrupt existing connection
			this.connectionState = previousState;
			api.setBaseUrl(previousBaseUrl);
			api.setToken(previousToken, previousExpiry ?? undefined);
			// Reconnect WebSocket to previous server if we were connected
			if (previousActiveId && previousState === 'connected') {
				const prevServer = settingsStore.getServer(previousActiveId);
				if (prevServer) {
					const wsUrl = prevServer.url.replace(/^http/, 'ws');
					wsClient.setBaseUrl(wsUrl);
					wsClient.setToken(prevServer.type === 'remote' ? (prevServer.token ?? null) : null);
					wsClient.connect();
				}
			}
			return false;
		}
	}

	async authenticateRemote(serverId: string, username: string, password: string): Promise<boolean> {
		const server = settingsStore.getServer(serverId);
		if (!server || server.type !== 'remote') {
			this.error = 'Invalid server';
			return false;
		}

		// Don't change connection state yet - test first
		this.error = null;

		// Store current API state to restore if auth fails
		const previousBaseUrl = api.getBaseUrl();
		const previousToken = api.getToken();

		try {
			// Test authentication
			api.setBaseUrl(server.url);
			api.setToken(null); // Clear token for login
			const auth = await api.login(username, password);

			// Calculate expiry (parse ISO string, or default to 30 days)
			const expiryDate = auth.expires_at
				? new Date(auth.expires_at).getTime()
				: Date.now() + 30 * 24 * 60 * 60 * 1000;

			// Save token and username
			settingsStore.updateServer(serverId, {
				token: auth.token,
				tokenExpiry: expiryDate,
				username
			});

			// Store username
			this.username = username;

			// Now connect (this will set connecting state)
			return await this.connectToServer(serverId);
		} catch (e) {
			// Restore previous API state so current connection keeps working
			if (previousBaseUrl) {
				api.setBaseUrl(previousBaseUrl);
				api.setToken(previousToken);
			}
			this.error = e instanceof Error ? e.message : 'Authentication failed';
			// Don't change connection state - keep current connection intact
			return false;
		}
	}

	disconnect() {
		this.stopReconnect();
		wsClient.disconnect();
		this.activeServerId = null;
		this.connectionState = 'disconnected';
		this.error = null;
		this.username = null;
		this.authState = 'valid';
		this.showSessionExpiredModal = false;
	}

	/**
	 * Re-authenticate with username and password after session expiry.
	 * Used by SessionExpiredModal.
	 */
	async reauthenticate(username: string, password: string): Promise<boolean> {
		if (!this.activeServerId) {
			this.error = 'No active server';
			return false;
		}

		const server = settingsStore.getServer(this.activeServerId);
		if (!server || server.type !== 'remote') {
			this.error = 'Invalid server';
			return false;
		}

		this.error = null;
		this.authState = 'refreshing';

		try {
			// Login to get new token
			api.setBaseUrl(server.url);
			api.setToken(null); // Clear old token for login
			const auth = await api.login(username, password);

			// Calculate expiry
			const expiryDate = auth.expires_at
				? new Date(auth.expires_at).getTime()
				: Date.now() + 24 * 60 * 60 * 1000; // Default 24 hours

			// Save new token
			settingsStore.updateServer(this.activeServerId, {
				token: auth.token,
				tokenExpiry: expiryDate,
				username
			});

			// Update API client
			api.setToken(auth.token, expiryDate);

			// Reconnect WebSocket with new token
			const wsUrl = server.url.replace(/^http/, 'ws');
			wsClient.setBaseUrl(wsUrl);
			wsClient.setToken(auth.token);
			wsClient.connect();

			// Update state
			this.username = username;
			this.authState = 'valid';
			this.showSessionExpiredModal = false;
			this.connectionState = 'connected';

			return true;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Authentication failed';
			this.authState = 'expired';
			return false;
		}
	}

	/**
	 * Dismiss the session expired modal and disconnect
	 */
	dismissSessionExpiredModal() {
		this.showSessionExpiredModal = false;
		this.disconnect();
	}

	async switchServer(serverId: string): Promise<boolean> {
		// Don't fully disconnect - just switch which server we're talking to
		// This keeps the local daemon running in the background
		// connectToServer will restore state if the switch fails
		return await this.connectToServer(serverId);
	}

	// Called when connection is lost unexpectedly
	handleConnectionLost() {
		if (this.connectionState === 'disconnected') return;

		this.connectionState = 'reconnecting';
		this.reconnectStartTime = Date.now();
		this.scheduleReconnect();
	}

	private scheduleReconnect() {
		if (!this.activeServerId) return;

		const elapsed = this.reconnectStartTime ? Date.now() - this.reconnectStartTime : 0;
		if (elapsed > MAX_RECONNECT_TIME) {
			this.connectionState = 'disconnected';
			this.error = 'Connection lost. Please reconnect manually.';
			return;
		}

		const delay = RECONNECT_DELAYS[Math.min(this.reconnectAttempt, RECONNECT_DELAYS.length - 1)];
		this.reconnectTimer = setTimeout(() => this.attemptReconnect(), delay);
	}

	private async attemptReconnect() {
		if (!this.activeServerId) return;

		this.reconnectAttempt++;

		try {
			const server = settingsStore.getServer(this.activeServerId);
			if (!server) throw new Error('Server not found');

			api.setBaseUrl(server.url);
			if (server.type === 'remote' && server.token) {
				api.setToken(server.token);
				await api.getStatus();
			} else {
				await api.checkHealth();
			}

			// Success - reconnect WebSocket
			const wsUrl = server.url.replace(/^http/, 'ws');
			wsClient.setBaseUrl(wsUrl);
			wsClient.setToken(server.type === 'remote' ? (server.token ?? null) : null);
			wsClient.connect();

			this.connectionState = 'connected';
			this.reconnectAttempt = 0;
			this.reconnectStartTime = null;
		} catch (e) {
			this.scheduleReconnect();
		}
	}

	retryNow() {
		if (this.reconnectTimer) {
			clearTimeout(this.reconnectTimer);
			this.reconnectTimer = null;
		}
		this.attemptReconnect();
	}

	private stopReconnect() {
		if (this.reconnectTimer) {
			clearTimeout(this.reconnectTimer);
			this.reconnectTimer = null;
		}
		this.reconnectAttempt = 0;
		this.reconnectStartTime = null;
	}

	async connectToLocal(): Promise<boolean> {
		this.connectionState = 'connecting';
		this.error = null;

		try {
			// Check if daemon already running
			let port = await invoke<number | null>('get_daemon_port');

			// If not running, start it with saved directories
			if (!port) {
				port = await invoke<number>('start_local_daemon', {
					dataDir: settingsStore.settings.localDaemonDataDir,
					libraryDir: settingsStore.settings.localDaemonLibraryDir
				});
			}

			// Update/create local server entry with actual port
			const url = `http://localhost:${port}`;
			settingsStore.upsertLocalServer(url);

			// Connect normally
			return await this.connectToServer('local');
		} catch (e) {
			this.error = e instanceof Error ? e.message : String(e);
			this.connectionState = 'disconnected';
			return false;
		}
	}

	async stopLocalDaemon(): Promise<void> {
		try {
			await invoke('stop_local_daemon');
		} catch (e) {
			console.error('Failed to stop daemon:', e);
		}
	}

	async isDaemonRunning(): Promise<boolean> {
		try {
			return await invoke<boolean>('is_daemon_running');
		} catch {
			return false;
		}
	}
}

export const connectionStore = new ConnectionStore();
