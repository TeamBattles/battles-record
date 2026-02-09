<script lang="ts">
	import { X, CheckCircle, AlertCircle, Info, AlertTriangle } from 'lucide-svelte';
	import type { ToastType } from '$lib/stores/toast.svelte';

	interface Props {
		id: string;
		type: ToastType;
		message: string;
		onDismiss: (id: string) => void;
	}

	let { id, type, message, onDismiss }: Props = $props();

	const styles = {
		success: {
			border: 'border-emerald-500/30',
			bg: 'bg-emerald-500/5',
			text: 'text-emerald-400',
			dot: 'bg-emerald-400'
		},
		error: {
			border: 'border-red-500/30',
			bg: 'bg-red-500/5',
			text: 'text-red-400',
			dot: 'bg-red-400'
		},
		warning: {
			border: 'border-amber-500/30',
			bg: 'bg-amber-500/5',
			text: 'text-amber-400',
			dot: 'bg-amber-400'
		},
		info: {
			border: 'border-blue-500/30',
			bg: 'bg-blue-500/5',
			text: 'text-blue-400',
			dot: 'bg-blue-400'
		}
	};

	const icons = {
		success: CheckCircle,
		error: AlertCircle,
		warning: AlertTriangle,
		info: Info
	};

	const style = $derived(styles[type]);
	const Icon = $derived(icons[type]);
</script>

<div
	class="relative rounded border {style.border} {style.bg} p-3 pr-8 shadow-lg backdrop-blur-sm"
	role="alert"
>
	<!-- Corner brackets -->
	<div class="absolute top-0 left-0 h-2 w-2 border-t border-l {style.border}"></div>
	<div class="absolute top-0 right-0 h-2 w-2 border-t border-r {style.border}"></div>
	<div class="absolute bottom-0 left-0 h-2 w-2 border-b border-l {style.border}"></div>
	<div class="absolute bottom-0 right-0 h-2 w-2 border-b border-r {style.border}"></div>

	<div class="flex items-start gap-2">
		<!-- Status dot -->
		<div class="size-2 rounded-full {style.dot} mt-1 shrink-0"></div>

		<!-- Message -->
		<p class="font-mono text-xs {style.text} leading-relaxed">{message}</p>
	</div>

	<!-- Dismiss button -->
	<button
		class="absolute top-2 right-2 p-0.5 rounded hover:bg-zinc-700/50 transition-colors"
		onclick={() => onDismiss(id)}
		aria-label="Dismiss"
	>
		<X size={12} class="text-zinc-500" />
	</button>
</div>
