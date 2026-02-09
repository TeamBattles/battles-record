<script lang="ts">
	interface Props {
		percent: number;
		size?: 'sm' | 'md' | 'lg';
		showLabel?: boolean;
		warningThreshold?: number;
		dangerThreshold?: number;
	}

	let {
		percent,
		size = 'md',
		showLabel = false,
		warningThreshold = 90,
		dangerThreshold = 100
	}: Props = $props();

	const barColor = $derived(
		percent >= dangerThreshold
			? 'bg-red-500'
			: percent >= warningThreshold
				? 'bg-amber-500'
				: 'bg-emerald-500'
	);

	const heightClass = $derived(size === 'sm' ? 'h-1' : size === 'md' ? 'h-2' : 'h-3');
</script>

<div class="w-full">
	<div class="relative {heightClass} rounded-full bg-zinc-200 dark:bg-zinc-700 overflow-hidden">
		<div
			class="absolute inset-y-0 left-0 {barColor} rounded-full transition-all duration-300"
			style="width: {Math.min(percent, 100)}%"
		></div>
	</div>
	{#if showLabel}
		<span class="font-mono text-[10px] text-zinc-500 mt-0.5 block">{percent}%</span>
	{/if}
</div>
