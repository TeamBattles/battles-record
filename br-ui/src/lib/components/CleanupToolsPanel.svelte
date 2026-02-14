<script lang="ts">
	import { storageStore } from '$lib/stores/storage.svelte';
	import { toastStore } from '$lib/stores/toast.svelte';
	import { Trash2, Eye, Loader2, AlertTriangle } from 'lucide-svelte';
	import type { RecordingStatus, CleanupLocation } from '$lib/api/types';
	import CornerBrackets from './ui/CornerBrackets.svelte';
	import { cn } from '$lib/utils/cn';

	// Tab state
	let activeTab = $state<'recordings' | 'downloads'>('recordings');

	// Recordings cleanup form state
	let olderThanDays = $state<number | null>(30);
	let selectedChannel = $state<string>('');
	let selectedStatus = $state<RecordingStatus | ''>('');
	let selectedLocation = $state<CleanupLocation>('both');

	// Downloads cleanup form state
	let dlOlderThanDays = $state<number | null>(30);
	let dlSelectedChannel = $state<string>('');
	let dlSelectedPlatform = $state<string>('');

	const ageOptions: { label: string; value: number | null }[] = [
		{ label: 'All items', value: null },
		{ label: '7 days', value: 7 },
		{ label: '14 days', value: 14 },
		{ label: '30 days', value: 30 },
		{ label: '60 days', value: 60 },
		{ label: '90 days', value: 90 },
		{ label: '180 days', value: 180 }
	];

	const channelsWithRecordings = $derived(
		storageStore.stats?.per_channel.map((c) => c.channel) ?? []
	);

	const channelsWithDownloads = $derived(
		storageStore.downloadStats?.per_channel.map((c) => c.channel) ?? []
	);

	const downloadPlatforms = $derived(() => {
		const platforms = new Set(
			storageStore.downloadStats?.per_channel.map((c) => c.platform) ?? []
		);
		return [...platforms];
	});

	const statusOptions: { label: string; value: RecordingStatus | '' }[] = [
		{ label: 'All Statuses', value: '' },
		{ label: 'Completed', value: 'completed' },
		{ label: 'Processed', value: 'processed' },
		{ label: 'Failed', value: 'failed' }
	];

	const locationOptions = $derived.by(() => {
		const options: { label: string; value: CleanupLocation }[] = [
			{ label: 'Both', value: 'both' },
			{ label: 'Recordings Only', value: 'recordings' }
		];
		if (storageStore.hasSeparateLibrary) {
			options.push({ label: 'Library Only', value: 'library' });
		}
		return options;
	});

	// Preview state (combined for both tabs)
	const hasPreview = $derived(
		activeTab === 'recordings'
			? storageStore.cleanupPreview !== null
			: storageStore.downloadCleanupPreview !== null
	);

	async function handlePreview() {
		if (activeTab === 'recordings') {
			await storageStore.previewCleanup({
				older_than_days: olderThanDays ?? undefined,
				channel_name: selectedChannel || undefined,
				status: (selectedStatus || undefined) as RecordingStatus | undefined,
				location: selectedLocation
			});
		} else {
			await storageStore.previewDownloadCleanup({
				older_than_days: dlOlderThanDays ?? undefined,
				channel_name: dlSelectedChannel || undefined,
				source_platform: dlSelectedPlatform || undefined
			});
		}
	}

	async function handleCleanup() {
		if (activeTab === 'recordings') {
			const result = await storageStore.executeCleanup({
				older_than_days: olderThanDays ?? undefined,
				channel_name: selectedChannel || undefined,
				status: (selectedStatus || undefined) as RecordingStatus | undefined,
				location: selectedLocation
			});
			if (result) {
				let message = `Deleted ${result.recordings_affected} recordings`;
				if (result.recordings_bytes_freed !== undefined && result.library_bytes_freed !== undefined) {
					if (result.recordings_bytes_freed > 0 && result.library_bytes_freed > 0) {
						message += `, freed ${storageStore.formatBytes(result.recordings_bytes_freed)} (recordings) + ${storageStore.formatBytes(result.library_bytes_freed)} (library)`;
					} else if (result.recordings_bytes_freed > 0) {
						message += `, freed ${storageStore.formatBytes(result.recordings_bytes_freed)}`;
					} else if (result.library_bytes_freed > 0) {
						message += `, freed ${storageStore.formatBytes(result.library_bytes_freed)} from library`;
					}
				} else {
					message += `, freed ${storageStore.formatBytes(result.bytes_to_free)}`;
				}
				toastStore.success(message);
			}
		} else {
			await storageStore.executeDownloadCleanup({
				older_than_days: dlOlderThanDays ?? undefined,
				channel_name: dlSelectedChannel || undefined,
				source_platform: dlSelectedPlatform || undefined
			});
			toastStore.success('Download cleanup complete');
		}
	}
</script>

<div class="relative border border-border bg-card">
	<CornerBrackets />

	<!-- Header with tabs -->
	<div class="flex items-center border-b border-border/60 bg-muted/30">
		<div class="flex items-center gap-2 px-4 py-2">
			<Trash2 class="size-4 text-zinc-500" />
			<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">Cleanup Tools</span>
		</div>
		<div class="flex ml-auto">
			<button
				class={cn(
					'px-4 py-2 font-mono text-[10px] uppercase tracking-wider transition-colors border-b-2',
					activeTab === 'recordings'
						? 'text-zinc-200 border-emerald-500 bg-zinc-800/50'
						: 'text-zinc-500 border-transparent hover:text-zinc-400'
				)}
				onclick={() => { activeTab = 'recordings'; storageStore.clearCleanupPreview(); }}
			>
				Recordings
			</button>
			<button
				class={cn(
					'px-4 py-2 font-mono text-[10px] uppercase tracking-wider transition-colors border-b-2',
					activeTab === 'downloads'
						? 'text-zinc-200 border-emerald-500 bg-zinc-800/50'
						: 'text-zinc-500 border-transparent hover:text-zinc-400'
				)}
				onclick={() => { activeTab = 'downloads'; storageStore.clearCleanupPreview(); }}
			>
				Downloads
			</button>
		</div>
	</div>

	<!-- Content -->
	<div class="p-4 space-y-4">
		{#if activeTab === 'recordings'}
			<!-- Recordings cleanup form -->
			<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
				<div>
					<label for="cleanup-age" class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 block mb-1">Older than</label>
					<select id="cleanup-age" class="w-full rounded border border-border bg-input px-3 py-1.5 font-mono text-xs" bind:value={olderThanDays}>
						{#each ageOptions as option (option.value)}
							<option value={option.value}>{option.label}</option>
						{/each}
					</select>
				</div>
				<div>
					<label for="cleanup-channel" class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 block mb-1">Channel</label>
					<select id="cleanup-channel" class="w-full rounded border border-border bg-input px-3 py-1.5 font-mono text-xs" bind:value={selectedChannel}>
						<option value="">All Channels</option>
						{#each channelsWithRecordings as channelName (channelName)}
							<option value={channelName}>{channelName}</option>
						{/each}
					</select>
				</div>
				<div>
					<label for="cleanup-status" class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 block mb-1">Status</label>
					<select id="cleanup-status" class="w-full rounded border border-border bg-input px-3 py-1.5 font-mono text-xs" bind:value={selectedStatus}>
						{#each statusOptions as option (option.value)}
							<option value={option.value}>{option.label}</option>
						{/each}
					</select>
				</div>
				<div>
					<label for="cleanup-location" class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 block mb-1">Location</label>
					<select id="cleanup-location" class="w-full rounded border border-border bg-input px-3 py-1.5 font-mono text-xs" bind:value={selectedLocation}>
						{#each locationOptions as option (option.value)}
							<option value={option.value}>{option.label}</option>
						{/each}
					</select>
				</div>
			</div>
		{:else}
			<!-- Downloads cleanup form -->
			<div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
				<div>
					<label for="dl-cleanup-age" class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 block mb-1">Older than</label>
					<select id="dl-cleanup-age" class="w-full rounded border border-border bg-input px-3 py-1.5 font-mono text-xs" bind:value={dlOlderThanDays}>
						{#each ageOptions as option (option.value)}
							<option value={option.value}>{option.label}</option>
						{/each}
					</select>
				</div>
				<div>
					<label for="dl-cleanup-channel" class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 block mb-1">Channel</label>
					<select id="dl-cleanup-channel" class="w-full rounded border border-border bg-input px-3 py-1.5 font-mono text-xs" bind:value={dlSelectedChannel}>
						<option value="">All Channels</option>
						{#each channelsWithDownloads as channelName (channelName)}
							<option value={channelName}>{channelName}</option>
						{/each}
					</select>
				</div>
				<div>
					<label for="dl-cleanup-platform" class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 block mb-1">Platform</label>
					<select id="dl-cleanup-platform" class="w-full rounded border border-border bg-input px-3 py-1.5 font-mono text-xs" bind:value={dlSelectedPlatform}>
						<option value="">All Platforms</option>
						{#each downloadPlatforms() as platform (platform)}
							<option value={platform}>{platform}</option>
						{/each}
					</select>
				</div>
			</div>
		{/if}

		<!-- Preview results -->
		{#if activeTab === 'recordings' && storageStore.cleanupPreview}
			<div class="rounded border border-amber-500/30 bg-amber-500/5 p-3">
				<div class="flex items-start gap-2">
					<AlertTriangle class="size-4 text-amber-400 flex-shrink-0 mt-0.5" />
					<div>
						<p class="font-mono text-sm text-amber-300">
							{storageStore.cleanupPreview.recordings_affected} recordings would be deleted
						</p>
						<p class="font-mono text-xs text-amber-400/80 mt-1">
							~{storageStore.formatBytes(storageStore.cleanupPreview.bytes_to_free)} would be freed
						</p>
					</div>
				</div>
			</div>
		{:else if activeTab === 'downloads' && storageStore.downloadCleanupPreview}
			<div class="rounded border border-amber-500/30 bg-amber-500/5 p-3">
				<div class="flex items-start gap-2">
					<AlertTriangle class="size-4 text-amber-400 flex-shrink-0 mt-0.5" />
					<div>
						<p class="font-mono text-sm text-amber-300">
							{storageStore.downloadCleanupPreview.affected} downloads would be deleted
						</p>
						<p class="font-mono text-xs text-amber-400/80 mt-1">
							~{storageStore.formatBytes(storageStore.downloadCleanupPreview.bytes_to_free)} would be freed
						</p>
					</div>
				</div>
			</div>
		{/if}

		{#if storageStore.cleanupError}
			<div class="rounded border border-red-500/30 bg-red-500/5 p-3">
				<p class="font-mono text-xs text-red-400">{storageStore.cleanupError}</p>
			</div>
		{/if}

		<!-- Action buttons -->
		<div class="flex flex-col sm:flex-row items-stretch sm:items-center justify-end gap-2">
			{#if hasPreview}
				<button
					class="rounded border border-border bg-input px-4 py-2 font-mono text-xs hover:bg-muted transition-colors"
					onclick={() => storageStore.clearCleanupPreview()}
				>
					Cancel
				</button>
			{/if}

			<button
				class="rounded border border-border bg-input px-4 py-2 font-mono text-xs hover:bg-muted transition-colors flex items-center justify-center gap-2"
				onclick={handlePreview}
				disabled={storageStore.isCleaningUp}
			>
				{#if storageStore.isCleaningUp && !hasPreview}
					<Loader2 class="size-3.5 animate-spin" />
				{:else}
					<Eye class="size-3.5" />
				{/if}
				Preview
			</button>

			{#if activeTab === 'recordings' && storageStore.cleanupPreview && storageStore.cleanupPreview.recordings_affected > 0}
				<button
					class="rounded border border-red-500/30 bg-red-500/10 px-4 py-2 font-mono text-xs text-red-400 hover:bg-red-500/20 transition-colors flex items-center justify-center gap-2"
					onclick={handleCleanup}
					disabled={storageStore.isCleaningUp}
				>
					{#if storageStore.isCleaningUp}
						<Loader2 class="size-3.5 animate-spin" />
					{:else}
						<Trash2 class="size-3.5" />
					{/if}
					Delete {storageStore.cleanupPreview.recordings_affected} Recordings
				</button>
			{:else if activeTab === 'downloads' && storageStore.downloadCleanupPreview && storageStore.downloadCleanupPreview.affected > 0}
				<button
					class="rounded border border-red-500/30 bg-red-500/10 px-4 py-2 font-mono text-xs text-red-400 hover:bg-red-500/20 transition-colors flex items-center justify-center gap-2"
					onclick={handleCleanup}
					disabled={storageStore.isCleaningUp}
				>
					{#if storageStore.isCleaningUp}
						<Loader2 class="size-3.5 animate-spin" />
					{:else}
						<Trash2 class="size-3.5" />
					{/if}
					Delete {storageStore.downloadCleanupPreview.affected} Downloads
				</button>
			{/if}
		</div>
	</div>
</div>
