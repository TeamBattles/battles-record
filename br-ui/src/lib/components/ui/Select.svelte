<script lang="ts">
	import type { HTMLSelectAttributes } from 'svelte/elements';
	import { cn } from '$lib/utils/cn';

	interface Props extends Omit<HTMLSelectAttributes, 'class'> {
		value?: string;
		options: { value: string; label: string }[] | readonly { value: string; label: string }[];
		placeholder?: string;
		class?: string;
	}

	let {
		value = $bindable(''),
		options,
		placeholder,
		class: className,
		...restProps
	}: Props = $props();
</script>

<select
	bind:value
	class={cn(
		'w-full rounded border border-border bg-input px-3 py-2',
		'font-mono text-sm text-foreground',
		'transition-colors',
		'focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-1 focus:ring-offset-background',
		'disabled:cursor-not-allowed disabled:opacity-50',
		className
	)}
	{...restProps}
>
	{#if placeholder}
		<option value="" disabled>{placeholder}</option>
	{/if}
	{#each options as option (option.value)}
		<option value={option.value}>{option.label}</option>
	{/each}
</select>
