<script lang="ts">
	import { X, Play, Square, RefreshCw, Users, Clock } from 'lucide-svelte';
	import type { Channel, ChannelProfile, ScheduleRule } from '$lib/api/types';
	import ChannelAvatar from './ChannelAvatar.svelte';
	import ChannelStatus from './ChannelStatus.svelte';
	import ChannelImageEditor from './ChannelImageEditor.svelte';
	import ScheduleRulesEditor from './ScheduleRulesEditor.svelte';
	import { ResponsivePanel } from '$lib';
	import { channelsStore } from '$lib/stores/channels.svelte';
	import { api } from '$lib/api';
	import { extractErrorMessage } from '$lib/utils';

	interface Props {
		channel: Channel;
		onclose: () => void;
		onsave: (data: Partial<Channel>) => Promise<void>;
	}

	let { channel, onclose, onsave }: Props = $props();

	// Channel profile for header avatar and images tab
	let channelProfile = $state<ChannelProfile | null>(null);
	let profileLoading = $state(false);
	let profileError = $state<string | null>(null);

	// Load channel profile (for header avatar and Images tab)
	async function loadProfile() {
		if (channelProfile) return; // Already loaded
		profileLoading = true;
		profileError = null;
		try {
			channelProfile = await api.getChannelProfile(channel.id);
		} catch (e) {
			profileError = extractErrorMessage(e, 'Failed to load profile');
		} finally {
			profileLoading = false;
		}
	}

	// Load profile on mount for header avatar
	$effect(() => {
		loadProfile();
	});

	// Refresh profile after image changes
	async function refreshProfile() {
		profileLoading = true;
		try {
			channelProfile = await api.getChannelProfile(channel.id);
		} catch (e) {
			// Ignore refresh errors
		} finally {
			profileLoading = false;
		}
	}

	// Get profile image URL: use channel data first, fallback to loaded profile
	const headerProfileImageUrl = $derived.by(() => {
		// First try the channel's pre-resolved URL
		if (channel.profile_image_url) {
			return channel.profile_image_url;
		}
		// Fallback to loaded profile data (custom > platform)
		if (channelProfile?.custom_profile_url) {
			return channelProfile.custom_profile_url;
		}
		if (channelProfile?.platform_profile_url) {
			return channelProfile.platform_profile_url;
		}
		return null;
	});

	// Quick action states
	let isStopping = $state(false);
	let isChecking = $state(false);

	async function handleStopRecording() {
		isStopping = true;
		const success = await channelsStore.stopRecording(channel.id);
		if (success) {
			onclose();
		}
		isStopping = false;
	}

	async function handleCheckNow() {
		isChecking = true;
		await channelsStore.checkChannel(channel.id);
		isChecking = false;
	}

	// Form state (copy of channel data for editing)
	let quality = $state('best');
	let enabled = $state(true);
	let scheduleEnabled = $state(false);
	let timezone = $state('UTC');

	// Schedule rules
	let scheduleRules = $state<ScheduleRule[]>([]);

	// Filters
	let titleIncludes = $state<string[]>([]);
	let titleExcludes = $state<string[]>([]);
	let gameIncludes = $state<string[]>([]);
	let gameExcludes = $state<string[]>([]);
	let minViewers = $state(0);

	// Storage
	let quotaGb = $state(0);
	let retentionDays = $state(0);

	// Sync form state from channel prop
	$effect(() => {
		quality = channel.quality;
		enabled = channel.enabled;
		scheduleEnabled = channel.schedule_enabled ?? false;
		timezone = channel.timezone ?? 'UTC';
		scheduleRules = channel.schedule_rules ?? [];
		titleIncludes = channel.filters?.title_includes ?? [];
		titleExcludes = channel.filters?.title_excludes ?? [];
		gameIncludes = channel.filters?.game_includes ?? [];
		gameExcludes = channel.filters?.game_excludes ?? [];
		minViewers = channel.filters?.min_viewers ?? 0;
		quotaGb = channel.quota_gb ?? 0;
		retentionDays = channel.retention_days ?? 0;
	});

	// Active tab
	let activeTab = $state<'general' | 'schedule' | 'filters' | 'storage' | 'images'>('general');

	// Loading state
	let isSaving = $state(false);
	let saveError = $state<string | null>(null);

	const tabs = [
		{ id: 'general', label: 'General' },
		{ id: 'schedule', label: 'Schedule' },
		{ id: 'filters', label: 'Filters' },
		{ id: 'storage', label: 'Storage' },
		{ id: 'images', label: 'Images' }
	] as const;

	// Load profile when switching to images tab
	$effect(() => {
		if (activeTab === 'images') {
			loadProfile();
		}
	});

	async function handleSave() {
		isSaving = true;
		saveError = null;
		try {
			await onsave({
				quality,
				enabled,
				schedule_enabled: scheduleEnabled,
				timezone,
				schedule_rules: scheduleRules,
				filters: {
					title_includes: titleIncludes,
					title_excludes: titleExcludes,
					game_includes: gameIncludes,
					game_excludes: gameExcludes,
					min_viewers: minViewers
				},
				// Send null for 0 to clear the value (unlimited)
				quota_gb: quotaGb || undefined,
				retention_days: retentionDays || undefined
			});
		} catch (e) {
			saveError = e instanceof Error ? e.message : 'Failed to save';
		} finally {
			isSaving = false;
		}
	}

	function formatDuration(startedAt: string): string {
		const start = new Date(startedAt);
		const now = new Date();
		const diffMs = now.getTime() - start.getTime();
		const hours = Math.floor(diffMs / (1000 * 60 * 60));
		const minutes = Math.floor((diffMs % (1000 * 60 * 60)) / (1000 * 60));
		return `${hours}h ${minutes}m`;
	}
</script>

<ResponsivePanel open={true} onClose={onclose}>
	{#snippet children()}
		<!-- Header -->
		<div class="p-4 border-b border-border flex-shrink-0">
			<div class="flex items-start justify-between">
				<div class="flex items-center gap-3">
					<ChannelAvatar
						src={headerProfileImageUrl}
						alt={channel.name}
						platform={channel.platform}
						size="lg"
					/>
					<div>
						<h2 class="font-display text-xl tracking-tight uppercase text-foreground">
							{channel.name}
						</h2>
						<div class="flex items-center gap-2 mt-1">
							<ChannelStatus
								isRecording={channel.status?.is_recording ?? false}
								isLive={channel.status?.is_live ?? false}
							/>
							<span class="text-xs text-muted-foreground">
								{channel.status?.is_recording
									? 'Recording'
									: channel.status?.is_live
										? 'Live'
										: 'Offline'}
							</span>
						</div>
					</div>
				</div>
				<button class="p-1 hover:bg-muted rounded transition-colors" onclick={onclose}>
					<X class="w-5 h-5 text-muted-foreground" />
				</button>
			</div>

			<!-- Current Stream Info -->
			{#if channel.status?.is_live && channel.status.current_stream}
				<div class="mt-4 p-3 bg-muted rounded border border-border text-sm">
					<p class="font-medium truncate text-foreground">{channel.status.current_stream.title}</p>
					{#if channel.status.current_stream.game}
						<p class="text-muted-foreground text-xs mt-1">{channel.status.current_stream.game}</p>
					{/if}
					<div class="flex items-center gap-4 mt-2 text-xs text-muted-foreground">
						{#if channel.status.current_stream.viewer_count}
							<span class="flex items-center gap-1">
								<Users class="w-3 h-3" />
								{channel.status.current_stream.viewer_count.toLocaleString()}
							</span>
						{/if}
						<span class="flex items-center gap-1">
							<Clock class="w-3 h-3" />
							{formatDuration(channel.status.current_stream.started_at)}
						</span>
					</div>
				</div>
			{/if}

			<!-- Quick Actions -->
			<div class="flex gap-2 mt-4">
				{#if channel.status?.is_recording}
					<button
						class="flex-1 bg-orange-600 text-white px-3 py-2 text-sm font-mono font-medium flex items-center justify-center gap-2 rounded hover:bg-orange-500 transition-colors disabled:opacity-50"
						onclick={handleStopRecording}
						disabled={isStopping}
					>
						{#if isStopping}
							<div
								class="size-4 animate-spin rounded-full border-2 border-white border-t-transparent"
							></div>
						{:else}
							<Square class="w-4 h-4" />
						{/if}
						Stop Recording
					</button>
				{:else if channel.status?.is_live}
					<button
						class="flex-1 bg-emerald-600 text-white px-3 py-2 text-sm font-mono font-medium flex items-center justify-center gap-2 rounded hover:bg-emerald-500 transition-colors"
					>
						<Play class="w-4 h-4" />
						Start Recording
					</button>
				{/if}
				<button
					class="px-3 py-2 border border-border bg-input text-sm font-mono flex items-center gap-2 hover:bg-muted rounded transition-colors text-foreground disabled:opacity-50"
					onclick={handleCheckNow}
					disabled={isChecking}
				>
					{#if isChecking}
						<RefreshCw class="w-4 h-4 animate-spin" />
					{:else}
						<RefreshCw class="w-4 h-4" />
					{/if}
					Check Now
				</button>
			</div>
		</div>

		<!-- Tabs -->
		<div class="flex border-b border-border flex-shrink-0 overflow-x-auto">
			{#each tabs as tab (tab.id)}
				<button
					class="flex-1 min-w-0 px-2 py-2 text-xs font-mono transition-colors whitespace-nowrap {activeTab === tab.id
						? 'text-foreground border-b-2 border-muted-foreground bg-muted/50'
						: 'text-muted-foreground hover:text-foreground hover:bg-muted/30'}"
					onclick={() => (activeTab = tab.id)}
				>
					{tab.label}
				</button>
			{/each}
		</div>

		<!-- Tab Content - scrollable -->
		<div class="flex-1 overflow-y-auto p-4 min-h-0">
			{#if activeTab === 'general'}
				<div class="space-y-4">
					<div>
						<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-1"
							>Platform</span
						>
						<p class="font-mono capitalize text-foreground">{channel.platform}</p>
					</div>

					<div>
						<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-1"
							>Quality</span
						>
						<select
							class="w-full bg-input border border-border px-3 py-2 rounded font-mono text-sm text-foreground"
							bind:value={quality}
						>
							<option value="best">Best</option>
							<option value="1080p">1080p</option>
							<option value="720p">720p</option>
							<option value="480p">480p</option>
							<option value="audio">Audio Only</option>
						</select>
					</div>

					<div
						class="flex items-center justify-between rounded border border-border bg-input p-3"
					>
						<span class="text-sm font-mono text-foreground">Enabled</span>
						<input type="checkbox" bind:checked={enabled} class="w-5 h-5 accent-emerald-500" />
					</div>
				</div>
			{:else if activeTab === 'schedule'}
				<div class="space-y-4">
					<div
						class="flex items-center justify-between rounded border border-border bg-input p-3"
					>
						<span class="text-sm font-mono text-foreground">Enable Schedule</span>
						<input
							type="checkbox"
							bind:checked={scheduleEnabled}
							class="w-5 h-5 accent-emerald-500"
						/>
					</div>

					{#if scheduleEnabled}
						<div>
							<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-1"
								>Timezone</span
							>
							<select
								class="w-full bg-input border border-border px-3 py-2 rounded font-mono text-sm text-foreground"
								bind:value={timezone}
							>
								<option value="UTC">UTC</option>
								<option value="America/New_York">Eastern Time</option>
								<option value="America/Los_Angeles">Pacific Time</option>
								<option value="Europe/London">London</option>
							</select>
						</div>

						<div>
							<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2"
								>Recording Windows</span
							>
							<ScheduleRulesEditor rules={scheduleRules} onchange={(r) => (scheduleRules = r)} />
						</div>
					{/if}
				</div>
			{:else if activeTab === 'filters'}
				<div class="space-y-4">
					<div>
						<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-1"
							>Title Includes</span
						>
						<input
							type="text"
							class="w-full bg-input border border-border px-3 py-2 text-sm rounded font-mono text-foreground placeholder:text-muted-foreground"
							placeholder="comma-separated keywords"
							value={titleIncludes.join(', ')}
							onchange={(e) =>
								(titleIncludes = e.currentTarget.value
									.split(',')
									.map((s) => s.trim())
									.filter(Boolean))}
						/>
					</div>

					<div>
						<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-1"
							>Title Excludes</span
						>
						<input
							type="text"
							class="w-full bg-input border border-border px-3 py-2 text-sm rounded font-mono text-foreground placeholder:text-muted-foreground"
							placeholder="comma-separated keywords"
							value={titleExcludes.join(', ')}
							onchange={(e) =>
								(titleExcludes = e.currentTarget.value
									.split(',')
									.map((s) => s.trim())
									.filter(Boolean))}
						/>
					</div>

					<div>
						<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-1"
							>Games Include</span
						>
						<input
							type="text"
							class="w-full bg-input border border-border px-3 py-2 text-sm rounded font-mono text-foreground placeholder:text-muted-foreground"
							placeholder="comma-separated games"
							value={gameIncludes.join(', ')}
							onchange={(e) =>
								(gameIncludes = e.currentTarget.value
									.split(',')
									.map((s) => s.trim())
									.filter(Boolean))}
						/>
					</div>

					<div>
						<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-1"
							>Games Exclude</span
						>
						<input
							type="text"
							class="w-full bg-input border border-border px-3 py-2 text-sm rounded font-mono text-foreground placeholder:text-muted-foreground"
							placeholder="comma-separated games"
							value={gameExcludes.join(', ')}
							onchange={(e) =>
								(gameExcludes = e.currentTarget.value
									.split(',')
									.map((s) => s.trim())
									.filter(Boolean))}
						/>
					</div>

					<div>
						<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-1"
							>Minimum Viewers</span
						>
						<input
							type="number"
							class="w-full bg-input border border-border px-3 py-2 text-sm rounded font-mono text-foreground"
							min="0"
							bind:value={minViewers}
						/>
					</div>
				</div>
			{:else if activeTab === 'storage'}
				<div class="space-y-4">
					<div>
						<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-1"
							>Quota (GB)</span
						>
						<input
							type="number"
							class="w-full bg-input border border-border px-3 py-2 text-sm rounded font-mono text-foreground"
							min="0"
							bind:value={quotaGb}
						/>
						<p class="text-xs text-muted-foreground mt-1 font-mono">0 = unlimited</p>
					</div>

					<div>
						<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-1"
							>Retention (days)</span
						>
						<input
							type="number"
							class="w-full bg-input border border-border px-3 py-2 text-sm rounded font-mono text-foreground"
							min="0"
							bind:value={retentionDays}
						/>
						<p class="text-xs text-muted-foreground mt-1 font-mono">0 = keep forever</p>
					</div>
				</div>
			{:else if activeTab === 'images'}
				{#if profileLoading}
					<div class="flex items-center justify-center py-8">
						<div class="size-6 animate-spin rounded-full border-2 border-muted-foreground border-t-transparent"></div>
					</div>
				{:else if profileError}
					<div class="text-center py-8">
						<p class="text-sm text-red-400">{profileError}</p>
						<button
							class="mt-2 text-sm text-muted-foreground hover:text-foreground"
							onclick={refreshProfile}
						>
							Try again
						</button>
					</div>
				{:else if channelProfile}
					<ChannelImageEditor
						channelId={channel.id}
						profile={channelProfile}
						onProfileUpdate={refreshProfile}
					/>
				{/if}
			{/if}
		</div>

		{#if saveError}
			<div class="px-4 py-2 bg-red-500/10 border-t border-red-500/30">
				<p class="font-mono text-xs text-red-400">{saveError}</p>
			</div>
		{/if}

		<!-- Footer -->
		<div class="p-4 border-t border-border flex gap-2 bg-card flex-shrink-0">
			<button
				class="flex-1 bg-muted border border-border text-foreground px-4 py-2 font-mono text-sm rounded hover:bg-muted/80 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
				onclick={handleSave}
				disabled={isSaving}
			>
				{#if isSaving}
					<div
						class="size-4 animate-spin rounded-full border-2 border-muted-foreground border-t-transparent"
					></div>
					Saving...
				{:else}
					Save
				{/if}
			</button>
			<button
				class="px-4 py-2 border border-border bg-input text-foreground font-mono text-sm rounded hover:bg-muted transition-colors"
				onclick={onclose}
			>
				Cancel
			</button>
		</div>
	{/snippet}
</ResponsivePanel>
