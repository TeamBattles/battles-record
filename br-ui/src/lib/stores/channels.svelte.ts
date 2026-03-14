import { api } from '$lib/api';
import type { Channel, QuotaStatus, ChannelProfile } from '$lib/api/types';
import type { BackendChannel } from '$lib/api/backend-types';
import { wsClient, type WebSocketEvent } from '$lib/api/websocket';
import { toastStore } from './toast.svelte';
import { formatBytes, extractErrorMessage, transformChannel } from '$lib/utils';

class ChannelsStore {
	channels = $state<Channel[]>([]);
	isLoading = $state(false);
	error = $state<string | null>(null);

	// Filters
	platformFilter = $state<'all' | 'twitch' | 'youtube' | 'kick'>('all');
	searchQuery = $state('');

	// Selection
	selectedChannelId = $state<string | null>(null);

	// Profile cache for channel images
	private profileCache = new Map<string, ChannelProfile>();
	private profileLoading = new Map<string, boolean>();

	// Track which server's data we have for stale-while-revalidate
	private _loadedServerId: string | null = null;

	// WebSocket subscription
	private unsubscribe: (() => void) | null = null;

	get filteredChannels() {
		return this.channels.filter((channel) => {
			const matchesPlatform =
				this.platformFilter === 'all' || channel.platform === this.platformFilter;
			const matchesSearch =
				this.searchQuery === '' ||
				channel.name.toLowerCase().includes(this.searchQuery.toLowerCase());
			return matchesPlatform && matchesSearch;
		});
	}

	get selectedChannel() {
		return this.channels.find((c) => c.id === this.selectedChannelId) ?? null;
	}

	get channelCount() {
		return this.channels.length;
	}

	/**
	 * Get cached channel profile (returns null if not cached)
	 */
	getCachedProfile(channelId: string): ChannelProfile | null {
		return this.profileCache.get(channelId) ?? null;
	}

	/**
	 * Check if a profile is currently being loaded
	 */
	isProfileLoading(channelId: string): boolean {
		return this.profileLoading.get(channelId) ?? false;
	}

	/**
	 * Get channel profile, loading from API if not cached
	 */
	async getChannelProfile(channelId: string): Promise<ChannelProfile | null> {
		// Return cached profile if available
		const cached = this.profileCache.get(channelId);
		if (cached) return cached;

		// Prevent duplicate requests
		if (this.profileLoading.get(channelId)) return null;

		this.profileLoading.set(channelId, true);
		try {
			const profile = await api.getChannelProfile(channelId);
			this.profileCache.set(channelId, profile);
			return profile;
		} catch (e) {
			console.error('[CH] Failed to load profile for', channelId, e);
			return null;
		} finally {
			this.profileLoading.set(channelId, false);
		}
	}

	/**
	 * Refresh channel profile from API (bypasses cache)
	 */
	async refreshChannelProfile(channelId: string): Promise<ChannelProfile | null> {
		this.profileLoading.set(channelId, true);
		try {
			const profile = await api.getChannelProfile(channelId);
			this.profileCache.set(channelId, profile);
			return profile;
		} catch (e) {
			console.error('[CH] Failed to refresh profile for', channelId, e);
			return null;
		} finally {
			this.profileLoading.set(channelId, false);
		}
	}

	/**
	 * Clear cached profile for a channel
	 */
	clearProfileCache(channelId: string): void {
		this.profileCache.delete(channelId);
	}

	/**
	 * Clear all cached profiles
	 */
	clearAllProfileCaches(): void {
		this.profileCache.clear();
	}

	async load(serverId?: string) {
		console.log('[CH] load() starting', { serverId, time: Date.now() });
		const isServerSwitch = serverId != null && serverId !== this._loadedServerId;
		const hasData = this.channels.length > 0;

		if (!hasData || isServerSwitch) {
			this.isLoading = true;
			if (isServerSwitch) this.channels = [];
		}
		this.error = null;
		try {
			this.channels = await api.getChannels();
			if (serverId) this._loadedServerId = serverId;
			console.log('[CH] load() complete', {
				channels: this.channels.map((c) => ({
					name: c.name,
					is_live: c.status?.is_live,
					is_recording: c.status?.is_recording
				})),
				time: Date.now()
			});
		} catch (e) {
			console.log('[CH] load() error', e);
			this.error = extractErrorMessage(e, 'Failed to load channels');
		} finally {
			this.isLoading = false;
		}
	}

	async createChannel(
		data: Partial<Channel>
	): Promise<{ success: true } | { success: false; error: string }> {
		try {
			const channel = await api.createChannel(data);
			this.channels = [...this.channels, channel];

			// Immediately check the channel to get updated live status
			// This runs in background - don't await to keep UI responsive
			api
				.checkChannel(channel.id)
				.then((updated) => {
					const idx = this.channels.findIndex((c) => c.id === channel.id);
					if (idx !== -1) {
						this.channels[idx] = updated;
					}
				})
				.catch(() => {
					// Ignore check errors - status will update via WebSocket
				});

			return { success: true };
		} catch (e) {
			return { success: false, error: extractErrorMessage(e, 'Failed to create channel') };
		}
	}

	async deleteChannel(id: string): Promise<boolean> {
		try {
			await api.deleteChannel(id);
			this.channels = this.channels.filter((c) => c.id !== id);
			if (this.selectedChannelId === id) {
				this.selectedChannelId = null;
			}
			return true;
		} catch (e) {
			this.error = extractErrorMessage(e, 'Failed to delete channel');
			return false;
		}
	}

	async updateChannel(id: string, data: Partial<Channel>): Promise<boolean> {
		try {
			const updated = await api.updateChannel(id, data);
			const index = this.channels.findIndex((c) => c.id === id);
			if (index !== -1) {
				this.channels[index] = updated;
			}
			return true;
		} catch (e) {
			this.error = extractErrorMessage(e, 'Failed to update channel');
			return false;
		}
	}

	async checkChannel(id: string): Promise<void> {
		try {
			const updated = await api.checkChannel(id);
			const idx = this.channels.findIndex((c) => c.id === id);
			if (idx !== -1) {
				this.channels[idx] = updated;
			}
		} catch (e) {
			toastStore.error(extractErrorMessage(e, 'Failed to check channel'));
		}
	}

	async stopRecording(id: string): Promise<boolean> {
		try {
			const channel = await api.stopRecording(id);
			const idx = this.channels.findIndex((c) => c.id === id);
			if (idx !== -1) {
				this.channels[idx] = channel;
			}
			toastStore.success('Recording stopped, channel paused');
			return true;
		} catch (e) {
			toastStore.error(extractErrorMessage(e, 'Failed to stop recording'));
			return false;
		}
	}

	selectChannel(id: string | null) {
		this.selectedChannelId = id;
	}

	setFilter(platform: 'all' | 'twitch' | 'youtube' | 'kick') {
		this.platformFilter = platform;
	}

	setSearch(query: string) {
		this.searchQuery = query;
	}

	// WebSocket event handling
	subscribe() {
		console.log('[CH] subscribe() called', {
			alreadySubscribed: !!this.unsubscribe,
			time: Date.now()
		});
		if (this.unsubscribe) return; // Already subscribed
		this.unsubscribe = wsClient.subscribe(this.handleEvent.bind(this));
	}

	unsubscribeEvents() {
		this.unsubscribe?.();
		this.unsubscribe = null;
	}

	private handleEvent(event: WebSocketEvent) {
		console.log('[CH] handleEvent()', { type: event.type, time: Date.now() });
		switch (event.type) {
			case 'connected':
				// Update channels from the WebSocket connected event
				// This ensures we have the latest status from the daemon
				this.handleConnected(event.channels);
				break;
			case 'channel_status':
				console.log('[CH] received channel_status event', event);
				this.updateChannelStatus(event.channel_id, event.status);
				break;
			case 'recording_started':
				this.handleRecordingStarted(event.channel_name);
				break;
			case 'recording_ended':
				this.handleRecordingEnded(event.duration_secs);
				break;
			case 'error':
				toastStore.error(event.message);
				break;
			case 'quota_skip':
				toastStore.warning(
					`Recording blocked: ${event.channel_name} quota exceeded (${formatBytes(event.quota_used_bytes)} / ${formatBytes(event.quota_limit_bytes)})`
				);
				break;
			case 'quota_status_changed':
				this.updateChannelQuota(
					event.channel_id,
					event.quota_status,
					event.quota_used_bytes,
					event.quota_percent
				);
				break;
		}
	}

	private handleConnected(backendChannels: BackendChannel[]) {
		console.log('[CH] handleConnected()', {
			received: backendChannels.map((c) => ({ name: c.name, status: c.status })),
			time: Date.now()
		});
		// Transform backend channels to frontend format and update store
		const channels = backendChannels.map(transformChannel);
		console.log('[CH] handleConnected() transformed', {
			transformed: channels.map((c) => ({
				name: c.name,
				is_live: c.status?.is_live,
				is_recording: c.status?.is_recording
			}))
		});

		// Merge with existing channels or replace entirely
		// We replace because the connected event has the authoritative state
		if (channels.length > 0) {
			console.log('[CH] handleConnected() updating store with', channels.length, 'channels');
			this.channels = channels;
		} else if (this.channels.length === 0) {
			// If no channels from WebSocket and store is empty, keep it empty
			// The load() call will populate from REST API
			console.log('[CH] handleConnected() no channels to update');
		}
	}

	private updateChannelStatus(channelId: string, status: 'live' | 'offline' | 'recording') {
		console.log('[CH] updateChannelStatus()', { channelId, status, time: Date.now() });

		// First do an immediate partial update for responsive UI
		const idx = this.channels.findIndex((c) => c.id === channelId);
		if (idx !== -1) {
			const channel = this.channels[idx];
			console.log('[CH] updating channel', {
				name: channel.name,
				oldStatus: channel.status,
				newStatus: status
			});
			this.channels[idx] = {
				...channel,
				status: {
					...channel.status,
					is_live: status === 'live' || status === 'recording',
					is_recording: status === 'recording'
				}
			};
		} else {
			console.log('[CH] channel not found in store', { channelId });
		}

		// Then fetch complete channel data to get stream info (title, game, viewers)
		api
			.getChannel(channelId)
			.then((updated) => {
				console.log('[CH] fetched full channel data', {
					name: updated.name,
					status: updated.status
				});
				const idx = this.channels.findIndex((c) => c.id === channelId);
				if (idx !== -1) {
					this.channels[idx] = updated;
				}
			})
			.catch((e) => {
				console.log('[CH] fetch channel error', e);
			});
	}

	private handleRecordingStarted(channelName: string) {
		// Find channel by name and update recording status
		const idx = this.channels.findIndex((c) => c.name === channelName);
		if (idx !== -1) {
			const channel = this.channels[idx];
			this.channels[idx] = {
				...channel,
				status: {
					...channel.status,
					is_live: true,
					is_recording: true
				}
			};
		}
		toastStore.success(`Recording started: ${channelName}`);
	}

	private handleRecordingEnded(duration: number) {
		const minutes = Math.floor(duration / 60);
		toastStore.info(`Recording ended (${minutes}m)`);
		// Reload to get fresh status
		this.load();
	}

	private updateChannelQuota(
		channelId: string,
		status: QuotaStatus,
		usedBytes: number,
		percent: number
	) {
		const idx = this.channels.findIndex((c) => c.id === channelId);
		if (idx !== -1) {
			this.channels[idx] = {
				...this.channels[idx],
				quota_status: status,
				quota_used_bytes: usedBytes,
				quota_percent: percent
			};
		}
	}
}

export const channelsStore = new ChannelsStore();
