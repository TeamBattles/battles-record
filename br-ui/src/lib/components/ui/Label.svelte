<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLLabelAttributes } from 'svelte/elements';
	import { cn } from '#lib/utils/cn.ts';

	interface Props extends Omit<HTMLLabelAttributes, 'class'> {
		children: Snippet;
		class?: string;
		required?: boolean;
	}

	let { children, class: className, required = false, ...restProps }: Props = $props();
</script>

<label
	class={cn('font-mono text-[10px] uppercase tracking-wider text-muted-foreground', className)}
	{...restProps}
>
	{@render children()}
	{#if required}
		<span class="text-destructive">*</span>
	{/if}
</label>
