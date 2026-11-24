<script lang="ts" generics="T extends string">
	import type { Snippet } from 'svelte';
	import { cn } from '#lib/utils/cn.ts';

	/** A one-of-many choice rendered as a strip of pressed/unpressed buttons. */
	interface Props {
		options: readonly { value: T; label: string }[];
		value: T;
		onChange: (value: T) => void;
		ariaLabel: string;
		/** Optional glyph drawn before each option's label. */
		glyph?: Snippet<[T]>;
		class?: string;
	}

	let { options, value, onChange, ariaLabel, glyph, class: className }: Props = $props();
</script>

<div
	role="group"
	aria-label={ariaLabel}
	class={cn(
		'inline-flex flex-wrap items-center gap-1 rounded-md border border-border bg-well p-1',
		className
	)}
>
	{#each options as option (option.value)}
		{@const selected = option.value === value}
		<button
			type="button"
			aria-pressed={selected}
			onclick={() => onChange(option.value)}
			class={cn(
				'inline-flex h-8 items-center gap-2 rounded px-3 font-mono text-[11px] tracking-wider uppercase transition-colors',
				'focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none',
				selected
					? 'bg-chip-strong text-foreground shadow-sm'
					: 'text-muted-foreground hover:bg-hover-wash hover:text-text-2'
			)}
		>
			{#if glyph}
				{@render glyph(option.value)}
			{/if}
			{option.label}
		</button>
	{/each}
</div>
