<script lang="ts">
	import PageHeader from '#lib/components/ui/PageHeader.svelte';
	import { Search, Trash2, Play, FolderOpen, Film, RotateCcw, Video } from '@lucide/svelte';
	import { Tooltip } from 'bits-ui';
	import { invoke } from '@tauri-apps/api/core';
	import { recordingsStore } from '#lib/stores/recordings.svelte.ts';
	import { connectionStore } from '#lib/stores/connection.svelte.ts';
	import { breakpointStore, ResponsiveModal } from '#lib';
	import PlatformIcon from '#lib/components/PlatformIcon.svelte';
	import RecordingCard from '#lib/components/RecordingCard.svelte';
	import OpenFolderModal from '#lib/components/OpenFolderModal.svelte';
	import LuxuryCornerSlivers from '#lib/components/ui/LuxuryCornerSlivers.svelte';
	import type { Recording, RecordingStatus } from '#lib/api/types.ts';
	import Select from '#lib/components/ui/Select.svelte';

	const PLATFORM_FILTER_OPTIONS: { value: string | null; label: string }[] = [
		{ value: null, label: 'All Platforms' },
		{ value: 'twitch', label: 'Twitch' },
		{ value: 'youtube', label: 'YouTube' },
		{ value: 'kick', label: 'Kick' }
	];

	const STATUS_FILTER_OPTIONS: { value: RecordingStatus | null; label: string }[] = [
		{ value: null, label: 'All Statuses' },
		{ value: 'recording', label: 'Recording' },
		{ value: 'stopping', label: 'Stopping' },
		{ value: 'pending_processing', label: 'Pending' },
		{ value: 'processing', label: 'Processing' },
		{ value: 'processed', label: 'Processed' },
		{ value: 'processing_failed', label: 'Failed (Retryable)' },
		{ value: 'failed', label: 'Failed' },
		{ value: 'completed', label: 'Completed' }
	];
	import { formatDuration, formatBytes, formatDate, RECORDING_STATUS_COLORS } from '#lib/utils/index.ts';
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
	<PageHeader
		title="Recordings"
		eyebrow="Record"
		subtitle="Everything captured from your channels"
		icon={Video}
		count={recordingsStore.filteredRecordings.length}
	>
		{#snippet actions()}
			<!-- Platform Filter -->
			<Select
				size="sm"
				class="sm:w-48"
				ariaLabel="Filter by platform"
				value={recordingsStore.platformFilter ?? null}
				options={PLATFORM_FILTER_OPTIONS}
				onValueChange={(platform) => recordingsStore.setFilter(platform)}
			/>

			<!-- Status Filter -->
			<Select
				size="sm"
				class="sm:w-48"
				ariaLabel="Filter by status"
				value={recordingsStore.statusFilter ?? null}
				options={STATUS_FILTER_OPTIONS}
				onValueChange={(status) => recordingsStore.setStatusFilter(status)}
			/>

			<!-- Search -->
			<div class="relative">
				<Search class="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground" />
				<input
					type="text"
					placeholder="Search..."
					class="rounded border border-border bg-well pl-8 pr-3 py-1.5 font-mono text-xs w-full sm:w-40"
					value={recordingsStore.searchQuery}
					oninput={(e) => recordingsStore.setSearch(e.currentTarget.value)}
				/>
			</div>
		{/snippet}
	</PageHeader>

	<!-- Content Area -->
	{#if !connectionStore.isConnected}
		<div class="rounded-lg border border-yellow/30 bg-yellow/5 p-4">
			<p class="font-mono text-xs text-yellow">Not connected to a server.</p>
		</div>
	{:else if recordingsStore.isLoading}
		<div class="flex items-center gap-2 text-muted-foreground">
			<div
				class="size-4 animate-spin rounded-full border-2 border-line-3 border-t-transparent"
			></div>
			<span class="font-mono text-xs">Loading recordings...</span>
		</div>
	{:else if recordingsStore.error}
		<div class="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
			<p class="font-mono text-xs text-destructive">{recordingsStore.error}</p>
		</div>
	{:else if recordingsStore.filteredRecordings.length === 0}
		<div class="relative rounded-lg border border-border bg-card p-8">

			<div class="flex flex-col items-center justify-center gap-2 text-muted-foreground">
				<Film class="size-8 opacity-30" />
				<p class="font-mono text-xs">No recordings found</p>
				{#if recordingsStore.platformFilter || recordingsStore.statusFilter || recordingsStore.searchQuery}
					<button
						class="mt-2 rounded border border-border bg-btn-bg px-3 py-1.5 font-mono text-xs hover:bg-hover-wash transition-colors"
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
		<div class="relative rounded-lg border border-border bg-card overflow-hidden">

			<table class="w-full">
				<thead>
					<tr class="border-b border-border/60 bg-muted/30">
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground w-20"
							>Status</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground w-16"
							>Platform</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground w-32"
							>Channel</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground"
							>Title</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground w-28"
							>Date</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground w-20"
							>Duration</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground w-20"
							>Size</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground w-28"
							>Actions</th
						>
					</tr>
				</thead>
				<tbody>
					{#each recordingsStore.filteredRecordings as recording (recording.id)}
						<tr class="border-b border-border/30 hover:bg-hover-wash transition-colors">
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
													] ?? 'bg-dim'}"
												></div>
												<span class="font-mono text-[10px] uppercase text-muted-foreground min-w-[4.5rem]"
													>{recording.status}</span
												>
											</Tooltip.Trigger>
											<Tooltip.Portal>
												<Tooltip.Content
													side="top"
													sideOffset={4}
													class="z-50 max-w-xs rounded border border-line-2 bg-well px-2 py-1.5 font-mono text-[10px] text-text-2 shadow-lg"
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
											] ?? 'bg-dim'} {recording.status === 'recording' ||
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
													<span class="font-mono text-[10px] text-blue min-w-[4.5rem]"
														>{progress}%</span
													>
												{:else}
													<span class="font-mono text-[10px] text-yellow min-w-[4.5rem]"
														>Queued</span
													>
												{/if}
											{:else}
												<span class="font-mono text-[10px] uppercase text-muted-foreground min-w-[4.5rem]"
													>{recording.status === 'pending_processing'
														? 'pending'
														: 'processing'}</span
												>
											{/if}
										{:else}
											<span class="font-mono text-[10px] uppercase text-muted-foreground min-w-[4.5rem]"
												>{recording.status}</span
											>
										{/if}
									</div>
								{/if}
							</td>
							<td class="px-4 py-3">
								<PlatformIcon
									platform={recording.platform as 'twitch' | 'youtube' | 'kick'}
									class="w-4 h-4 text-muted-foreground"
								/>
							</td>
							<td class="px-4 py-3">
								<span class="font-mono text-sm">{recording.channel_name}</span>
							</td>
							<td class="px-4 py-3">
								{#if recording.title}
									<span class="font-mono text-xs text-muted-foreground truncate block max-w-xs"
										>{recording.title}</span
									>
								{:else}
									<span class="font-mono text-[10px] text-muted-foreground">—</span>
								{/if}
								{#if recording.game}
									<span class="font-mono text-[10px] text-muted-foreground">{recording.game}</span>
								{/if}
							</td>
							<td class="px-4 py-3">
								<span class="font-mono text-xs text-muted-foreground"
									>{formatDate(recording.started_at)}</span
								>
							</td>
							<td class="px-4 py-3">
								<span class="font-mono text-xs text-muted-foreground"
									>{formatDuration(recording.duration_secs)}</span
								>
							</td>
							<td class="px-4 py-3">
								<span class="font-mono text-xs text-muted-foreground"
									>{formatBytes(recording.size_bytes)}</span
								>
							</td>
							<td class="px-4 py-3">
								<div class="flex items-center gap-1">
									{#if isLocalConnection}
										<button
											class="p-1.5 hover:bg-hover-wash rounded transition-colors"
											title="Open Folder"
											onclick={() => handleOpenFolderClick(recording)}
										>
											<FolderOpen class="w-3.5 h-3.5 text-muted-foreground" />
										</button>
									{/if}
									{#if recording.status === 'completed' || recording.status === 'failed' || recording.status === 'processed' || (recording.status === 'processing_failed' && (recording.processing_attempts ?? 0) < 5)}
										<button
											class="p-1.5 hover:bg-hover-wash rounded transition-colors"
											title={recording.status === 'processing_failed'
												? 'Retry'
												: recording.status === 'processed'
													? 'Reprocess'
													: 'Process'}
											onclick={() => handleProcess(recording.id)}
										>
											{#if recording.status === 'processing_failed' || recording.status === 'processed'}
												<RotateCcw class="w-3.5 h-3.5 text-muted-foreground" />
											{:else}
												<Play class="w-3.5 h-3.5 text-muted-foreground" />
											{/if}
										</button>
									{/if}
									<button
										class="p-1.5 hover:bg-destructive/10 rounded transition-colors"
										title="Delete"
										onclick={() => openDeleteModal(recording)}
									>
										<Trash2 class="w-3.5 h-3.5 text-destructive" />
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
			<p class="font-mono text-sm text-text-2">
				Are you sure you want to delete this recording? This action cannot be undone.
			</p>

			<!-- Recording Details -->
			<div class="group relative rounded border border-line-2 bg-chip/50 p-3">
				<LuxuryCornerSlivers corners="tl" length="md" hoverFadeIn />

				<div class="flex items-center gap-2 mb-2">
					<PlatformIcon
						platform={recordingToDelete.platform as 'twitch' | 'youtube' | 'kick'}
						class="w-4 h-4"
					/>
					<span class="font-mono text-sm text-foreground">{recordingToDelete.channel_name}</span>
				</div>

				{#if recordingToDelete.title}
					<p class="font-mono text-xs text-muted-foreground mb-2">{recordingToDelete.title}</p>
				{/if}

				<div class="flex items-center gap-3 text-muted-foreground">
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
				class="flex-1 rounded border border-border bg-btn-bg px-3 py-2 font-mono text-xs hover:bg-hover-wash transition-colors"
				onclick={() => {
					deleteModalOpen = false;
					recordingToDelete = null;
				}}
			>
				Cancel
			</button>
			<button
				class="flex-1 rounded border border-destructive/30 bg-destructive/10 px-3 py-2 font-mono text-xs text-destructive hover:bg-destructive/20 transition-colors flex items-center justify-center gap-2"
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
