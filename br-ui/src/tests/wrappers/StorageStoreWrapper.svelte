<script lang="ts">
	import { storageStore } from '$lib/stores/storage.svelte';

	// Expose store state for testing via data attributes
</script>

<div data-testid="storage-wrapper">
	<span data-testid="is-loading">{storageStore.isLoading}</span>
	<span data-testid="error">{storageStore.error ?? ''}</span>
	<span data-testid="total-size-gb">{storageStore.totalSizeGB}</span>
	<span data-testid="disk-used-percent">{storageStore.diskUsedPercent}</span>
	<span data-testid="disk-free-gb">{storageStore.diskFreeGB}</span>
	<span data-testid="disk-total-gb">{storageStore.diskTotalGB}</span>
	<span data-testid="recordings-usage-percent">{storageStore.recordingsUsagePercent}</span>
	<span data-testid="has-separate-library">{storageStore.hasSeparateLibrary}</span>
	<span data-testid="library-on-different-disk">{storageStore.libraryOnDifferentDisk}</span>
	<span data-testid="library-size-gb">{storageStore.librarySizeGB}</span>
	<span data-testid="sort-by">{storageStore.sortBy}</span>
	<span data-testid="sort-order">{storageStore.sortOrder}</span>
	<span data-testid="channel-stats-count">{storageStore.sortedChannelStats.length}</span>

	{#if storageStore.stats}
		<span data-testid="has-stats">true</span>
		<span data-testid="total-recordings">{storageStore.stats.total_recordings}</span>
		{#each storageStore.sortedChannelStats as channelStat, i}
			<div data-testid="channel-stat-{i}">
				<span data-testid="channel-stat-{i}-name">{channelStat.channel}</span>
				<span data-testid="channel-stat-{i}-size">{channelStat.size_bytes}</span>
				<span data-testid="channel-stat-{i}-percent"
					>{storageStore.getChannelPercent(channelStat)}</span
				>
			</div>
		{/each}
	{:else}
		<span data-testid="has-stats">false</span>
	{/if}

	{#if storageStore.isCleaningUp}
		<span data-testid="is-cleaning-up">true</span>
	{/if}

	{#if storageStore.cleanupPreview}
		<span data-testid="cleanup-preview-recordings"
			>{storageStore.cleanupPreview.recordings_affected}</span
		>
		<span data-testid="cleanup-preview-bytes">{storageStore.cleanupPreview.bytes_to_free}</span>
	{/if}

	{#if storageStore.cleanupError}
		<span data-testid="cleanup-error">{storageStore.cleanupError}</span>
	{/if}
</div>
