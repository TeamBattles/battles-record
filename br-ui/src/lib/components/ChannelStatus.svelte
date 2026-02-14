<script lang="ts">
	import type { QuotaStatus } from '$lib/api/types';
	import { AlertTriangle } from 'lucide-svelte';

	interface Props {
		isRecording: boolean;
		isLive: boolean;
		quotaStatus?: QuotaStatus;
		quotaPercent?: number;
	}

	let { isRecording, isLive, quotaStatus, quotaPercent }: Props = $props();

	const dotClass = $derived(
		quotaStatus === 'exceeded'
			? 'bg-red-400'
			: isRecording
				? 'bg-orange-400 animate-pulse'
				: isLive
					? 'bg-emerald-400'
					: 'bg-zinc-500'
	);

	const statusLabel = $derived(
		quotaStatus === 'exceeded'
			? 'Paused (quota)'
			: isRecording
				? 'Recording'
				: isLive
					? 'Live'
					: 'Offline'
	);
</script>

<div class="flex items-center gap-1.5">
	<span class="size-2 rounded-full {dotClass}"></span>
	{#if quotaStatus === 'exceeded'}
		<AlertTriangle class="size-3 text-red-400" />
	{:else if quotaStatus === 'warning'}
		<AlertTriangle class="size-3 text-amber-400" />
	{/if}
	<span class="sr-only">{statusLabel}</span>
</div>
