<script lang="ts">
	import { onMount } from 'svelte';
	import { Collapsible } from 'bits-ui';
	import { Copy, Check, ChevronRight } from 'lucide-svelte';
	import { api } from '$lib/api/client';
	import type { MessageLogEntry } from '$lib/api/types';
	import { cn } from '$lib/utils/cn';
	import { toastStore } from '$lib';

	let { connectionId }: { connectionId: string } = $props();

	let logs = $state<MessageLogEntry[]>([]);
	let expandedKey = $state<string | null>(null);
	let refreshInterval: ReturnType<typeof setInterval> | null = null;

	async function fetchLogs() {
		try {
			logs = await api.getExtensionConnectionLogs(connectionId);
			logs = [...logs].reverse();
		} catch {
			// Connection may have dropped
		}
	}

	onMount(() => {
		fetchLogs();
		refreshInterval = setInterval(fetchLogs, 3000);
		return () => {
			if (refreshInterval) clearInterval(refreshInterval);
		};
	});

	function formatPayload(payload?: string): string {
		if (!payload) return '';
		try {
			return JSON.stringify(JSON.parse(payload), null, 2);
		} catch {
			return payload;
		}
	}

	function highlightJson(payload?: string): string {
		const text = formatPayload(payload);
		if (!text) return '';
		return text.replace(
			/("(?:\\.|[^"\\])*")\s*:/g, // keys
			'<span class="text-sky-400">$1</span>:'
		).replace(
			/:\s*("(?:\\.|[^"\\])*")/g, // string values
			': <span class="text-emerald-400">$1</span>'
		).replace(
			/:\s*(\d+\.?\d*)/g, // numbers
			': <span class="text-amber-400">$1</span>'
		).replace(
			/:\s*(true|false)/g, // booleans
			': <span class="text-purple-400">$1</span>'
		).replace(
			/:\s*(null)/g, // null
			': <span class="text-zinc-500">$1</span>'
		);
	}

	function summarizePayload(payload?: string): string {
		if (!payload) return '';
		try {
			// Compact single-line JSON, no whitespace
			return JSON.stringify(JSON.parse(payload));
		} catch {
			return payload;
		}
	}

	function relativeTime(timestamp: string): string {
		const diff = Date.now() - new Date(timestamp).getTime();
		const secs = Math.floor(diff / 1000);
		if (secs < 60) return `${secs}s ago`;
		const mins = Math.floor(secs / 60);
		if (mins < 60) return `${mins}m ago`;
		const hrs = Math.floor(mins / 60);
		return `${hrs}h ago`;
	}

	let copiedKey = $state<string | null>(null);

	function entryKey(entry: MessageLogEntry): string {
		return entry.timestamp + entry.message_type + entry.direction;
	}

	async function copyPayload(payload: string | undefined, key: string) {
		if (!payload) return;
		try {
			await navigator.clipboard.writeText(formatPayload(payload));
			copiedKey = key;
			toastStore.success('Copied to clipboard');
			setTimeout(() => {
				if (copiedKey === key) copiedKey = null;
			}, 2000);
		} catch {
			toastStore.error('Failed to copy');
		}
	}

	function toggleExpand(key: string) {
		expandedKey = expandedKey === key ? null : key;
	}
</script>

<div class="max-h-80 overflow-y-auto border border-border rounded bg-muted/20 p-2">
	{#if logs.length === 0}
		<p class="font-mono text-[10px] text-muted-foreground/70 py-2 text-center">No messages yet</p>
	{:else}
		<div class="space-y-0.5">
			{#each logs as entry, i (entryKey(entry))}
				{@const hasPayload = !!entry.payload}
				{@const key = entryKey(entry)}
				<Collapsible.Root
					open={expandedKey === key}
					onOpenChange={(open) => (expandedKey = open ? key : null)}
				>
					<Collapsible.Trigger
						class={cn(
							'flex w-full items-center gap-2 rounded px-2 py-1 text-left transition-colors',
							'hover:bg-muted/50',
							expandedKey === key && 'bg-muted/50'
						)}
						disabled={!hasPayload}
					>
						{#if hasPayload}
							<ChevronRight
								size={12}
								class={cn(
									'shrink-0 text-muted-foreground transition-transform',
									expandedKey === key && 'rotate-90'
								)}
							/>
						{:else}
							<span class="inline-block w-3 shrink-0"></span>
						{/if}

						<span
							class={cn(
								'font-mono text-xs font-medium shrink-0',
								entry.direction === 'sent' ? 'text-emerald-500' : 'text-blue-400'
							)}
						>
							{entry.direction === 'sent' ? '\u2192' : '\u2190'}
						</span>

						<span class="font-mono text-xs text-foreground shrink-0">
							{entry.message_type}
						</span>

						{#if hasPayload}
							<span class="font-mono text-[10px] text-muted-foreground/40 truncate min-w-0">
								{summarizePayload(entry.payload)}
							</span>
						{/if}

						<span class="font-mono text-[10px] text-muted-foreground/60 ml-auto shrink-0">
							{relativeTime(entry.timestamp)}
						</span>
					</Collapsible.Trigger>

					{#if hasPayload}
						<Collapsible.Content class="overflow-hidden">
							<div class="relative ml-5 mt-1 mb-1">
								<button
									class="absolute right-2 top-2 rounded p-1 text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
									onclick={() => copyPayload(entry.payload, key)}
									title="Copy JSON"
								>
									{#if copiedKey === key}
										<Check size={12} class="text-emerald-400" />
									{:else}
										<Copy size={12} />
									{/if}
								</button>
								<pre
									class="bg-muted/50 rounded p-2 pr-8 overflow-x-auto font-mono text-[10px] text-foreground/80 leading-relaxed"
								>{@html highlightJson(entry.payload)}</pre>
							</div>
						</Collapsible.Content>
					{/if}
				</Collapsible.Root>
			{/each}
		</div>
	{/if}
</div>
