<script lang="ts">
	import { untrack } from 'svelte';
	import { HardDrive, Loader2 } from 'lucide-svelte';
	import { connectionStore } from '$lib/stores/connection.svelte';
	import { channelsStore } from '$lib/stores/channels.svelte';
	import { storageStore } from '$lib/stores/storage.svelte';
	import { extensionsStore } from '$lib/stores/extensions.svelte';
	import ExtensionStatusBar from './ExtensionStatusBar.svelte';

	// Load storage and extension data when connected, and refresh periodically
	$effect(() => {
		if (connectionStore.connectionState !== 'connected') return;

		// Initial load
		untrack(() => {
			storageStore.load();
			extensionsStore.load();
		});

		// Refresh every 60 seconds
		const interval = setInterval(() => {
			storageStore.load();
		}, 60000);

		return () => clearInterval(interval);
	});

	// Computed: count of active recordings from channels (always loaded)
	const activeRecordings = $derived(
		channelsStore.channels.filter((c) => c.status?.is_recording).length
	);

	// Computed: disk usage percent from storage store
	const diskUsagePercent = $derived(storageStore.diskUsedPercent);

	const statusColor = $derived.by(() => {
		switch (connectionStore.connectionState) {
			case 'connected':
				return 'bg-emerald-400';
			case 'connecting':
			case 'reconnecting':
				return 'bg-amber-400';
			default:
				return 'bg-red-400';
		}
	});

	const statusText = $derived.by(() => {
		switch (connectionStore.connectionState) {
			case 'connected':
				return { full: 'Connected', short: 'ON' };
			case 'connecting':
				return { full: 'Connecting', short: '...' };
			case 'reconnecting':
				return { full: 'Reconnecting', short: '...' };
			default:
				return { full: 'Disconnected', short: 'OFF' };
		}
	});

	const isAnimating = $derived(
		connectionStore.connectionState === 'connecting' ||
			connectionStore.connectionState === 'reconnecting'
	);
</script>

<footer class="h-8 bg-card border-t border-border flex items-center px-4">
	<!-- Left: existing status indicators -->
	<div class="flex items-center gap-4 sm:gap-6">
		<!-- Connection Status -->
		<div class="flex items-center gap-1.5">
			{#if isAnimating}
				<Loader2 size={12} class="text-amber-400 animate-spin" />
			{:else}
				<span class="size-2 rounded-full {statusColor}"></span>
			{/if}
			<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500">
				<span class="hidden sm:inline">{statusText.full}</span>
				<span class="sm:hidden">{statusText.short}</span>
			</span>
		</div>

		<!-- Recording Status -->
		<div class="flex items-center gap-1.5">
			<span
				class="size-2 rounded-full {activeRecordings > 0
					? 'bg-orange-400 animate-pulse'
					: 'bg-zinc-500'}"
			></span>
			<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500">
				{activeRecordings}
				<span class="hidden lg:inline"> Recording</span>
				<span class="hidden sm:inline lg:hidden"> Rec</span>
			</span>
		</div>

		<!-- Disk Usage -->
		<div class="flex items-center gap-1.5">
			<HardDrive size={12} class="text-zinc-500" />
			<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500">
				<span class="hidden sm:inline">Disk: </span>{diskUsagePercent}%
			</span>
		</div>
	</div>

	<!-- Right: extension status -->
	<div class="ml-auto">
		<ExtensionStatusBar />
	</div>
</footer>
