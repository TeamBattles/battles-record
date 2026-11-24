<script lang="ts">
	import { cn } from '#lib/utils/cn.ts';

	/**
	 * Accent-filled toggle switch; give it `label` or `labelledby` so it has an accessible name.
	 *
	 * The thumb is sized and placed from the track (percentages of its padding box plus half a
	 * spacing unit of inset), never fixed pixels, so it stays inside the track when Compact density
	 * shrinks the spacing scale or high contrast thickens the border.
	 */
	interface Props {
		checked: boolean;
		onCheckedChange: (checked: boolean) => void;
		label?: string;
		labelledby?: string;
		disabled?: boolean;
		class?: string;
	}

	let {
		checked,
		onCheckedChange,
		label,
		labelledby,
		disabled = false,
		class: className
	}: Props = $props();
</script>

<button
	type="button"
	role="switch"
	aria-checked={checked}
	aria-label={label}
	aria-labelledby={labelledby}
	{disabled}
	class={cn(
		'relative inline-flex h-6 w-11 shrink-0 rounded-full border transition-colors',
		'focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none',
		'disabled:cursor-not-allowed disabled:opacity-50',
		checked ? 'border-primary bg-primary' : 'border-line-2 bg-chip-strong',
		className
	)}
	onclick={() => onCheckedChange(!checked)}
>
	<span
		class={cn(
			'absolute top-0.5 aspect-square h-[calc(100%-var(--spacing))] rounded-full shadow-sm transition-all',
			checked
				? 'left-[calc(100%-var(--spacing)*0.5)] -translate-x-full bg-primary-foreground'
				: 'left-0.5 bg-text-2'
		)}
	></span>
</button>
