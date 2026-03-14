<script lang="ts">
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import AppShell from '$lib/components/AppShell.svelte';
	import {
		breakpointStore,
		themeStore,
		settingsStore,
		connectionStore,
		SetupWizard,
		AddServerModal,
		ReconnectOverlay,
		ToastContainer,
		LoadingScreen,
		SessionExpiredModal
	} from '$lib';
	import { versionStore } from '$lib/stores/version.svelte';
	import CloseConfirmationDialog from '$lib/components/CloseConfirmationDialog.svelte';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';

	let { children } = $props();

	let showWizard = $state(false);
	let showAddServerModal = $state(false);
	let isInitialized = $state(false);

	// Check if we need to show auth form in overlay (token expired for remote)
	const needsReauth = $derived.by(() => {
		if (connectionStore.connectionState !== 'disconnected') return false;
		const server = connectionStore.activeServer;
		if (!server || server.type !== 'remote') return false;
		// Token expired or missing
		if (!server.token) return true;
		if (server.tokenExpiry && Date.now() > server.tokenExpiry) return true;
		return false;
	});

	let cleanupBreakpoint: (() => void) | undefined;
	let cleanupTheme: (() => void) | undefined;
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let eruda: any = null;

	async function toggleDebugConsole(forceShow?: boolean) {
		if (!eruda) {
			const mod = await import('eruda');
			eruda = mod.default ?? mod;
			eruda.init();
		} else {
			if (forceShow === true) {
				eruda.show();
			} else if (forceShow === false) {
				eruda.hide();
			} else {
				const el = document.getElementById('eruda');
				if (el?.style.display === 'none') {
					eruda.show();
				} else {
					eruda.hide();
				}
			}
		}
	}

	// React to settings toggle changes
	$effect(() => {
		const enabled = settingsStore.settings.debugConsole;
		if (!isInitialized) return;
		if (enabled) {
			toggleDebugConsole(true);
		} else if (eruda) {
			eruda.destroy();
			eruda = null;
		}
	});

	function handleKeydown(e: KeyboardEvent) {
		if (e.ctrlKey && e.shiftKey && e.key === 'D') {
			e.preventDefault();
			// Toggle the setting, which triggers the $effect above
			settingsStore.settings.debugConsole = !settingsStore.settings.debugConsole;
			settingsStore.save();
		}
	}

	onMount(() => {
		cleanupBreakpoint = breakpointStore.init();
		cleanupTheme = themeStore.init();

		// Initialize async
		initializeApp();

		return () => {
			cleanupBreakpoint?.();
			cleanupTheme?.();
			versionStore.destroy();
			// Clean up eruda
			if (eruda) {
				eruda.destroy();
				eruda = null;
			}
		};
	});

	async function initializeApp() {
		await settingsStore.init();
		versionStore.init();

		// Auto-init debug console if previously enabled
		if (settingsStore.settings.debugConsole) {
			toggleDebugConsole();
		}

		// Determine what to do on startup
		if (settingsStore.hasStartupServer) {
			const startupServer = settingsStore.startupServer;
			let success = false;

			if (startupServer?.type === 'local') {
				// For local server, use connectToLocal which starts the daemon
				success = await connectionStore.connectToLocal();
			} else {
				// For remote servers, connect directly
				success = await connectionStore.connectToServer(settingsStore.settings.startupServerId!);
			}

			if (!success) {
				// Connection failed - could be token expired or server down
				// Let the reconnect overlay handle it
			}
		} else {
			// No startup server - show wizard
			showWizard = true;
		}

		isInitialized = true;
	}

	function handleWizardComplete() {
		showWizard = false;
	}

	function handleAddServer() {
		showAddServerModal = true;
	}

	function handleManageServers() {
		goto('/settings');
	}

	function handleSwitchServer() {
		// Just open the add server modal for now, or they can use the dropdown
		showAddServerModal = true;
	}

	function handleReconnect() {
		if (connectionStore.activeServerId) {
			connectionStore.connectToServer(connectionStore.activeServerId);
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

{#if showWizard}
	<SetupWizard onComplete={handleWizardComplete} />
{:else if isInitialized}
	<AppShell onAddServer={handleAddServer} onManageServers={handleManageServers}>
		{#if connectionStore.connectionState === 'disconnected' && connectionStore.activeServerId}
			<ReconnectOverlay
				onSwitchServer={handleSwitchServer}
				onReconnect={handleReconnect}
				showAuthForm={needsReauth}
			/>
		{/if}
		{@render children()}
	</AppShell>
{:else}
	<LoadingScreen status="Starting service..." />
{/if}

<AddServerModal open={showAddServerModal} onOpenChange={(v) => (showAddServerModal = v)} />

<CloseConfirmationDialog />

<SessionExpiredModal />

<ToastContainer />
