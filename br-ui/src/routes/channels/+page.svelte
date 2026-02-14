<script lang="ts">
	import { onDestroy } from 'svelte';
	import { Search, Plus, Calendar, Trash2, RefreshCw, Edit, Terminal, ExternalLink } from 'lucide-svelte';
	import { channelsStore } from '$lib/stores/channels.svelte';
	import { connectionStore } from '$lib/stores/connection.svelte';
	import { breakpointStore, ChannelCard, toastStore } from '$lib';
	import { PLATFORM_PROFILE_URLS } from '$lib/utils/constants';
	import { open } from '@tauri-apps/plugin-shell';
	import PlatformIcon from '$lib/components/PlatformIcon.svelte';
	import ChannelStatus from '$lib/components/ChannelStatus.svelte';
	import ChannelDetailPanel from '$lib/components/ChannelDetailPanel.svelte';
	import AddChannelModal from '$lib/components/AddChannelModal.svelte';
	import CornerBrackets from '$lib/components/ui/CornerBrackets.svelte';
	import type { Channel } from '$lib/api/types';

	import { untrack } from 'svelte';

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
	<!-- Header Bar -->
	<div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
		<div class="flex items-center gap-3">
			<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">Channels</span>
			<span
				class="rounded bg-zinc-200 px-1.5 py-0.5 font-mono text-[10px] text-zinc-600 dark:bg-zinc-800 dark:text-zinc-500"
			>
				{channelsStore.channelCount}
			</span>
		</div>

		<div class="flex flex-col gap-2 sm:flex-row sm:items-center sm:gap-2">
			<!-- Platform Filter -->
			<select
				class="rounded border border-border bg-input px-3 py-1.5 font-mono text-xs w-full sm:w-auto"
				value={channelsStore.platformFilter}
				onchange={(e) =>
					channelsStore.setFilter(e.currentTarget.value as 'all' | 'twitch' | 'youtube' | 'kick')}
			>
				<option value="all">All Platforms</option>
				<option value="twitch">Twitch</option>
				<option value="youtube">YouTube</option>
				<option value="kick">Kick</option>
			</select>

			<!-- Search -->
			<div class="relative">
				<Search class="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-zinc-500" />
				<input
					type="text"
					placeholder="Search..."
					class="rounded border border-border bg-input pl-8 pr-3 py-1.5 font-mono text-xs w-full sm:w-40"
					value={channelsStore.searchQuery}
					oninput={(e) => channelsStore.setSearch(e.currentTarget.value)}
				/>
			</div>

			<!-- Add Channel Button -->
			<button
				class="rounded border border-border bg-input px-3 py-1.5 font-mono text-xs flex items-center justify-center gap-2 hover:bg-muted transition-colors w-full sm:w-auto"
				onclick={() => (showAddModal = true)}
			>
				<Plus class="w-3.5 h-3.5" />
				Add Channel
			</button>
		</div>
	</div>

	<!-- Channels Table -->
	{#if !connectionStore.isConnected}
		<div class="rounded-lg border border-amber-500/30 bg-amber-500/5 p-4">
			<p class="font-mono text-xs text-amber-400">Not connected to a server.</p>
		</div>
	{:else if channelsStore.isLoading}
		<div class="flex items-center gap-2 text-zinc-500">
			<div
				class="size-4 animate-spin rounded-full border-2 border-zinc-500 border-t-transparent"
			></div>
			<span class="font-mono text-xs">Loading channels...</span>
		</div>
	{:else if channelsStore.error}
		<div class="rounded-lg border border-red-500/30 bg-red-500/5 p-4">
			<p class="font-mono text-xs text-red-400">{channelsStore.error}</p>
		</div>
	{:else if channelsStore.filteredChannels.length === 0}
		<div class="relative border border-border bg-card p-8">
			<CornerBrackets />

			<div class="flex flex-col items-center justify-center gap-2 text-zinc-500">
				<Terminal class="size-8 opacity-30" />
				<p class="font-mono text-xs">No channels found</p>
				<button
					class="mt-2 rounded border border-border bg-input px-3 py-1.5 font-mono text-xs hover:bg-muted transition-colors"
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
		<div class="relative border border-border bg-card overflow-hidden">
			<CornerBrackets class="z-10" />

			<table class="w-full">
				<thead>
					<tr class="border-b border-border/60 bg-muted/30">
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500 w-16"
							>Status</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500 w-16"
							>Platform</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500"
							>Channel</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500"
							>Current</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500 w-20"
							>Quality</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500 w-16"
							>Schedule</th
						>
						<th
							class="px-4 py-2 text-left font-mono text-[10px] uppercase tracking-wider text-zinc-500 w-28"
							>Actions</th
						>
					</tr>
				</thead>
				<tbody>
					{#each channelsStore.filteredChannels as channel (channel.id)}
						<tr
							class="border-b border-border/30 hover:bg-muted/30 cursor-pointer transition-colors"
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
								<PlatformIcon platform={channel.platform} class="w-4 h-4 text-zinc-500" />
							</td>
							<td class="px-4 py-3">
								<span class="font-mono text-sm">{channel.name}</span>
							</td>
							<td class="px-4 py-3">
								{#if channel.status?.is_live && channel.status.current_stream}
									<span class="font-mono text-xs text-zinc-400 truncate block max-w-xs">
										{channel.status.current_stream.title}
									</span>
									{#if channel.status.current_stream.game}
										<span class="font-mono text-[10px] text-zinc-500"
											>{channel.status.current_stream.game}</span
										>
									{/if}
								{:else}
									<span class="font-mono text-[10px] text-zinc-500">Offline</span>
								{/if}
							</td>
							<td class="px-4 py-3">
								<span
									class="rounded bg-zinc-200 px-1.5 py-0.5 font-mono text-[9px] uppercase text-zinc-600 dark:bg-zinc-800 dark:text-zinc-500"
								>
									{channel.quality}
								</span>
							</td>
							<td class="px-4 py-3">
								{#if channel.schedule_enabled}
									<Calendar class="w-4 h-4 text-blue-400" />
								{:else}
									<span class="font-mono text-zinc-500">—</span>
								{/if}
							</td>
							<td class="px-4 py-3">
								<div class="flex items-center gap-1">
									<button
										class="p-1.5 hover:bg-muted rounded transition-colors"
										title="Open {channel.platform} profile"
										onclick={(e) => openProfile(e, channel)}
									>
										<ExternalLink class="w-3.5 h-3.5 text-zinc-500" />
									</button>
									<button
										class="p-1.5 hover:bg-muted rounded transition-colors"
										title="Check Now"
										onclick={(e) => handleCheckNow(e, channel.id)}
									>
										<RefreshCw class="w-3.5 h-3.5 text-zinc-500" />
									</button>
									<button
										class="p-1.5 hover:bg-muted rounded transition-colors"
										title="Edit"
										onclick={(e) => {
											e.stopPropagation();
											channelsStore.selectChannel(channel.id);
										}}
									>
										<Edit class="w-3.5 h-3.5 text-zinc-500" />
									</button>
									<button
										class="p-1.5 hover:bg-red-500/10 rounded transition-colors"
										title="Delete"
										onclick={(e) => handleDelete(e, channel.id)}
									>
										<Trash2 class="w-3.5 h-3.5 text-red-400" />
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
