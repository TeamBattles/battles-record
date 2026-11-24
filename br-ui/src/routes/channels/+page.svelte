<script lang="ts">
	import PageHeader from '#lib/components/ui/PageHeader.svelte';
	import { onDestroy } from 'svelte';
	import { Search, Plus, Calendar, Trash2, RefreshCw, Edit, Terminal, ExternalLink, Tv } from '@lucide/svelte';
	import { channelsStore } from '#lib/stores/channels.svelte.ts';
	import { connectionStore } from '#lib/stores/connection.svelte.ts';
	import { breakpointStore, ChannelCard, toastStore } from '#lib';
	import { PLATFORM_PROFILE_URLS } from '#lib/utils/constants.ts';
	import { open } from '@tauri-apps/plugin-shell';
	import PlatformIcon from '#lib/components/PlatformIcon.svelte';
	import Select from '#lib/components/ui/Select.svelte';
	import ChannelStatus from '#lib/components/ChannelStatus.svelte';
	import ChannelDetailPanel from '#lib/components/ChannelDetailPanel.svelte';
	import AddChannelModal from '#lib/components/AddChannelModal.svelte';
	import type { Channel } from '#lib/api/types.ts';

	import { untrack } from 'svelte';

	const PLATFORM_FILTER_OPTIONS: { value: 'all' | 'twitch' | 'youtube' | 'kick'; label: string }[] = [
		{ value: 'all', label: 'All Platforms' },
		{ value: 'twitch', label: 'Twitch' },
		{ value: 'youtube', label: 'YouTube' },
		{ value: 'kick', label: 'Kick' }
	];

	let showAddModal = $state(false);

	// Reload channels when server changes or connection is established
	$effect(() => {
		// Track activeServerId to detect server switches
		const serverId = connectionStore.activeServerId;
		if (connectionStore.isConnected && serverId) {
			untrack(() => {
				channelsStore.load(serverId);
				channelsStore.subscribe();
			});
		}
	});

	onDestroy(() => {
		channelsStore.unsubscribeEvents();
	});

	function handleRowClick(channelId: string) {
		channelsStore.selectChannel(channelId);
	}

	async function handleDelete(e: Event, channelId: string) {
		e.stopPropagation();
		const success = await channelsStore.deleteChannel(channelId);
		if (success) {
			toastStore.success('Channel deleted');
		}
	}

	async function handleCheckNow(e: Event, channelId: string) {
		e.stopPropagation();
		await channelsStore.checkChannel(channelId);
	}

	async function openProfile(e: Event, channel: Channel) {
		e.stopPropagation();
		const baseUrl = PLATFORM_PROFILE_URLS[channel.platform];
		await open(`${baseUrl}${channel.name}`);
	}

	async function handleSaveChannel(data: Partial<Channel>): Promise<void> {
		const channel = channelsStore.selectedChannel;
		if (!channel) return;

		const success = await channelsStore.updateChannel(channel.id, data);
		if (success) {
			channelsStore.selectChannel(null);
		} else {
			throw new Error(channelsStore.error ?? 'Failed to update channel');
		}
	}

	async function handleCreateChannel(data: { platform: string; name: string; quality: string }) {
		const result = await channelsStore.createChannel({
			name: data.name,
			platform: data.platform as Channel['platform'],
			quality: data.quality,
			enabled: true
		});
		if (result.success) {
			showAddModal = false;
			toastStore.success(`Channel "${data.name}" added`);
		} else {
			toastStore.error(result.error);
		}
	}
</script>

<div class="space-y-4">
	<PageHeader
		title="Channels"
		eyebrow="Record"
		subtitle="Channels watched for live broadcasts"
		icon={Tv}
		count={channelsStore.channelCount}
	>
		{#snippet actions()}
			<!-- Platform Filter -->
			<Select
				size="sm"
				class="sm:w-48"
				ariaLabel="Filter by platform"
				value={channelsStore.platformFilter}
				options={PLATFORM_FILTER_OPTIONS}
				onValueChange={(platform) => channelsStore.setFilter(platform)}
			/>

			<!-- Search -->
			<div class="relative">
				<Search class="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground" />
				<input
					type="text"
					placeholder="Search..."
					class="rounded border border-border bg-well pl-8 pr-3 py-1.5 font-mono text-xs w-full sm:w-40"
					value={channelsStore.searchQuery}
					oninput={(e) => channelsStore.setSearch(e.currentTarget.value)}
				/>
			</div>

			<!-- Add Channel Button -->
			<button
				class="rounded border border-border bg-btn-bg px-3 py-1.5 font-mono text-xs flex items-center justify-center gap-2 hover:bg-hover-wash transition-colors w-full sm:w-auto"
				onclick={() => (showAddModal = true)}
			>
				<Plus class="w-3.5 h-3.5" />
				Add Channel
			</button>
		{/snippet}
	</PageHeader>

	<!-- Channels Table -->
	{#if !connectionStore.isConnected}
		<div class="rounded-lg border border-yellow/30 bg-yellow/5 p-4">
			<p class="font-mono text-xs text-yellow">Not connected to a server.</p>
		</div>
	{:else if channelsStore.isLoading}
		<div class="flex items-center gap-2 text-muted-foreground">
			<div
				class="size-4 animate-spin rounded-full border-2 border-line-3 border-t-transparent"
			></div>
			<span class="font-mono text-xs">Loading channels...</span>
		</div>
	{:else if channelsStore.error}
		<div class="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
			<p class="font-mono text-xs text-destructive">{channelsStore.error}</p>
		</div>
	{:else if channelsStore.filteredChannels.length === 0}
		<div class="relative rounded-lg border border-border bg-card p-8">

			<div class="flex flex-col items-center justify-center gap-2 text-muted-foreground">
				<Terminal class="size-8 opacity-30" />
				<p class="font-mono text-xs">No channels found</p>
				<button
					class="mt-2 rounded border border-border bg-btn-bg px-3 py-1.5 font-mono text-xs hover:bg-hover-wash transition-colors"
					onclick={() => (showAddModal = true)}
				>
					Add your first channel
				</button>
			</div>
		</div>
	{:else if breakpointStore.isMobile}
		<!-- Mobile: Card Layout -->
		<div class="space-y-2">
			{#each channelsStore.filteredChannels as channel (channel.id)}
				<ChannelCard
					{channel}
					onSelect={() => handleRowClick(channel.id)}
					onCheckNow={() => channelsStore.checkChannel(channel.id)}
					onEdit={() => channelsStore.selectChannel(channel.id)}
					onDelete={async () => {
						const success = await channelsStore.deleteChannel(channel.id);
						if (success) {
							toastStore.success('Channel deleted');
						}
					}}
				/>
			{/each}
		</div>
	{:else}
		<!-- Desktop/Tablet: Table Layout -->
		<div class="relative rounded-lg border border-border bg-card overflow-hidden">

			<table class="w-full">
				<thead>
					<tr class="border-b border-border/60 bg-muted/30">
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground w-16"
							>Status</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground w-16"
							>Platform</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground"
							>Channel</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground"
							>Current</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground w-20"
							>Quality</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground w-16"
							>Schedule</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-muted-foreground w-28"
							>Actions</th
						>
					</tr>
				</thead>
				<tbody>
					{#each channelsStore.filteredChannels as channel (channel.id)}
						<tr
							class="border-b border-border/30 hover:bg-hover-wash cursor-pointer transition-colors"
							onclick={() => handleRowClick(channel.id)}
						>
							<td class="px-4 py-3">
								<ChannelStatus
									isRecording={channel.status?.is_recording ?? false}
									isLive={channel.status?.is_live ?? false}
									quotaStatus={channel.quota_status}
									quotaPercent={channel.quota_percent}
								/>
							</td>
							<td class="px-4 py-3">
								<PlatformIcon platform={channel.platform} class="w-4 h-4 text-muted-foreground" />
							</td>
							<td class="px-4 py-3">
								<span class="font-mono text-sm">{channel.name}</span>
							</td>
							<td class="px-4 py-3">
								{#if channel.status?.is_live && channel.status.current_stream}
									<span class="font-mono text-xs text-muted-foreground truncate block max-w-xs">
										{channel.status.current_stream.title}
									</span>
									{#if channel.status.current_stream.game}
										<span class="font-mono text-[10px] text-muted-foreground"
											>{channel.status.current_stream.game}</span
										>
									{/if}
								{:else}
									<span class="font-mono text-[10px] text-muted-foreground">Offline</span>
								{/if}
							</td>
							<td class="px-4 py-3">
								<span
									class="rounded px-1.5 py-0.5 font-mono text-[9px] uppercase bg-chip text-muted-foreground"
								>
									{channel.quality}
								</span>
							</td>
							<td class="px-4 py-3">
								{#if channel.schedule_enabled}
									<Calendar class="w-4 h-4 text-blue" />
								{:else}
									<span class="font-mono text-muted-foreground">—</span>
								{/if}
							</td>
							<td class="px-4 py-3">
								<div class="flex items-center gap-1">
									<button
										class="p-1.5 hover:bg-hover-wash rounded transition-colors"
										title="Open {channel.platform} profile"
										onclick={(e) => openProfile(e, channel)}
									>
										<ExternalLink class="w-3.5 h-3.5 text-muted-foreground" />
									</button>
									<button
										class="p-1.5 hover:bg-hover-wash rounded transition-colors"
										title="Check Now"
										onclick={(e) => handleCheckNow(e, channel.id)}
									>
										<RefreshCw class="w-3.5 h-3.5 text-muted-foreground" />
									</button>
									<button
										class="p-1.5 hover:bg-hover-wash rounded transition-colors"
										title="Edit"
										onclick={(e) => {
											e.stopPropagation();
											channelsStore.selectChannel(channel.id);
										}}
									>
										<Edit class="w-3.5 h-3.5 text-muted-foreground" />
									</button>
									<button
										class="p-1.5 hover:bg-destructive/10 rounded transition-colors"
										title="Delete"
										onclick={(e) => handleDelete(e, channel.id)}
									>
										<Trash2 class="w-3.5 h-3.5 text-destructive" />
									</button>
								</div>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</div>

<!-- Slide-out Panel -->
{#if channelsStore.selectedChannel}
	<ChannelDetailPanel
		channel={channelsStore.selectedChannel}
		onclose={() => channelsStore.selectChannel(null)}
		onsave={handleSaveChannel}
	/>
{/if}

<!-- Add Channel Modal -->
{#if showAddModal}
	<AddChannelModal onclose={() => (showAddModal = false)} oncreate={handleCreateChannel} />
{/if}
