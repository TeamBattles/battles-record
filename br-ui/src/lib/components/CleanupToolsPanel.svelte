<script lang="ts">
	import { storageStore } from '$lib/stores/storage.svelte';
	import { toastStore } from '$lib/stores/toast.svelte';
	import { Trash2, Eye, Loader2, AlertTriangle } from 'lucide-svelte';
	import type { RecordingStatus, CleanupLocation } from '$lib/api/types';
	import CornerBrackets from './ui/CornerBrackets.svelte';

	// Cleanup form state
	let olderThanDays = $state<number | null>(30);
	let selectedChannel = $state<string>('');
	let selectedStatus = $state<RecordingStatus | ''>('');
	let selectedLocation = $state<CleanupLocation>('both');

	const ageOptions: { label: string; value: number | null }[] = [
		{ label: 'All recordings', value: null },
		{ label: '7 days', value: 7 },
		{ label: '14 days', value: 14 },
		{ label: '30 days', value: 30 },
		{ label: '60 days', value: 60 },
		{ label: '90 days', value: 90 },
		{ label: '180 days', value: 180 }
	];

	// Get channels from storage stats (all channels with recordings)
	const channelsWithRecordings = $derived(
		storageStore.stats?.per_channel.map((c) => c.channel) ?? []
	);

	const statusOptions: { label: string; value: RecordingStatus | '' }[] = [
		{ label: 'All Statuses', value: '' },
		{ label: 'Completed', value: 'completed' },
		{ label: 'Processed', value: 'processed' },
		{ label: 'Failed', value: 'failed' }
	];

	// Location options - only show "Library Only" if recordings_dir !== library_dir
	const locationOptions = $derived.by(() => {
		const options: { label: string; value: CleanupLocation }[] = [
			{ label: 'Both', value: 'both' },
			{ label: 'Recordings Only', value: 'recordings' }
		];
		// Only show "Library Only" option if library is separate from recordings
		if (storageStore.hasSeparateLibrary) {
			options.push({ label: 'Library Only', value: 'library' });
		}
		return options;
	});

	async function handlePreview() {
		await storageStore.previewCleanup({
			older_than_days: olderThanDays ?? undefined,
			channel_name: selectedChannel || undefined,
			status: (selectedStatus || undefined) as RecordingStatus | undefined,
			location: selectedLocation
		});
	}

	async function handleCleanup() {
		const result = await storageStore.executeCleanup({
			older_than_days: olderThanDays ?? undefined,
			channel_name: selectedChannel || undefined,
			status: (selectedStatus || undefined) as RecordingStatus | undefined,
			location: selectedLocation
		});

		if (result) {
			// Build a detailed message based on what was freed
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
	}
</script>

<div class="relative border border-border bg-card">
	<CornerBrackets />

	<!-- Header -->
	<div class="flex items-center gap-2 border-b border-border/60 bg-muted/30 px-4 py-2">
		<Trash2 class="size-4 text-zinc-500" />
		<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">Cleanup Tools</span>
	</div>

	<!-- Content -->
	<div class="p-4 space-y-4">
		<!-- Form controls -->
		<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
			<!-- Age selector -->
			<div>
				<label
					for="cleanup-age"
					class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 block mb-1"
				>
					Older than
				</label>
				<select
					id="cleanup-age"
					class="w-full rounded border border-border bg-input px-3 py-1.5 font-mono text-xs"
					bind:value={olderThanDays}
				>
					{#each ageOptions as option (option.value)}
						<option value={option.value}>{option.label}</option>
					{/each}
				</select>
			</div>

			<!-- Channel filter -->
			<div>
				<label
					for="cleanup-channel"
					class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 block mb-1"
				>
					Channel
				</label>
				<select
					id="cleanup-channel"
					class="w-full rounded border border-border bg-input px-3 py-1.5 font-mono text-xs"
					bind:value={selectedChannel}
				>
					<option value="">All Channels</option>
					{#each channelsWithRecordings as channelName (channelName)}
						<option value={channelName}>{channelName}</option>
					{/each}
				</select>
			</div>

			<!-- Status filter -->
			<div>
				<label
					for="cleanup-status"
					class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 block mb-1"
				>
					Status
				</label>
				<select
					id="cleanup-status"
					class="w-full rounded border border-border bg-input px-3 py-1.5 font-mono text-xs"
					bind:value={selectedStatus}
				>
					{#each statusOptions as option (option.value)}
						<option value={option.value}>{option.label}</option>
					{/each}
				</select>
			</div>

			<!-- Location filter -->
			<div>
				<label
					for="cleanup-location"
					class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 block mb-1"
				>
					Location
				</label>
				<select
					id="cleanup-location"
					class="w-full rounded border border-border bg-input px-3 py-1.5 font-mono text-xs"
					bind:value={selectedLocation}
				>
					{#each locationOptions as option (option.value)}
						<option value={option.value}>{option.label}</option>
					{/each}
				</select>
			</div>
		</div>

		<!-- Preview results -->
		{#if storageStore.cleanupPreview}
			<div class="rounded border border-amber-500/30 bg-amber-500/5 p-3">
				<div class="flex items-start gap-2">
					<AlertTriangle class="size-4 text-amber-400 flex-shrink-0 mt-0.5" />
					<div>
						<p class="font-mono text-sm text-amber-300">
							{#if selectedLocation === 'library'}
								{storageStore.cleanupPreview.recordings_affected} library entries would be cleaned
							{:else}
								{storageStore.cleanupPreview.recordings_affected} recordings would be deleted
							{/if}
						</p>
						<p class="font-mono text-xs text-amber-400/80 mt-1">
							{#if selectedLocation === 'both'}
								~{storageStore.formatBytes(storageStore.cleanupPreview.bytes_to_free)} would be freed
								(recordings + library)
							{:else if selectedLocation === 'library'}
								Library files would be removed (recordings kept)
							{:else}
								~{storageStore.formatBytes(storageStore.cleanupPreview.bytes_to_free)} would be freed
							{/if}
						</p>
					</div>
				</div>
			</div>
		{/if}

		<!-- Error display -->
		{#if storageStore.cleanupError}
			<div class="rounded border border-red-500/30 bg-red-500/5 p-3">
				<p class="font-mono text-xs text-red-400">{storageStore.cleanupError}</p>
			</div>
		{/if}

		<!-- Action buttons -->
		<div class="flex flex-col sm:flex-row items-stretch sm:items-center justify-end gap-2">
			{#if storageStore.cleanupPreview}
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
				{#if storageStore.isCleaningUp && !storageStore.cleanupPreview}
					<Loader2 class="size-3.5 animate-spin" />
				{:else}
					<Eye class="size-3.5" />
				{/if}
				Preview
			</button>

			{#if storageStore.cleanupPreview && storageStore.cleanupPreview.recordings_affected > 0}
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
					{#if selectedLocation === 'library'}
						Clean {storageStore.cleanupPreview.recordings_affected} Library Entries
					{:else}
						Delete {storageStore.cleanupPreview.recordings_affected} Recordings
					{/if}
				</button>
			{/if}
		</div>
	</div>
</div>
