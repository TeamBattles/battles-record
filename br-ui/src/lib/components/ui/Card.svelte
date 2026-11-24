<script lang="ts">
	import { tv, type VariantProps } from 'tailwind-variants';
	import type { Snippet } from 'svelte';
	import LuxuryCornerSlivers from './LuxuryCornerSlivers.svelte';
	import { cn } from '#lib/utils/cn.ts';

	/**
	 * The canonical surface: a rounded hairline card. `accent` adds the corner-sliver border used
	 * on featured surfaces; `hoverAccent` fades it in on hover for clickable tiles.
	 */
	const card = tv({
		base: 'relative rounded-lg border border-border bg-card text-card-foreground',
		variants: {
			padding: {
				none: '',
				sm: 'p-3',
				md: 'p-4',
				lg: 'p-6'
			},
			hoverAccent: {
				true: 'group transition-colors hover:border-line-2'
			}
		},
		defaultVariants: {
			padding: 'md'
		}
	});

	type CardVariants = VariantProps<typeof card>;

	interface Props extends CardVariants {
		children: Snippet;
		accent?: boolean;
		class?: string;
	}

	let {
		children,
		padding,
		accent = false,
		hoverAccent = false,
		class: className
	}: Props = $props();
</script>

<div data-slot="card" class={cn(card({ padding, hoverAccent }), className)}>
	{#if accent || hoverAccent}
		<LuxuryCornerSlivers hoverFadeIn={hoverAccent && !accent} length="lg" />
	{/if}
	{@render children()}
</div>
