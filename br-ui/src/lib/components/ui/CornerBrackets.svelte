<script lang="ts">
	import { tv, type VariantProps } from 'tailwind-variants';
	import { cn } from '$lib/utils/cn';
	import { settingsStore } from '$lib/stores/settings.svelte';

	const cornerBrackets = tv({
		base: 'pointer-events-none absolute border-muted-foreground',
		variants: {
			size: {
				sm: 'h-3 w-3',
				md: 'h-4 w-4',
				lg: 'h-5 w-5'
			}
		},
		defaultVariants: {
			size: 'md'
		}
	});

	type CornerBracketsVariants = VariantProps<typeof cornerBrackets>;

	interface Props extends CornerBracketsVariants {
		class?: string;
	}

	let { size, class: className }: Props = $props();

	const base = $derived(cn(cornerBrackets({ size }), className));
	const showBrackets = $derived(settingsStore.settings.showCornerBrackets);
</script>

{#if showBrackets}
	<div class="{base} left-0 top-0 border-l-2 border-t-2"></div>
	<div class="{base} right-0 top-0 border-r-2 border-t-2"></div>
	<div class="{base} left-0 bottom-0 border-b-2 border-l-2"></div>
	<div class="{base} right-0 bottom-0 border-b-2 border-r-2"></div>
{/if}
