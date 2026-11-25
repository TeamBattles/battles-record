import { api } from '#lib/api/client.ts';
import { wsClient, type WebSocketEvent } from '#lib/api/websocket.ts';
import type { LibraryStatus } from '#lib/api/types.ts';
import { extractErrorMessage } from '#lib/utils/errors.ts';

/** Library status on the active server. Browser connections live in extensionGatewayStore. */
class ExtensionsStore {
	libraryStatus = $state<LibraryStatus | null>(null);
	isLoading = $state(false);
	error = $state<string | null>(null);

	private _loadedServerId: string | null = null;

	constructor() {
		wsClient.subscribe((event: WebSocketEvent) => this.handleWebSocketEvent(event));
	}

	private handleWebSocketEvent(event: WebSocketEvent) {
		switch (event.type) {
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
		if (!this.libraryStatus || isServerSwitch) {
			this.isLoading = true;
		}
		this.error = null;

		try {
			this.libraryStatus = await api.getLibraryStatus();
			if (serverId) this._loadedServerId = serverId;
		} catch (e) {
			this.error = extractErrorMessage(e, 'Failed to load library status');
		} finally {
			this.isLoading = false;
		}
	}

	async loadLibraryStatus() {
		try {
			this.libraryStatus = await api.getLibraryStatus();
		} catch {
			// Best-effort refresh
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
