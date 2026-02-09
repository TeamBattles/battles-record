<script lang="ts">
	import { Copy, Check } from 'lucide-svelte';
	import { ResponsiveModal } from '$lib';

	interface Props {
		open: boolean;
		onOpenChange: (open: boolean) => void;
		logs: string[];
	}

	let { open, onOpenChange, logs }: Props = $props();

	let copied = $state(false);
	let logsContainer: HTMLDivElement | null = $state(null);

	// Auto-scroll to bottom when logs change
	$effect(() => {
		if (logsContainer && logs.length > 0) {
			logsContainer.scrollTop = logsContainer.scrollHeight;
		}
	});

	async function copyLogs() {
		const text = logs.join('\n');
		await navigator.clipboard.writeText(text);
		copied = true;
		setTimeout(() => (copied = false), 2000);
	}
</script>

<ResponsiveModal {open} {onOpenChange} title="Daemon Logs">
	{#snippet children()}
		<div class="flex flex-col gap-3">
			<div
				bind:this={logsContainer}
				class="h-80 overflow-y-auto bg-zinc-950 rounded border border-zinc-700 p-3"
			>
				{#if logs.length === 0}
					<p class="font-mono text-xs text-zinc-500">No logs available</p>
				{:else}
					<pre class="font-mono text-xs text-zinc-300 whitespace-pre-wrap">{logs.join('\n')}</pre>
				{/if}
			</div>
		</div>
	{/snippet}
	{#snippet footer()}
		<div class="flex gap-2">
			<button
				class="flex-1 rounded border border-zinc-700 bg-zinc-800 px-4 py-2 font-mono text-sm text-zinc-300 hover:bg-zinc-700 transition-colors flex items-center justify-center gap-2 disabled:opacity-50"
				onclick={copyLogs}
				disabled={logs.length === 0}
			>
				{#if copied}
					<Check size={14} class="text-emerald-400" />
					Copied
				{:else}
					<Copy size={14} />
					Copy All
				{/if}
			</button>
			<button
				class="rounded border border-zinc-700 bg-zinc-800 px-4 py-2 font-mono text-sm text-zinc-300 hover:bg-zinc-700 transition-colors"
				onclick={() => onOpenChange(false)}
			>
				Close
			</button>
		</div>
	{/snippet}
</ResponsiveModal>
