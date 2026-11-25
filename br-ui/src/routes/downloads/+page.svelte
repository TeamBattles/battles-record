<script lang="ts">
	import PageHeader from '#lib/components/ui/PageHeader.svelte';
	import { Search, Download, Pause, Play, X } from '@lucide/svelte';
	import { untrack } from 'svelte';
	import { connectionStore, downloadsStore, breakpointStore, toastStore } from '#lib';
	import DownloadCard from '#lib/components/DownloadCard.svelte';
	import MergeDialog from '#lib/components/MergeDialog.svelte';
	import type { DownloadStatus, DownloadSummary } from '#lib/api/types.ts';
	import { openPath, showInFolder } from '#lib/utils/desktop.ts';
	import { extractErrorMessage } from '#lib/utils/errors.ts';
	import Select from '#lib/components/ui/Select.svelte';

	const STATUS_FILTER_OPTIONS: { value: DownloadStatus | 'all'; label: string }[] = [
		{ value: 'all', label: 'All Statuses' },
		{ value: 'downloading', label: 'Downloading' },
		{ value: 'queued', label: 'Queued' },
		{ value: 'extracting_info', label: 'Extracting Info' },
		{ value: 'processing', label: 'Processing' },
		{ value: 'paused', label: 'Paused' },
		{ value: 'complete', label: 'Complete' },
		{ value: 'failed', label: 'Failed' },
		{ value: 'cancelled', label: 'Cancelled' }
	];

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

	// Download paths are on the daemon's machine, so file actions only make sense for this PC
	const isLocalConnection = $derived(connectionStore.activeServer?.type === 'local');

	async function openFile(path: string) {
		try {
			await openPath(path);
		} catch (e) {
			toastStore.error(extractErrorMessage(e, 'Could not open the file'));
		}
	}

	/** Selects the file when it still exists, otherwise opens the download's folder. */
	async function openFolder(download: DownloadSummary) {
		try {
			if (download.output_file) {
				try {
					await showInFolder(download.output_file);
					return;
				} catch (e) {
					if (!download.output_dir) throw e;
				}
			}
			if (!download.output_dir) {
				toastStore.error('This download has no folder yet');
				return;
			}
			await openPath(download.output_dir);
		} catch (e) {
			toastStore.error(extractErrorMessage(e, 'Could not open the folder'));
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
	<PageHeader
		title="Downloads"
		eyebrow="Record"
		subtitle="VODs and clips queued for download"
		icon={Download}
		count={downloadsStore.filteredDownloads.length}
	>
		{#snippet badges()}
			{#if downloadsStore.activeCount > 0}
				<span
					class="rounded bg-green/10 px-1.5 py-0.5 font-mono text-[10px] text-green"
				>
					{downloadsStore.activeCount} active
				</span>
			{/if}
			{#if downloadsStore.queuedCount > 0}
				<span
					class="rounded bg-yellow/10 px-1.5 py-0.5 font-mono text-[10px] text-yellow"
				>
					{downloadsStore.queuedCount} queued
				</span>
			{/if}
		{/snippet}
		{#snippet actions()}
			<!-- Bulk actions -->
			<div class="flex items-center gap-2">
				{#if hasActive}
					<button
						class="flex items-center gap-1.5 rounded border border-border bg-btn-bg px-2.5 py-1.5 font-mono text-xs hover:bg-hover-wash transition-colors"
						onclick={pauseAll}
					>
						<Pause class="w-3 h-3" />
						Pause All
					</button>
				{/if}
				{#if hasPaused}
					<button
						class="flex items-center gap-1.5 rounded border border-border bg-btn-bg px-2.5 py-1.5 font-mono text-xs hover:bg-hover-wash transition-colors"
						onclick={resumeAll}
					>
						<Play class="w-3 h-3" />
						Resume All
					</button>
				{/if}
				{#if hasCancellable}
					<button
						class="flex items-center gap-1.5 rounded border border-destructive/30 bg-destructive/10 px-2.5 py-1.5 font-mono text-xs text-destructive hover:bg-destructive/20 transition-colors"
						onclick={cancelAll}
					>
						<X class="w-3 h-3" />
						Cancel All
					</button>
				{/if}
			</div>
		{/snippet}
	</PageHeader>

	<!-- Filters -->
	<div class="flex flex-col gap-2 sm:flex-row sm:items-center sm:gap-2">
		<!-- Status Filter -->
		<Select
				size="sm"
				class="sm:w-48"
				ariaLabel="Filter by status"
				value={downloadsStore.statusFilter}
				options={STATUS_FILTER_OPTIONS}
				onValueChange={(status) => (downloadsStore.statusFilter = status)}
			/>

		<!-- Search -->
		<div class="relative">
			<Search class="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground" />
			<input
				type="text"
				placeholder="Search..."
				class="rounded border border-border bg-well pl-8 pr-3 py-1.5 font-mono text-xs w-full sm:w-40"
				value={downloadsStore.searchQuery}
				oninput={(e) => (downloadsStore.searchQuery = e.currentTarget.value)}
			/>
		</div>
	</div>

	<!-- Content Area -->
	{#if !connectionStore.isConnected}
		<div class="rounded-lg border border-yellow/30 bg-yellow/5 p-4">
			<p class="font-mono text-xs text-yellow">Not connected to a server.</p>
		</div>
	{:else if downloadsStore.isLoading}
		<div class="flex items-center gap-2 text-muted-foreground">
			<div
				class="size-4 animate-spin rounded-full border-2 border-line-3 border-t-transparent"
			></div>
			<span class="font-mono text-xs">Loading downloads...</span>
		</div>
	{:else if downloadsStore.error}
		<div class="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
			<p class="font-mono text-xs text-destructive">{downloadsStore.error}</p>
		</div>
	{:else if downloadsStore.filteredDownloads.length === 0}
		<div class="relative rounded-lg border border-border bg-card p-8">

			<div class="flex flex-col items-center justify-center gap-2 text-muted-foreground">
				<Download class="size-8 opacity-30" />
				<p class="font-mono text-xs">No downloads found</p>
				{#if downloadsStore.statusFilter !== 'all' || downloadsStore.searchQuery}
					<button
						class="mt-2 rounded border border-border bg-btn-bg px-3 py-1.5 font-mono text-xs hover:bg-hover-wash transition-colors"
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
					onOpenFile={isLocalConnection ? openFile : undefined}
					onOpenFolder={isLocalConnection ? openFolder : undefined}
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
