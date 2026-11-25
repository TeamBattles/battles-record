<script lang="ts">
	import { Search, X } from '@lucide/svelte';
	import type { EventCategory } from '#lib/stores/activity.svelte.ts';
	import Select from './ui/Select.svelte';

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

	const channelOptions = $derived([
		{ value: 'all', label: 'All Channels' },
		...channels.map((channel) => ({ value: channel, label: channel }))
	]);

	const categories: { value: EventCategory | 'all'; label: string }[] = [
		{ value: 'all', label: 'All' },
		{ value: 'recording', label: 'Recording' },
		{ value: 'channel', label: 'Channel' },
		{ value: 'processing', label: 'Processing' },
		{ value: 'download', label: 'Download' },
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
					? 'bg-primary/15 text-primary'
					: 'bg-chip text-muted-foreground hover:bg-chip-strong hover:text-text-2'}"
				onclick={() => onCategoryChange(cat.value)}
			>
				{cat.label}
			</button>
		{/each}
	</div>

	<!-- Channel Filter -->
	<Select
		size="sm"
		class="sm:w-48"
		ariaLabel="Filter by channel"
		value={channelFilter}
		options={channelOptions}
		onValueChange={onChannelChange}
	/>

	<!-- Search -->
	<div class="relative flex-1 sm:max-w-xs">
		<Search class="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground" />
		<input
			type="text"
			placeholder="Search events..."
			class="w-full rounded border border-border bg-well pl-8 pr-8 py-1.5 font-mono text-xs"
			value={searchQuery}
			oninput={(e) => onSearchChange(e.currentTarget.value)}
		/>
		{#if searchQuery}
			<button
				type="button"
				class="absolute right-2 top-1/2 -translate-y-1/2 p-0.5 hover:bg-hover-wash rounded transition-colors"
				onclick={() => onSearchChange('')}
			>
				<X class="w-3.5 h-3.5 text-muted-foreground" />
			</button>
		{/if}
	</div>
</div>
