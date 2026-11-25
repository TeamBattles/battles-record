import { describe, it, expect, vi, beforeEach } from 'vitest';

const handlers: Record<string, (event: { payload: unknown }) => void> = {};

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn((name: string, handler: (event: { payload: unknown }) => void) => {
		handlers[name] = handler;
		return Promise.resolve(() => {});
	})
}));
vi.mock('#lib/stores/settings.svelte.ts', () => ({
	settingsStore: {
		isLoaded: true,
		settings: {
			autoStartForExtension: false,
			localDaemonDataDir: null,
			localDaemonLibraryDir: null,
			localDaemonDownloadsDir: null
		},
		save: vi.fn()
	}
}));
vi.mock('#lib/stores/connection.svelte.ts', () => ({
	connectionStore: { activeServer: { type: 'remote' }, connectToLocal: vi.fn() }
}));
vi.mock('#lib/stores/toast.svelte.ts', () => ({
	toastStore: { success: vi.fn(), error: vi.fn() }
}));

import { invoke } from '@tauri-apps/api/core';
import { settingsStore } from '#lib/stores/settings.svelte.ts';
import { extensionGatewayStore } from './extensionGateway.svelte';

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

function emitRequest(kind: 'start' | 'allow' | 'start_and_allow') {
	handlers['extension-request']({
		payload: { requestId: 'r1', kind, identifier: 'Chrome on Linux', origin: null }
	});
}

async function settle() {
	for (let i = 0; i < 5; i++) await Promise.resolve();
}

function calls(command: string) {
	return invokeMock.mock.calls.filter(([name]) => name === command);
}

describe('extensionGatewayStore', () => {
	beforeEach(() => {
		invokeMock.mockReset();
		invokeMock.mockResolvedValue(undefined);
		settingsStore.isLoaded = true;
		settingsStore.settings.autoStartForExtension = false;
		extensionGatewayStore.pending = null;
		extensionGatewayStore.init();
	});

	it('auto-starts a known browser silently when the setting is on', async () => {
		settingsStore.settings.autoStartForExtension = true;
		emitRequest('start');
		await settle();
		expect(calls('start_local_daemon')).toHaveLength(1);
		expect(invokeMock).toHaveBeenCalledWith('respond_extension_request', {
			requestId: 'r1',
			decision: 'allow',
			reason: null
		});
		expect(extensionGatewayStore.pending).toBeNull();
	});

	it('asks before approving a new browser even with autostart on', async () => {
		settingsStore.settings.autoStartForExtension = true;
		emitRequest('start_and_allow');
		await settle();
		expect(extensionGatewayStore.pending?.requestId).toBe('r1');
		expect(calls('start_local_daemon')).toHaveLength(0);
		expect(calls('respond_extension_request')).toHaveLength(0);
	});

	it('shows the dialog when settings are not loaded yet', async () => {
		settingsStore.isLoaded = false;
		settingsStore.settings.autoStartForExtension = true;
		emitRequest('start');
		await settle();
		expect(extensionGatewayStore.pending?.kind).toBe('start');
		expect(calls('start_local_daemon')).toHaveLength(0);
	});

	it('keeps a prompt that arrives while the previous one is being answered', async () => {
		emitRequest('allow');
		await settle();
		invokeMock.mockImplementation(async (command: string) => {
			if (command === 'respond_extension_request') {
				handlers['extension-request']({
					payload: { requestId: 'r2', kind: 'allow', identifier: 'Firefox on Linux', origin: null }
				});
			}
		});
		await extensionGatewayStore.respond('allow');
		expect(extensionGatewayStore.pending?.requestId).toBe('r2');
	});

	it('picks up a prompt that was raised before it was listening', async () => {
		invokeMock.mockImplementation(async (command: string) => {
			if (command === 'get_pending_extension_request') {
				return { requestId: 'early', kind: 'allow', identifier: 'Chrome on Linux', origin: null };
			}
		});
		extensionGatewayStore.init();
		await settle();
		expect(extensionGatewayStore.pending?.requestId).toBe('early');
	});

	it('continues without asking when the service is already starting', async () => {
		invokeMock.mockImplementation(async (command: string) => {
			if (command === 'get_daemon_status') return ['Starting', 8080];
		});
		emitRequest('start');
		await settle();
		expect(extensionGatewayStore.pending).toBeNull();
		expect(calls('start_local_daemon')).toHaveLength(1);
		expect(invokeMock).toHaveBeenCalledWith('respond_extension_request', {
			requestId: 'r1',
			decision: 'allow',
			reason: null
		});
	});

	it('reports a failed start instead of allowing', async () => {
		emitRequest('start');
		await settle();
		invokeMock.mockImplementation(async (command: string) => {
			if (command === 'start_local_daemon') throw new Error('boom');
		});
		await extensionGatewayStore.respond('allow');
		const [, args] = calls('respond_extension_request')[0];
		expect(args.decision).toBe('failed');
		expect(args.reason).toContain('boom');
		expect(extensionGatewayStore.pending).toBeNull();
	});
});
