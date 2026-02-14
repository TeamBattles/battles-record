<script lang="ts">
	import type { ComponentType } from 'svelte';
	import Card from './ui/Card.svelte';

	interface Props {
		label: string;
		value: string | number;
		icon: ComponentType<any>;
		status?: 'default' | 'success' | 'warning' | 'danger' | 'info';
	}

	let { label, value, icon: Icon, status = 'default' }: Props = $props();

	const statusColors: Record<string, string> = {
		default: 'text-zinc-500',
		success: 'text-emerald-400',
		warning: 'text-amber-400',
		danger: 'text-orange-400',
		info: 'text-blue-400'
	};

	const iconColor = $derived(statusColors[status]);
</script>

<Card>
	<div class="flex items-start justify-between">
		<div>
			<!-- Label -->
			<p class="font-mono text-[10px] uppercase tracking-wider text-zinc-500">{label}</p>
			<!-- Value -->
			<p class="mt-1 font-mono text-2xl font-bold tabular-nums">{value}</p>
		</div>
		<!-- Icon -->
		<Icon class="size-5 {iconColor}" />
	</div>
</Card>
