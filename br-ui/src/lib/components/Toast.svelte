<script lang="ts">
	import { X, CheckCircle, AlertCircle, Info, AlertTriangle } from '@lucide/svelte';
	import type { ToastAction, ToastType } from '#lib/stores/toast.svelte.ts';
	import { cn } from '#lib/utils/cn.ts';

	/** Arena toast: popover surface with a status-colored glyph. */
	interface Props {
		id: string;
		type: ToastType;
		message: string;
		action?: ToastAction;
		onDismiss: (id: string) => void;
	}

	let { id, type, message, action, onDismiss }: Props = $props();

	const iconColors = {
		success: 'text-green',
		error: 'text-destructive',
		warning: 'text-yellow',
		info: 'text-blue'
	};

	const icons = {
		success: CheckCircle,
		error: AlertCircle,
		warning: AlertTriangle,
		info: Info
	};

	const iconColor = $derived(iconColors[type]);
	const Icon = $derived(icons[type]);
</script>

<div
	class="animate-luxury-fade-up relative overflow-hidden rounded-lg border border-border bg-popover py-3 pr-8 pl-4 shadow-xl"
	role="alert"
>
	<div class="flex min-w-0 items-start gap-2.5">
		<Icon size={14} class={cn('mt-0.5 shrink-0', iconColor)} />
		<p class="font-mono text-xs leading-relaxed break-all text-popover-foreground">{message}</p>
	</div>

	{#if action}
		<button
			class="mt-2 ml-6 rounded px-2 py-1 font-mono text-[10px] uppercase tracking-[0.14em] text-primary transition-colors hover:bg-hover-wash focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
			onclick={() => {
				action.onClick();
				onDismiss(id);
			}}
		>
			{action.label}
		</button>
	{/if}

	<button
		class="absolute top-2 right-2 rounded p-0.5 transition-colors hover:bg-hover-wash"
		onclick={() => onDismiss(id)}
		aria-label="Dismiss"
	>
		<X size={12} class="text-muted-foreground" />
	</button>
</div>
