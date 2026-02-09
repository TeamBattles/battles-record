import { browser } from '$app/environment';

export interface SavedServer {
	id: string;
	name: string;
	type: 'local' | 'remote';
	url: string;
	token?: string;
	tokenExpiry?: number;
	username?: string;
}

export interface AppSettings {
	startupServerId: string | null;
	servers: SavedServer[];
	closeAction: 'ask' | 'background' | 'shutdown';
	localDaemonDataDir: string | null;
	localDaemonLibraryDir: string | null;
	// Appearance
	showCornerBrackets: boolean;
}

const DEFAULT_SETTINGS: AppSettings = {
	startupServerId: null,
	servers: [],
	closeAction: 'ask' as const,
	localDaemonDataDir: null,
	localDaemonLibraryDir: null,
	showCornerBrackets: false
};

const SETTINGS_FILE = 'settings.json';

class SettingsStore {
	settings = $state<AppSettings>(DEFAULT_SETTINGS);
	isLoaded = $state(false);
	private tauriFs: typeof import('@tauri-apps/plugin-fs') | null = null;
	private tauriPath: typeof import('@tauri-apps/api/path') | null = null;

	async init() {
		if (!browser) return;

		try {
			// Dynamic import for Tauri APIs (won't work in browser dev mode)
			this.tauriFs = await import('@tauri-apps/plugin-fs');
			this.tauriPath = await import('@tauri-apps/api/path');
			await this.load();
		} catch (e) {
			// Running in browser without Tauri - use localStorage fallback
			console.warn('Tauri not available, using localStorage fallback');
			this.loadFromLocalStorage();
		}
		this.isLoaded = true;
	}

	private async load() {
		if (!this.tauriFs || !this.tauriPath) return;

		try {
			const appDataDir = await this.tauriPath.appDataDir();
			const filePath = await this.tauriPath.join(appDataDir, SETTINGS_FILE);
			const contents = await this.tauriFs.readTextFile(filePath);
			// Merge with defaults to handle new settings fields
			this.settings = { ...DEFAULT_SETTINGS, ...JSON.parse(contents) };
		} catch (e) {
			// File doesn't exist yet, use defaults
			this.settings = { ...DEFAULT_SETTINGS };
		}
	}

	private loadFromLocalStorage() {
		try {
			const stored = localStorage.getItem('br-settings');
			if (stored) {
				// Merge with defaults to handle new settings fields
				this.settings = { ...DEFAULT_SETTINGS, ...JSON.parse(stored) };
			}
		} catch (e) {
			this.settings = { ...DEFAULT_SETTINGS };
		}
	}

	async save() {
		if (!browser) return;

		if (this.tauriFs && this.tauriPath) {
			try {
				const appDataDir = await this.tauriPath.appDataDir();
				// Ensure directory exists
				await this.tauriFs.mkdir(appDataDir, { recursive: true });
				const filePath = await this.tauriPath.join(appDataDir, SETTINGS_FILE);
				await this.tauriFs.writeTextFile(filePath, JSON.stringify(this.settings, null, 2));
			} catch (e) {
				console.error('Failed to save settings:', e);
			}
		} else {
			// localStorage fallback
			localStorage.setItem('br-settings', JSON.stringify(this.settings));
		}
	}

	// Server management
	getServer(id: string): SavedServer | undefined {
		return this.settings.servers.find((s) => s.id === id);
	}

	addServer(server: SavedServer) {
		this.settings.servers.push(server);
		this.save();
	}

	updateServer(id: string, updates: Partial<SavedServer>) {
		const index = this.settings.servers.findIndex((s) => s.id === id);
		if (index !== -1) {
			this.settings.servers[index] = { ...this.settings.servers[index], ...updates };
			this.save();
		}
	}

	removeServer(id: string) {
		this.settings.servers = this.settings.servers.filter((s) => s.id !== id);
		if (this.settings.startupServerId === id) {
			this.settings.startupServerId = null;
		}
		this.save();
	}

	setStartupServer(id: string | null) {
		this.settings.startupServerId = id;
		this.save();
	}

	upsertLocalServer(url: string) {
		const existing = this.settings.servers.find((s) => s.id === 'local');
		if (existing) {
			this.updateServer('local', { url });
		} else {
			this.addServer({
				id: 'local',
				name: 'Local',
				type: 'local',
				url
			});
		}
	}

	setCloseAction(action: 'ask' | 'background' | 'shutdown') {
		this.settings.closeAction = action;
		this.save();
	}

	async setCloseActionAsync(action: 'ask' | 'background' | 'shutdown') {
		this.settings.closeAction = action;
		await this.save();
	}

	setLocalDaemonDataDir(dir: string | null) {
		this.settings.localDaemonDataDir = dir;
		this.save();
	}

	setLocalDaemonLibraryDir(dir: string | null) {
		this.settings.localDaemonLibraryDir = dir;
		this.save();
	}

	setShowCornerBrackets(show: boolean) {
		this.settings.showCornerBrackets = show;
		this.save();
	}

	get hasStartupServer(): boolean {
		return this.settings.startupServerId !== null;
	}

	get startupServer(): SavedServer | undefined {
		if (!this.settings.startupServerId) return undefined;
		return this.getServer(this.settings.startupServerId);
	}
}

export const settingsStore = new SettingsStore();
