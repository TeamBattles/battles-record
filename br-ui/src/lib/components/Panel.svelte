<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { LucideIcon } from '@lucide/svelte';
	import HexMesh from './ui/HexMesh.svelte';
	import { cn } from '#lib/utils/cn.ts';

	/**
	 * Card with a header strip - the Arena "card head" recipe: a hairline-separated header carrying
	 * a mono eyebrow, with the honeycomb pooled in its top-right corner.
	 */
	interface Props {
		title?: string;
		icon?: LucideIcon;
		count?: number | string;
		description?: string;
		actions?: Snippet;
		children: Snippet;
		/** Padding of the body; `none` for full-bleed lists and tables. */
		padding?: 'none' | 'md';
		id?: string;
		class?: string;
		bodyClass?: string;
	}

	let {
		title,
		icon: Icon,
		count,
		description,
		actions,
		children,
		padding = 'md',
		id,
		class: className,
		bodyClass
	}: Props = $props();
</script>

<section
	{id}
	data-slot="panel"
	class={cn(
		'relative flex flex-col overflow-clip rounded-lg border border-border bg-card',
		className
	)}
>
	{#if title}
		<header class="relative shrink-0 overflow-hidden border-b border-border">
			<HexMesh />
			<div class="relative z-10 flex items-center gap-2.5 px-4 py-3">
				{#if Icon}
					<span
						class="flex size-7 shrink-0 items-center justify-center rounded bg-primary/10 text-primary"
					>
						<Icon class="size-3.5" />
					</span>
				{/if}
				<div class="min-w-0 flex-1">
					<div class="flex items-center gap-2">
						<h2
							class="truncate font-mono text-[11px] tracking-[0.14em] text-muted-foreground uppercase"
						>
							{title}
						</h2>
						{#if count !== undefined}
							<span
								class="rounded bg-chip px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground tabular-nums"
							>
								{count}
							</span>
						{/if}
					</div>
					{#if description}
						<p class="mt-0.5 text-xs text-muted-foreground">{description}</p>
					{/if}
				</div>
				{#if actions}
					<div class="flex shrink-0 items-center gap-2">
						{@render actions()}
					</div>
				{/if}
			</div>
		</header>
	{/if}

	<div class={cn('flex-1', padding === 'md' && 'p-4', bodyClass)}>
		{@render children()}
	</div>
</section>
