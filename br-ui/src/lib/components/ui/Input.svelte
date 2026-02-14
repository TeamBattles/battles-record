<script lang="ts">
	import { input, type InputVariants } from './input.variants';
	import type { HTMLInputAttributes } from 'svelte/elements';
	import { cn } from '$lib/utils/cn';

	interface Props extends InputVariants, Omit<HTMLInputAttributes, 'class' | 'size'> {
		value?: string;
		class?: string;
	}

	let {
		variant,
		inputSize,
		value = $bindable(''),
		class: className,
		...restProps
	}: Props = $props();
</script>

<input bind:value class={cn(input({ variant, inputSize }), className)} {...restProps} />
