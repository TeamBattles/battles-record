<script lang="ts">
	import { cn } from '#lib/utils/cn.ts';

	interface Props {
		text?: string;
		size?: 'sm' | 'md' | 'lg';
		class?: string;
	}

	let { text = 'Loading...', size = 'md', class: className }: Props = $props();

	const sizeClasses = {
		sm: 'size-3',
		md: 'size-4',
		lg: 'size-6'
	};

	const textSizeClasses = {
		sm: 'text-[10px]',
		md: 'text-xs',
		lg: 'text-sm'
	};
</script>

<div class={cn('flex items-center gap-2 text-muted-foreground', className)}>
	<div
		class={cn(
			'animate-spin rounded-full border-2 border-line-3 border-t-primary',
			sizeClasses[size]
		)}
	></div>
	{#if text}
		<span class={cn('font-mono', textSizeClasses[size])}>{text}</span>
	{/if}
</div>
