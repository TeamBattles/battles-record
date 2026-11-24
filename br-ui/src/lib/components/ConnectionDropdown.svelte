<script lang="ts">
	import { Popover } from 'bits-ui';
	import { Drawer } from 'vaul-svelte';
	import { Plug } from '@lucide/svelte';
	import { breakpointStore, extensionsStore } from '#lib';
	import { connectionStore } from '#lib/stores/connection.svelte.ts';
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
			const { api } = await import('#lib/api/client.ts');
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
		<Plug size={12} class="text-muted-foreground" />
	{/if}
	<span class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground whitespace-nowrap">
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
		<p class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">Pair Browser Extension</p>
		<p class="text-xs text-muted-foreground">No browser extensions paired. Generate a code to pair one.</p>
		{#if pairCode}
			<div class="rounded border border-line-2 bg-chip px-3 py-2 text-center">
				<p class="font-mono text-[10px] text-muted-foreground mb-1">PAIR CODE</p>
				<p class="font-mono text-lg tracking-widest text-green">{pairCode}</p>
				<p class="font-mono text-[9px] text-muted-foreground mt-1">Enter this code in the browser extension</p>
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
			class="flex items-center gap-1.5 hover:bg-hover-wash px-2 py-1 rounded transition-colors"
			onclick={() => (open = true)}
		>
			{@render triggerContent()}
		</button>

		<Drawer.Root bind:open>
			<Drawer.Portal>
				<Drawer.Overlay class="fixed inset-0 bg-black/60 z-40" />
				<Drawer.Content
					class="fixed bottom-0 left-0 right-0 bg-well border-t border-line-2 z-50 rounded-t-lg max-h-[80vh] overflow-y-auto"
				>
					<div class="flex justify-center py-3">
						<div class="w-10 h-1 bg-faint rounded-full"></div>
					</div>
					<div class="px-4 pb-2">
						<Drawer.Title class="font-mono text-xs uppercase tracking-wider text-muted-foreground">
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
						class="flex items-center gap-1.5 hover:bg-hover-wash px-2 py-1 rounded transition-colors"
					>
						{@render triggerContent()}
					</button>
				{/snippet}
			</Popover.Trigger>
			<Popover.Content
				side="top"
				sideOffset={8}
				align="end"
				class="z-50 w-72 max-h-96 overflow-y-auto rounded border border-line-2 bg-well shadow-lg"
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
