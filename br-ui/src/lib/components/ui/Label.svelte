<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLLabelAttributes } from 'svelte/elements';
	import { cn } from '$lib/utils/cn';

	interface Props extends Omit<HTMLLabelAttributes, 'class'> {
		children: Snippet;
		class?: string;
		required?: boolean;
	}

	let { children, class: className, required = false, ...restProps }: Props = $props();
</script>

<label
	class={cn('font-mono text-[10px] uppercase tracking-wider text-zinc-500', className)}
	{...restProps}
>
	{@render children()}
	{#if required}
		<span class="text-red-400">*</span>
	{/if}
</label>
