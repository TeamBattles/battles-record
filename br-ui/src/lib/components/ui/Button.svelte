<script lang="ts">
	import { button, type ButtonVariants } from './button.variants';
	import type { Snippet } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import { cn } from '$lib/utils/cn';

	interface Props extends ButtonVariants, Omit<HTMLButtonAttributes, 'class'> {
		children?: Snippet;
		class?: string;
	}

	let { intent, size, fullWidth, children, class: className, ...restProps }: Props = $props();
</script>

<button class={cn(button({ intent, size, fullWidth }), className)} {...restProps}>
	{#if children}
		{@render children()}
	{/if}
</button>
