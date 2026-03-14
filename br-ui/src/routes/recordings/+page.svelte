<script lang="ts">
	import { Search, Trash2, Play, FolderOpen, Film, RotateCcw } from 'lucide-svelte';
	import { Tooltip } from 'bits-ui';
	import { invoke } from '@tauri-apps/api/core';
	import { recordingsStore } from '$lib/stores/recordings.svelte';
	import { connectionStore } from '$lib/stores/connection.svelte';
	import { breakpointStore, ResponsiveModal } from '$lib';
	import PlatformIcon from '$lib/components/PlatformIcon.svelte';
	import RecordingCard from '$lib/components/RecordingCard.svelte';
	import OpenFolderModal from '$lib/components/OpenFolderModal.svelte';
	import CornerBrackets from '$lib/components/ui/CornerBrackets.svelte';
	import type { Recording, RecordingStatus } from '$lib/api/types';
	import { formatDuration, formatBytes, formatDate, RECORDING_STATUS_COLORS } from '$lib/utils';
	import { untrack } from 'svelte';

	let deleteModalOpen = $state(false);
	let recordingToDelete = $state<Recording | null>(null);
	let hoveredStatusId = $state<string | null>(null);

	// Open folder modal state
	let openFolderModalOpen = $state(false);
	let openFolderRecording = $state<Recording | null>(null);

	// Reload recordings when server changes or connection is established
	$effect(() => {
		const serverId = connectionStore.activeServerId;
		if (connectionStore.isConnected && serverId) {
			untrack(() => {
				recordingsStore.load(serverId);
			});
		}
	});

	function openDeleteModal(recording: Recording) {
		recordingToDelete = recording;
		deleteModalOpen = true;
	}

	async function handleDelete() {
		if (!recordingToDelete) return;
		const success = await recordingsStore.deleteRecording(recordingToDelete.id);
		if (success) {
			deleteModalOpen = false;
			recordingToDelete = null;
		}
	}

	async function handleProcess(id: string) {
		await recordingsStore.processRecording(id);
	}

	async function handleOpenFolder(path: string) {
		try {
			await invoke('show_in_folder', { path });
		} catch (e) {
			console.error('Failed to open folder:', e);
		}
	}

	function handleOpenFolderClick(recording: Recording) {
		// If recording is processed and has an output file, show modal to choose
		if (recording.status === 'processed' && recording.output_file) {
			openFolderRecording = recording;
			openFolderModalOpen = true;
		} else {
			// Otherwise just open the recording folder directly
			handleOpenFolder(recording.path);
		}
	}

	async function handleOpenLibraryFolder() {
		if (openFolderRecording?.output_file) {
			try {
				await invoke('show_in_folder', { path: openFolderRecording.output_file });
			} catch (e) {
				console.error('Failed to open library folder:', e);
			}
		}
	}

	// Only show folder actions for local connections
	const isLocalConnection = $derived(connectionStore.activeServer?.type === 'local');
</script>

<div class="space-y-4">
	<!-- Header Bar -->
	<div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
		<div class="flex items-center gap-3">
			<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">Recordings</span>
			<span
				class="rounded bg-zinc-200 px-1.5 py-0.5 font-mono text-[10px] text-zinc-600 dark:bg-zinc-800 dark:text-zinc-500"
			>
				{recordingsStore.filteredRecordings.length}
			</span>
		</div>

		<div class="flex flex-col gap-2 sm:flex-row sm:items-center sm:gap-2">
			<!-- Platform Filter -->
			<select
				class="rounded border border-border bg-input px-3 py-1.5 font-mono text-xs w-full sm:w-auto"
				value={recordingsStore.platformFilter ?? ''}
				onchange={(e) => recordingsStore.setFilter(e.currentTarget.value || null)}
			>
				<option value="">All Platforms</option>
				<option value="twitch">Twitch</option>
				<option value="youtube">YouTube</option>
				<option value="kick">Kick</option>
			</select>

			<!-- Status Filter -->
			<select
				class="rounded border border-border bg-input px-3 py-1.5 font-mono text-xs w-full sm:w-auto"
				value={recordingsStore.statusFilter ?? ''}
				onchange={(e) =>
					recordingsStore.setStatusFilter(
						(e.currentTarget.value || null) as RecordingStatus | null
					)}
			>
				<option value="">All Statuses</option>
				<option value="recording">Recording</option>
				<option value="stopping">Stopping</option>
				<option value="pending_processing">Pending</option>
				<option value="processing">Processing</option>
				<option value="processed">Processed</option>
				<option value="processing_failed">Failed (Retryable)</option>
				<option value="failed">Failed</option>
				<option value="completed">Completed</option>
			</select>

			<!-- Search -->
			<div class="relative">
				<Search class="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-zinc-500" />
				<input
					type="text"
					placeholder="Search..."
					class="rounded border border-border bg-input pl-8 pr-3 py-1.5 font-mono text-xs w-full sm:w-40"
					value={recordingsStore.searchQuery}
					oninput={(e) => recordingsStore.setSearch(e.currentTarget.value)}
				/>
			</div>
		</div>
	</div>

	<!-- Content Area -->
	{#if !connectionStore.isConnected}
		<div class="rounded-lg border border-amber-500/30 bg-amber-500/5 p-4">
			<p class="font-mono text-xs text-amber-400">Not connected to a server.</p>
		</div>
	{:else if recordingsStore.isLoading}
		<div class="flex items-center gap-2 text-zinc-500">
			<div
				class="size-4 animate-spin rounded-full border-2 border-zinc-500 border-t-transparent"
			></div>
			<span class="font-mono text-xs">Loading recordings...</span>
		</div>
	{:else if recordingsStore.error}
		<div class="rounded-lg border border-red-500/30 bg-red-500/5 p-4">
			<p class="font-mono text-xs text-red-400">{recordingsStore.error}</p>
		</div>
	{:else if recordingsStore.filteredRecordings.length === 0}
		<div class="relative border border-border bg-card p-8">
			<CornerBrackets />

			<div class="flex flex-col items-center justify-center gap-2 text-zinc-500">
				<Film class="size-8 opacity-30" />
				<p class="font-mono text-xs">No recordings found</p>
				{#if recordingsStore.platformFilter || recordingsStore.statusFilter || recordingsStore.searchQuery}
					<button
						class="mt-2 rounded border border-border bg-input px-3 py-1.5 font-mono text-xs hover:bg-muted transition-colors"
						onclick={() => {
							recordingsStore.setFilter(null);
							recordingsStore.setStatusFilter(null);
							recordingsStore.setSearch('');
						}}
					>
						Clear filters
					</button>
				{/if}
			</div>
		</div>
	{:else if breakpointStore.isMobile}
		<!-- Mobile: Card Layout -->
		<div class="space-y-2">
			{#each recordingsStore.filteredRecordings as recording (recording.id)}
				<RecordingCard
					{recording}
					onDelete={() => openDeleteModal(recording)}
					onProcess={() => handleProcess(recording.id)}
					onOpenFolder={() => handleOpenFolderClick(recording)}
					showOpenFolder={isLocalConnection}
				/>
			{/each}
		</div>
	{:else}
		<!-- Desktop/Tablet: Table Layout -->
		<div class="relative border border-border bg-card overflow-hidden">
			<CornerBrackets class="z-10" />

			<table class="w-full">
				<thead>
					<tr class="border-b border-border/60 bg-muted/30">
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500 w-20"
							>Status</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500 w-16"
							>Platform</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500 w-32"
							>Channel</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500"
							>Title</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500 w-28"
							>Date</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500 w-20"
							>Duration</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500 w-20"
							>Size</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500 w-28"
							>Actions</th
						>
					</tr>
				</thead>
				<tbody>
					{#each recordingsStore.filteredRecordings as recording (recording.id)}
						<tr class="border-b border-border/30 hover:bg-muted/30 transition-colors">
							<td class="px-4 py-3">
								{#if recording.status === 'processing_failed' && recording.failure_reason}
									<Tooltip.Provider>
										<Tooltip.Root delayDuration={200}>
											<Tooltip.Trigger
												class="flex items-center gap-1.5 cursor-default"
												onmouseenter={() => (hoveredStatusId = recording.id)}
												onmouseleave={() => (hoveredStatusId = null)}
											>
												<div
													class="size-2 rounded-full flex-shrink-0 {RECORDING_STATUS_COLORS[
														recording.status
													] ?? 'bg-zinc-500'}"
												></div>
												<span class="font-mono text-[10px] uppercase text-zinc-500 min-w-[4.5rem]"
													>{recording.status}</span
												>
											</Tooltip.Trigger>
											<Tooltip.Portal>
												<Tooltip.Content
													side="top"
													sideOffset={4}
													class="z-50 max-w-xs rounded border border-zinc-700 bg-zinc-900 px-2 py-1.5 font-mono text-[10px] text-zinc-300 shadow-lg"
												>
													{recording.failure_reason}
												</Tooltip.Content>
											</Tooltip.Portal>
										</Tooltip.Root>
									</Tooltip.Provider>
								{:else}
									<!-- svelte-ignore a11y_no_static_element_interactions -->
									<div
										class="flex items-center gap-1.5 cursor-default"
										onmouseenter={() => (hoveredStatusId = recording.id)}
										onmouseleave={() => (hoveredStatusId = null)}
									>
										<div
											class="size-2 rounded-full flex-shrink-0 {RECORDING_STATUS_COLORS[
												recording.status
											] ?? 'bg-zinc-500'} {recording.status === 'recording' ||
											recording.status === 'processing' ||
											recording.status === 'pending_processing'
												? 'animate-pulse'
												: ''}"
										></div>
										{#if recording.status === 'processing' || recording.status === 'pending_processing'}
											{@const progress = recordingsStore.processingProgress.get(recording.id)}
											{@const isHovered = hoveredStatusId === recording.id}
											{#if isHovered}
												{#if progress !== undefined}
													<span class="font-mono text-[10px] text-blue-400 min-w-[4.5rem]"
														>{progress}%</span
													>
												{:else}
													<span class="font-mono text-[10px] text-amber-400 min-w-[4.5rem]"
														>Queued</span
													>
												{/if}
											{:else}
												<span class="font-mono text-[10px] uppercase text-zinc-500 min-w-[4.5rem]"
													>{recording.status === 'pending_processing'
														? 'pending'
														: 'processing'}</span
												>
											{/if}
										{:else}
											<span class="font-mono text-[10px] uppercase text-zinc-500 min-w-[4.5rem]"
												>{recording.status}</span
											>
										{/if}
									</div>
								{/if}
							</td>
							<td class="px-4 py-3">
								<PlatformIcon
									platform={recording.platform as 'twitch' | 'youtube' | 'kick'}
									class="w-4 h-4 text-zinc-500"
								/>
							</td>
							<td class="px-4 py-3">
								<span class="font-mono text-sm">{recording.channel_name}</span>
							</td>
							<td class="px-4 py-3">
								{#if recording.title}
									<span class="font-mono text-xs text-zinc-400 truncate block max-w-xs"
										>{recording.title}</span
									>
								{:else}
									<span class="font-mono text-[10px] text-zinc-500">—</span>
								{/if}
								{#if recording.game}
									<span class="font-mono text-[10px] text-zinc-500">{recording.game}</span>
								{/if}
							</td>
							<td class="px-4 py-3">
								<span class="font-mono text-xs text-zinc-400"
									>{formatDate(recording.started_at)}</span
								>
							</td>
							<td class="px-4 py-3">
								<span class="font-mono text-xs text-zinc-400"
									>{formatDuration(recording.duration_secs)}</span
								>
							</td>
							<td class="px-4 py-3">
								<span class="font-mono text-xs text-zinc-400"
									>{formatBytes(recording.size_bytes)}</span
								>
							</td>
							<td class="px-4 py-3">
								<div class="flex items-center gap-1">
									{#if isLocalConnection}
										<button
											class="p-1.5 hover:bg-muted rounded transition-colors"
											title="Open Folder"
											onclick={() => handleOpenFolderClick(recording)}
										>
											<FolderOpen class="w-3.5 h-3.5 text-zinc-500" />
										</button>
									{/if}
									{#if recording.status === 'completed' || recording.status === 'failed' || recording.status === 'processed' || (recording.status === 'processing_failed' && (recording.processing_attempts ?? 0) < 5)}
										<button
											class="p-1.5 hover:bg-muted rounded transition-colors"
											title={recording.status === 'processing_failed'
												? 'Retry'
												: recording.status === 'processed'
													? 'Reprocess'
													: 'Process'}
											onclick={() => handleProcess(recording.id)}
										>
											{#if recording.status === 'processing_failed' || recording.status === 'processed'}
												<RotateCcw class="w-3.5 h-3.5 text-zinc-500" />
											{:else}
												<Play class="w-3.5 h-3.5 text-zinc-500" />
											{/if}
										</button>
									{/if}
									<button
										class="p-1.5 hover:bg-red-500/10 rounded transition-colors"
										title="Delete"
										onclick={() => openDeleteModal(recording)}
									>
										<Trash2 class="w-3.5 h-3.5 text-red-400" />
									</button>
								</div>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</div>

<!-- Delete Confirmation Modal -->
<ResponsiveModal
	open={deleteModalOpen}
	onOpenChange={(open) => {
		deleteModalOpen = open;
		if (!open) recordingToDelete = null;
	}}
	title="Delete Recording"
>
	{#if recordingToDelete}
		<div class="space-y-4">
			<p class="font-mono text-sm text-zinc-300">
				Are you sure you want to delete this recording? This action cannot be undone.
			</p>

			<!-- Recording Details -->
			<div class="relative rounded border border-zinc-700 bg-zinc-800/50 p-3">
				<CornerBrackets size="sm" />

				<div class="flex items-center gap-2 mb-2">
					<PlatformIcon
						platform={recordingToDelete.platform as 'twitch' | 'youtube' | 'kick'}
						class="w-4 h-4"
					/>
					<span class="font-mono text-sm text-zinc-100">{recordingToDelete.channel_name}</span>
				</div>

				{#if recordingToDelete.title}
					<p class="font-mono text-xs text-zinc-400 mb-2">{recordingToDelete.title}</p>
				{/if}

				<div class="flex items-center gap-3 text-zinc-500">
					<span class="font-mono text-[10px]">{formatDate(recordingToDelete.started_at)}</span>
					<span class="font-mono text-[10px]"
						>{formatDuration(recordingToDelete.duration_secs)}</span
					>
					<span class="font-mono text-[10px]">{formatBytes(recordingToDelete.size_bytes)}</span>
				</div>
			</div>
		</div>
	{/if}

	{#snippet footer()}
		<div class="flex items-center gap-2">
			<button
				class="flex-1 rounded border border-border bg-input px-3 py-2 font-mono text-xs hover:bg-muted transition-colors"
				onclick={() => {
					deleteModalOpen = false;
					recordingToDelete = null;
				}}
			>
				Cancel
			</button>
			<button
				class="flex-1 rounded border border-red-500/30 bg-red-500/10 px-3 py-2 font-mono text-xs text-red-400 hover:bg-red-500/20 transition-colors flex items-center justify-center gap-2"
				onclick={handleDelete}
			>
				<Trash2 class="w-3.5 h-3.5" />
				Delete
			</button>
		</div>
	{/snippet}
</ResponsiveModal>

<!-- Open Folder Modal for processed recordings -->
<OpenFolderModal
	open={openFolderModalOpen}
	recordingPath={openFolderRecording?.path ?? ''}
	outputFile={openFolderRecording?.output_file}
	onClose={() => {
		openFolderModalOpen = false;
		openFolderRecording = null;
	}}
	onOpenRecording={() => {
		if (openFolderRecording) {
			handleOpenFolder(openFolderRecording.path);
		}
	}}
	onOpenLibrary={handleOpenLibraryFolder}
/>
