<script lang="ts">
	import { cn } from '#lib/utils/cn.ts';

	interface Props {
		message: string;
		class?: string;
		variant?: 'error' | 'warning';
	}

	let { message, class: className, variant = 'error' }: Props = $props();

	const variantClasses = {
		error: 'border-destructive/30 bg-destructive/5 text-destructive',
		warning: 'border-yellow/30 bg-yellow/5 text-yellow'
	};
</script>

<div class={cn('rounded-lg border p-4', variantClasses[variant], className)}>
	<p class="font-mono text-xs">{message}</p>
</div>
