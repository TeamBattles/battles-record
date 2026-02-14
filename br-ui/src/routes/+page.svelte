<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { Video, Tv, HardDrive, ListTodo, Circle, Terminal } from 'lucide-svelte';
	import StatusCard from '$lib/components/StatusCard.svelte';
	import { Panel } from '$lib';
	import { dashboardStore } from '$lib/stores/dashboard.svelte';
	import { connectionStore } from '$lib/stores/connection.svelte';
	import { activityStore } from '$lib/stores/activity.svelte';

	// Reload dashboard when server changes or connection is established
	$effect(() => {
		const serverId = connectionStore.activeServerId;
		if (connectionStore.isConnected && serverId) {
			untrack(() => {
				dashboardStore.load(serverId);
				dashboardStore.subscribe();
			});
		}
	});

	onDestroy(() => {
		dashboardStore.unsubscribeEvents();
	});

	const diskPercent = $derived.by(() => {
		if (!dashboardStore.status?.disk) return 0;
		const total = dashboardStore.status.disk.total_bytes;
		const used = dashboardStore.status.disk.used_bytes;
		if (!total || total === 0) return 0;
		return Math.round((used / total) * 100);
	});

	const diskStatus = $derived(
		diskPercent > 90 ? 'danger' : diskPercent > 75 ? 'warning' : 'default'
	);
</script>

<div class="space-y-4">
	<!-- Page Header -->
	<div class="flex items-center gap-2">
		<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">Dashboard</span>
	</div>

	{#if !connectionStore.isConnected}
		<div class="rounded-lg border border-amber-500/30 bg-amber-500/5 p-4">
			<p class="font-mono text-xs text-amber-400">
				Not connected to a server. Go to Settings to connect.
			</p>
		</div>
	{:else if dashboardStore.isLoading}
		<div class="flex items-center gap-2 text-zinc-500">
			<div
				class="size-4 animate-spin rounded-full border-2 border-zinc-500 border-t-transparent"
			></div>
			<span class="font-mono text-xs">Loading...</span>
		</div>
	{:else if dashboardStore.error}
		<div class="rounded-lg border border-red-500/30 bg-red-500/5 p-4">
			<p class="font-mono text-xs text-red-400">{dashboardStore.error}</p>
		</div>
	{:else}
		<!-- Status Cards -->
		<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
			<StatusCard
				label="Active Recordings"
				value={dashboardStore.activeRecordings.length}
				icon={Video}
				status={dashboardStore.activeRecordings.length > 0 ? 'danger' : 'default'}
			/>
			<StatusCard
				label="Channels Online"
				value="{dashboardStore.channels.filter((c) => c.status?.is_live).length} / {dashboardStore
					.channels.length}"
				icon={Tv}
				status="success"
			/>
			<StatusCard label="Disk Usage" value="{diskPercent}%" icon={HardDrive} status={diskStatus} />
			<StatusCard label="Processing Queue" value="0" icon={ListTodo} status="info" />
		</div>

		<div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
			<!-- Active Recordings Panel -->
			<Panel title="Active Recordings" icon={Video} count={dashboardStore.activeRecordings.length} class="h-full">
				{#if dashboardStore.activeRecordings.length === 0}
					<div class="flex h-full flex-col items-center justify-center gap-2 py-8 text-zinc-500">
						<Terminal class="size-8 opacity-30" />
						<p class="font-mono text-xs">No active recordings</p>
					</div>
				{:else}
					<div class="space-y-2">
						{#each dashboardStore.activeRecordings as recording (recording.id)}
							<div class="flex items-center gap-3 rounded border border-border bg-input p-3">
								<span class="size-2 rounded-full bg-orange-400 animate-pulse"></span>
								<div class="flex-1 min-w-0">
									<p class="font-mono text-sm truncate">{recording.channel_name}</p>
									<p class="font-mono text-[10px] text-zinc-500 truncate">{recording.title}</p>
								</div>
								<span class="font-mono text-xs text-zinc-500">
									{Math.floor((recording.duration_secs ?? 0) / 60)}m
								</span>
							</div>
						{/each}
					</div>
				{/if}
			</Panel>

			<!-- Recent Activity Panel -->
			<Panel title="Recent Activity" count={activityStore.events.length} class="h-full">
				{#if activityStore.events.length === 0}
					<div class="flex h-full flex-col items-center justify-center gap-2 py-8 text-zinc-500">
						<Terminal class="size-8 opacity-30" />
						<p class="font-mono text-xs">No recent activity</p>
					</div>
				{:else}
					<div class="h-full space-y-1 overflow-auto">
						{#each activityStore.events.slice(0, 10) as event (event.id)}
							<div class="flex gap-3 py-1.5 text-sm">
								<span class="font-mono text-[10px] text-zinc-500 shrink-0">
									{event.timestamp.toLocaleTimeString()}
								</span>
								<span class="font-mono text-xs truncate">{event.message}</span>
							</div>
						{/each}
					</div>
				{/if}
			</Panel>
		</div>

		<!-- System Health Panel -->
		<Panel title="System Health">
			<div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
				<div class="flex flex-col gap-1">
					<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500"> Uptime </span>
					<span class="font-mono text-sm">
						{Math.floor((dashboardStore.status?.uptime_secs ?? 0) / 3600)}h
						{Math.floor(((dashboardStore.status?.uptime_secs ?? 0) % 3600) / 60)}m
					</span>
				</div>
				<div class="flex flex-col gap-1">
					<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500">
						Version
					</span>
					<span class="font-mono text-sm">{dashboardStore.status?.version ?? 'Unknown'}</span>
				</div>
				<div class="flex flex-col gap-1">
					<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500"> Status </span>
					<div class="flex items-center gap-1.5">
						<span class="size-2 rounded-full bg-emerald-400"></span>
						<span class="font-mono text-sm">Operational</span>
					</div>
				</div>
				<div class="flex flex-col gap-1">
					<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500">
						Disk Free
					</span>
					<span class="font-mono text-sm">{100 - diskPercent}%</span>
				</div>
			</div>
		</Panel>
	{/if}
</div>
