<script lang="ts">
	import { Trash2, Play, FolderOpen, RotateCcw } from '@lucide/svelte';
	import { Tooltip } from 'bits-ui';
	import type { Recording } from '#lib/api/types.ts';
	import { recordingsStore } from '#lib/stores/recordings.svelte.ts';
	import PlatformIcon from './PlatformIcon.svelte';
	import LuxuryCornerSlivers from '#lib/components/ui/LuxuryCornerSlivers.svelte';
	import { formatDuration, formatBytes, formatDate, RECORDING_STATUS_COLORS } from '#lib/utils/index.ts';

	interface Props {
		recording: Recording;
		onDelete: () => void;
		onProcess: () => void;
		onOpenFolder?: () => void;
		showOpenFolder?: boolean;
	}

	let { recording, onDelete, onProcess, onOpenFolder, showOpenFolder = true }: Props = $props();

	const progress = $derived(recordingsStore.getProcessingProgress(recording.id));
	let isHoveringStatus = $state(false);

	// Hide retry button after 5 attempts
	const canRetry = $derived((recording.processing_attempts ?? 0) < 5);
</script>

<div class="group relative rounded border border-line-2 bg-well p-3">
	<LuxuryCornerSlivers corners="tl" length="md" hoverFadeIn />

	<div class="flex items-start justify-between gap-3">
		<div class="flex items-center gap-2 min-w-0">
			<PlatformIcon
				platform={recording.platform as 'twitch' | 'youtube' | 'kick'}
				class="w-4 h-4 text-muted-foreground flex-shrink-0"
			/>
			<span class="font-mono text-sm text-foreground truncate">{recording.channel_name}</span>
		</div>
		{#if recording.status === 'processing_failed' && recording.failure_reason}
			<Tooltip.Provider>
				<Tooltip.Root delayDuration={200}>
					<Tooltip.Trigger
						class="flex items-center gap-1 cursor-default"
						onmouseenter={() => (isHoveringStatus = true)}
						onmouseleave={() => (isHoveringStatus = false)}
					>
						<div
							class="size-2 rounded-full {RECORDING_STATUS_COLORS[recording.status] ??
								'bg-dim'}"
						></div>
						<span class="font-mono text-[10px] uppercase text-muted-foreground">{recording.status}</span>
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
				class="flex items-center gap-1 cursor-default"
				onmouseenter={() => (isHoveringStatus = true)}
				onmouseleave={() => (isHoveringStatus = false)}
			>
				<div
					class="size-2 rounded-full {RECORDING_STATUS_COLORS[recording.status] ??
						'bg-dim'} {recording.status === 'recording' ||
					recording.status === 'processing' ||
					recording.status === 'pending_processing'
						? 'animate-pulse'
						: ''}"
				></div>
				{#if (recording.status === 'processing' || recording.status === 'pending_processing') && isHoveringStatus}
					{#if progress !== undefined}
						<span class="font-mono text-[10px] text-blue">{progress}%</span>
					{:else}
						<span class="font-mono text-[10px] text-yellow">Queued</span>
					{/if}
				{:else}
					<span class="font-mono text-[10px] uppercase text-muted-foreground"
						>{recording.status === 'pending_processing' ? 'pending' : recording.status}</span
					>
				{/if}
			</div>
		{/if}
	</div>

	{#if recording.title}
		<p class="font-mono text-xs text-muted-foreground mt-2 line-clamp-2">{recording.title}</p>
	{/if}

	<div class="flex items-center gap-3 mt-2 text-muted-foreground">
		<span class="font-mono text-[10px]">{formatDate(recording.started_at)}</span>
		<span class="font-mono text-[10px]">{formatDuration(recording.duration_secs)}</span>
		<span class="font-mono text-[10px]">{formatBytes(recording.size_bytes)}</span>
	</div>

	<div class="flex items-center gap-1 mt-3 pt-3 border-t border-line">
		{#if showOpenFolder && onOpenFolder}
			<button
				class="flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded hover:bg-hover-wash transition-colors font-mono text-xs text-muted-foreground"
				onclick={onOpenFolder}
			>
				<FolderOpen size={12} />
				Open
			</button>
		{/if}
		{#if recording.status === 'completed' || recording.status === 'failed' || recording.status === 'processed' || (recording.status === 'processing_failed' && canRetry)}
			<button
				class="flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded hover:bg-hover-wash transition-colors font-mono text-xs text-muted-foreground"
				onclick={onProcess}
			>
				{#if recording.status === 'processing_failed' || recording.status === 'processed'}
					<RotateCcw size={12} />
				{:else}
					<Play size={12} />
				{/if}
				{recording.status === 'processing_failed'
					? 'Retry'
					: recording.status === 'processed'
						? 'Reprocess'
						: 'Process'}
			</button>
		{/if}
		<button
			class="flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded hover:bg-destructive/10 transition-colors font-mono text-xs text-destructive"
			onclick={onDelete}
		>
			<Trash2 size={12} />
			Delete
		</button>
	</div>
</div>
