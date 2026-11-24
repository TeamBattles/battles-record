<script lang="ts">
	import { Calendar, Clock, ChevronRight } from '@lucide/svelte';
	import PageHeader from '#lib/components/ui/PageHeader.svelte';
	import { untrack } from 'svelte';
	import { channelsStore } from '#lib/stores/channels.svelte.ts';
	import { connectionStore } from '#lib/stores/connection.svelte.ts';
	import PlatformIcon from '#lib/components/PlatformIcon.svelte';
	import ScheduleSummary from '#lib/components/ScheduleSummary.svelte';
	import ChannelDetailPanel from '#lib/components/ChannelDetailPanel.svelte';
	import LuxuryCornerSlivers from '#lib/components/ui/LuxuryCornerSlivers.svelte';
	import type { Channel } from '#lib/api/types.ts';

	// Reload channels when server changes or connection is established
	$effect(() => {
		const serverId = connectionStore.activeServerId;
		if (connectionStore.isConnected && serverId) {
			untrack(() => {
				channelsStore.load(serverId);
			});
		}
	});

	const scheduledChannels = $derived(
		channelsStore.channels.filter((c) => c.schedule_enabled && (c.schedule_rules?.length ?? 0) > 0)
	);

	const unscheduledChannels = $derived(
		channelsStore.channels.filter(
			(c) => !c.schedule_enabled || (c.schedule_rules?.length ?? 0) === 0
		)
	);

	function openChannelPanel(channel: Channel) {
		channelsStore.selectChannel(channel.id);
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
</script>

<div class="space-y-6">
	<PageHeader
		title="Schedules"
		eyebrow="Record"
		subtitle="Manage recording schedules for your channels"
		icon={Calendar}
	/>

	{#if !connectionStore.isConnected}
		<div class="rounded-lg border border-yellow/30 bg-yellow/5 p-4">
			<p class="font-mono text-xs text-yellow">Not connected to a server.</p>
		</div>
	{:else if channelsStore.isLoading}
		<div class="flex items-center gap-2 text-muted-foreground">
			<div
				class="size-4 animate-spin rounded-full border-2 border-muted-foreground border-t-transparent"
			></div>
			<span class="font-mono text-xs">Loading channels...</span>
		</div>
	{:else}
		<!-- Scheduled Channels -->
		<section>
			<div class="flex items-center gap-2 mb-3">
				<Calendar size={16} class="text-green" />
				<h2 class="font-mono text-xs uppercase tracking-wider text-muted-foreground">
					Scheduled ({scheduledChannels.length})
				</h2>
			</div>

			{#if scheduledChannels.length === 0}
				<div class="relative rounded-lg border border-border bg-card p-6">

					<p class="font-mono text-xs text-muted-foreground text-center">
						No channels have schedules configured
					</p>
				</div>
			{:else}
				<div class="space-y-2">
					{#each scheduledChannels as channel (channel.id)}
						<button
							class="group w-full relative rounded-lg border border-border bg-card p-4 text-left hover:bg-hover-wash transition-colors"
							onclick={() => openChannelPanel(channel)}
						>
							<LuxuryCornerSlivers corners="tl" length="md" hoverFadeIn />

							<div class="flex items-start justify-between gap-4">
								<div class="flex-1 min-w-0">
									<div class="flex items-center gap-2 mb-2">
										<PlatformIcon platform={channel.platform} class="w-4 h-4 text-muted-foreground" />
										<span class="font-mono text-sm text-foreground">{channel.name}</span>
										{#if channel.timezone && channel.timezone !== 'UTC'}
											<span class="font-mono text-[10px] text-muted-foreground/70">({channel.timezone})</span>
										{/if}
									</div>
									<ScheduleSummary rules={channel.schedule_rules ?? []} />
								</div>
								<ChevronRight size={16} class="text-muted-foreground flex-shrink-0" />
							</div>
						</button>
					{/each}
				</div>
			{/if}
		</section>

		<!-- Unscheduled Channels -->
		{#if unscheduledChannels.length > 0}
			<section>
				<div class="flex items-center gap-2 mb-3">
					<Clock size={16} class="text-muted-foreground" />
					<h2 class="font-mono text-xs uppercase tracking-wider text-muted-foreground">
						Without Schedule ({unscheduledChannels.length})
					</h2>
				</div>

				<div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
					{#each unscheduledChannels as channel (channel.id)}
						<button
							class="flex items-center gap-3 rounded border border-border bg-card/50 p-3 text-left hover:bg-hover-wash transition-colors"
							onclick={() => openChannelPanel(channel)}
						>
							<PlatformIcon platform={channel.platform} class="w-4 h-4 text-muted-foreground" />
							<span class="font-mono text-sm text-muted-foreground truncate">{channel.name}</span>
						</button>
					{/each}
				</div>
			</section>
		{/if}
	{/if}
</div>

<!-- Channel Detail Panel -->
{#if channelsStore.selectedChannel}
	<ChannelDetailPanel
		channel={channelsStore.selectedChannel}
		onclose={() => channelsStore.selectChannel(null)}
		onsave={handleSaveChannel}
	/>
{/if}
