<script lang="ts">
	import { cn } from '#lib/utils/cn.ts';

	interface Props {
		status: 'recording' | 'live' | 'offline' | 'error' | 'success' | 'warning' | 'info';
		pulse?: boolean;
		size?: 'sm' | 'md' | 'lg';
		class?: string;
	}

	let { status, pulse = false, size = 'md', class: className }: Props = $props();

	const statusColors = {
		recording: 'bg-orange',
		live: 'bg-green',
		offline: 'bg-dim',
		error: 'bg-destructive',
		success: 'bg-green',
		warning: 'bg-yellow',
		info: 'bg-blue'
	};

	const sizeClasses = {
		sm: 'size-1.5',
		md: 'size-2',
		lg: 'size-3'
	};
</script>

<div
	class={cn(
		'rounded-full',
		statusColors[status],
		sizeClasses[size],
		pulse && 'animate-pulse',
		className
	)}
></div>
