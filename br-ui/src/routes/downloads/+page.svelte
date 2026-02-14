<script lang="ts">
	import { Search, Download, Pause, Play, X } from 'lucide-svelte';
	import { untrack } from 'svelte';
	import { connectionStore, downloadsStore, breakpointStore } from '$lib';
	import DownloadCard from '$lib/components/DownloadCard.svelte';
	import MergeDialog from '$lib/components/MergeDialog.svelte';
	import CornerBrackets from '$lib/components/ui/CornerBrackets.svelte';
	import type { DownloadStatus } from '$lib/api/types';

	// Load downloads on connection
	$effect(() => {
		const serverId = connectionStore.activeServerId;
		if (connectionStore.isConnected && serverId) {
			untrack(() => {
				downloadsStore.load(serverId);
			});
		}
	});

	// Bulk actions
	async function pauseAll() {
		const active = downloadsStore.downloads.filter(
			(d) =>
				d.status === 'downloading' ||
				d.status === 'extracting_info' ||
				d.status === 'processing'
		);
		for (const d of active) {
			await downloadsStore.pause(d.id);
		}
	}

	async function resumeAll() {
		const paused = downloadsStore.downloads.filter((d) => d.status === 'paused');
		for (const d of paused) {
			await downloadsStore.resume(d.id);
		}
	}

	async function cancelAll() {
		const cancellable = downloadsStore.downloads.filter(
			(d) =>
				d.status === 'downloading' ||
				d.status === 'extracting_info' ||
				d.status === 'processing' ||
				d.status === 'queued' ||
				d.status === 'paused'
		);
		for (const d of cancellable) {
			await downloadsStore.cancel(d.id);
		}
	}

	// Merge dialog state
	let mergeOpen = $state(false);
	let mergePlatform = $state('');
	let mergeChannel = $state('');

	function openMergeDialog(platform: string, channel: string) {
		mergePlatform = platform;
		mergeChannel = channel;
		mergeOpen = true;
	}

	function handleMerged() {
		const serverId = connectionStore.activeServerId;
		if (serverId) {
			untrack(() => {
				downloadsStore.load(serverId);
			});
		}
	}

	const hasActive = $derived(downloadsStore.activeCount > 0);
	const hasPaused = $derived(
		downloadsStore.downloads.some((d) => d.status === 'paused')
	);
	const hasCancellable = $derived(
		downloadsStore.downloads.some(
			(d) =>
				d.status === 'downloading' ||
				d.status === 'extracting_info' ||
				d.status === 'processing' ||
				d.status === 'queued' ||
				d.status === 'paused'
		)
	);
</script>

<div class="space-y-4">
	<!-- Header Bar -->
	<div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
		<div class="flex items-center gap-3">
			<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">Downloads</span>
			<span
				class="rounded bg-zinc-200 px-1.5 py-0.5 font-mono text-[10px] text-zinc-600 dark:bg-zinc-800 dark:text-zinc-500"
			>
				{downloadsStore.filteredDownloads.length}
			</span>
			{#if downloadsStore.activeCount > 0}
				<span
					class="rounded bg-emerald-500/10 px-1.5 py-0.5 font-mono text-[10px] text-emerald-400"
				>
					{downloadsStore.activeCount} active
				</span>
			{/if}
			{#if downloadsStore.queuedCount > 0}
				<span
					class="rounded bg-amber-500/10 px-1.5 py-0.5 font-mono text-[10px] text-amber-400"
				>
					{downloadsStore.queuedCount} queued
				</span>
			{/if}
		</div>

		<!-- Bulk actions -->
		<div class="flex items-center gap-2">
			{#if hasActive}
				<button
					class="flex items-center gap-1.5 rounded border border-border bg-input px-2.5 py-1.5 font-mono text-xs hover:bg-muted transition-colors"
					onclick={pauseAll}
				>
					<Pause class="w-3 h-3" />
					Pause All
				</button>
			{/if}
			{#if hasPaused}
				<button
					class="flex items-center gap-1.5 rounded border border-border bg-input px-2.5 py-1.5 font-mono text-xs hover:bg-muted transition-colors"
					onclick={resumeAll}
				>
					<Play class="w-3 h-3" />
					Resume All
				</button>
			{/if}
			{#if hasCancellable}
				<button
					class="flex items-center gap-1.5 rounded border border-red-500/30 bg-red-500/10 px-2.5 py-1.5 font-mono text-xs text-red-400 hover:bg-red-500/20 transition-colors"
					onclick={cancelAll}
				>
					<X class="w-3 h-3" />
					Cancel All
				</button>
			{/if}
		</div>
	</div>

	<!-- Filters -->
	<div class="flex flex-col gap-2 sm:flex-row sm:items-center sm:gap-2">
		<!-- Status Filter -->
		<select
			class="rounded border border-border bg-input px-3 py-1.5 font-mono text-xs w-full sm:w-auto"
			value={downloadsStore.statusFilter}
			onchange={(e) =>
				(downloadsStore.statusFilter = e.currentTarget.value as DownloadStatus | 'all')}
		>
			<option value="all">All Statuses</option>
			<option value="downloading">Downloading</option>
			<option value="queued">Queued</option>
			<option value="extracting_info">Extracting Info</option>
			<option value="processing">Processing</option>
			<option value="paused">Paused</option>
			<option value="complete">Complete</option>
			<option value="failed">Failed</option>
			<option value="cancelled">Cancelled</option>
		</select>

		<!-- Search -->
		<div class="relative">
			<Search class="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-zinc-500" />
			<input
				type="text"
				placeholder="Search..."
				class="rounded border border-border bg-input pl-8 pr-3 py-1.5 font-mono text-xs w-full sm:w-40"
				value={downloadsStore.searchQuery}
				oninput={(e) => (downloadsStore.searchQuery = e.currentTarget.value)}
			/>
		</div>
	</div>

	<!-- Content Area -->
	{#if !connectionStore.isConnected}
		<div class="rounded-lg border border-amber-500/30 bg-amber-500/5 p-4">
			<p class="font-mono text-xs text-amber-400">Not connected to a server.</p>
		</div>
	{:else if downloadsStore.isLoading}
		<div class="flex items-center gap-2 text-zinc-500">
			<div
				class="size-4 animate-spin rounded-full border-2 border-zinc-500 border-t-transparent"
			></div>
			<span class="font-mono text-xs">Loading downloads...</span>
		</div>
	{:else if downloadsStore.error}
		<div class="rounded-lg border border-red-500/30 bg-red-500/5 p-4">
			<p class="font-mono text-xs text-red-400">{downloadsStore.error}</p>
		</div>
	{:else if downloadsStore.filteredDownloads.length === 0}
		<div class="relative border border-border bg-card p-8">
			<CornerBrackets />

			<div class="flex flex-col items-center justify-center gap-2 text-zinc-500">
				<Download class="size-8 opacity-30" />
				<p class="font-mono text-xs">No downloads found</p>
				{#if downloadsStore.statusFilter !== 'all' || downloadsStore.searchQuery}
					<button
						class="mt-2 rounded border border-border bg-input px-3 py-1.5 font-mono text-xs hover:bg-muted transition-colors"
						onclick={() => {
							downloadsStore.statusFilter = 'all';
							downloadsStore.searchQuery = '';
						}}
					>
						Clear filters
					</button>
				{/if}
			</div>
		</div>
	{:else}
		<!-- Card Layout (both mobile and desktop) -->
		<div class={breakpointStore.isMobile ? 'space-y-2' : 'grid grid-cols-2 xl:grid-cols-3 gap-3'}>
			{#each downloadsStore.filteredDownloads as download (download.id)}
				<DownloadCard
					{download}
					onPause={(id) => downloadsStore.pause(id)}
					onResume={(id) => downloadsStore.resume(id)}
					onCancel={(id) => downloadsStore.cancel(id)}
					onPrioritize={(id) => downloadsStore.prioritize(id)}
					onRemove={(id) => downloadsStore.remove(id)}
					onMerge={openMergeDialog}
				/>
			{/each}
		</div>
	{/if}
</div>

<MergeDialog
	bind:open={mergeOpen}
	sourcePlatform={mergePlatform}
	sourceChannel={mergeChannel}
	onMerged={handleMerged}
/>
