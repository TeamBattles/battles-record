<script lang="ts">
	import type { Snippet } from 'svelte';
	import Label from './Label.svelte';
	import { cn } from '#lib/utils/cn.ts';

	interface Props {
		label: string;
		children: Snippet;
		error?: string;
		hint?: string;
		required?: boolean;
		class?: string;
		labelFor?: string;
	}

	let {
		label,
		children,
		error,
		hint,
		required = false,
		class: className,
		labelFor
	}: Props = $props();
</script>

<div class={cn('space-y-1.5', className)}>
	<Label for={labelFor} {required}>{label}</Label>
	{@render children()}
	{#if error}
		<p class="font-mono text-xs text-destructive">{error}</p>
	{:else if hint}
		<p class="font-mono text-xs text-muted-foreground">{hint}</p>
	{/if}
</div>
