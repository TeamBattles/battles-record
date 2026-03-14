import { api } from '$lib/api';
import type {
	Platform,
	PlatformAuth,
	PlatformAuthStatus,
	SetPlatformAuthRequest,
	TestConnectionResponse
} from '$lib/api/types';
import { toastStore } from './toast.svelte';

class PlatformAuthStore {
	platforms = $state<PlatformAuth[]>([]);
	isLoading = $state(false);
	error = $state<string | null>(null);
	testingPlatform = $state<Platform | null>(null);

	// OAuth state
	oauthPending = $state<Platform | null>(null);
	oauthState = $state<string | null>(null);
	oauthUrl = $state<string | null>(null);
	oauthBrowserFailed = $state(false);
	oauthAvailability = $state<{ twitch: boolean; youtube: boolean; kick: boolean }>({
		twitch: false,
		youtube: false,
		kick: false
	});

	get twitch(): PlatformAuth | null {
		return this.platforms.find((p) => p.platform === 'twitch') ?? null;
	}

	get youtube(): PlatformAuth | null {
		return this.platforms.find((p) => p.platform === 'youtube') ?? null;
	}

	get kick(): PlatformAuth | null {
		return this.platforms.find((p) => p.platform === 'kick') ?? null;
	}

	get connectedCount(): number {
		return this.platforms.filter((p) => p.status === 'connected').length;
	}

	// Track which server's data we have for stale-while-revalidate
	private _loadedServerId: string | null = null;

	async load(serverId?: string): Promise<void> {
		const isServerSwitch = serverId != null && serverId !== this._loadedServerId;
		const hasData = this.platforms.length > 0;

		if (!hasData || isServerSwitch) {
			this.isLoading = true;
			if (isServerSwitch) this.platforms = [];
		}
		this.error = null;
		try {
			const [platforms] = await Promise.all([
				api.getPlatformAuth(),
				this.loadOAuthAvailability()
			]);
			this.platforms = platforms;
			if (serverId) this._loadedServerId = serverId;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to load platform authentication';
		} finally {
			this.isLoading = false;
		}
	}

	async setCredentials(platform: Platform, credentials: SetPlatformAuthRequest): Promise<boolean> {
		try {
			const result = await api.setPlatformAuth(platform, credentials);

			// Update local state
			const idx = this.platforms.findIndex((p) => p.platform === platform);
			const updated: PlatformAuth = {
				platform: result.platform,
				status: result.status,
				username: result.username,
				expires_at: result.expires_at
			};

			if (idx !== -1) {
				this.platforms[idx] = updated;
			} else {
				this.platforms = [...this.platforms, updated];
			}

			toastStore.success(`${this.getPlatformName(platform)} connected successfully`);
			return true;
		} catch (e) {
			const message = e instanceof Error ? e.message : 'Failed to set credentials';
			toastStore.error(message);
			return false;
		}
	}

	async disconnect(platform: Platform): Promise<boolean> {
		try {
			await api.deletePlatformAuth(platform);

			// Update local state
			const idx = this.platforms.findIndex((p) => p.platform === platform);
			if (idx !== -1) {
				this.platforms[idx] = {
					...this.platforms[idx],
					status: 'not_connected',
					username: undefined,
					expires_at: undefined,
					last_validated: undefined
				};
			}

			toastStore.success(`${this.getPlatformName(platform)} disconnected`);
			return true;
		} catch (e) {
			const message = e instanceof Error ? e.message : 'Failed to disconnect';
			toastStore.error(message);
			return false;
		}
	}

	async testConnection(platform: Platform): Promise<TestConnectionResponse | null> {
		this.testingPlatform = platform;
		try {
			const result = await api.testPlatformAuth(platform);

			if (result.success) {
				toastStore.success(result.message);
				// Reload to get updated last_validated timestamp
				await this.load();
			} else {
				toastStore.error(result.message);
			}

			return result;
		} catch (e) {
			const message = e instanceof Error ? e.message : 'Connection test failed';
			toastStore.error(message);
			return null;
		} finally {
			this.testingPlatform = null;
		}
	}

	async loadOAuthAvailability(): Promise<void> {
		try {
			this.oauthAvailability = await api.getOAuthAvailability();
		} catch {
			// OAuth not available - that's fine, manual entry still works
			this.oauthAvailability = { twitch: false, youtube: false, kick: false };
		}
	}

	isOAuthAvailable(platform: Platform): boolean {
		return this.oauthAvailability[platform] ?? false;
	}

	/**
	 * Start OAuth flow for a platform
	 * @param platform - The platform to authenticate with
	 * @param redirectUri - The redirect URI to use (from OAuth plugin or web callback)
	 * @param options - Optional custom credentials for advanced users
	 * @returns The authorization URL to open in the browser, or null on error
	 */
	async startOAuth(
		platform: Platform,
		redirectUri: string,
		options?: { clientId?: string; clientSecret?: string }
	): Promise<string | null> {
		if (this.oauthPending) {
			toastStore.error('OAuth flow already in progress');
			return null;
		}

		try {
			const response = await api.startOAuth(platform, {
				redirectUri,
				clientId: options?.clientId,
				clientSecret: options?.clientSecret
			});

			this.oauthPending = platform;
			this.oauthState = response.state;
			this.oauthUrl = response.auth_url;

			return response.auth_url;
		} catch (e) {
			const message = e instanceof Error ? e.message : 'Failed to start OAuth';
			toastStore.error(message);
			return null;
		}
	}

	async completeOAuth(platform: Platform, code: string, state: string): Promise<boolean> {
		// Validate state
		if (state !== this.oauthState) {
			toastStore.error('Invalid OAuth state - possible CSRF attack');
			this.cancelOAuth();
			return false;
		}

		if (platform !== this.oauthPending) {
			toastStore.error('OAuth platform mismatch');
			this.cancelOAuth();
			return false;
		}

		try {
			const result = await api.completeOAuth(platform, code, state);

			// Update local state
			const idx = this.platforms.findIndex((p) => p.platform === platform);
			const updated: PlatformAuth = {
				platform: result.platform,
				status: result.status,
				username: result.username,
				expires_at: result.expires_at
			};

			if (idx !== -1) {
				this.platforms[idx] = updated;
			} else {
				this.platforms = [...this.platforms, updated];
			}

			toastStore.success(`${this.getPlatformName(platform)} connected successfully`);
			return true;
		} catch (e) {
			const message = e instanceof Error ? e.message : 'OAuth failed';
			toastStore.error(message);
			return false;
		} finally {
			this.cancelOAuth();
		}
	}

	cancelOAuth(): void {
		this.oauthPending = null;
		this.oauthState = null;
		this.oauthUrl = null;
		this.oauthBrowserFailed = false;
	}

	setBrowserFailed(failed: boolean): void {
		this.oauthBrowserFailed = failed;
	}

	private getPlatformName(platform: Platform): string {
		switch (platform) {
			case 'twitch':
				return 'Twitch';
			case 'youtube':
				return 'YouTube';
			case 'kick':
				return 'Kick';
		}
	}

	handleAuthUpdated(event: {
		platform: Platform;
		status: string;
		username?: string;
		expires_at?: string;
	}): void {
		const idx = this.platforms.findIndex((p) => p.platform === event.platform);
		const updated: PlatformAuth = {
			platform: event.platform,
			status: event.status as PlatformAuthStatus,
			username: event.username,
			expires_at: event.expires_at
		};

		if (idx !== -1) {
			this.platforms[idx] = updated;
		} else {
			this.platforms = [...this.platforms, updated];
		}
	}

	handleAuthExpired(event: { platform: Platform; reason: string }): void {
		const idx = this.platforms.findIndex((p) => p.platform === event.platform);
		if (idx !== -1) {
			this.platforms[idx] = {
				...this.platforms[idx],
				status: 'expired'
			};
		}

		toastStore.warning(`${this.getPlatformName(event.platform)} authentication expired: ${event.reason}`);
	}

	getExpiryInfo(auth: PlatformAuth | null): {
		text: string;
		isExpired: boolean;
		isExpiringSoon: boolean;
	} {
		if (!auth || auth.status === 'not_connected') {
			return { text: '', isExpired: false, isExpiringSoon: false };
		}

		if (auth.status === 'expired') {
			return { text: 'Token expired', isExpired: true, isExpiringSoon: false };
		}

		if (!auth.expires_at) {
			return { text: 'No expiry (permanent token)', isExpired: false, isExpiringSoon: false };
		}

		const expiry = new Date(auth.expires_at);
		const now = new Date();
		const diffMs = expiry.getTime() - now.getTime();
		const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

		if (diffMs < 0) {
			return { text: 'Token expired', isExpired: true, isExpiringSoon: false };
		}

		if (diffDays > 30) {
			return { text: `Expires in ${diffDays} days`, isExpired: false, isExpiringSoon: false };
		}

		if (diffDays > 1) {
			return {
				text: `Expires in ${diffDays} days`,
				isExpired: false,
				isExpiringSoon: diffDays < 7
			};
		}

		const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
		if (diffHours > 1) {
			return { text: `Expires in ${diffHours} hours`, isExpired: false, isExpiringSoon: true };
		}

		const diffMinutes = Math.floor(diffMs / (1000 * 60));
		return { text: `Expires in ${diffMinutes} minutes`, isExpired: false, isExpiringSoon: true };
	}
}

export const platformAuthStore = new PlatformAuthStore();
