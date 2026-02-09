/**
 * Settings Store Integration Tests
 *
 * Tests for the settings store which manages app configuration,
 * saved servers, and persistence.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { SavedServer, AppSettings } from './settings.svelte';

// Mock browser environment
vi.mock('$app/environment', () => ({
	browser: true
}));

// Mock Tauri modules - these need to be hoisted for dynamic imports to work
const mockReadTextFile = vi.fn();
const mockWriteTextFile = vi.fn();
const mockMkdir = vi.fn();
const mockAppDataDir = vi.fn();
const mockJoin = vi.fn();

vi.mock('@tauri-apps/plugin-fs', () => ({
	readTextFile: mockReadTextFile,
	writeTextFile: mockWriteTextFile,
	mkdir: mockMkdir
}));

vi.mock('@tauri-apps/api/path', () => ({
	appDataDir: mockAppDataDir,
	join: mockJoin
}));

// Import the store after mocks are set up - use dynamic import to get fresh instance
let settingsStore: typeof import('./settings.svelte').settingsStore;

// Test data
const mockServer: SavedServer = {
	id: 'test-server',
	name: 'Test Server',
	type: 'remote',
	url: 'https://test.example.com',
	token: 'test-token',
	tokenExpiry: Date.now() + 86400000
};

const mockLocalServer: SavedServer = {
	id: 'local',
	name: 'Local',
	type: 'local',
	url: 'http://localhost:8080'
};

const mockSettings: AppSettings = {
	startupServerId: null,
	servers: [mockServer, mockLocalServer],
	closeAction: 'ask',
	localDaemonDataDir: null,
	localDaemonLibraryDir: null,
	showCornerBrackets: false
};

// Helper to reset store state
function resetStore() {
	if (settingsStore) {
		settingsStore.settings = {
			startupServerId: null,
			servers: [],
			closeAction: 'ask',
			localDaemonDataDir: null,
			localDaemonLibraryDir: null,
			showCornerBrackets: false
		};
		settingsStore.isLoaded = false;
	}
}

describe('SettingsStore', () => {
	beforeEach(async () => {
		vi.clearAllMocks();

		// Setup default mock behavior
		mockAppDataDir.mockResolvedValue('/app/data');
		mockJoin.mockImplementation((...args: string[]) => args.join('/'));

		// Get fresh store instance
		const module = await import('./settings.svelte');
		settingsStore = module.settingsStore;
		resetStore();
	});

	afterEach(() => {
		vi.clearAllMocks();
	});

	describe('initialization', () => {
		it('init() loads from Tauri file system', async () => {
			mockReadTextFile.mockResolvedValue(JSON.stringify(mockSettings));

			await settingsStore.init();

			expect(mockReadTextFile).toHaveBeenCalled();
			expect(settingsStore.settings.servers).toHaveLength(2);
			expect(settingsStore.isLoaded).toBe(true);
		});

		it('falls back to defaults if file not found', async () => {
			mockReadTextFile.mockRejectedValue(new Error('File not found'));

			await settingsStore.init();

			expect(settingsStore.settings.servers).toHaveLength(0);
			expect(settingsStore.settings.closeAction).toBe('ask');
			expect(settingsStore.isLoaded).toBe(true);
		});

		it('merges loaded settings with defaults', async () => {
			// Settings with only some fields
			const partialSettings = {
				servers: [mockServer],
				closeAction: 'background'
			};
			mockReadTextFile.mockResolvedValue(JSON.stringify(partialSettings));

			await settingsStore.init();

			expect(settingsStore.settings.servers).toHaveLength(1);
			expect(settingsStore.settings.closeAction).toBe('background');
			// Should have defaults for missing fields
			expect(settingsStore.settings.startupServerId).toBeNull();
		});

		it('sets isLoaded=true on completion', async () => {
			mockReadTextFile.mockResolvedValue(JSON.stringify(mockSettings));

			expect(settingsStore.isLoaded).toBe(false);
			await settingsStore.init();
			expect(settingsStore.isLoaded).toBe(true);
		});
	});

	describe('server management', () => {
		beforeEach(async () => {
			mockReadTextFile.mockResolvedValue(JSON.stringify(mockSettings));
			mockWriteTextFile.mockResolvedValue(undefined);
			mockMkdir.mockResolvedValue(undefined);
			await settingsStore.init();
		});

		it('addServer() appends and saves', async () => {
			const newServer: SavedServer = {
				id: 'new-server',
				name: 'New Server',
				type: 'remote',
				url: 'https://new.example.com'
			};

			settingsStore.addServer(newServer);

			expect(settingsStore.settings.servers).toHaveLength(3);
			expect(settingsStore.getServer('new-server')).toEqual(newServer);
			// Wait for async save to complete
			await vi.waitFor(() => {
				expect(mockWriteTextFile).toHaveBeenCalled();
			});
		});

		it('updateServer() updates partial fields', async () => {
			settingsStore.updateServer('test-server', { name: 'Updated Name' });

			const server = settingsStore.getServer('test-server');
			expect(server?.name).toBe('Updated Name');
			expect(server?.url).toBe('https://test.example.com'); // Unchanged
			await vi.waitFor(() => {
				expect(mockWriteTextFile).toHaveBeenCalled();
			});
		});

		it('removeServer() removes and saves', async () => {
			expect(settingsStore.settings.servers).toHaveLength(2);

			settingsStore.removeServer('test-server');

			expect(settingsStore.settings.servers).toHaveLength(1);
			expect(settingsStore.getServer('test-server')).toBeUndefined();
			await vi.waitFor(() => {
				expect(mockWriteTextFile).toHaveBeenCalled();
			});
		});

		it('removeServer() clears startupServerId if removing startup server', async () => {
			settingsStore.setStartupServer('test-server');
			expect(settingsStore.settings.startupServerId).toBe('test-server');

			settingsStore.removeServer('test-server');

			expect(settingsStore.settings.startupServerId).toBeNull();
		});

		it('getServer() returns correct server', () => {
			const server = settingsStore.getServer('test-server');

			expect(server).toEqual(mockServer);
		});

		it('getServer() returns undefined for unknown id', () => {
			const server = settingsStore.getServer('nonexistent');

			expect(server).toBeUndefined();
		});

		it('upsertLocalServer() creates local server if not exists', async () => {
			// Remove existing local server first
			settingsStore.removeServer('local');
			vi.clearAllMocks();
			mockWriteTextFile.mockResolvedValue(undefined);
			mockMkdir.mockResolvedValue(undefined);

			settingsStore.upsertLocalServer('http://localhost:9090');

			const local = settingsStore.getServer('local');
			expect(local).toBeDefined();
			expect(local?.url).toBe('http://localhost:9090');
			expect(local?.type).toBe('local');
		});

		it('upsertLocalServer() updates existing local server', async () => {
			settingsStore.upsertLocalServer('http://localhost:9090');

			const local = settingsStore.getServer('local');
			expect(local?.url).toBe('http://localhost:9090');
		});
	});

	describe('settings persistence', () => {
		beforeEach(async () => {
			mockReadTextFile.mockResolvedValue(JSON.stringify(mockSettings));
			mockWriteTextFile.mockResolvedValue(undefined);
			mockMkdir.mockResolvedValue(undefined);
			await settingsStore.init();
		});

		it('save() writes to Tauri file', async () => {
			await settingsStore.save();

			expect(mockMkdir).toHaveBeenCalled();
			expect(mockWriteTextFile).toHaveBeenCalledWith(
				expect.stringContaining('settings.json'),
				expect.any(String)
			);
		});

		it('save() handles errors gracefully', async () => {
			mockWriteTextFile.mockRejectedValue(new Error('Write failed'));

			// Should not throw
			await expect(settingsStore.save()).resolves.not.toThrow();
		});

		it('setCloseAction() saves immediately', async () => {
			vi.clearAllMocks();
			mockWriteTextFile.mockResolvedValue(undefined);
			mockMkdir.mockResolvedValue(undefined);

			settingsStore.setCloseAction('shutdown');

			expect(settingsStore.settings.closeAction).toBe('shutdown');
			await vi.waitFor(() => {
				expect(mockWriteTextFile).toHaveBeenCalled();
			});
		});
	});

	describe('startup server', () => {
		beforeEach(async () => {
			mockReadTextFile.mockResolvedValue(JSON.stringify(mockSettings));
			mockWriteTextFile.mockResolvedValue(undefined);
			mockMkdir.mockResolvedValue(undefined);
			await settingsStore.init();
		});

		it('setStartupServer() updates and saves', async () => {
			vi.clearAllMocks();
			mockWriteTextFile.mockResolvedValue(undefined);
			mockMkdir.mockResolvedValue(undefined);

			settingsStore.setStartupServer('test-server');

			expect(settingsStore.settings.startupServerId).toBe('test-server');
			await vi.waitFor(() => {
				expect(mockWriteTextFile).toHaveBeenCalled();
			});
		});

		it('setStartupServer(null) clears startup server', async () => {
			settingsStore.setStartupServer('test-server');
			expect(settingsStore.settings.startupServerId).toBe('test-server');

			vi.clearAllMocks();
			mockWriteTextFile.mockResolvedValue(undefined);
			mockMkdir.mockResolvedValue(undefined);

			settingsStore.setStartupServer(null);

			expect(settingsStore.settings.startupServerId).toBeNull();
		});

		it('hasStartupServer getter works', () => {
			expect(settingsStore.hasStartupServer).toBe(false);

			settingsStore.setStartupServer('test-server');

			expect(settingsStore.hasStartupServer).toBe(true);
		});

		it('startupServer getter returns correct server', () => {
			settingsStore.setStartupServer('test-server');

			expect(settingsStore.startupServer).toEqual(mockServer);
		});

		it('startupServer getter returns undefined when not set', () => {
			expect(settingsStore.startupServer).toBeUndefined();
		});
	});

	describe('local daemon directories', () => {
		beforeEach(async () => {
			mockReadTextFile.mockResolvedValue(JSON.stringify(mockSettings));
			mockWriteTextFile.mockResolvedValue(undefined);
			mockMkdir.mockResolvedValue(undefined);
			await settingsStore.init();
		});

		it('setLocalDaemonDataDir() updates setting', () => {
			settingsStore.setLocalDaemonDataDir('/custom/data');

			expect(settingsStore.settings.localDaemonDataDir).toBe('/custom/data');
		});

		it('setLocalDaemonLibraryDir() updates setting', () => {
			settingsStore.setLocalDaemonLibraryDir('/custom/library');

			expect(settingsStore.settings.localDaemonLibraryDir).toBe('/custom/library');
		});
	});

	describe('localStorage fallback', () => {
		beforeEach(() => {
			resetStore();
			// Clear tauri mocks to simulate not being in Tauri
			vi.clearAllMocks();
		});

		it('falls back to localStorage when Tauri not available', async () => {
			// Simulate Tauri not being available by making import fail
			mockReadTextFile.mockRejectedValue(new Error('Module not found'));

			// Set up localStorage mock
			const localStorageData = JSON.stringify(mockSettings);
			const getItemSpy = vi.spyOn(Storage.prototype, 'getItem').mockReturnValue(localStorageData);

			await settingsStore.init();

			expect(settingsStore.isLoaded).toBe(true);
			getItemSpy.mockRestore();
		});
	});
});
