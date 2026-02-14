<script lang="ts">
	import { channelsStore } from '$lib/stores/channels.svelte';

	// Expose store state for testing via data attributes
</script>

<div data-testid="channels-wrapper">
	<span data-testid="is-loading">{channelsStore.isLoading}</span>
	<span data-testid="error">{channelsStore.error ?? ''}</span>
	<span data-testid="channel-count">{channelsStore.channelCount}</span>
	<span data-testid="filtered-count">{channelsStore.filteredChannels.length}</span>
	<span data-testid="platform-filter">{channelsStore.platformFilter}</span>
	<span data-testid="search-query">{channelsStore.searchQuery}</span>
	<span data-testid="selected-channel-id">{channelsStore.selectedChannelId ?? ''}</span>

	{#if channelsStore.selectedChannel}
		<div data-testid="selected-channel">
			<span data-testid="selected-channel-name">{channelsStore.selectedChannel.name}</span>
			<span data-testid="selected-channel-platform">{channelsStore.selectedChannel.platform}</span>
		</div>
	{/if}

	{#each channelsStore.channels as channel}
		<div data-testid="channel-{channel.id}">
			<span data-testid="channel-{channel.id}-name">{channel.name}</span>
			<span data-testid="channel-{channel.id}-platform">{channel.platform}</span>
			<span data-testid="channel-{channel.id}-enabled">{channel.enabled}</span>
			<span data-testid="channel-{channel.id}-quality">{channel.quality}</span>
			<span data-testid="channel-{channel.id}-is-live">{channel.status?.is_live ?? false}</span>
			<span data-testid="channel-{channel.id}-is-recording"
				>{channel.status?.is_recording ?? false}</span
			>
			{#if channel.status?.current_stream}
				<span data-testid="channel-{channel.id}-stream-title"
					>{channel.status.current_stream.title}</span
				>
				<span data-testid="channel-{channel.id}-stream-game"
					>{channel.status.current_stream.game ?? ''}</span
				>
				<span data-testid="channel-{channel.id}-stream-viewers"
					>{channel.status.current_stream.viewer_count}</span
				>
			{/if}
			{#if channel.quota_gb}
				<span data-testid="channel-{channel.id}-quota-gb">{channel.quota_gb}</span>
				<span data-testid="channel-{channel.id}-quota-status">{channel.quota_status}</span>
				<span data-testid="channel-{channel.id}-quota-percent">{channel.quota_percent}</span>
			{/if}
		</div>
	{/each}

	{#each channelsStore.filteredChannels as channel}
		<div data-testid="filtered-channel-{channel.id}">
			<span data-testid="filtered-channel-{channel.id}-name">{channel.name}</span>
		</div>
	{/each}
</div>
