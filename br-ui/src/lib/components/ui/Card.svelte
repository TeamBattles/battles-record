<script lang="ts">
	import { tv, type VariantProps } from 'tailwind-variants';
	import type { Snippet } from 'svelte';
	import CornerBrackets from './CornerBrackets.svelte';
	import { cn } from '$lib/utils/cn';

	const card = tv({
		base: 'relative border border-border bg-card',
		variants: {
			padding: {
				none: '',
				sm: 'p-3',
				md: 'p-4',
				lg: 'p-6'
			}
		},
		defaultVariants: {
			padding: 'md'
		}
	});

	type CardVariants = VariantProps<typeof card>;

	interface Props extends CardVariants {
		children: Snippet;
		class?: string;
		bracketSize?: 'sm' | 'md' | 'lg';
	}

	let { children, padding, bracketSize = 'md', class: className }: Props = $props();
</script>

<div class={cn(card({ padding }), className)}>
	<CornerBrackets size={bracketSize} />
	{@render children()}
</div>
