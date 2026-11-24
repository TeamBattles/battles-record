<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Check } from '@lucide/svelte';
	import { cn } from '#lib/utils/cn.ts';

	/** A pressable tile with a visual preview above its label (theme and accent pickers). */
	interface Props {
		selected: boolean;
		onSelect: () => void;
		label: string;
		preview: Snippet;
		disabled?: boolean;
		class?: string;
	}

	let { selected, onSelect, label, preview, disabled = false, class: className }: Props = $props();
</script>

<button
	type="button"
	aria-pressed={selected}
	{disabled}
	onclick={onSelect}
	class={cn(
		'relative flex min-w-0 flex-col items-stretch gap-2 rounded-lg border p-2 text-left transition-colors',
		'hover:bg-hover-wash focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none',
		'disabled:cursor-not-allowed disabled:opacity-50',
		selected ? 'border-primary bg-primary/5' : 'border-border',
		className
	)}
>
	{@render preview()}
	<span class="flex items-center justify-between gap-1 px-0.5">
		<span
			class={cn(
				'truncate font-mono text-[11px] tracking-wider uppercase',
				selected ? 'text-foreground' : 'text-muted-foreground'
			)}
		>
			{label}
		</span>
		{#if selected}
			<Check class="size-3.5 shrink-0 text-primary" />
		{/if}
	</span>
</button>
