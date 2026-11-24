<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { LucideIcon } from '@lucide/svelte';
	import HexMesh from './HexMesh.svelte';
	import LuxuryCornerSlivers from './LuxuryCornerSlivers.svelte';
	import { cn } from '#lib/utils/cn.ts';

	/**
	 * Page header strip shared by every route: mono eyebrow, display title and an optional count
	 * chip over the honeycomb with its accent ripple, plus a slot for filters and actions.
	 */
	interface Props {
		title: string;
		eyebrow?: string;
		subtitle?: string;
		icon?: LucideIcon;
		count?: number | string;
		/** Extra chips beside the count (active/queued/online tallies). */
		badges?: Snippet;
		actions?: Snippet;
		class?: string;
	}

	let {
		title,
		eyebrow = 'Battles Record',
		subtitle,
		icon: Icon,
		count,
		badges,
		actions,
		class: className
	}: Props = $props();
</script>

<header
	class={cn(
		'relative shrink-0 overflow-clip rounded-xl border border-border bg-card [overflow-clip-margin:1px]',
		className
	)}
>
	<HexMesh ripple class="overflow-hidden rounded-[inherit]" />
	<LuxuryCornerSlivers intensity="subtle" length="2xl" />
	<div
		class="relative z-10 flex flex-col gap-4 px-4 py-4 sm:px-5 lg:flex-row lg:items-center lg:justify-between"
	>
		<div class="flex min-w-0 items-center gap-3">
			{#if Icon}
				<span
					class="hidden size-10 shrink-0 items-center justify-center rounded-md border border-primary/30 bg-primary/10 text-primary sm:flex"
				>
					<Icon class="size-5" />
				</span>
			{/if}
			<div class="flex min-w-0 flex-col gap-1">
				<span class="font-mono text-[10px] tracking-[0.18em] text-muted-foreground uppercase">
					{eyebrow}
				</span>
				<div class="flex flex-wrap items-center gap-2">
					<h1 class="font-display text-3xl leading-none tracking-wide text-foreground uppercase">
						{title}
					</h1>
					{#if count !== undefined}
						<span
							class="rounded bg-chip px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground tabular-nums"
						>
							{count}
						</span>
					{/if}
					{#if badges}
						{@render badges()}
					{/if}
				</div>
				{#if subtitle}
					<p class="text-sm text-muted-foreground">{subtitle}</p>
				{/if}
			</div>
		</div>

		{#if actions}
			<div class="flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:items-center lg:justify-end">
				{@render actions()}
			</div>
		{/if}
	</div>
</header>
