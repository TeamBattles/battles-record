<script lang="ts">
	import {
		Pause,
		Play,
		X,
		ArrowUp,
		Trash2,
		AlertTriangle,
		RefreshCw,
		GitMerge,
		FileVideo,
		FolderOpen
	} from '@lucide/svelte';
	import { Tween } from 'svelte/motion';
	import { cubicOut } from 'svelte/easing';
	import type { DownloadSummary } from '#lib/api/types.ts';
	import LuxuryCornerSlivers from '#lib/components/ui/LuxuryCornerSlivers.svelte';
	import StatusDot from './ui/StatusDot.svelte';
	import Button from './ui/Button.svelte';
	import { cn } from '#lib/utils/cn.ts';
	import { formatBytes, formatDate, formatEta, formatPlatformName } from '#lib/utils/index.ts';
	import { DOWNLOAD_STATUS_COLORS, DOWNLOAD_STATUS_LABELS } from '#lib/utils/constants.ts';

	interface Props {
		download: DownloadSummary;
		onPause?: (id: string) => void;
		onResume?: (id: string) => void;
		onCancel?: (id: string) => void;
		onPrioritize?: (id: string) => void;
		onRemove?: (id: string) => void;
		onMerge?: (platform: string, channel: string) => void;
		/** Omit (with onOpenFolder) when the files are on another machine. */
		onOpenFile?: (path: string) => void;
		onOpenFolder?: (download: DownloadSummary) => void;
	}

	let {
		download,
		onPause,
		onResume,
		onCancel,
		onPrioritize,
		onRemove,
		onMerge,
		onOpenFile,
		onOpenFolder
	}: Props = $props();

	const statusColor = $derived(
		(DOWNLOAD_STATUS_COLORS[download.status] ?? 'offline') as
			| 'recording'
			| 'live'
			| 'offline'
			| 'error'
			| 'success'
			| 'warning'
			| 'info'
	);
	const statusLabel = $derived(DOWNLOAD_STATUS_LABELS[download.status] ?? download.status);
	const isActive = $derived(
		download.status === 'downloading' ||
			download.status === 'extracting_info' ||
			download.status === 'processing'
	);
	// A broadcast still running has no total, so there is no percent until it ends.
	const isLive = $derived(download.live === true && download.status === 'downloading');
	const isTerminal = $derived(
		download.status === 'complete' ||
			download.status === 'cancelled' ||
			download.status === 'failed'
	);

	const progress = new Tween(download.percent, { duration: 400, easing: cubicOut });
	$effect(() => {
		progress.set(download.percent);
	});
</script>

<div class="group relative flex flex-col rounded border border-border bg-card p-3">
	<LuxuryCornerSlivers corners="tl" length="md" hoverFadeIn />

	<!-- Header: status + platform badge -->
	<div class="flex items-start justify-between gap-3">
		<div class="flex items-center gap-2 min-w-0">
			<StatusDot status={statusColor} pulse={isActive} size="sm" />
			<span class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
				{statusLabel}
			</span>
			{#if isLive}
				<span
					class="rounded bg-green/15 px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-wider text-green"
					title="Recording the broadcast from its start; finishes when the stream ends"
				>
					Live
				</span>
			{/if}
		</div>
		<span
			class="shrink-0 rounded bg-chip px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground"
		>
			{formatPlatformName(download.source_platform)}
		</span>
	</div>

	<!-- Title -->
	{#if download.title}
		<p class="font-mono text-xs text-foreground mt-2 truncate" title={download.title}>
			{download.title}
		</p>
	{:else}
		<p class="font-mono text-xs text-muted-foreground mt-2 truncate" title={download.url}>
			{download.url}
		</p>
	{/if}

	<!-- Channel name -->
	<p class="font-mono text-[10px] text-muted-foreground mt-1">{download.channel_name}</p>

	<!-- Progress bar (when downloading) -->
	{#if download.status === 'downloading' || download.status === 'processing'}
		<div class="mt-2 space-y-1">
			<div class="h-1.5 w-full rounded-full bg-chip overflow-hidden">
				<div
					class={cn(
						'h-full rounded-full',
						download.status === 'processing' ? 'bg-blue' : 'bg-green',
						isLive && 'bg-green/40 motion-safe:animate-pulse'
					)}
					style="width: {isLive ? 100 : Math.min(progress.current, 100)}%"
				></div>
			</div>
			<div class="flex flex-wrap items-center gap-x-3 gap-y-1 text-muted-foreground">
				{#if !isLive}
					<span class="font-mono text-[10px]">{progress.current.toFixed(1)}%</span>
				{/if}
				{#if download.speed}
					<span class="font-mono text-[10px]">{download.speed}</span>
				{/if}
				{#if download.eta}
					<span class="font-mono text-[10px]">ETA {formatEta(download.eta)}</span>
				{/if}
				{#if download.downloaded_bytes > 0}
					<span class="font-mono text-[10px]">
						{formatBytes(download.downloaded_bytes)}{download.total_bytes
							? ` / ${formatBytes(download.total_bytes)}`
							: ''}
					</span>
				{/if}
			</div>
		</div>
	{:else if download.downloaded_bytes > 0}
		<!-- Size info for non-active states -->
		<div class="flex items-center gap-3 mt-2 text-muted-foreground">
			<span class="font-mono text-[10px]">
				{formatBytes(download.downloaded_bytes)}{download.total_bytes
					? ` / ${formatBytes(download.total_bytes)}`
					: ''}
			</span>
			{#if download.format}
				<span class="font-mono text-[10px]">{download.format}</span>
			{/if}
		</div>
	{/if}

	<!-- Error message -->
	{#if download.status === 'failed' && download.error}
		<div class="mt-2 rounded border border-destructive/20 bg-destructive/5 px-2 py-1.5">
			<div class="flex items-start gap-1.5">
				<AlertTriangle class="w-3 h-3 text-destructive mt-0.5 shrink-0" />
				<span class="font-mono text-[10px] text-destructive break-all">{download.error}</span>
			</div>
			{#if download.update_available}
				<Button
					intent="ghost"
					size="sm"
					class="mt-1.5 border border-yellow/30 bg-yellow/10 text-yellow hover:bg-yellow/20"
					onclick={() => {
						/* TODO: trigger yt-dlp update */
					}}
				>
					{#snippet children()}
						<RefreshCw class="w-3 h-3" />
						Update yt-dlp
					{/snippet}
				</Button>
			{/if}
		</div>
	{/if}

	<!-- Date -->
	<div class="flex items-center gap-3 mt-2 mb-3 text-muted-foreground">
		<span class="font-mono text-[10px]">{formatDate(download.created_at)}</span>
	</div>

	<!-- Action buttons -->
	<!-- Grid rows stretch every card to the tallest; mt-auto pins the actions to the bottom -->
	<div class="flex items-center gap-1 mt-auto pt-3 border-t border-line">
		{#if download.status === 'downloading' || download.status === 'extracting_info' || download.status === 'processing'}
			{#if onPause}
				<Button
					intent="ghost"
					size="sm"
					class="flex-1"
					onclick={() => onPause(download.id)}
				>
					{#snippet children()}
						<Pause size={12} />
						Pause
					{/snippet}
				</Button>
			{/if}
		{/if}

		{#if download.status === 'paused'}
			{#if onResume}
				<Button
					intent="ghost"
					size="sm"
					class="flex-1"
					onclick={() => onResume(download.id)}
				>
					{#snippet children()}
						<Play size={12} />
						Resume
					{/snippet}
				</Button>
			{/if}
		{/if}

		{#if download.status === 'queued'}
			{#if onPrioritize}
				<Button
					intent="ghost"
					size="sm"
					class="flex-1"
					onclick={() => onPrioritize(download.id)}
				>
					{#snippet children()}
						<ArrowUp size={12} />
						Prioritize
					{/snippet}
				</Button>
			{/if}
		{/if}

		{#if !isTerminal}
			{#if onCancel}
				<Button
					intent="danger"
					size="sm"
					class="flex-1"
					onclick={() => onCancel(download.id)}
				>
					{#snippet children()}
						<X size={12} />
						Cancel
					{/snippet}
				</Button>
			{/if}
		{/if}

		{#if download.status === 'complete' && download.output_file && onOpenFile}
			{@const outputFile = download.output_file}
			<Button intent="ghost" size="sm" class="flex-1" onclick={() => onOpenFile(outputFile)}>
				<FileVideo size={12} />
				Open file
			</Button>
		{/if}

		{#if isTerminal}
			{#if onRemove}
				<Button
					intent="danger"
					size="sm"
					class="flex-1"
					onclick={() => onRemove(download.id)}
				>
					{#snippet children()}
						<Trash2 size={12} />
						Remove
					{/snippet}
				</Button>
			{/if}
		{/if}

		{#if onOpenFolder}
			<Button
				intent="ghost"
				size="sm"
				title="Open folder"
				aria-label="Open folder"
				onclick={() => onOpenFolder(download)}
			>
				<FolderOpen size={12} />
			</Button>
		{/if}

		{#if onMerge}
			<Button
				intent="ghost"
				size="sm"
				title="Merge into another channel"
				onclick={() => onMerge(download.source_platform, download.channel_name)}
			>
				{#snippet children()}
					<GitMerge size={12} />
				{/snippet}
			</Button>
		{/if}
	</div>
</div>
