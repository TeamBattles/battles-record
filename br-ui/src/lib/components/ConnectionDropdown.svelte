<script lang="ts">
	import { Popover } from 'bits-ui';
	import { Drawer } from 'vaul-svelte';
	import { Plug } from 'lucide-svelte';
	import { breakpointStore, extensionsStore } from '$lib';
	import { connectionStore } from '$lib/stores/connection.svelte';
	import StatusDot from './ui/StatusDot.svelte';
	import Button from './ui/Button.svelte';
	import ConnectionPanelContent from './ConnectionPanelContent.svelte';

	let open = $state(false);
	let pairCode = $state<string | null>(null);
	let isGeneratingCode = $state(false);

	const connections = $derived(extensionsStore.connections);
	const connectedCount = $derived(extensionsStore.connectedCount);
	const totalPaired = $derived(extensionsStore.totalPaired);

	const hasConnections = $derived(totalPaired > 0);
	const isLoaded = $derived(extensionsStore.config !== null);
	const isConnected = $derived(connectionStore.isConnected);
	const isSingle = $derived(totalPaired === 1);
	const singleConnection = $derived(isSingle ? connections[0] : null);

	const displayText = $derived.by(() => {
		if (!hasConnections) return 'No Extensions';
		if (isSingle && singleConnection) {
			return singleConnection.identifier;
		}
		return `${totalPaired} Connection${totalPaired !== 1 ? 's' : ''}`;
	});

	const dotStatus = $derived.by<'success' | 'warning' | 'error'>(() => {
		if (!hasConnections) return 'warning';
		if (isSingle && singleConnection) {
			return singleConnection.connected ? 'success' : 'error';
		}
		if (connectedCount === totalPaired) return 'success';
		if (connectedCount > 0) return 'warning';
		return 'error';
	});

	function close() {
		open = false;
		pairCode = null;
	}

	async function generatePairCode() {
		if (isGeneratingCode) return;
		isGeneratingCode = true;
		try {
			const { api } = await import('$lib/api/client');
			const result = await api.generatePairCode();
			pairCode = result.code;
		} catch (e) {
			console.error('Failed to generate pair code:', e);
		} finally {
			isGeneratingCode = false;
		}
	}
</script>

{#snippet triggerContent()}
	{#if hasConnections}
		<StatusDot status={dotStatus} size="sm" />
	{:else}
		<Plug size={12} class="text-zinc-500" />
	{/if}
	<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 whitespace-nowrap">
		{#if hasConnections}
			<span class="hidden sm:inline">{displayText}</span>
			<span class="sm:hidden">{totalPaired > 1 ? `${totalPaired} Ext` : 'Ext'}</span>
		{:else}
			<span class="hidden sm:inline">No Extensions</span>
			<span class="sm:hidden">Ext</span>
		{/if}
	</span>
{/snippet}

{#snippet noPairedContent()}
	<div class="p-3 space-y-3">
		<p class="font-mono text-[10px] uppercase tracking-wider text-zinc-400">Pair Browser Extension</p>
		<p class="text-xs text-zinc-400">No browser extensions paired. Generate a code to pair one.</p>
		{#if pairCode}
			<div class="rounded border border-zinc-700 bg-zinc-800 px-3 py-2 text-center">
				<p class="font-mono text-[10px] text-zinc-500 mb-1">PAIR CODE</p>
				<p class="font-mono text-lg tracking-widest text-emerald-400">{pairCode}</p>
				<p class="font-mono text-[9px] text-zinc-500 mt-1">Enter this code in the browser extension</p>
			</div>
			<Button
				intent="secondary"
				size="sm"
				fullWidth={true}
				class="font-mono"
				disabled={isGeneratingCode}
				onclick={generatePairCode}
			>
				{#snippet children()}
					{isGeneratingCode ? 'Generating...' : 'Regenerate Code'}
				{/snippet}
			</Button>
		{:else}
			<Button
				intent="primary"
				size="sm"
				fullWidth={true}
				class="font-mono"
				disabled={isGeneratingCode}
				onclick={generatePairCode}
			>
				{#snippet children()}
					{isGeneratingCode ? 'Generating...' : 'Generate Pair Code'}
				{/snippet}
			</Button>
		{/if}
	</div>
{/snippet}

{#if isConnected && isLoaded}
	{#if breakpointStore.isMobile}
		<button
			class="flex items-center gap-1.5 hover:bg-muted px-2 py-1 rounded transition-colors"
			onclick={() => (open = true)}
		>
			{@render triggerContent()}
		</button>

		<Drawer.Root bind:open>
			<Drawer.Portal>
				<Drawer.Overlay class="fixed inset-0 bg-black/60 z-40" />
				<Drawer.Content
					class="fixed bottom-0 left-0 right-0 bg-zinc-900 border-t border-zinc-700 z-50 rounded-t-lg max-h-[80vh] overflow-y-auto"
				>
					<div class="flex justify-center py-3">
						<div class="w-10 h-1 bg-zinc-600 rounded-full"></div>
					</div>
					<div class="px-4 pb-2">
						<Drawer.Title class="font-mono text-xs uppercase tracking-wider text-zinc-400">
							Extensions
						</Drawer.Title>
					</div>
					{#if hasConnections}
						<ConnectionPanelContent onClose={close} {pairCode} {isGeneratingCode} onGeneratePairCode={generatePairCode} />
					{:else}
						{@render noPairedContent()}
					{/if}
					<div class="h-4"></div>
				</Drawer.Content>
			</Drawer.Portal>
		</Drawer.Root>
	{:else}
		<Popover.Root bind:open>
			<Popover.Trigger>
				{#snippet child({ props })}
					<button
						{...props}
						class="flex items-center gap-1.5 hover:bg-muted px-2 py-1 rounded transition-colors"
					>
						{@render triggerContent()}
					</button>
				{/snippet}
			</Popover.Trigger>
			<Popover.Content
				side="top"
				sideOffset={8}
				align="end"
				class="z-50 w-72 max-h-96 overflow-y-auto rounded border border-zinc-700 bg-zinc-900 shadow-lg"
			>
				{#if hasConnections}
					<ConnectionPanelContent onClose={close} {pairCode} {isGeneratingCode} onGeneratePairCode={generatePairCode} />
				{:else}
					{@render noPairedContent()}
				{/if}
			</Popover.Content>
		</Popover.Root>
	{/if}
{/if}
