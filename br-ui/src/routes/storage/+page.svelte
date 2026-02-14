<script lang="ts">
	import {
		HardDrive,
		Film,
		AlertTriangle,
		Settings,
		ArrowUpDown,
		ChevronUp,
		ChevronDown,
		Database,
		Library
	} from 'lucide-svelte';
	import { storageStore } from '$lib/stores/storage.svelte';
	import { channelsStore } from '$lib/stores/channels.svelte';
	import { connectionStore } from '$lib/stores/connection.svelte';
	import { breakpointStore } from '$lib';
	import StatusCard from '$lib/components/StatusCard.svelte';
	import PlatformIcon from '$lib/components/PlatformIcon.svelte';
	import StorageProgressBar from '$lib/components/StorageProgressBar.svelte';
	import CleanupToolsPanel from '$lib/components/CleanupToolsPanel.svelte';
	import ChannelQuotaModal from '$lib/components/ChannelQuotaModal.svelte';
	import CornerBrackets from '$lib/components/ui/CornerBrackets.svelte';
	import type { Channel } from '$lib/api/types';

	let quotaEditChannel = $state<Channel | null>(null);
	let isDiskHovered = $state(false);

	// Reload storage data when server changes or connection is established
	$effect(() => {
		const serverId = connectionStore.activeServerId;
		if (connectionStore.isConnected && serverId) {
			storageStore.load();
			channelsStore.load();
		}
	});

	function getChannelByName(name: string): Channel | undefined {
		return channelsStore.channels.find((c) => c.name === name);
	}
</script>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center gap-3">
		<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">Storage</span>
	</div>

	{#if !connectionStore.isConnected}
		<div class="rounded-lg border border-amber-500/30 bg-amber-500/5 p-4">
			<p class="font-mono text-xs text-amber-400">Not connected to a server.</p>
		</div>
	{:else if storageStore.isLoading}
		<div class="flex items-center gap-2 text-zinc-500">
			<div
				class="size-4 animate-spin rounded-full border-2 border-zinc-500 border-t-transparent"
			></div>
			<span class="font-mono text-xs">Loading storage stats...</span>
		</div>
	{:else if storageStore.error}
		<div class="rounded-lg border border-red-500/30 bg-red-500/5 p-4">
			<p class="font-mono text-xs text-red-400">{storageStore.error}</p>
		</div>
	{:else if storageStore.stats}
		<!-- Stats Cards Grid -->
		{#if storageStore.hasSeparateLibrary}
			<!-- Separate recordings and library directories: show combined Total Size + Recordings, add Library Usage -->
			<div class="grid grid-cols-2 lg:grid-cols-4 gap-4">
				<!-- Combined Total Size + Recordings Card -->
				<div class="relative border border-border bg-card p-4">
					<CornerBrackets />

					<div class="flex items-start justify-between">
						<div class="flex-1">
							<p class="font-mono text-[10px] uppercase tracking-wider text-zinc-500">Total Size</p>
							<p class="mt-1 font-mono text-2xl font-bold tabular-nums">
								{storageStore.totalSizeGB} GB
							</p>
							<p class="font-mono text-[10px] text-zinc-500 mt-1">
								{storageStore.stats.total_recordings} recording{storageStore.stats
									.total_recordings !== 1
									? 's'
									: ''}
							</p>
						</div>
						<HardDrive class="size-5 text-zinc-500" />
					</div>
				</div>

				<!-- Disk Usage Card (with hover for library disk if on different disk) -->
				<div
					class="relative border border-border bg-card p-4 transition-colors {storageStore.libraryOnDifferentDisk
						? 'cursor-pointer hover:border-zinc-500'
						: ''}"
					onmouseenter={() => (isDiskHovered = true)}
					onmouseleave={() => (isDiskHovered = false)}
					role={storageStore.libraryOnDifferentDisk ? 'button' : undefined}
					{...(storageStore.libraryOnDifferentDisk ? { tabindex: 0 } : {})}
				>
					<CornerBrackets />

					<div class="flex items-start justify-between">
						<div class="flex-1">
							{#if isDiskHovered && storageStore.libraryOnDifferentDisk}
								<p class="font-mono text-[10px] uppercase tracking-wider text-cyan-400">
									Library Disk
								</p>
								<p class="mt-1 font-mono text-2xl font-bold tabular-nums">
									{storageStore.libraryDiskUsedPercent}%
								</p>
								<div class="mt-2">
									<StorageProgressBar
										percent={storageStore.libraryDiskUsedPercent}
										size="md"
										warningThreshold={70}
										dangerThreshold={90}
									/>
								</div>
								<p class="font-mono text-[10px] text-zinc-500 mt-1">
									{storageStore.libraryDiskFreeGB} GB free of {storageStore.libraryDiskTotalGB} GB
								</p>
							{:else}
								<p class="font-mono text-[10px] uppercase tracking-wider text-zinc-500">
									{storageStore.libraryOnDifferentDisk ? 'Recordings Disk' : 'Disk Usage'}
								</p>
								<p class="mt-1 font-mono text-2xl font-bold tabular-nums">
									{storageStore.diskUsedPercent}%
								</p>
								<div class="mt-2">
									<StorageProgressBar
										percent={storageStore.diskUsedPercent}
										size="md"
										warningThreshold={70}
										dangerThreshold={90}
									/>
								</div>
								<p class="font-mono text-[10px] text-zinc-500 mt-1">
									{storageStore.diskFreeGB} GB free of {storageStore.diskTotalGB} GB
								</p>
							{/if}
						</div>
						<Database class="size-5 text-zinc-500" />
					</div>
				</div>

				<!-- Recordings Usage Card -->
				<div class="relative border border-border bg-card p-4">
					<CornerBrackets />

					<div class="flex items-start justify-between">
						<div class="flex-1">
							<p class="font-mono text-[10px] uppercase tracking-wider text-zinc-500">
								Recordings Usage
							</p>
							<p class="mt-1 font-mono text-2xl font-bold tabular-nums">
								{storageStore.recordingsUsagePercent}%
							</p>
							<div class="mt-2">
								<StorageProgressBar
									percent={parseFloat(storageStore.recordingsUsagePercent)}
									size="md"
									warningThreshold={50}
									dangerThreshold={75}
								/>
							</div>
							<p class="font-mono text-[10px] text-zinc-500 mt-1">
								{storageStore.totalSizeGB} GB of {storageStore.diskTotalGB} GB total
							</p>
						</div>
						<Film class="size-5 text-zinc-500" />
					</div>
				</div>

				<!-- Library Usage Card -->
				<div class="relative border border-border bg-card p-4">
					<CornerBrackets />

					<div class="flex items-start justify-between">
						<div class="flex-1">
							<p class="font-mono text-[10px] uppercase tracking-wider text-zinc-500">
								Library Usage
							</p>
							<p class="mt-1 font-mono text-2xl font-bold tabular-nums">
								{storageStore.libraryUsagePercent}%
							</p>
							<div class="mt-2">
								<StorageProgressBar
									percent={parseFloat(storageStore.libraryUsagePercent)}
									size="md"
									warningThreshold={50}
									dangerThreshold={75}
								/>
							</div>
							<p class="font-mono text-[10px] text-zinc-500 mt-1">
								{storageStore.librarySizeGB} GB of {storageStore.libraryDiskTotalForUsage} GB total
							</p>
						</div>
						<Library class="size-5 text-zinc-500" />
					</div>
				</div>
			</div>
		{:else}
			<!-- Same directory: show original 4-card layout -->
			<div class="grid grid-cols-2 lg:grid-cols-4 gap-4">
				<StatusCard
					label="Total Size"
					value="{storageStore.totalSizeGB} GB"
					icon={HardDrive}
					status="default"
				/>
				<StatusCard
					label="Recordings"
					value={storageStore.stats.total_recordings}
					icon={Film}
					status="default"
				/>
				<!-- Disk Usage Card (custom) -->
				<div class="relative border border-border bg-card p-4">
					<CornerBrackets />

					<div class="flex items-start justify-between">
						<div class="flex-1">
							<p class="font-mono text-[10px] uppercase tracking-wider text-zinc-500">Disk Usage</p>
							<p class="mt-1 font-mono text-2xl font-bold tabular-nums">
								{storageStore.diskUsedPercent}%
							</p>
							<div class="mt-2">
								<StorageProgressBar
									percent={storageStore.diskUsedPercent}
									size="md"
									warningThreshold={70}
									dangerThreshold={90}
								/>
							</div>
							<p class="font-mono text-[10px] text-zinc-500 mt-1">
								{storageStore.diskFreeGB} GB free of {storageStore.diskTotalGB} GB
							</p>
						</div>
						<Database class="size-5 text-zinc-500" />
					</div>
				</div>
				<!-- Recordings Usage Card (custom) -->
				<div class="relative border border-border bg-card p-4">
					<CornerBrackets />

					<div class="flex items-start justify-between">
						<div class="flex-1">
							<p class="font-mono text-[10px] uppercase tracking-wider text-zinc-500">
								Recordings Usage
							</p>
							<p class="mt-1 font-mono text-2xl font-bold tabular-nums">
								{storageStore.recordingsUsagePercent}%
							</p>
							<div class="mt-2">
								<StorageProgressBar
									percent={parseFloat(storageStore.recordingsUsagePercent)}
									size="md"
									warningThreshold={50}
									dangerThreshold={75}
								/>
							</div>
							<p class="font-mono text-[10px] text-zinc-500 mt-1">
								{storageStore.totalSizeGB} GB of {storageStore.diskTotalGB} GB total
							</p>
						</div>
						<Film class="size-5 text-zinc-500" />
					</div>
				</div>
			</div>
		{/if}

		<!-- Disk Warning Banner -->
		{#if storageStore.diskUsedPercent >= 90}
			<div class="rounded-lg border border-red-500/30 bg-red-500/5 p-4 flex items-start gap-3">
				<AlertTriangle class="size-5 text-red-400 flex-shrink-0" />
				<div>
					<p class="font-mono text-sm text-red-300">Low disk space warning</p>
					<p class="font-mono text-xs text-red-400/80 mt-1">
						Only {storageStore.diskFreeGB} GB remaining. Consider cleaning up old recordings.
					</p>
				</div>
			</div>
		{:else if storageStore.diskUsedPercent >= 70}
			<div class="rounded-lg border border-amber-500/30 bg-amber-500/5 p-4 flex items-start gap-3">
				<AlertTriangle class="size-5 text-amber-400 flex-shrink-0" />
				<div>
					<p class="font-mono text-sm text-amber-300">Disk space notice</p>
					<p class="font-mono text-xs text-amber-400/80 mt-1">
						{storageStore.diskUsedPercent}% of disk space used. {storageStore.diskFreeGB} GB remaining.
					</p>
				</div>
			</div>
		{/if}

		<!-- Storage by Channel -->
		{#if storageStore.stats.per_channel.length > 0}
			<div class="relative border border-border bg-card overflow-hidden">
				<CornerBrackets class="z-10" />

				<!-- Header -->
				<div class="flex items-center gap-2 border-b border-border/60 bg-muted/30 px-4 py-2">
					<HardDrive class="size-4 text-zinc-500" />
					<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">
						Storage by Channel
					</span>
					<span
						class="rounded bg-zinc-200 px-1.5 py-0.5 font-mono text-[10px] text-zinc-600 dark:bg-zinc-800 dark:text-zinc-500"
					>
						{storageStore.stats.per_channel.length}
					</span>
				</div>

				{#if breakpointStore.isMobile}
					<!-- Mobile: Card Layout -->
					<div class="divide-y divide-border/30">
						{#each storageStore.sortedChannelStats as channelStats}
							{@const channel = getChannelByName(channelStats.channel)}
							<div class="p-4 space-y-2">
								<div class="flex items-center justify-between">
									<div class="flex items-center gap-2">
										<PlatformIcon
											platform={channelStats.platform as 'twitch' | 'youtube' | 'kick'}
											class="w-4 h-4 text-zinc-500"
										/>
										<span class="font-mono text-sm">{channelStats.channel}</span>
									</div>
									{#if channel}
										<button
											class="p-1.5 hover:bg-muted rounded transition-colors"
											title="Edit Quota"
											onclick={() => (quotaEditChannel = channel)}
										>
											<Settings class="w-3.5 h-3.5 text-zinc-500" />
										</button>
									{/if}
								</div>

								<div class="flex items-center gap-4">
									<StorageProgressBar
										percent={storageStore.getChannelPercent(channelStats)}
										size="sm"
									/>
								</div>

								<div class="flex items-center justify-between text-zinc-500">
									<span class="font-mono text-xs"
										>{channelStats.count} recording{channelStats.count !== 1 ? 's' : ''}</span
									>
									<span class="font-mono text-xs"
										>{storageStore.formatBytes(channelStats.size_bytes)}</span
									>
								</div>

								{#if channel?.quota_gb || channel?.retention_days}
									<div class="flex items-center gap-2 text-zinc-500">
										{#if channel.quota_gb}
											<span class="font-mono text-[10px]">Max: {channel.quota_gb} GB</span>
										{/if}
										{#if channel.retention_days}
											<span class="font-mono text-[10px]">Keep: {channel.retention_days} days</span>
										{/if}
									</div>
								{/if}
							</div>
						{/each}
					</div>
				{:else}
					<!-- Desktop: Table Layout -->
					<table class="w-full">
						<thead>
							<tr class="border-b border-border/60 bg-muted/30">
								<th class="px-4 py-2 text-left w-8"></th>
								<th class="px-4 py-2 text-left">
									<button
										class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 flex items-center gap-1 hover:text-zinc-300 transition-colors"
										onclick={() => storageStore.setSort('channel')}
									>
										Channel
										{#if storageStore.sortBy === 'channel'}
											{#if storageStore.sortOrder === 'desc'}
												<ChevronDown class="size-3" />
											{:else}
												<ChevronUp class="size-3" />
											{/if}
										{:else}
											<ArrowUpDown class="size-3" />
										{/if}
									</button>
								</th>
								<th class="px-4 py-2 text-left w-48">
									<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500">
										Usage
									</span>
								</th>
								<th class="px-4 py-2 text-left">
									<button
										class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 flex items-center gap-1 hover:text-zinc-300 transition-colors"
										onclick={() => storageStore.setSort('count')}
									>
										Count
										{#if storageStore.sortBy === 'count'}
											{#if storageStore.sortOrder === 'desc'}
												<ChevronDown class="size-3" />
											{:else}
												<ChevronUp class="size-3" />
											{/if}
										{:else}
											<ArrowUpDown class="size-3" />
										{/if}
									</button>
								</th>
								<th class="px-4 py-2 text-left">
									<button
										class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 flex items-center gap-1 hover:text-zinc-300 transition-colors"
										onclick={() => storageStore.setSort('size')}
									>
										Size
										{#if storageStore.sortBy === 'size'}
											{#if storageStore.sortOrder === 'desc'}
												<ChevronDown class="size-3" />
											{:else}
												<ChevronUp class="size-3" />
											{/if}
										{:else}
											<ArrowUpDown class="size-3" />
										{/if}
									</button>
								</th>
								<th class="px-4 py-2 text-left">
									<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500">
										Quota
									</span>
								</th>
								<th class="px-4 py-2 text-left w-20">
									<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500">
										Actions
									</span>
								</th>
							</tr>
						</thead>
						<tbody>
							{#each storageStore.sortedChannelStats as channelStats}
								{@const channel = getChannelByName(channelStats.channel)}
								{@const percent = storageStore.getChannelPercent(channelStats)}
								<tr class="border-b border-border/30 hover:bg-muted/30 transition-colors">
									<td class="px-4 py-3">
										<PlatformIcon
											platform={channelStats.platform as 'twitch' | 'youtube' | 'kick'}
											class="w-4 h-4 text-zinc-500"
										/>
									</td>
									<td class="px-4 py-3">
										<span class="font-mono text-sm">{channelStats.channel}</span>
									</td>
									<td class="px-4 py-3">
										<div class="flex items-center gap-2">
											<div class="flex-1">
												<StorageProgressBar {percent} size="sm" />
											</div>
											<span class="font-mono text-[10px] text-zinc-500 w-8 text-right"
												>{percent}%</span
											>
										</div>
									</td>
									<td class="px-4 py-3">
										<span class="font-mono text-xs text-zinc-400">{channelStats.count}</span>
									</td>
									<td class="px-4 py-3">
										<span class="font-mono text-xs text-zinc-400">
											{storageStore.formatBytes(channelStats.size_bytes)}
										</span>
									</td>
									<td class="px-4 py-3">
										{#if channel?.quota_gb}
											<span class="font-mono text-xs text-zinc-400">{channel.quota_gb} GB</span>
										{:else if channel?.retention_days}
											<span class="font-mono text-xs text-zinc-400"
												>{channel.retention_days} days</span
											>
										{:else}
											<span class="font-mono text-xs text-zinc-500">Unlimited</span>
										{/if}
									</td>
									<td class="px-4 py-3">
										{#if channel}
											<button
												class="p-1.5 hover:bg-muted rounded transition-colors"
												title="Edit Quota"
												onclick={() => (quotaEditChannel = channel)}
											>
												<Settings class="w-3.5 h-3.5 text-zinc-500" />
											</button>
										{:else}
											<span class="font-mono text-[10px] text-zinc-600">-</span>
										{/if}
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				{/if}
			</div>
		{:else}
			<div class="relative border border-border bg-card p-8">
				<CornerBrackets />

				<div class="flex flex-col items-center justify-center gap-2 text-zinc-500">
					<Film class="size-8 opacity-30" />
					<p class="font-mono text-xs">No recordings yet</p>
				</div>
			</div>
		{/if}

		<!-- Cleanup Tools -->
		<CleanupToolsPanel />

		<!-- Storage Directories Panel -->
		<div class="relative border border-border bg-card overflow-hidden">
			<CornerBrackets class="z-10" />

			<!-- Header -->
			<div class="flex items-center gap-2 border-b border-border/60 bg-muted/30 px-4 py-2">
				<HardDrive class="size-4 text-zinc-500" />
				<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">
					Storage Directories
				</span>
			</div>

			<div class="divide-y divide-border/30">
				<div class="px-4 py-3">
					<p class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-1">
						Recordings Directory
					</p>
					<p class="font-mono text-xs text-zinc-300 break-all">
						{storageStore.stats?.recordings_dir ?? 'Not configured'}
					</p>
					<p class="font-mono text-[10px] text-zinc-500 mt-1">
						Raw .ts segments stored during recording
					</p>
				</div>
				<div class="px-4 py-3">
					<p class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-1">
						Library Directory
					</p>
					<p class="font-mono text-xs text-zinc-300 break-all">
						{storageStore.stats?.library_dir ?? 'Not configured'}
					</p>
					<p class="font-mono text-[10px] text-zinc-500 mt-1">
						Processed files and Jellyfin metadata
					</p>
				</div>
			</div>
		</div>

		<!-- Channel Quotas Panel -->
		<div class="relative border border-border bg-card overflow-hidden">
			<CornerBrackets class="z-10" />

			<!-- Header -->
			<div class="flex items-center gap-2 border-b border-border/60 bg-muted/30 px-4 py-2">
				<Settings class="size-4 text-zinc-500" />
				<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">
					Channel Quotas
				</span>
			</div>

			{#if channelsStore.channels.length === 0}
				<div class="p-4 text-center">
					<p class="font-mono text-xs text-zinc-500">No channels configured</p>
				</div>
			{:else}
				<div class="divide-y divide-border/30">
					{#each channelsStore.channels as channel}
						<div
							class="flex items-center justify-between px-4 py-3 hover:bg-muted/30 transition-colors"
						>
							<div class="flex items-center gap-3">
								<PlatformIcon platform={channel.platform} class="w-4 h-4 text-zinc-500" />
								<span class="font-mono text-sm">{channel.name}</span>
							</div>
							<div class="flex items-center gap-4">
								<div class="text-right">
									<span class="font-mono text-xs text-zinc-400">
										{channel.quota_gb ? `${channel.quota_gb} GB` : 'Unlimited'}
									</span>
									{#if channel.retention_days}
										<span class="font-mono text-[10px] text-zinc-500 ml-2">
											{channel.retention_days} days
										</span>
									{/if}
								</div>
								<button
									class="rounded border border-border bg-input px-3 py-1 font-mono text-xs hover:bg-muted transition-colors"
									onclick={() => (quotaEditChannel = channel)}
								>
									Edit
								</button>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	{/if}
</div>

<!-- Channel Quota Modal -->
{#if quotaEditChannel}
	<ChannelQuotaModal channel={quotaEditChannel} onclose={() => (quotaEditChannel = null)} />
{/if}
