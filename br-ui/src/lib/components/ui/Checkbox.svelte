<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';
	import { cn } from '$lib/utils/cn';

	interface Props extends Omit<HTMLInputAttributes, 'class' | 'type'> {
		checked?: boolean;
		label?: string;
		class?: string;
	}

	let { checked = $bindable(false), label, class: className, ...restProps }: Props = $props();
</script>

<label class={cn('flex items-center gap-2 cursor-pointer', className)}>
	<input
		type="checkbox"
		bind:checked
		class={cn(
			'size-4 rounded border border-border bg-input',
			'text-emerald-500 focus:ring-2 focus:ring-emerald-500 focus:ring-offset-1 focus:ring-offset-background',
			'disabled:cursor-not-allowed disabled:opacity-50'
		)}
		{...restProps}
	/>
	{#if label}
		<span class="font-mono text-sm text-foreground">{label}</span>
	{/if}
</label>
