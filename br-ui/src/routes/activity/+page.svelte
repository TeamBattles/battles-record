<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Activity, Trash2, Download, ArrowDown, Pause, Play } from 'lucide-svelte';
	import { activityStore, type EventCategory } from '$lib/stores/activity.svelte';
	import { connectionStore, toastStore } from '$lib';
	import ActivityEventRow from '$lib/components/ActivityEventRow.svelte';
	import ActivityFilters from '$lib/components/ActivityFilters.svelte';
	import ActivityEventDetails from '$lib/components/ActivityEventDetails.svelte';
	import CornerBrackets from '$lib/components/ui/CornerBrackets.svelte';

	let eventListRef = $state<HTMLDivElement | null>(null);

	// Auto-scroll effect
	$effect(() => {
		if (activityStore.autoScroll && eventListRef && activityStore.filteredEvents.length > 0) {
			eventListRef.scrollTop = 0;
		}
	});

	function handleClear() {
		activityStore.clear();
		toastStore.info('Activity log cleared');
	}

	function handleExport() {
		activityStore.downloadExport();
		toastStore.success('Activity log exported');
	}

	function toggleAutoScroll() {
		activityStore.setAutoScroll(!activityStore.autoScroll);
	}
</script>

<div class="space-y-4 h-full flex flex-col">
	<!-- Header Bar -->
	<div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between flex-shrink-0">
		<div class="flex items-center gap-3">
			<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">Activity</span>
			<span
				class="rounded bg-zinc-200 px-1.5 py-0.5 font-mono text-[10px] text-zinc-600 dark:bg-zinc-800 dark:text-zinc-500"
			>
				{activityStore.filteredCount}/{activityStore.eventCount}
			</span>
		</div>

		<div class="flex flex-wrap items-center gap-2">
			<!-- Auto-scroll toggle -->
			<button
				class="rounded border border-border bg-input px-3 py-1.5 font-mono text-xs flex items-center gap-2 hover:bg-muted transition-colors {activityStore.autoScroll
					? 'bg-muted'
					: ''}"
				onclick={toggleAutoScroll}
				title={activityStore.autoScroll ? 'Auto-scroll enabled' : 'Auto-scroll disabled'}
			>
				{#if activityStore.autoScroll}
					<ArrowDown class="w-3.5 h-3.5 text-emerald-400" />
					<span class="hidden sm:inline">Auto</span>
				{:else}
					<Pause class="w-3.5 h-3.5 text-zinc-500" />
					<span class="hidden sm:inline">Paused</span>
				{/if}
			</button>

			<!-- Export -->
			<button
				class="rounded border border-border bg-input px-3 py-1.5 font-mono text-xs flex items-center gap-2 hover:bg-muted transition-colors"
				onclick={handleExport}
				disabled={activityStore.eventCount === 0}
			>
				<Download class="w-3.5 h-3.5" />
				<span class="hidden sm:inline">Export</span>
			</button>

			<!-- Clear -->
			<button
				class="rounded border border-border bg-input px-3 py-1.5 font-mono text-xs flex items-center gap-2 hover:bg-red-500/10 hover:text-red-400 hover:border-red-500/30 transition-colors"
				onclick={handleClear}
				disabled={activityStore.eventCount === 0}
			>
				<Trash2 class="w-3.5 h-3.5" />
				<span class="hidden sm:inline">Clear</span>
			</button>
		</div>
	</div>

	<!-- Filters -->
	<div class="flex-shrink-0">
		<ActivityFilters
			categoryFilter={activityStore.categoryFilter}
			channelFilter={activityStore.channelFilter}
			searchQuery={activityStore.searchQuery}
			channels={activityStore.uniqueChannels}
			onCategoryChange={(cat) => activityStore.setCategoryFilter(cat)}
			onChannelChange={(ch) => activityStore.setChannelFilter(ch)}
			onSearchChange={(q) => activityStore.setSearch(q)}
		/>
	</div>

	<!-- Event List -->
	{#if !connectionStore.isConnected}
		<div class="rounded-lg border border-amber-500/30 bg-amber-500/5 p-4">
			<p class="font-mono text-xs text-amber-400">
				Not connected to a server. Events will appear once connected.
			</p>
		</div>
	{:else if activityStore.filteredEvents.length === 0}
		<div class="relative border border-border bg-card flex-1 min-h-[200px]">
			<CornerBrackets />

			<div class="flex flex-col items-center justify-center h-full gap-2 text-zinc-500 py-12">
				<Activity class="size-8 opacity-30" />
				{#if activityStore.eventCount === 0}
					<p class="font-mono text-xs">No activity yet</p>
					<p class="font-mono text-[10px] text-zinc-600">Events will appear as they occur</p>
				{:else}
					<p class="font-mono text-xs">No events match filters</p>
					<button
						class="mt-2 rounded border border-border bg-input px-3 py-1.5 font-mono text-xs hover:bg-muted transition-colors"
						onclick={() => {
							activityStore.setCategoryFilter('all');
							activityStore.setChannelFilter('all');
							activityStore.setSearch('');
						}}
					>
						Clear filters
					</button>
				{/if}
			</div>
		</div>
	{:else}
		<div class="relative border border-border bg-card flex-1 overflow-hidden min-h-0">
			<CornerBrackets class="z-10" />

			<!-- Header -->
			<div class="flex items-center gap-2 border-b border-border/60 bg-muted/30 px-4 py-2">
				<Activity class="size-4 text-zinc-500" />
				<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">Events</span>
				<span
					class="rounded bg-zinc-200 px-1.5 py-0.5 font-mono text-[10px] text-zinc-600 dark:bg-zinc-800 dark:text-zinc-500"
				>
					{activityStore.filteredCount}
				</span>
			</div>

			<!-- Scrollable list -->
			<div bind:this={eventListRef} class="overflow-y-auto h-[calc(100%-40px)]">
				{#each activityStore.filteredEvents as event (event.id)}
					<ActivityEventRow
						{event}
						isSelected={activityStore.selectedEventId === event.id}
						onclick={() => activityStore.selectEvent(event.id)}
					/>
				{/each}
			</div>
		</div>
	{/if}
</div>

<!-- Event Details Panel -->
{#if activityStore.selectedEvent}
	<ActivityEventDetails
		event={activityStore.selectedEvent}
		onclose={() => activityStore.selectEvent(null)}
	/>
{/if}
