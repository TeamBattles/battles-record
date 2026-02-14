<script lang="ts">
	import { cn } from '$lib/utils/cn';

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

<div class={cn('flex items-center gap-2 text-zinc-500', className)}>
	<div
		class={cn(
			'animate-spin rounded-full border-2 border-zinc-500 border-t-transparent',
			sizeClasses[size]
		)}
	></div>
	{#if text}
		<span class={cn('font-mono', textSizeClasses[size])}>{text}</span>
	{/if}
</div>
