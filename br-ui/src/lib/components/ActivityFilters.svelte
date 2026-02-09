<script lang="ts">
	import { Search, X } from 'lucide-svelte';
	import type { EventCategory } from '$lib/stores/activity.svelte';

	interface Props {
		categoryFilter: EventCategory | 'all';
		channelFilter: string | 'all';
		searchQuery: string;
		channels: string[];
		onCategoryChange: (category: EventCategory | 'all') => void;
		onChannelChange: (channel: string | 'all') => void;
		onSearchChange: (query: string) => void;
	}

	let {
		categoryFilter,
		channelFilter,
		searchQuery,
		channels,
		onCategoryChange,
		onChannelChange,
		onSearchChange
	}: Props = $props();

	const categories: { value: EventCategory | 'all'; label: string }[] = [
		{ value: 'all', label: 'All' },
		{ value: 'recording', label: 'Recording' },
		{ value: 'channel', label: 'Channel' },
		{ value: 'processing', label: 'Processing' },
		{ value: 'system', label: 'System' }
	];
</script>

<div class="flex flex-col gap-2 sm:flex-row sm:items-center sm:gap-3">
	<!-- Category Filter -->
	<div class="flex items-center gap-1 flex-wrap">
		{#each categories as cat (cat.value)}
			<button
				type="button"
				class="rounded px-2 py-1 font-mono text-[10px] uppercase tracking-wider transition-colors {categoryFilter ===
				cat.value
					? 'bg-zinc-700 text-zinc-100 dark:bg-zinc-600 dark:text-zinc-100'
					: 'bg-zinc-200 text-zinc-600 hover:bg-zinc-300 dark:bg-zinc-800 dark:text-zinc-400 dark:hover:bg-zinc-700'}"
				onclick={() => onCategoryChange(cat.value)}
			>
				{cat.label}
			</button>
		{/each}
	</div>

	<!-- Channel Filter -->
	<select
		class="rounded border border-border bg-input px-3 py-1.5 font-mono text-xs w-full sm:w-auto"
		value={channelFilter}
		onchange={(e) => onChannelChange(e.currentTarget.value)}
	>
		<option value="all">All Channels</option>
		{#each channels as channel (channel)}
			<option value={channel}>{channel}</option>
		{/each}
	</select>

	<!-- Search -->
	<div class="relative flex-1 sm:max-w-xs">
		<Search class="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-zinc-500" />
		<input
			type="text"
			placeholder="Search events..."
			class="w-full rounded border border-border bg-input pl-8 pr-8 py-1.5 font-mono text-xs"
			value={searchQuery}
			oninput={(e) => onSearchChange(e.currentTarget.value)}
		/>
		{#if searchQuery}
			<button
				type="button"
				class="absolute right-2 top-1/2 -translate-y-1/2 p-0.5 hover:bg-muted rounded transition-colors"
				onclick={() => onSearchChange('')}
			>
				<X class="w-3.5 h-3.5 text-zinc-500" />
			</button>
		{/if}
	</div>
</div>
