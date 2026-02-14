<script lang="ts">
	import { recordingsStore } from '$lib/stores/recordings.svelte';

	// Expose store state for testing via data attributes
</script>

<div data-testid="recordings-wrapper">
	<span data-testid="is-loading">{recordingsStore.isLoading}</span>
	<span data-testid="error">{recordingsStore.error ?? ''}</span>
	<span data-testid="recording-count">{recordingsStore.recordings.length}</span>
	<span data-testid="filtered-count">{recordingsStore.filteredRecordings.length}</span>
	<span data-testid="platform-filter">{recordingsStore.platformFilter ?? ''}</span>
	<span data-testid="status-filter">{recordingsStore.statusFilter ?? ''}</span>
	<span data-testid="search-query">{recordingsStore.searchQuery}</span>
	<span data-testid="sort-by">{recordingsStore.sortBy}</span>
	<span data-testid="sort-order">{recordingsStore.sortOrder}</span>

	{#each recordingsStore.recordings as recording}
		<div data-testid="recording-{recording.id}">
			<span data-testid="recording-{recording.id}-channel">{recording.channel_name}</span>
			<span data-testid="recording-{recording.id}-platform">{recording.platform}</span>
			<span data-testid="recording-{recording.id}-status">{recording.status}</span>
			<span data-testid="recording-{recording.id}-size">{recording.size_bytes}</span>
			<span data-testid="recording-{recording.id}-duration">{recording.duration_secs ?? 0}</span>
			{#if recording.title}
				<span data-testid="recording-{recording.id}-title">{recording.title}</span>
			{/if}
			{#if recording.game}
				<span data-testid="recording-{recording.id}-game">{recording.game}</span>
			{/if}
			{#if recordingsStore.getProcessingProgress(recording.id) !== undefined}
				<span data-testid="recording-{recording.id}-progress"
					>{recordingsStore.getProcessingProgress(recording.id)}</span
				>
			{/if}
		</div>
	{/each}

	{#each recordingsStore.filteredRecordings as recording}
		<div data-testid="filtered-recording-{recording.id}">
			<span data-testid="filtered-recording-{recording.id}-channel">{recording.channel_name}</span>
		</div>
	{/each}
</div>
