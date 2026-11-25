import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { WebSocketEvent } from '#lib/api/websocket.ts';

const ws = vi.hoisted(() => ({ emit: null as ((event: WebSocketEvent) => void) | null }));

vi.mock('#lib/api/websocket.ts', () => ({
	wsClient: {
		subscribe: vi.fn((handler: (event: WebSocketEvent) => void) => {
			ws.emit = handler;
			return () => {};
		})
	}
}));

vi.mock('#lib/api/client.ts', () => ({
	api: { getLibraryStatus: vi.fn() }
}));

import { api } from '#lib/api/client.ts';
import { extensionsStore } from './extensions.svelte.ts';

const notInstalled = { installed: false };

describe('extensionsStore installs started elsewhere', () => {
	beforeEach(() => {
		vi.mocked(api.getLibraryStatus).mockResolvedValue({
			ytdlp: notInstalled,
			ffmpeg: notInstalled,
			bun: notInstalled
		});
		extensionsStore.libraryStatus = { ytdlp: notInstalled, ffmpeg: notInstalled, bun: notInstalled };
		ws.emit?.({ type: 'library_install_failed', library: 'all', code: 'download_failed' });
		vi.clearAllMocks();
	});

	it('shows a library as installing until its new status arrives', () => {
		ws.emit?.({ type: 'library_install_started', library: 'ytdlp' });
		expect(extensionsStore.installing.has('ytdlp')).toBe(true);

		ws.emit?.({ type: 'library_status_changed', library: 'ytdlp', installed: true, version: '2026.11.01' });
		expect(extensionsStore.installing.has('ytdlp')).toBe(false);
	});

	it('clears every pending install when one fails, then reloads the real status', () => {
		ws.emit?.({ type: 'library_install_started', library: 'ytdlp' });
		ws.emit?.({ type: 'library_install_started', library: 'ffmpeg' });

		ws.emit?.({ type: 'library_install_failed', library: 'ffmpeg', code: 'download_failed' });

		expect(extensionsStore.installing.size).toBe(0);
		expect(api.getLibraryStatus).toHaveBeenCalled();
	});
});
