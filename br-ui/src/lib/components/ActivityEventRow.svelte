<script lang="ts">
	import type { ActivityEvent } from '$lib/stores/activity.svelte';
	import {
		Circle,
		AlertCircle,
		PlayCircle,
		StopCircle,
		Radio,
		Cog,
		HardDrive,
		Calendar,
		Filter,
		Wifi,
		AlertTriangle,
		CheckCircle,
		XCircle
	} from 'lucide-svelte';

	interface Props {
		event: ActivityEvent;
		isSelected: boolean;
		onclick: () => void;
	}

	let { event, isSelected, onclick }: Props = $props();

	function getIcon(type: string) {
		switch (type) {
			case 'recording_started':
				return PlayCircle;
			case 'recording_ended':
				return StopCircle;
			case 'channel_status':
				return Radio;
			case 'channel_error':
				return AlertCircle;
			case 'processing_started':
				return Cog;
			case 'processing_complete':
				return CheckCircle;
			case 'processing_failed':
				return XCircle;
			case 'disk_warning':
				return HardDrive;
			case 'config_reloaded':
				return Cog;
			case 'schedule_skip':
				return Calendar;
			case 'filter_skip':
				return Filter;
			case 'connected':
				return Wifi;
			default:
				return Circle;
		}
	}

	function getIconColor(type: string): string {
		switch (type) {
			case 'recording_started':
				return 'text-orange-400';
			case 'recording_ended':
				return 'text-zinc-400';
			case 'channel_status':
				return (event.data.status as string) === 'live' ? 'text-emerald-400' : 'text-zinc-500';
			case 'channel_error':
			case 'processing_failed':
				return 'text-red-400';
			case 'processing_started':
				return 'text-blue-400';
			case 'processing_complete':
				return 'text-emerald-400';
			case 'disk_warning':
				return 'text-amber-400';
			case 'schedule_skip':
			case 'filter_skip':
				return 'text-zinc-500';
			case 'connected':
				return 'text-emerald-400';
			default:
				return 'text-zinc-500';
		}
	}

	function formatTime(date: Date): string {
		return date.toLocaleTimeString('en-US', {
			hour: '2-digit',
			minute: '2-digit',
			second: '2-digit',
			hour12: false
		});
	}

	function formatDate(date: Date): string {
		const today = new Date();
		const isToday = date.toDateString() === today.toDateString();

		if (isToday) {
			return formatTime(date);
		}

		return (
			date.toLocaleDateString('en-US', {
				month: 'short',
				day: 'numeric'
			}) +
			' ' +
			formatTime(date)
		);
	}

	const Icon = $derived(getIcon(event.type));
	const iconColor = $derived(getIconColor(event.type));
</script>

<button
	type="button"
	class="w-full flex items-start gap-3 px-4 py-2.5 text-left hover:bg-muted/30 transition-colors border-b border-border/30 {isSelected
		? 'bg-muted/50'
		: ''}"
	{onclick}
>
	<!-- Icon -->
	<div class="flex-shrink-0 mt-0.5">
		<Icon class="size-4 {iconColor}" />
	</div>

	<!-- Content -->
	<div class="flex-1 min-w-0">
		<div class="flex items-center gap-2 flex-wrap">
			<!-- Message -->
			<span class="font-mono text-xs text-foreground truncate">{event.message}</span>
		</div>

		<!-- Meta row -->
		<div class="flex items-center gap-2 mt-0.5">
			<!-- Timestamp -->
			<span class="font-mono text-[10px] text-zinc-500">{formatDate(event.timestamp)}</span>

			<!-- Category badge -->
			<span
				class="rounded bg-zinc-200 px-1.5 py-0.5 font-mono text-[9px] uppercase text-zinc-600 dark:bg-zinc-800 dark:text-zinc-500"
			>
				{event.category}
			</span>

			<!-- Channel badge (if applicable) -->
			{#if event.channelName}
				<span
					class="rounded bg-zinc-200 px-1.5 py-0.5 font-mono text-[9px] text-zinc-600 dark:bg-zinc-800 dark:text-zinc-500"
				>
					{event.channelName}
				</span>
			{/if}
		</div>
	</div>
</button>
