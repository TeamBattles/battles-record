<script lang="ts">
	import { cn } from '$lib/utils/cn';

	interface Props {
		status: 'recording' | 'live' | 'offline' | 'error' | 'success' | 'warning' | 'info';
		pulse?: boolean;
		size?: 'sm' | 'md' | 'lg';
		class?: string;
	}

	let { status, pulse = false, size = 'md', class: className }: Props = $props();

	const statusColors = {
		recording: 'bg-orange-500',
		live: 'bg-emerald-500',
		offline: 'bg-zinc-500',
		error: 'bg-red-500',
		success: 'bg-emerald-500',
		warning: 'bg-amber-500',
		info: 'bg-blue-500'
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
