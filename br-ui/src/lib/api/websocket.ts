import type { BackendChannel } from './backend-types';
import type { Platform } from './types';

export type WebSocketEvent =
	| {
			type: 'channel_status';
			channel_id: string;
			name: string;
			platform: string;
			status: 'live' | 'offline' | 'recording';
			stream?: { title: string; game?: string; viewers: number };
	  }
	| { type: 'channel_error'; channel_id: string; name: string; error: string }
	| {
			type: 'recording_started';
			recording_id: string;
			channel_id: string;
			channel_name: string;
			platform: string;
			quality: string;
	  }
	| {
			type: 'recording_ended';
			recording_id: string;
			duration_secs: number;
			size_bytes: number;
			segment_count: number;
			reason: string;
	  }
	| {
			type: 'segment_downloaded';
			recording_id: string;
			sequence: number;
			size_bytes: number;
			total_segments: number;
			total_bytes: number;
	  }
	| { type: 'processing_started'; recording_id: string }
	| { type: 'processing_progress'; recording_id: string; percent: number }
	| { type: 'processing_complete'; recording_id: string; output_file: string; size_bytes: number }
	| { type: 'processing_failed'; recording_id: string; error: string }
	| { type: 'disk_warning'; usage_percent: number; free_bytes: number }
	| { type: 'config_reloaded'; sections: string[] }
	| { type: 'schedule_skip'; channel_id: string; channel_name: string; platform: string }
	| {
			type: 'filter_skip';
			channel_id: string;
			channel_name: string;
			platform: string;
			reason: unknown;
	  }
	| {
			type: 'quota_skip';
			channel_id: string;
			channel_name: string;
			platform: string;
			quota_used_bytes: number;
			quota_limit_bytes: number;
			message: string;
	  }
	| {
			type: 'quota_status_changed';
			channel_id: string;
			channel_name: string;
			quota_status: import('./types').QuotaStatus;
			quota_used_bytes: number;
			quota_percent: number;
	  }
	| {
			type: 'platform_auth_updated';
			platform: Platform;
			status: string;
			username?: string;
			expires_at?: string;
	  }
	| {
			type: 'platform_auth_expired';
			platform: Platform;
			reason: string;
	  }
	| { type: 'connected'; channels: BackendChannel[]; active_recordings: unknown[] }
	| { type: 'error'; message: string };

type EventHandler = (event: WebSocketEvent) => void;

export class WebSocketClient {
	private ws: WebSocket | null = null;
	private baseUrl: string;
	private token: string | null = null;
	private handlers: Set<EventHandler> = new Set();
	private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
	// Cache the last 'connected' event to replay to new subscribers
	private lastConnectedEvent: WebSocketEvent | null = null;

	// Callback for auth failures (set by connection store)
	onAuthFailure: (() => void) | null = null;

	constructor(baseUrl: string = 'ws://localhost:8080') {
		this.baseUrl = baseUrl.replace(/\/+$/, '');
	}

	setToken(token: string | null) {
		this.token = token;
	}

	setBaseUrl(url: string) {
		this.baseUrl = url.replace(/\/+$/, '');
	}

	connect() {
		console.log('[WS] connect() called', { readyState: this.ws?.readyState, time: Date.now() });
		if (this.ws?.readyState === WebSocket.OPEN) {
			console.log('[WS] already connected, skipping');
			return;
		}

		const url = this.token
			? `${this.baseUrl}/api/events?token=${this.token}`
			: `${this.baseUrl}/api/events`;

		console.log('[WS] creating WebSocket to', url);
		this.ws = new WebSocket(url);

		this.ws.onopen = () => {
			console.log('[WS] onopen fired', { time: Date.now() });
		};

		this.ws.onmessage = (event) => {
			try {
				const data = JSON.parse(event.data) as WebSocketEvent;
				// Cache the connected event to replay to new subscribers
				if (data.type === 'connected') {
					console.log('[WS] connected event received', {
						channelCount: data.channels.length,
						statuses: data.channels.map((c) => ({ name: c.name, status: c.status })),
						time: Date.now()
					});
					this.lastConnectedEvent = data;
				}
				this.handlers.forEach((handler) => handler(data));
				// Feed to activity store - dynamic import to avoid circular dependency
				import('../stores/activity.svelte').then(({ activityStore }) => {
					activityStore.handleWebSocketEvent(data as WebSocketEvent & Record<string, unknown>);
				});
				// Handle platform auth events - dynamic import to avoid circular dependency
				if (data.type === 'platform_auth_updated' || data.type === 'platform_auth_expired') {
					import('../stores/platformAuth.svelte').then(({ platformAuthStore }) => {
						if (data.type === 'platform_auth_updated') {
							platformAuthStore.handleAuthUpdated(data);
						} else if (data.type === 'platform_auth_expired') {
							platformAuthStore.handleAuthExpired(data);
						}
					});
				}
			} catch (e) {
				console.error('WebSocket parse error:', e);
			}
		};

		this.ws.onclose = (event) => {
			console.log('[WS] onclose fired', { code: event.code, reason: event.reason, time: Date.now() });

			// Handle auth-related close codes (4001 = unauthorized, 4003 = forbidden)
			if (event.code === 4001 || event.code === 4003) {
				console.log('[WS] Auth failure detected, notifying callback');
				this.onAuthFailure?.();
				// Don't auto-reconnect on auth failure
				return;
			}

			this.scheduleReconnect();
		};

		this.ws.onerror = (error) => {
			console.error('WebSocket error:', error);
		};
	}

	disconnect() {
		if (this.reconnectTimer) {
			clearTimeout(this.reconnectTimer);
			this.reconnectTimer = null;
		}
		this.ws?.close();
		this.ws = null;
		// Clear cached event on disconnect to avoid stale data
		this.lastConnectedEvent = null;
	}

	private scheduleReconnect() {
		if (this.reconnectTimer) return;
		this.reconnectTimer = setTimeout(() => {
			this.reconnectTimer = null;
			this.connect();
		}, 5000);
	}

	subscribe(handler: EventHandler): () => void {
		console.log('[WS] subscribe() called', {
			hasCachedEvent: !!this.lastConnectedEvent,
			cachedChannels:
				this.lastConnectedEvent?.type === 'connected' ? this.lastConnectedEvent.channels.length : 0,
			time: Date.now()
		});
		this.handlers.add(handler);
		// Replay the cached connected event to new subscribers so they get the initial state
		if (this.lastConnectedEvent) {
			console.log('[WS] replaying cached connected event to new subscriber');
			handler(this.lastConnectedEvent);
		}
		return () => this.handlers.delete(handler);
	}

	get isConnected(): boolean {
		return this.ws?.readyState === WebSocket.OPEN;
	}
}

export const wsClient = new WebSocketClient();
