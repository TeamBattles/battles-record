<script lang="ts">
	import type { Channel } from '$lib/api/types';
	import ChannelAvatar from './ChannelAvatar.svelte';
	import ChannelStatus from './ChannelStatus.svelte';
	import CornerBrackets from './ui/CornerBrackets.svelte';
	import { RefreshCw, Pencil, Trash2, AlertTriangle, ExternalLink, Calendar } from 'lucide-svelte';
	import { PLATFORM_PROFILE_URLS } from '$lib/utils/constants';
	import { open } from '@tauri-apps/plugin-shell';

	interface Props {
		channel: Channel;
		onSelect?: () => void;
		onCheckNow?: () => void;
		onEdit?: () => void;
		onDelete?: () => void;
	}

	let { channel, onSelect, onCheckNow, onEdit, onDelete }: Props = $props();

	// Open streamer's profile on their platform
	async function openProfile() {
		const baseUrl = PLATFORM_PROFILE_URLS[channel.platform];
		await open(`${baseUrl}${channel.name}`);
	}
</script>

<div
	class="relative border border-border bg-card p-4 hover:bg-muted/30 transition-colors cursor-pointer"
	onclick={onSelect}
	onkeydown={(e) => e.key === 'Enter' && onSelect?.()}
	role="button"
	tabindex="0"
>
	<CornerBrackets size="sm" />

	<!-- Header: Avatar + Name + Status -->
	<div class="flex items-center gap-3 mb-2">
		<ChannelAvatar
			src={channel.profile_image_url}
			alt={channel.name}
			platform={channel.platform}
			size="sm"
		/>
		<span class="font-mono text-sm flex-1">{channel.name}</span>
		<ChannelStatus
			isRecording={channel.status?.is_recording ?? false}
			isLive={channel.status?.is_live ?? false}
			quotaStatus={channel.quota_status}
			quotaPercent={channel.quota_percent}
		/>
	</div>

	<!-- Stream Info (if live) -->
	{#if channel.status?.current_stream}
		<p class="font-mono text-[10px] text-zinc-500 truncate mb-2">
			{channel.status.current_stream.title || channel.status.current_stream.game || 'Live'}
		</p>
	{:else}
		<p class="font-mono text-[10px] text-zinc-500 mb-2">Offline</p>
	{/if}

	<!-- Quota Warning -->
	{#if channel.quota_status === 'exceeded'}
		<div class="flex items-center gap-1.5 text-red-400 mb-2">
			<AlertTriangle class="size-3" />
			<span class="font-mono text-[10px]">Quota exceeded - recordings paused</span>
		</div>
	{:else if channel.quota_status === 'warning'}
		<div class="flex items-center gap-1.5 text-amber-400 mb-2">
			<AlertTriangle class="size-3" />
			<span class="font-mono text-[10px]">{channel.quota_percent}% of quota used</span>
		</div>
	{/if}

	<!-- Quality + Schedule + Actions -->
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-2">
			<span
				class="rounded bg-zinc-200 px-1.5 py-0.5 font-mono text-[9px] uppercase text-zinc-600 dark:bg-zinc-800 dark:text-zinc-500"
			>
				{channel.quality}
			</span>
			{#if channel.schedule_enabled}
				<Calendar class="size-3.5 text-blue-400" />
			{/if}
		</div>

		<!-- Actions -->
		<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div class="flex items-center gap-1" onclick={(e) => e.stopPropagation()} role="group">
			<button
				class="p-1.5 hover:bg-muted rounded transition-colors"
				onclick={openProfile}
				aria-label="Open profile"
				title="Open {channel.platform} profile"
			>
				<ExternalLink size={14} class="text-zinc-500" />
			</button>
			<button
				class="p-1.5 hover:bg-muted rounded transition-colors"
				onclick={onCheckNow}
				aria-label="Check now"
			>
				<RefreshCw size={14} class="text-zinc-500" />
			</button>
			<button
				class="p-1.5 hover:bg-muted rounded transition-colors"
				onclick={onEdit}
				aria-label="Edit"
			>
				<Pencil size={14} class="text-zinc-500" />
			</button>
			<button
				class="p-1.5 hover:bg-red-500/10 rounded transition-colors"
				onclick={onDelete}
				aria-label="Delete"
			>
				<Trash2 size={14} class="text-red-400" />
			</button>
		</div>
	</div>
</div>
