import type {
	ApiResponse,
	Channel,
	Recording,
	DaemonStatus,
	AuthTokens,
	Platform,
	PlatformAuth,
	PlatformAuthListResponse,
	SetPlatformAuthRequest,
	SetPlatformAuthResponse,
	SetYouTubeCookiesResponse,
	TestConnectionResponse,
	DeletePlatformAuthResponse,
	User,
	Session,
	CreateUserRequest,
	UpdateUserRequest,
	StorageStats,
	CleanupRequest,
	CleanupResponse,
	PostProcessingConfig,
	ChannelProfile,
	ImageUploadResponse,
	ImageDeleteResponse,
	AuthErrorCode,
	RefreshTokenResponse,
	StartOAuthResponse,
	OAuthCallbackResponse,
	OAuthAvailabilityResponse,
	DependenciesResponse,
	Download,
	DownloadSummary,
	CreateDownloadRequest,
	ExtensionConnection,
	ExtensionConfig,
	MessageLogEntry,
	LibraryStatus,
	DownloadsConfig,
	DownloadStorageStats,
	DownloadCleanupRequest,
	DownloadCleanupResponse
} from './types';
import { type BackendChannel, transformChannel } from './backend-types';

// Tauri HTTP plugin fetch - routes requests through Rust to bypass
// CORS and mixed-content browser restrictions for remote servers.
// Lazy-initialized for SSR compatibility.
let _httpFetch: typeof globalThis.fetch | null = null;

async function httpFetch(
	input: RequestInfo | URL,
	init?: RequestInit & { connectTimeout?: number }
): Promise<Response> {
	if (!_httpFetch) {
		try {
			const mod = await import('@tauri-apps/plugin-http');
			_httpFetch = mod.fetch;
		} catch {
			_httpFetch = globalThis.fetch.bind(globalThis);
		}
	}

	const { connectTimeout, ...fetchInit } = init ?? {};
	const timeout = connectTimeout ?? 15_000;

	const controller = new AbortController();
	const timer = setTimeout(() => controller.abort(), timeout);

	// Chain signals if caller provided one
	if (fetchInit.signal) {
		fetchInit.signal.addEventListener('abort', () => controller.abort());
	}

	try {
		const response = await _httpFetch(input, {
			...fetchInit,
			signal: controller.signal
		});
		return response;
	} catch (e) {
		if (controller.signal.aborted && !(fetchInit.signal?.aborted)) {
			throw new Error(`Connection timed out after ${timeout / 1000}s`);
		}
		throw e;
	} finally {
		clearTimeout(timer);
	}
}

/**
 * Custom error class for authentication failures.
 * Contains the error code from the backend for handling specific cases.
 */
export class AuthenticationError extends Error {
	constructor(
		message: string,
		public code: AuthErrorCode,
		public status: number
	) {
		super(message);
		this.name = 'AuthenticationError';
	}
}

export class ApiClient {
	private baseUrl: string;
	private token: string | null = null;
	private tokenExpiry: number | null = null;
	private refreshPromise: Promise<boolean> | null = null;

	// Callbacks for auth events - set by connection store
	onAuthFailure: ((code: AuthErrorCode) => void) | null = null;
	onTokenRefreshed: ((token: string, expiry: number) => void) | null = null;

	constructor(baseUrl: string = 'http://localhost:8080') {
		this.baseUrl = baseUrl.replace(/\/+$/, '');
	}

	setToken(token: string | null, expiry?: number) {
		this.token = token;
		this.tokenExpiry = expiry ?? null;
	}

	getToken(): string | null {
		return this.token;
	}

	getTokenExpiry(): number | null {
		return this.tokenExpiry;
	}

	setBaseUrl(url: string) {
		this.baseUrl = url.replace(/\/+$/, '');
	}

	getBaseUrl(): string {
		return this.baseUrl;
	}

	/**
	 * Check if the token should be proactively refreshed (expires in <5 minutes)
	 */
	private shouldProactivelyRefresh(): boolean {
		if (!this.token || !this.tokenExpiry) return false;
		const fiveMinutes = 5 * 60 * 1000;
		return Date.now() > this.tokenExpiry - fiveMinutes;
	}

	/**
	 * Extract auth error code from response body
	 */
	private extractAuthErrorCode(errorBody: unknown): AuthErrorCode | null {
		if (typeof errorBody === 'object' && errorBody !== null) {
			const body = errorBody as Record<string, unknown>;
			if (typeof body.code === 'string') {
				return body.code as AuthErrorCode;
			}
		}
		return null;
	}

	/**
	 * Attempt to refresh the token
	 * Returns true if successful, false otherwise
	 */
	async refreshToken(): Promise<boolean> {
		// Deduplicate concurrent refresh attempts
		if (this.refreshPromise) {
			return this.refreshPromise;
		}

		this.refreshPromise = this.doRefreshToken();
		try {
			return await this.refreshPromise;
		} finally {
			this.refreshPromise = null;
		}
	}

	private async doRefreshToken(): Promise<boolean> {
		if (!this.token) return false;

		try {
			const headers: Record<string, string> = {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${this.token}`
			};

			const response = await httpFetch(`${this.baseUrl}/api/auth/refresh`, {
				method: 'POST',
				headers
			});

			if (!response.ok) {
				const errorBody = await response.json().catch(() => null);
				const code = this.extractAuthErrorCode(errorBody);

				// If token is expired beyond grace period, notify failure
				if (code === 'TOKEN_EXPIRED' || code === 'TOKEN_INVALID') {
					this.onAuthFailure?.(code);
				}
				return false;
			}

			const result: ApiResponse<RefreshTokenResponse> = await response.json();
			const newToken = result.data.token;
			const newExpiry = new Date(result.data.expires_at).getTime();

			// Update internal state
			this.token = newToken;
			this.tokenExpiry = newExpiry;

			// Notify connection store
			this.onTokenRefreshed?.(newToken, newExpiry);

			return true;
		} catch (e) {
			console.error('Token refresh failed:', e);
			return false;
		}
	}

	private async fetch<T>(path: string, options: RequestInit = {}): Promise<T> {
		// Proactively refresh token if it's about to expire
		if (this.shouldProactivelyRefresh()) {
			await this.refreshToken();
		}

		return this.doFetch<T>(path, options, true);
	}

	private async doFetch<T>(
		path: string,
		options: RequestInit,
		allowRetry: boolean
	): Promise<T> {
		const headers: Record<string, string> = {
			'Content-Type': 'application/json'
		};

		if (this.token) {
			headers['Authorization'] = `Bearer ${this.token}`;
		}

		// Merge any additional headers from options
		if (options.headers) {
			const optHeaders = options.headers as Record<string, string>;
			Object.assign(headers, optHeaders);
		}

		const response = await httpFetch(`${this.baseUrl}${path}`, {
			...options,
			headers
		});

		if (!response.ok) {
			const errorBody = await response.json().catch(() => null);
			const code = this.extractAuthErrorCode(errorBody);

			// Handle auth errors specially
			if (response.status === 401 || response.status === 403) {
				// If it's TOKEN_EXPIRED and we haven't retried yet, try refreshing
				if (code === 'TOKEN_EXPIRED' && allowRetry) {
					const refreshed = await this.refreshToken();
					if (refreshed) {
						// Retry the original request with new token
						return this.doFetch<T>(path, options, false);
					}
				}

				// Auth failure - notify callback
				if (code) {
					this.onAuthFailure?.(code);
					throw new AuthenticationError(
						errorBody?.error || `Authentication failed: ${code}`,
						code,
						response.status
					);
				}
			}

			const message =
				errorBody?.error?.message ||
				errorBody?.error ||
				errorBody?.message ||
				`API Error: ${response.status} ${response.statusText}`;
			throw new Error(message);
		}

		// 204 No Content has no response body
		if (response.status === 204) {
			return undefined as T;
		}

		return response.json();
	}

	// Auth
	async login(username: string, password: string): Promise<AuthTokens> {
		const res = await this.fetch<ApiResponse<AuthTokens>>('/api/auth/login', {
			method: 'POST',
			body: JSON.stringify({ username, password })
		});
		return res.data;
	}

	/**
	 * Test login against a specific URL without modifying global API state.
	 * Used by authenticateRemote to avoid disrupting the active connection.
	 */
	async loginToUrl(url: string, username: string, password: string): Promise<AuthTokens> {
		const response = await httpFetch(`${url.replace(/\/+$/, '')}/api/auth/login`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ username, password })
		});

		if (!response.ok) {
			const errorBody = await response.json().catch(() => null);
			const message =
				errorBody?.error || `Login failed: ${response.status} ${response.statusText}`;
			throw new Error(message);
		}

		const res: ApiResponse<AuthTokens> = await response.json();
		return res.data;
	}

	// Health check (no auth required)
	async checkHealth(): Promise<{ status: string; version: string }> {
		const response = await httpFetch(`${this.baseUrl}/health`);
		if (!response.ok) {
			throw new Error(`Health check failed: ${response.status}`);
		}
		return response.json();
	}

	// Status (requires auth)
	async getStatus(): Promise<DaemonStatus> {
		const res = await this.fetch<ApiResponse<DaemonStatus>>('/api/status');
		return res.data;
	}

	// Channels
	async getChannels(): Promise<Channel[]> {
		const res = await this.fetch<ApiResponse<{ channels: BackendChannel[] }>>('/api/channels');
		return res.data.channels.map(transformChannel);
	}

	async getChannel(id: string): Promise<Channel> {
		const res = await this.fetch<ApiResponse<BackendChannel>>(`/api/channels/${id}`);
		return transformChannel(res.data);
	}

	async createChannel(channel: Partial<Channel>): Promise<Channel> {
		const res = await this.fetch<ApiResponse<{ id: string; channel: BackendChannel }>>(
			'/api/channels',
			{
				method: 'POST',
				body: JSON.stringify(channel)
			}
		);
		return transformChannel(res.data.channel);
	}

	async deleteChannel(id: string): Promise<void> {
		await this.fetch(`/api/channels/${id}`, { method: 'DELETE' });
	}

	async updateChannel(id: string, data: Partial<Channel>): Promise<Channel> {
		const res = await this.fetch<ApiResponse<BackendChannel>>(`/api/channels/${id}`, {
			method: 'PUT',
			body: JSON.stringify(data)
		});
		return transformChannel(res.data);
	}

	async checkChannel(id: string): Promise<Channel> {
		const res = await this.fetch<ApiResponse<{ channel: BackendChannel; message: string }>>(
			`/api/channels/${id}/check`,
			{ method: 'POST' }
		);
		return transformChannel(res.data.channel);
	}

	async stopRecording(id: string): Promise<Channel> {
		const res = await this.fetch<ApiResponse<{ channel: BackendChannel; message: string }>>(
			`/api/channels/${id}/stop-recording`,
			{ method: 'POST' }
		);
		return transformChannel(res.data.channel);
	}

	// Recordings
	async getRecordings(): Promise<Recording[]> {
		const res = await this.fetch<ApiResponse<{ recordings: Recording[] }>>('/api/recordings');
		return res.data.recordings;
	}

	async deleteRecording(id: string): Promise<void> {
		await this.fetch(`/api/recordings/${id}`, { method: 'DELETE' });
	}

	async processRecording(id: string): Promise<void> {
		await this.fetch(`/api/recordings/${id}/process`, {
			method: 'POST',
			body: JSON.stringify({})
		});
	}

	// Platform Authentication
	async getPlatformAuth(): Promise<PlatformAuth[]> {
		const res = await this.fetch<ApiResponse<PlatformAuthListResponse>>('/api/auth/platforms');
		return res.data.platforms;
	}

	async getPlatformAuthStatus(platform: Platform): Promise<PlatformAuth> {
		const res = await this.fetch<ApiResponse<PlatformAuth>>(`/api/auth/platforms/${platform}`);
		return res.data;
	}

	async setPlatformAuth(
		platform: Platform,
		credentials: SetPlatformAuthRequest
	): Promise<SetPlatformAuthResponse> {
		const res = await this.fetch<ApiResponse<SetPlatformAuthResponse>>(
			`/api/auth/platforms/${platform}`,
			{
				method: 'PUT',
				body: JSON.stringify(credentials)
			}
		);
		return res.data;
	}

	async deletePlatformAuth(platform: Platform): Promise<DeletePlatformAuthResponse> {
		const res = await this.fetch<ApiResponse<DeletePlatformAuthResponse>>(
			`/api/auth/platforms/${platform}`,
			{
				method: 'DELETE'
			}
		);
		return res.data;
	}

	async testPlatformAuth(platform: Platform): Promise<TestConnectionResponse> {
		const res = await this.fetch<ApiResponse<TestConnectionResponse>>(
			`/api/auth/platforms/${platform}/test`,
			{
				method: 'POST'
			}
		);
		return res.data;
	}

	async setYouTubeCookies(cookieContent: string): Promise<SetYouTubeCookiesResponse> {
		const res = await this.fetch<ApiResponse<SetYouTubeCookiesResponse>>(
			'/api/auth/platforms/youtube/cookies',
			{
				method: 'POST',
				body: JSON.stringify({ cookie_content: cookieContent })
			}
		);
		return res.data;
	}

	// OAuth
	async getOAuthAvailability(): Promise<OAuthAvailabilityResponse> {
		const res = await this.fetch<ApiResponse<OAuthAvailabilityResponse>>(
			'/api/auth/platforms/oauth/availability'
		);
		return res.data;
	}

	async startOAuth(
		platform: Platform,
		options?: {
			redirectUri?: string;
			clientId?: string;
			clientSecret?: string;
		}
	): Promise<StartOAuthResponse> {
		const res = await this.fetch<ApiResponse<StartOAuthResponse>>(
			`/api/auth/platforms/${platform}/oauth/start`,
			{
				method: 'POST',
				body: JSON.stringify({
					redirect_uri: options?.redirectUri,
					client_id: options?.clientId,
					client_secret: options?.clientSecret
				})
			}
		);
		return res.data;
	}

	async completeOAuth(
		platform: Platform,
		code: string,
		state: string
	): Promise<OAuthCallbackResponse> {
		const res = await this.fetch<ApiResponse<OAuthCallbackResponse>>(
			`/api/auth/platforms/${platform}/oauth/callback`,
			{
				method: 'POST',
				body: JSON.stringify({ code, state })
			}
		);
		return res.data;
	}

	// User Management
	async getUsers(): Promise<User[]> {
		const res = await this.fetch<ApiResponse<{ users: User[] }>>('/api/users');
		return res.data.users;
	}

	async createUser(data: CreateUserRequest): Promise<User> {
		const res = await this.fetch<ApiResponse<User>>('/api/users', {
			method: 'POST',
			body: JSON.stringify(data)
		});
		return res.data;
	}

	async updateUser(id: number, data: UpdateUserRequest): Promise<User> {
		const res = await this.fetch<ApiResponse<User>>(`/api/users/${id}`, {
			method: 'PUT',
			body: JSON.stringify(data)
		});
		return res.data;
	}

	async deleteUser(id: number): Promise<void> {
		await this.fetch(`/api/users/${id}`, { method: 'DELETE' });
	}

	async getUserSessions(userId: number): Promise<Session[]> {
		const res = await this.fetch<ApiResponse<{ sessions: Session[] }>>(
			`/api/users/${userId}/sessions`
		);
		return res.data.sessions;
	}

	async revokeAllUserSessions(userId: number): Promise<number> {
		const res = await this.fetch<ApiResponse<{ revoked_count: number }>>(
			`/api/users/${userId}/sessions`,
			{
				method: 'DELETE'
			}
		);
		return res.data.revoked_count;
	}

	async revokeUserSession(userId: number, sessionId: string): Promise<boolean> {
		const res = await this.fetch<ApiResponse<{ revoked: boolean }>>(
			`/api/users/${userId}/sessions/${sessionId}`,
			{
				method: 'DELETE'
			}
		);
		return res.data.revoked;
	}

	// Storage
	async getStorageStats(): Promise<StorageStats> {
		const res = await this.fetch<ApiResponse<StorageStats>>('/api/storage/stats');
		return res.data;
	}

	async cleanupStorage(request: CleanupRequest): Promise<CleanupResponse> {
		const res = await this.fetch<ApiResponse<CleanupResponse>>('/api/storage/cleanup', {
			method: 'POST',
			body: JSON.stringify(request)
		});
		return res.data;
	}

	async getDownloadStorageStats(): Promise<DownloadStorageStats> {
		const res = await this.fetch<ApiResponse<DownloadStorageStats>>('/api/downloads/stats');
		return res.data;
	}

	async cleanupDownloads(request: DownloadCleanupRequest): Promise<DownloadCleanupResponse> {
		const res = await this.fetch<ApiResponse<DownloadCleanupResponse>>(
			'/api/downloads/cleanup',
			{
				method: 'POST',
				body: JSON.stringify(request)
			}
		);
		return res.data;
	}

	// Post-Processing Config
	async getPostProcessingConfig(): Promise<PostProcessingConfig> {
		const res = await this.fetch<ApiResponse<PostProcessingConfig>>('/api/config/post-processing');
		return res.data;
	}

	async updatePostProcessingConfig(
		config: Partial<PostProcessingConfig>
	): Promise<PostProcessingConfig> {
		const res = await this.fetch<ApiResponse<PostProcessingConfig>>('/api/config/post-processing', {
			method: 'PUT',
			body: JSON.stringify(config)
		});
		return res.data;
	}

	// Reprocess Recording
	async reprocessRecording(id: string): Promise<void> {
		await this.fetch(`/api/recordings/${id}/reprocess`, {
			method: 'POST'
		});
	}

	// Channel Images
	async getChannelProfile(channelId: string): Promise<ChannelProfile> {
		const res = await this.fetch<ApiResponse<ChannelProfile>>(
			`/api/channels/${channelId}/profile`
		);
		return res.data;
	}

	async uploadChannelImage(
		channelId: string,
		imageType: 'profile' | 'banner',
		file: File
	): Promise<ImageUploadResponse> {
		const formData = new FormData();
		formData.append('file', file);

		const headers: Record<string, string> = {};
		if (this.token) {
			headers['Authorization'] = `Bearer ${this.token}`;
		}
		// Don't set Content-Type - browser will set it with boundary for multipart

		const response = await httpFetch(
			`${this.baseUrl}/api/channels/${channelId}/images/${imageType}`,
			{
				method: 'POST',
				headers,
				body: formData
			}
		);

		if (!response.ok) {
			const error = await response.json().catch(() => ({ error: response.statusText }));
			throw new Error(error.error?.message || error.error || 'Upload failed');
		}

		const result: ApiResponse<ImageUploadResponse> = await response.json();
		return result.data;
	}

	async deleteChannelImage(
		channelId: string,
		imageType: 'profile' | 'banner'
	): Promise<ImageDeleteResponse> {
		const res = await this.fetch<ApiResponse<ImageDeleteResponse>>(
			`/api/channels/${channelId}/images/${imageType}`,
			{ method: 'DELETE' }
		);
		return res.data;
	}

	async fetchPlatformImages(channelId: string): Promise<{
		success: boolean;
		profile_image_url: string | null;
		banner_image_url: string | null;
	}> {
		const res = await this.fetch<
			ApiResponse<{
				success: boolean;
				profile_image_url: string | null;
				banner_image_url: string | null;
			}>
		>(`/api/channels/${channelId}/images/fetch-platform`, { method: 'POST' });
		return res.data;
	}

	getChannelImageUrl(channelId: string, imageType: 'profile' | 'banner'): string {
		return `${this.baseUrl}/api/channels/${channelId}/images/${imageType}`;
	}

	// System
	async getSystemDependencies(): Promise<DependenciesResponse> {
		const res = await this.fetch<ApiResponse<DependenciesResponse>>('/api/system/dependencies');
		return res.data;
	}

	// ─── Downloads ───

	async getDownloads(): Promise<DownloadSummary[]> {
		const res = await this.fetch<ApiResponse<DownloadSummary[]>>('/api/downloads');
		return res.data;
	}

	async getDownload(id: string): Promise<Download> {
		const res = await this.fetch<ApiResponse<Download>>(`/api/downloads/${id}`);
		return res.data;
	}

	async createDownload(request: CreateDownloadRequest): Promise<{ id: string }> {
		const res = await this.fetch<ApiResponse<{ id: string }>>('/api/downloads', {
			method: 'POST',
			body: JSON.stringify(request)
		});
		return res.data;
	}

	async deleteDownload(id: string): Promise<void> {
		await this.fetch<void>(`/api/downloads/${id}`, { method: 'DELETE' });
	}

	async pauseDownload(id: string): Promise<void> {
		await this.fetch<void>(`/api/downloads/${id}/pause`, { method: 'POST' });
	}

	async resumeDownload(id: string): Promise<void> {
		await this.fetch<void>(`/api/downloads/${id}/resume`, { method: 'POST' });
	}

	async cancelDownload(id: string): Promise<void> {
		await this.fetch<void>(`/api/downloads/${id}/cancel`, { method: 'POST' });
	}

	async prioritizeDownload(id: string): Promise<void> {
		await this.fetch<void>(`/api/downloads/${id}/prioritize`, { method: 'POST' });
	}

	// ─── Extensions ───

	async getExtensionConnections(): Promise<ExtensionConnection[]> {
		const res = await this.fetch<ApiResponse<ExtensionConnection[]>>('/api/extensions/connections');
		return res.data;
	}

	async disconnectExtension(id: string): Promise<void> {
		await this.fetch<void>(`/api/extensions/connections/${id}`, { method: 'DELETE' });
	}

	async getExtensionConnectionLogs(id: string): Promise<MessageLogEntry[]> {
		const res = await this.fetch<ApiResponse<MessageLogEntry[]>>(
			`/api/extensions/connections/${id}/logs`
		);
		return res.data;
	}

	async getExtensionConfig(): Promise<ExtensionConfig> {
		const res = await this.fetch<ApiResponse<ExtensionConfig>>('/api/extensions/config');
		return res.data;
	}

	async updateExtensionConfig(config: Partial<ExtensionConfig>): Promise<ExtensionConfig> {
		const res = await this.fetch<ApiResponse<ExtensionConfig>>('/api/extensions/config', {
			method: 'PUT',
			body: JSON.stringify(config)
		});
		return res.data;
	}

	async generatePairCode(): Promise<{ code: string }> {
		const res = await this.fetch<ApiResponse<{ code: string }>>('/api/extensions/pair-code', {
			method: 'POST'
		});
		return res.data;
	}

	// ─── Libraries ───

	async getLibraryStatus(): Promise<LibraryStatus> {
		const res = await this.fetch<ApiResponse<LibraryStatus>>('/api/libraries');
		return res.data;
	}

	async installLibraries(): Promise<void> {
		await this.fetch<void>('/api/libraries/install', { method: 'POST' });
	}

	async updateLibrary(name: string): Promise<void> {
		await this.fetch<void>(`/api/libraries/${name}/update`, { method: 'POST' });
	}

	async uninstallLibrary(name: string): Promise<void> {
		await this.fetch<void>(`/api/libraries/${name}`, { method: 'DELETE' });
	}

	// ─── Downloads Config ───

	async getDownloadsConfig(): Promise<DownloadsConfig> {
		const res = await this.fetch<ApiResponse<DownloadsConfig>>('/api/config/downloads');
		return res.data;
	}

	async updateDownloadsConfig(config: Partial<DownloadsConfig>): Promise<DownloadsConfig> {
		const res = await this.fetch<ApiResponse<DownloadsConfig>>('/api/config/downloads', {
			method: 'PUT',
			body: JSON.stringify(config)
		});
		return res.data;
	}

	// ─── Merge & Aliases ───

	async mergeDownloads(
		platform: string,
		source: string,
		target: string
	): Promise<{ files_moved: number }> {
		const res = await this.fetch<ApiResponse<{ files_moved: number }>>('/api/downloads/merge', {
			method: 'POST',
			body: JSON.stringify({ platform, source, target })
		});
		return res.data;
	}

	async getAliases(): Promise<{
		download_aliases: Record<string, string>;
		recording_aliases: Record<string, string>;
	}> {
		const res = await this.fetch<
			ApiResponse<{
				download_aliases: Record<string, string>;
				recording_aliases: Record<string, string>;
			}>
		>('/api/aliases');
		return res.data;
	}

	async createAlias(type: 'download' | 'recording', key: string, target: string): Promise<void> {
		await this.fetch<void>('/api/aliases', {
			method: 'POST',
			body: JSON.stringify({ type, key, target })
		});
	}

	async deleteAlias(type: string, key: string): Promise<void> {
		await this.fetch<void>(`/api/aliases/${type}/${encodeURIComponent(key)}`, {
			method: 'DELETE'
		});
	}
}

export const api = new ApiClient();
