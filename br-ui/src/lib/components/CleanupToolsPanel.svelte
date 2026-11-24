<script lang="ts">
	import { storageStore } from '#lib/stores/storage.svelte.ts';
	import { toastStore } from '#lib/stores/toast.svelte.ts';
	import { Trash2, Eye, Loader2, AlertTriangle } from '@lucide/svelte';
	import type { RecordingStatus, CleanupLocation } from '#lib/api/types.ts';
	import { cn } from '#lib/utils/cn.ts';
	import Select from './ui/Select.svelte';

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

	const recordingChannelOptions = $derived([
		{ label: 'All Channels', value: '' },
		...channelsWithRecordings.map((channel) => ({ label: channel, value: channel }))
	]);

	const downloadChannelOptions = $derived([
		{ label: 'All Channels', value: '' },
		...channelsWithDownloads.map((channel) => ({ label: channel, value: channel }))
	]);

	const downloadPlatformOptions = $derived([
		{ label: 'All Platforms', value: '' },
		...downloadPlatforms().map((platform) => ({ label: platform, value: platform }))
	]);

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

<div class="relative rounded-lg border border-border bg-card">

	<!-- Header with tabs -->
	<div class="flex items-center border-b border-border/60 bg-muted/30">
		<div class="flex items-center gap-2 px-4 py-2">
			<Trash2 class="size-4 text-muted-foreground" />
			<span class="font-mono text-xs uppercase tracking-wider text-muted-foreground">Cleanup Tools</span>
		</div>
		<div class="flex ml-auto">
			<button
				class={cn(
					'px-4 py-2 font-mono text-[10px] uppercase tracking-wider transition-colors border-b-2',
					activeTab === 'recordings'
						? 'text-foreground border-primary bg-chip/50'
						: 'text-muted-foreground border-transparent hover:text-text-2'
				)}
				onclick={() => { activeTab = 'recordings'; storageStore.clearCleanupPreview(); }}
			>
				Recordings
			</button>
			<button
				class={cn(
					'px-4 py-2 font-mono text-[10px] uppercase tracking-wider transition-colors border-b-2',
					activeTab === 'downloads'
						? 'text-foreground border-primary bg-chip/50'
						: 'text-muted-foreground border-transparent hover:text-text-2'
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
					<label for="cleanup-age" class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground block mb-1">Older than</label>
					<Select id="cleanup-age" size="sm" options={ageOptions} bind:value={olderThanDays} />
				</div>
				<div>
					<label for="cleanup-channel" class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground block mb-1">Channel</label>
					<Select id="cleanup-channel" size="sm" options={recordingChannelOptions} bind:value={selectedChannel} />
				</div>
				<div>
					<label for="cleanup-status" class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground block mb-1">Status</label>
					<Select id="cleanup-status" size="sm" options={statusOptions} bind:value={selectedStatus} />
				</div>
				<div>
					<label for="cleanup-location" class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground block mb-1">Location</label>
					<Select id="cleanup-location" size="sm" options={locationOptions} bind:value={selectedLocation} />
				</div>
			</div>
		{:else}
			<!-- Downloads cleanup form -->
			<div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
				<div>
					<label for="dl-cleanup-age" class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground block mb-1">Older than</label>
					<Select id="dl-cleanup-age" size="sm" options={ageOptions} bind:value={dlOlderThanDays} />
				</div>
				<div>
					<label for="dl-cleanup-channel" class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground block mb-1">Channel</label>
					<Select id="dl-cleanup-channel" size="sm" options={downloadChannelOptions} bind:value={dlSelectedChannel} />
				</div>
				<div>
					<label for="dl-cleanup-platform" class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground block mb-1">Platform</label>
					<Select id="dl-cleanup-platform" size="sm" options={downloadPlatformOptions} bind:value={dlSelectedPlatform} />
				</div>
			</div>
		{/if}

		<!-- Preview results -->
		{#if activeTab === 'recordings' && storageStore.cleanupPreview}
			<div class="rounded border border-yellow/30 bg-yellow/5 p-3">
				<div class="flex items-start gap-2">
					<AlertTriangle class="size-4 text-yellow flex-shrink-0 mt-0.5" />
					<div>
						<p class="font-mono text-sm text-yellow">
							{storageStore.cleanupPreview.recordings_affected} recordings would be deleted
						</p>
						<p class="font-mono text-xs text-yellow/80 mt-1">
							~{storageStore.formatBytes(storageStore.cleanupPreview.bytes_to_free)} would be freed
						</p>
					</div>
				</div>
			</div>
		{:else if activeTab === 'downloads' && storageStore.downloadCleanupPreview}
			<div class="rounded border border-yellow/30 bg-yellow/5 p-3">
				<div class="flex items-start gap-2">
					<AlertTriangle class="size-4 text-yellow flex-shrink-0 mt-0.5" />
					<div>
						<p class="font-mono text-sm text-yellow">
							{storageStore.downloadCleanupPreview.affected} downloads would be deleted
						</p>
						<p class="font-mono text-xs text-yellow/80 mt-1">
							~{storageStore.formatBytes(storageStore.downloadCleanupPreview.bytes_to_free)} would be freed
						</p>
					</div>
				</div>
			</div>
		{/if}

		{#if storageStore.cleanupError}
			<div class="rounded border border-destructive/30 bg-destructive/5 p-3">
				<p class="font-mono text-xs text-destructive">{storageStore.cleanupError}</p>
			</div>
		{/if}

		<!-- Action buttons -->
		<div class="flex flex-col sm:flex-row items-stretch sm:items-center justify-end gap-2">
			{#if hasPreview}
				<button
					class="rounded border border-border bg-btn-bg px-4 py-2 font-mono text-xs hover:bg-hover-wash transition-colors"
					onclick={() => storageStore.clearCleanupPreview()}
				>
					Cancel
				</button>
			{/if}

			<button
				class="rounded border border-border bg-btn-bg px-4 py-2 font-mono text-xs hover:bg-hover-wash transition-colors flex items-center justify-center gap-2"
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
					class="rounded border border-destructive/30 bg-destructive/10 px-4 py-2 font-mono text-xs text-destructive hover:bg-destructive/20 transition-colors flex items-center justify-center gap-2"
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
					class="rounded border border-destructive/30 bg-destructive/10 px-4 py-2 font-mono text-xs text-destructive hover:bg-destructive/20 transition-colors flex items-center justify-center gap-2"
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
