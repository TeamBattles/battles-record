<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';
	import { Check } from '@lucide/svelte';
	import { cn } from '#lib/utils/cn.ts';

	/**
	 * Checkbox drawn by the app rather than the platform. A native checkbox ignores border, radius
	 * and background classes (WebKitGTK paints its own GTK widget), so it never matched the theme or
	 * the density scale. The real input stays, with `appearance-none`, for keyboard, form and
	 * screen-reader behaviour; the check icon is decoration layered on top of it.
	 */
	interface Props extends Omit<HTMLInputAttributes, 'class' | 'type'> {
		checked?: boolean;
		label?: string;
		labelClass?: string;
		class?: string;
	}

	let {
		checked = $bindable(false),
		label,
		labelClass,
		class: className,
		...restProps
	}: Props = $props();
</script>

<label class={cn('flex items-center gap-2 cursor-pointer', className)}>
	<span class="relative flex size-4 shrink-0">
		<input
			type="checkbox"
			bind:checked
			class={cn(
				'peer size-full cursor-[inherit] appearance-none rounded-[min(var(--radius),5px)] border border-line-2 bg-well transition-colors',
				'checked:border-primary checked:bg-primary',
				'focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none',
				'disabled:cursor-not-allowed disabled:opacity-50'
			)}
			{...restProps}
		/>
		<Check
			class="pointer-events-none absolute inset-0 m-auto size-3 text-primary-foreground opacity-0 transition-opacity peer-checked:opacity-100 peer-disabled:peer-checked:opacity-50"
			strokeWidth={3}
		/>
	</span>
	{#if label}
		<span class={cn('font-mono text-sm text-foreground', labelClass)}>{label}</span>
	{/if}
</label>
