<script lang="ts">
	import type { Snippet, Component } from 'svelte';
	import { cn } from '#lib/utils/cn.ts';

	interface Props {
		title: string;
		description?: string;
		icon?: Component<{ class?: string }>;
		action?: Snippet;
		class?: string;
	}

	let { title, description, icon: Icon, action, class: className }: Props = $props();
</script>

<div class={cn('flex flex-col items-center justify-center py-12 text-center', className)}>
	{#if Icon}
		<div class="mb-4 text-muted-foreground">
			<Icon class="size-12" />
		</div>
	{/if}
	<h3 class="font-mono text-sm font-medium text-text-2">{title}</h3>
	{#if description}
		<p class="mt-1 font-mono text-xs text-muted-foreground">{description}</p>
	{/if}
	{#if action}
		<div class="mt-4">
			{@render action()}
		</div>
	{/if}
</div>
