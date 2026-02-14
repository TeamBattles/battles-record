<script lang="ts">
	import { DropdownMenu } from 'bits-ui';
	import { Drawer } from 'vaul-svelte';
	import { ChevronDown, Plus, Settings, Check, Loader2 } from 'lucide-svelte';
	import { breakpointStore, settingsStore, connectionStore } from '$lib';
	import type { SavedServer } from '$lib';
	import LocalServiceOfflineDialog from './LocalServiceOfflineDialog.svelte';

	interface Props {
		onAddServer: () => void;
		onManageServers: () => void;
		compact?: boolean;
	}

	let { onAddServer, onManageServers, compact = false }: Props = $props();

	let open = $state(false);
	let showLocalOfflineDialog = $state(false);

	// Track health status of non-active servers
	// 'unknown' = not checked, 'checking' = in progress, 'healthy' = reachable, 'unreachable' = failed
	let serverHealth = $state<Record<string, 'unknown' | 'checking' | 'healthy' | 'unreachable'>>({});

	// Track local daemon running state separately
	let localDaemonRunning = $state<boolean | null>(null);

	// Check health of all non-active servers when dropdown opens
	$effect(() => {
		if (open) {
			checkAllServersHealth();
			checkLocalDaemonStatus();
		}
	});

	// Update localDaemonRunning when connection state changes
	// This ensures the trigger button reflects the correct state after starting service via dialog
	$effect(() => {
		const activeServer = connectionStore.activeServer;
		const state = connectionStore.connectionState;

		if (activeServer?.type === 'local') {
			if (state === 'connected') {
				localDaemonRunning = true;
			} else if (state === 'disconnected') {
				// Re-check daemon status when disconnected to see if it's still running
				checkLocalDaemonStatus();
			}
		}
	});

	async function checkLocalDaemonStatus() {
		try {
			localDaemonRunning = await connectionStore.isDaemonRunning();
		} catch {
			localDaemonRunning = false;
		}
	}

	async function checkServerHealth(server: SavedServer): Promise<boolean> {
		try {
			const response = await fetch(`${server.url}/health`, {
				method: 'GET',
				signal: AbortSignal.timeout(3000) // 3 second timeout
			});
			return response.ok;
		} catch {
			return false;
		}
	}

	async function checkAllServersHealth() {
		for (const server of settingsStore.settings.servers) {
			// Skip the active server - we already know its status
			if (server.id === connectionStore.activeServerId) continue;

			serverHealth[server.id] = 'checking';

			const isHealthy = await checkServerHealth(server);
			serverHealth[server.id] = isHealthy ? 'healthy' : 'unreachable';
		}
	}

	function getServerStatusColor(server: SavedServer): string {
		// Special handling for local server
		if (server.type === 'local') {
			// If daemon definitely not running, always show red
			if (localDaemonRunning === false) {
				return 'bg-red-400';
			}

			if (server.id === connectionStore.activeServerId) {
				// Active local server - use connection state
				switch (connectionStore.connectionState) {
					case 'connected':
						return 'bg-emerald-400';
					case 'connecting':
					case 'reconnecting':
						// Only show yellow if daemon is actually running
						return localDaemonRunning ? 'bg-amber-400' : 'bg-red-400';
					default:
						return 'bg-red-400';
				}
			}

			// Non-active local server with daemon running
			return localDaemonRunning ? 'bg-emerald-400' : 'bg-red-400';
		}

		if (server.id === connectionStore.activeServerId) {
			// Active remote server uses connection state
			switch (connectionStore.connectionState) {
				case 'connected':
					return 'bg-emerald-400';
				case 'connecting':
				case 'reconnecting':
					return 'bg-amber-400';
				default:
					return 'bg-red-400';
			}
		}

		// Non-active remote servers use health check status
		const health = serverHealth[server.id];
		switch (health) {
			case 'healthy':
				return 'bg-emerald-400';
			case 'unreachable':
				return 'bg-red-400';
			case 'checking':
				return 'bg-amber-400';
			default:
				return 'bg-zinc-600';
		}
	}

	function isServerAnimating(server: SavedServer): boolean {
		if (server.id === connectionStore.activeServerId) {
			// For local server, don't animate if daemon is not running
			if (server.type === 'local' && localDaemonRunning === false) {
				return false;
			}
			return (
				connectionStore.connectionState === 'connecting' ||
				connectionStore.connectionState === 'reconnecting'
			);
		}
		return serverHealth[server.id] === 'checking';
	}

	// Status color for the trigger button (shows active server status)
	const triggerStatusColor = $derived.by(() => {
		const activeServer = connectionStore.activeServer;

		// If local server is active, check daemon status
		if (activeServer?.type === 'local' && localDaemonRunning === false) {
			return 'bg-red-400';
		}

		switch (connectionStore.connectionState) {
			case 'connected':
				return 'bg-emerald-400';
			case 'connecting':
			case 'reconnecting':
				// For local server, only show yellow if daemon is running
				if (activeServer?.type === 'local' && localDaemonRunning === false) {
					return 'bg-red-400';
				}
				return 'bg-amber-400';
			default:
				return 'bg-red-400';
		}
	});

	const triggerAnimating = $derived.by(() => {
		const activeServer = connectionStore.activeServer;

		// For local server, don't animate if daemon is not running
		if (activeServer?.type === 'local' && localDaemonRunning === false) {
			return false;
		}

		return (
			connectionStore.connectionState === 'connecting' ||
			connectionStore.connectionState === 'reconnecting'
		);
	});

	async function handleServerClick(server: SavedServer) {
		if (server.id === connectionStore.activeServerId) return;

		// Special handling for local server
		if (server.type === 'local') {
			const isRunning = await connectionStore.isDaemonRunning();
			if (!isRunning) {
				open = false; // Close dropdown
				showLocalOfflineDialog = true;
				return;
			}
		}

		open = false;
		await connectionStore.switchServer(server.id);
	}

	function handleAddServer() {
		open = false;
		onAddServer();
	}

	function handleManageServers() {
		open = false;
		onManageServers();
	}
</script>

{#snippet serverList()}
	<div class="py-1">
		{#each settingsStore.settings.servers as server (server.id)}
			<button
				class="w-full flex items-center gap-3 px-3 py-2 hover:bg-zinc-800 transition-colors text-left"
				onclick={() => handleServerClick(server)}
			>
				<span
					class="size-2 rounded-full {getServerStatusColor(server)}"
					class:animate-pulse={isServerAnimating(server)}
				></span>
				<span class="flex-1 font-mono text-sm text-zinc-300">{server.name}</span>
				{#if server.id === connectionStore.activeServerId}
					<Check size={14} class="text-emerald-400" />
				{/if}
			</button>
		{/each}

		{#if settingsStore.settings.servers.length === 0}
			<p class="px-3 py-2 font-mono text-xs text-zinc-500">No servers configured</p>
		{/if}
	</div>

	<div class="border-t border-zinc-700 py-1">
		<button
			class="w-full flex items-center gap-3 px-3 py-2 hover:bg-zinc-800 transition-colors text-left"
			onclick={handleAddServer}
		>
			<Plus size={14} class="text-zinc-500" />
			<span class="font-mono text-sm text-zinc-400">Add Remote Server</span>
		</button>
		<button
			class="w-full flex items-center gap-3 px-3 py-2 hover:bg-zinc-800 transition-colors text-left"
			onclick={handleManageServers}
		>
			<Settings size={14} class="text-zinc-500" />
			<span class="font-mono text-sm text-zinc-400">Manage Servers</span>
		</button>
	</div>
{/snippet}

{#if breakpointStore.isMobile}
	<!-- Mobile: Bottom Sheet -->
	<button
		class="flex items-center gap-2 px-3 py-1.5 hover:bg-muted rounded transition-colors {compact ? '' : 'border border-border bg-input'}"
		onclick={() => (open = true)}
	>
		{#if !compact}
			{#if triggerAnimating}
				<Loader2 size={12} class="text-amber-400 animate-spin" />
			{:else}
				<span class="size-2 rounded-full {triggerStatusColor}"></span>
			{/if}
		{/if}
		<span class="font-mono text-xs text-zinc-400">
			{connectionStore.activeServer?.name ?? 'Select'}
		</span>
		<ChevronDown size={12} class="text-zinc-500" />
	</button>

	<Drawer.Root bind:open>
		<Drawer.Portal>
			<Drawer.Overlay class="fixed inset-0 bg-black/60 z-40" />
			<Drawer.Content
				class="fixed bottom-0 left-0 right-0 bg-zinc-900 border-t border-zinc-700 z-50 rounded-t-lg"
			>
				<div class="flex justify-center py-3">
					<div class="w-10 h-1 bg-zinc-600 rounded-full"></div>
				</div>
				<div class="px-4 pb-2">
					<Drawer.Title class="font-mono text-xs uppercase tracking-wider text-zinc-400">
						Servers
					</Drawer.Title>
				</div>
				{@render serverList()}
				<div class="h-4"></div>
			</Drawer.Content>
		</Drawer.Portal>
	</Drawer.Root>
{:else}
	<!-- Desktop: Dropdown -->
	<DropdownMenu.Root bind:open>
		<DropdownMenu.Trigger>
			{#snippet child({ props })}
				<button
					{...props}
					class="flex items-center gap-2 px-3 py-1.5 hover:bg-muted rounded transition-colors {compact ? '' : 'border border-border bg-input'}"
				>
					<span class="font-mono text-xs text-zinc-400">
						{connectionStore.activeServer?.name ?? 'Not Connected'}
					</span>
					{#if !compact}
						{#if triggerAnimating}
							<Loader2 size={12} class="text-amber-400 animate-spin" />
						{:else}
							<span class="size-2 rounded-full {triggerStatusColor}"></span>
						{/if}
					{/if}
					<ChevronDown size={12} class="text-zinc-500" />
				</button>
			{/snippet}
		</DropdownMenu.Trigger>

		<DropdownMenu.Content
			class="z-50 min-w-[200px] rounded border border-zinc-700 bg-zinc-900 shadow-lg"
			sideOffset={4}
			align="start"
		>
			{@render serverList()}
		</DropdownMenu.Content>
	</DropdownMenu.Root>
{/if}

<!-- Local Service Offline Dialog -->
<LocalServiceOfflineDialog
	open={showLocalOfflineDialog}
	onOpenChange={(value) => (showLocalOfflineDialog = value)}
/>
