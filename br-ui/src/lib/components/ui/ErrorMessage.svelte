<script lang="ts">
	import { cn } from '$lib/utils/cn';

	interface Props {
		message: string;
		class?: string;
		variant?: 'error' | 'warning';
	}

	let { message, class: className, variant = 'error' }: Props = $props();

	const variantClasses = {
		error: 'border-red-500/30 bg-red-500/5 text-red-400',
		warning: 'border-amber-500/30 bg-amber-500/5 text-amber-400'
	};
</script>

<div class={cn('rounded-lg border p-4', variantClasses[variant], className)}>
	<p class="font-mono text-xs">{message}</p>
</div>
