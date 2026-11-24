<script lang="ts">
	import { Copy, Check } from '@lucide/svelte';
	import { ResponsiveModal } from '#lib';

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
				class="h-80 overflow-y-auto bg-well rounded border border-line-2 p-3"
			>
				{#if logs.length === 0}
					<p class="font-mono text-xs text-muted-foreground">No logs available</p>
				{:else}
					<pre class="font-mono text-xs text-text-2 whitespace-pre-wrap">{logs.join('\n')}</pre>
				{/if}
			</div>
		</div>
	{/snippet}
	{#snippet footer()}
		<div class="flex gap-2">
			<button
				class="flex-1 rounded border border-line-2 bg-btn-bg px-4 py-2 font-mono text-sm text-text-2 hover:bg-btn-bg-hover transition-colors flex items-center justify-center gap-2 disabled:opacity-50"
				onclick={copyLogs}
				disabled={logs.length === 0}
			>
				{#if copied}
					<Check size={14} class="text-green" />
					Copied
				{:else}
					<Copy size={14} />
					Copy All
				{/if}
			</button>
			<button
				class="rounded border border-line-2 bg-btn-bg px-4 py-2 font-mono text-sm text-text-2 hover:bg-btn-bg-hover transition-colors"
				onclick={() => onOpenChange(false)}
			>
				Close
			</button>
		</div>
	{/snippet}
</ResponsiveModal>
