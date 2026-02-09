<script lang="ts">
	import type { Snippet, ComponentType } from 'svelte';
	import CornerBrackets from './ui/CornerBrackets.svelte';
	import { cn } from '$lib/utils/cn';

	interface Props {
		title?: string;
		icon?: ComponentType<any>;
		count?: number | string;
		children: Snippet;
		class?: string;
	}

	let { title, icon: Icon, count, children, class: className = '' }: Props = $props();
</script>

<div class={cn('relative flex flex-col border border-border bg-card', className)}>
	<CornerBrackets />

	<!-- Header bar (optional) -->
	{#if title}
		<div class="flex shrink-0 items-center gap-2 border-b border-border/60 bg-muted/30 px-4 py-2">
			{#if Icon}
				<Icon class="size-4 text-muted-foreground" />
			{/if}
			<span class="font-mono text-xs uppercase tracking-wider text-muted-foreground">
				{title}
			</span>
			{#if count !== undefined}
				<span
					class="rounded bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground"
				>
					{count}
				</span>
			{/if}
		</div>
	{/if}

	<!-- Content -->
	<div class="flex-1 p-4">
		{@render children()}
	</div>
</div>
