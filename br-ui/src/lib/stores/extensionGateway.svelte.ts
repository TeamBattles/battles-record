import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { GatewayPromptEvent, GatewayStatus, MessageLogEntry } from '#lib/api/types.ts';
import { settingsStore } from '#lib/stores/settings.svelte.ts';
import { connectionStore } from '#lib/stores/connection.svelte.ts';
import { toastStore } from '#lib/stores/toast.svelte.ts';
import { extractErrorMessage } from '#lib/utils/errors.ts';

type Answer = 'allow' | 'deny' | 'failed';

/**
 * The desktop app's front door for Battles Replay, as the UI sees it. Status and the approved
 * browsers always describe this PC's local service, whichever server the app is viewing.
 *
 * Prompts that need the local service are answered only after this store has started it, because
 * the shell does not know the user's chosen directories. Autostart covers known browsers only; a
 * new browser always waits for a click.
 */
class ExtensionGatewayStore {
	status = $state<GatewayStatus | null>(null);
	pending = $state<GatewayPromptEvent | null>(null);
	isResponding = $state(false);

	init(): () => void {
		// listen() rejects outside Tauri (web dev mode); there is no gateway to hear from there.
		const unlisteners = [
			listen<GatewayPromptEvent>('extension-request', (event) => {
				void this.handleRequest(event.payload);
			}).catch(() => null),
			listen('extension-gateway-changed', () => {
				void this.refresh();
			}).catch(() => null)
		];
		void this.refresh();
		void this.pickUpPendingRequest();
		return () => {
			for (const unlisten of unlisteners) {
				void unlisten.then((fn) => fn?.());
			}
		};
	}

	async refresh() {
		try {
			this.status = await invoke<GatewayStatus>('get_extension_gateway_status');
		} catch {
			// Not running inside Tauri (web dev mode)
		}
	}

	async respond(decision: 'allow' | 'deny', alwaysStart = false) {
		const request = this.pending;
		if (!request) return;
		this.isResponding = true;
		try {
			if (decision === 'allow' && alwaysStart) {
				settingsStore.settings.autoStartForExtension = true;
				void settingsStore.save();
			}
			if (decision === 'allow' && request.kind !== 'allow') {
				await this.startAndAnswer(request.requestId);
			} else {
				await this.answer(request.requestId, decision);
			}
		} finally {
			// The shell raises the next queued prompt as soon as this one is answered.
			if (this.pending === request) this.pending = null;
			this.isResponding = false;
		}
	}

	async removeBrowser(clientId: string) {
		try {
			await invoke('remove_extension_browser', { clientId });
		} catch (e) {
			toastStore.error(extractErrorMessage(e, 'Failed to remove browser'));
		}
		await this.refresh();
	}

	getMessageLog(clientId: string): Promise<MessageLogEntry[]> {
		return invoke<MessageLogEntry[]>('get_extension_message_log', { clientId });
	}

	/** A prompt raised before the listener existed (app launch) is never delivered as an event. */
	private async pickUpPendingRequest() {
		try {
			const request = await invoke<GatewayPromptEvent | null>('get_pending_extension_request');
			if (request && this.pending?.requestId !== request.requestId) {
				await this.handleRequest(request);
			}
		} catch {
			// Not running inside Tauri (web dev mode)
		}
	}

	private async handleRequest(request: GatewayPromptEvent) {
		// Before settings load the saved preference is unknown, so ask rather than guess. A service
		// that is already starting (the app's own startup) needs no question either.
		const autoStart =
			request.kind === 'start' &&
			((settingsStore.isLoaded && settingsStore.settings.autoStartForExtension) ||
				(await this.isServiceStarting()));
		if (autoStart) {
			await this.startAndAnswer(request.requestId);
			return;
		}
		this.pending = request;
	}

	private async isServiceStarting(): Promise<boolean> {
		try {
			const [status] = await invoke<[string, number | null]>('get_daemon_status');
			return status === 'Starting' || status === 'Running';
		} catch {
			return false;
		}
	}

	private async startAndAnswer(requestId: string) {
		try {
			await invoke<number>('start_local_daemon', {
				dataDir: settingsStore.settings.localDaemonDataDir,
				libraryDir: settingsStore.settings.localDaemonLibraryDir,
				downloadsDir: settingsStore.settings.localDaemonDownloadsDir
			});
		} catch (e) {
			await this.answer(requestId, 'failed', extractErrorMessage(e, 'Failed to start local service'));
			return;
		}
		await this.answer(requestId, 'allow');
		if (connectionStore.activeServer?.type !== 'local') {
			toastStore.success('Local service started for Battles Replay', {
				label: 'View',
				onClick: () => void connectionStore.connectToLocal()
			});
		}
	}

	private async answer(requestId: string, decision: Answer, reason: string | null = null) {
		try {
			await invoke('respond_extension_request', { requestId, decision, reason });
		} catch (e) {
			console.error('Failed to answer extension request:', e);
		}
	}
}

export const extensionGatewayStore = new ExtensionGatewayStore();
