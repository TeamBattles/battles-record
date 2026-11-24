<script lang="ts">
	import type { LucideIcon } from '@lucide/svelte';
	import { cn } from '#lib/utils/cn.ts';

	/**
	 * Arena metric tile: a lifted panel with the accent corner bloom, mono label, display-font value
	 * and a status-tinted icon box.
	 */
	interface Props {
		label: string;
		value: string | number;
		icon: LucideIcon;
		status?: 'default' | 'success' | 'warning' | 'danger' | 'info';
	}

	let { label, value, icon: Icon, status = 'default' }: Props = $props();

	const statusColors: Record<string, string> = {
		default: 'text-muted-foreground bg-chip',
		success: 'text-green bg-green/10',
		warning: 'text-yellow bg-yellow/10',
		danger: 'text-orange bg-orange/10',
		info: 'text-blue bg-blue/10'
	};
</script>

<div
	data-slot="card"
	class="relative overflow-hidden rounded-lg border border-border bg-panel-lift p-4 bg-[radial-gradient(120px_circle_at_0_0,color-mix(in_oklab,var(--primary)_14%,transparent),transparent)]"
>
	<div class="flex items-start justify-between gap-3">
		<div class="min-w-0">
			<p class="font-mono text-[10px] tracking-[0.14em] text-muted-foreground uppercase">{label}</p>
			<p class="mt-2 font-display text-3xl leading-none tracking-wide text-foreground tabular-nums">
				{value}
			</p>
		</div>
		<span
			class={cn(
				'flex size-9 shrink-0 items-center justify-center rounded-md',
				statusColors[status]
			)}
		>
			<Icon class="size-4" />
		</span>
	</div>
</div>
