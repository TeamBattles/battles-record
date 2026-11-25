<script lang="ts">
	import { Popover } from 'bits-ui';
	import { Drawer } from 'vaul-svelte';
	import { Plug, Settings } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import { breakpointStore } from '#lib';
	import { extensionGatewayStore } from '#lib/stores/extensionGateway.svelte.ts';
	import StatusDot from './ui/StatusDot.svelte';
	import ExtensionGatewayStatus from './ExtensionGatewayStatus.svelte';

	let open = $state(false);

	const status = $derived(extensionGatewayStore.status);
	const browsers = $derived(status?.browsers ?? []);
	const connectedCount = $derived(browsers.filter((b) => b.connected).length);

	const displayText = $derived.by(() => {
		if (browsers.length === 0) return 'No Extensions';
		if (browsers.length === 1) return browsers[0].identifier;
		return `${browsers.length} Browsers`;
	});

	const dotStatus = $derived.by<'success' | 'warning' | 'offline'>(() => {
		if (connectedCount === 0) return 'offline';
		return connectedCount === browsers.length ? 'success' : 'warning';
	});
</script>

{#snippet triggerContent()}
	{#if browsers.length > 0}
		<StatusDot status={dotStatus} size="sm" />
	{:else}
		<Plug size={12} class="text-muted-foreground" />
	{/if}
	<span class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground whitespace-nowrap">
		<span class="hidden sm:inline">{displayText}</span>
		<span class="sm:hidden">{browsers.length > 1 ? `${browsers.length} Ext` : 'Ext'}</span>
	</span>
{/snippet}

{#snippet panelContent()}
	<div class="space-y-3 p-3">
		<p class="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">Battles Replay</p>
		<ExtensionGatewayStatus showHelp={browsers.length === 0} />
		{#if browsers.length > 0}
			<ul class="space-y-1.5 border-t border-border pt-3">
				{#each browsers as browser (browser.clientId)}
					<li class="flex items-center gap-2">
						<StatusDot status={browser.connected ? 'success' : 'offline'} size="sm" />
						<span class="min-w-0 flex-1 truncate font-mono text-xs text-foreground">
							{browser.identifier}
						</span>
						<span class="font-mono text-[10px] text-muted-foreground">
							{browser.connected ? 'Connected' : 'Offline'}
						</span>
					</li>
				{/each}
			</ul>
		{/if}
		<a
			href={resolve('/settings')}
			class="flex items-center gap-1.5 rounded px-2 py-1 font-mono text-[10px] uppercase tracking-wider text-primary transition-colors hover:bg-hover-wash focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none"
			onclick={() => (open = false)}
		>
			<Settings size={12} />
			Browser extension settings
		</a>
	</div>
{/snippet}

{#if status}
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
					<Drawer.Title class="sr-only">Extensions</Drawer.Title>
					{@render panelContent()}
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
				{@render panelContent()}
			</Popover.Content>
		</Popover.Root>
	{/if}
{/if}
