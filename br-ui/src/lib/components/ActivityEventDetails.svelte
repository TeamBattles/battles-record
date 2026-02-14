<script lang="ts">
	import type { ActivityEvent } from '$lib/stores/activity.svelte';
	import { X, Copy, Check } from 'lucide-svelte';
	import { ResponsivePanel } from '$lib';
	import { formatDuration, formatBytes } from '$lib/utils';

	interface Props {
		event: ActivityEvent;
		onclose: () => void;
	}

	let { event, onclose }: Props = $props();

	let copied = $state(false);

	function formatTimestamp(date: Date): string {
		return date.toLocaleString('en-US', {
			year: 'numeric',
			month: 'short',
			day: 'numeric',
			hour: '2-digit',
			minute: '2-digit',
			second: '2-digit',
			hour12: false
		});
	}

	function formatJson(data: Record<string, unknown>): string {
		return JSON.stringify(data, null, 2);
	}

	async function copyToClipboard() {
		try {
			const text = formatJson({
				id: event.id,
				timestamp: event.timestamp.toISOString(),
				type: event.type,
				category: event.category,
				channelName: event.channelName,
				platform: event.platform,
				message: event.message,
				data: event.data
			});
			await navigator.clipboard.writeText(text);
			copied = true;
			setTimeout(() => (copied = false), 2000);
		} catch (e) {
			console.error('Failed to copy:', e);
		}
	}

	// Get relevant fields to display based on event type
	function getDisplayFields(): { label: string; value: string }[] {
		const fields: { label: string; value: string }[] = [];

		// Common fields
		fields.push({ label: 'Event ID', value: event.id });
		fields.push({ label: 'Timestamp', value: formatTimestamp(event.timestamp) });
		fields.push({ label: 'Type', value: event.type });
		fields.push({ label: 'Category', value: event.category });

		if (event.channelName) {
			fields.push({ label: 'Channel', value: event.channelName });
		}

		if (event.platform) {
			fields.push({ label: 'Platform', value: event.platform });
		}

		// Type-specific fields
		const data = event.data;

		if (data.recording_id) {
			fields.push({ label: 'Recording ID', value: data.recording_id as string });
		}

		if (data.channel_id) {
			fields.push({ label: 'Channel ID', value: data.channel_id as string });
		}

		if (data.quality) {
			fields.push({ label: 'Quality', value: data.quality as string });
		}

		if (data.duration_secs !== undefined) {
			fields.push({ label: 'Duration', value: formatDuration(data.duration_secs as number) });
		}

		if (data.size_bytes !== undefined) {
			fields.push({ label: 'Size', value: formatBytes(data.size_bytes as number) });
		}

		if (data.segment_count !== undefined) {
			fields.push({ label: 'Segments', value: String(data.segment_count) });
		}

		if (data.reason) {
			fields.push({ label: 'Reason', value: data.reason as string });
		}

		if (data.error) {
			fields.push({ label: 'Error', value: data.error as string });
		}

		if (data.output_file) {
			fields.push({ label: 'Output File', value: data.output_file as string });
		}

		if (data.usage_percent !== undefined) {
			fields.push({ label: 'Disk Usage', value: `${data.usage_percent}%` });
		}

		if (data.free_bytes !== undefined) {
			fields.push({ label: 'Free Space', value: formatBytes(data.free_bytes as number) });
		}

		if (data.stream) {
			const stream = data.stream as { title: string; game?: string; viewers: number };
			if (stream.title) {
				fields.push({ label: 'Stream Title', value: stream.title });
			}
			if (stream.game) {
				fields.push({ label: 'Game', value: stream.game });
			}
			if (stream.viewers !== undefined) {
				fields.push({ label: 'Viewers', value: stream.viewers.toLocaleString() });
			}
		}

		return fields;
	}

	const displayFields = $derived(getDisplayFields());
</script>

<ResponsivePanel open={true} onClose={onclose}>
	{#snippet children()}
		<!-- Header -->
		<div class="p-4 border-b border-zinc-700 flex-shrink-0">
			<div class="flex items-start justify-between">
				<div>
					<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">Event Details</span
					>
					<h2 class="font-display text-xl tracking-tight uppercase text-zinc-100 mt-1">
						{event.type.replace(/_/g, ' ')}
					</h2>
				</div>
				<button class="p-1 hover:bg-zinc-800 rounded transition-colors" onclick={onclose}>
					<X class="w-5 h-5 text-zinc-500" />
				</button>
			</div>
		</div>

		<!-- Content - scrollable -->
		<div class="flex-1 overflow-y-auto p-4 min-h-0 space-y-4">
			<!-- Message -->
			<div class="rounded-lg border border-zinc-700 bg-zinc-800/50 p-3">
				<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 block mb-1"
					>Message</span
				>
				<p class="font-mono text-sm text-zinc-100">{event.message}</p>
			</div>

			<!-- Fields -->
			<div class="space-y-3">
				{#each displayFields as field (field.label)}
					<div class="flex flex-col sm:flex-row sm:items-baseline gap-1">
						<span
							class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 sm:w-28 flex-shrink-0"
						>
							{field.label}
						</span>
						<span class="font-mono text-xs text-zinc-300 break-all">{field.value}</span>
					</div>
				{/each}
			</div>

			<!-- Raw Data Section -->
			<div class="border-t border-zinc-700 pt-4">
				<div class="flex items-center justify-between mb-2">
					<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500">Raw Data</span>
					<button
						type="button"
						class="flex items-center gap-1 rounded px-2 py-1 font-mono text-[10px] text-zinc-400 hover:bg-zinc-800 transition-colors"
						onclick={copyToClipboard}
					>
						{#if copied}
							<Check class="size-3 text-emerald-400" />
							<span class="text-emerald-400">Copied</span>
						{:else}
							<Copy class="size-3" />
							<span>Copy</span>
						{/if}
					</button>
				</div>
				<pre
					class="rounded-lg border border-zinc-700 bg-zinc-900/50 p-3 font-mono text-[10px] text-zinc-400 overflow-x-auto max-h-48 overflow-y-auto">{formatJson(
						event.data
					)}</pre>
			</div>
		</div>
	{/snippet}
</ResponsivePanel>
