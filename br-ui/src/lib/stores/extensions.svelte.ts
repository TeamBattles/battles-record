import { api } from '$lib/api/client';
import { wsClient, type WebSocketEvent } from '$lib/api/websocket';
import type { ExtensionConnection, ExtensionConfig, LibraryStatus } from '$lib/api/types';
import { extractErrorMessage } from '$lib/utils/errors';

class ExtensionsStore {
	connections = $state<ExtensionConnection[]>([]);
	config = $state<ExtensionConfig | null>(null);
	libraryStatus = $state<LibraryStatus | null>(null);
	isLoading = $state(false);
	error = $state<string | null>(null);

	private _loadedServerId: string | null = null;

	constructor() {
		wsClient.subscribe((event: WebSocketEvent) => this.handleWebSocketEvent(event));
	}

	private handleWebSocketEvent(event: WebSocketEvent) {
		switch (event.type) {
			case 'extension_connected': {
				const existing = this.connections.find((c) => c.client_id === event.client_id);
				if (existing) {
					this.connections = this.connections.map((c) =>
						c.client_id === event.client_id ? { ...c, connected: true } : c
					);
				} else {
					// New connection not yet in our list - add it
					this.connections = [
						...this.connections,
						{
							client_id: event.client_id,
							identifier: event.identifier,
							paired_at: new Date().toISOString(),
							last_connected: new Date().toISOString(),
							connected: true
						}
					];
				}
				break;
			}
			case 'extension_disconnected': {
				this.connections = this.connections.map((c) =>
					c.client_id === event.client_id ? { ...c, connected: false } : c
				);
				break;
			}
			case 'library_status_changed': {
				if (this.libraryStatus) {
					const key = event.library as keyof LibraryStatus;
					if (key in this.libraryStatus) {
						if (event.installed) {
							// Optimistic update for installs, then fetch full status for path
							this.libraryStatus = {
								...this.libraryStatus,
								[key]: {
									...this.libraryStatus[key],
									installed: event.installed,
									version: event.version
								}
							};
						}
						// Always fetch full status to get path, update_available,
						// and detect system PATH fallback after uninstall
						this.loadLibraryStatus();
					}
				}
				break;
			}
		}
	}

	async load(serverId?: string) {
		const isServerSwitch = serverId != null && serverId !== this._loadedServerId;
		if (!this.connections.length || isServerSwitch) {
			this.isLoading = true;
		}
		this.error = null;

		try {
			const [connections, config, libraryStatus] = await Promise.all([
				api.getExtensionConnections(),
				api.getExtensionConfig(),
				api.getLibraryStatus()
			]);
			this.connections = connections;
			this.config = config;
			this.libraryStatus = libraryStatus;
			if (serverId) this._loadedServerId = serverId;
		} catch (e) {
			this.error = extractErrorMessage(e, 'Failed to load extension data');
		} finally {
			this.isLoading = false;
		}
	}

	get connectedCount(): number {
		return this.connections.filter((c) => c.connected).length;
	}

	get totalPaired(): number {
		return this.connections.length;
	}

	async loadLibraryStatus() {
		try {
			this.libraryStatus = await api.getLibraryStatus();
		} catch {
			// Best-effort refresh
		}
	}

	async disconnect(clientId: string) {
		try {
			await api.disconnectExtension(clientId);
			this.connections = this.connections.filter((c) => c.client_id !== clientId);
		} catch (e) {
			this.error = extractErrorMessage(e, 'Failed to disconnect extension');
		}
	}

	async installLibraries() {
		try {
			await api.installLibraries();
		} catch (e) {
			this.error = extractErrorMessage(e, 'Failed to install libraries');
		}
	}

	async updateLibrary(name: string) {
		try {
			await api.updateLibrary(name);
		} catch (e) {
			this.error = extractErrorMessage(e, `Failed to update ${name}`);
		}
	}

	async uninstallLibrary(name: string): Promise<boolean> {
		try {
			await api.uninstallLibrary(name);
			await this.loadLibraryStatus();
			return true;
		} catch (e) {
			this.error = extractErrorMessage(e, `Failed to uninstall ${name}`);
			return false;
		}
	}
}

export const extensionsStore = new ExtensionsStore();
