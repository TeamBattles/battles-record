<script lang="ts">
	import type { QuotaStatus } from '#lib/api/types.ts';
	import { AlertTriangle } from '@lucide/svelte';

	interface Props {
		isRecording: boolean;
		isLive: boolean;
		quotaStatus?: QuotaStatus;
		quotaPercent?: number;
	}

	let { isRecording, isLive, quotaStatus, quotaPercent }: Props = $props();

	const dotClass = $derived(
		quotaStatus === 'exceeded'
			? 'bg-destructive'
			: isRecording
				? 'bg-orange animate-pulse'
				: isLive
					? 'bg-green'
					: 'bg-dim'
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
		<AlertTriangle class="size-3 text-destructive" />
	{:else if quotaStatus === 'warning'}
		<AlertTriangle class="size-3 text-yellow" />
	{/if}
	<span class="sr-only">{statusLabel}</span>
</div>
