<script lang="ts">
	import { ResponsiveModal } from '$lib';
	import DependencyInstaller from './DependencyInstaller.svelte';
	import PlatformIcon from './PlatformIcon.svelte';
	import Button from './ui/Button.svelte';
	import Input from './ui/Input.svelte';
	import Select from './ui/Select.svelte';
	import Checkbox from './ui/Checkbox.svelte';
	import { extractChannelName, validateChannelName } from '$lib/utils/channel';

	interface Props {
		onclose: () => void;
		oncreate: (data: {
			platform: string;
			name: string;
			quality: string;
			scheduleEnabled: boolean;
			filtersEnabled: boolean;
		}) => void;
		isRemoteConnection?: boolean;
	}

	let { onclose, oncreate, isRemoteConnection = false }: Props = $props();

	let platform = $state<'twitch' | 'youtube' | 'kick'>('twitch');
	let channelName = $state('');
	let quality = $state('best');
	let scheduleEnabled = $state(false);
	let filtersEnabled = $state(false);
	let youtubeDepsInstalled = $state(true); // Assume true until checked

	const platforms = [
		{ id: 'twitch', label: 'Twitch' },
		{ id: 'youtube', label: 'YouTube' },
		{ id: 'kick', label: 'Kick' }
	] as const;

	const qualityOptions = [
		{ value: 'best', label: 'Best Available' },
		{ value: '1080p', label: '1080p' },
		{ value: '720p', label: '720p' },
		{ value: '480p', label: '480p' },
		{ value: 'audio', label: 'Audio Only' }
	];

	const placeholders = {
		twitch: 'username or URL (e.g., shroud)',
		youtube: 'channel URL or handle',
		kick: 'username or URL (e.g., xqc)'
	};

	// Extract channel name from URL if needed
	const extractedName = $derived(extractChannelName(platform, channelName));

	// Validate the extracted name
	const validation = $derived(
		channelName.trim().length > 0
			? validateChannelName(platform, extractedName)
			: { valid: false, warning: undefined }
	);

	const canCreate = $derived(
		channelName.trim().length > 0 &&
			validation.valid &&
			(platform !== 'youtube' || youtubeDepsInstalled)
	);

	function handleCreate() {
		if (!canCreate) return;
		oncreate({
			platform,
			name: extractedName,
			quality,
			scheduleEnabled,
			filtersEnabled
		});
	}

	function handleOpenChange(open: boolean) {
		if (!open) onclose();
	}
</script>

<ResponsiveModal open={true} onOpenChange={handleOpenChange} title="Add Channel">
	{#snippet children()}
		<form
			class="space-y-4"
			onsubmit={(e) => {
				e.preventDefault();
				handleCreate();
			}}
		>
			<!-- Platform Selector -->
			<div>
				<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Platform
				</span>
				<div class="flex rounded border border-border overflow-hidden">
					{#each platforms as p (p.id)}
						<button
							type="button"
							class="flex-1 flex items-center justify-center gap-2 px-3 py-2 font-mono text-xs transition-colors {platform ===
							p.id
								? 'bg-muted text-foreground'
								: 'bg-input text-muted-foreground hover:bg-muted/50'}"
							onclick={() => (platform = p.id)}
						>
							<PlatformIcon platform={p.id} class="w-3.5 h-3.5" />
							{p.label}
						</button>
					{/each}
				</div>
			</div>

			<!-- Channel Name -->
			<div>
				<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Channel
				</span>
				<Input
					type="text"
					class={validation.warning ? 'border-amber-500/50' : ''}
					placeholder={placeholders[platform]}
					bind:value={channelName}
				/>
				{#if validation.warning}
					<p class="mt-1 font-mono text-[10px] text-amber-400">{validation.warning}</p>
				{:else if extractedName && extractedName !== channelName.trim()}
					<p class="mt-1 font-mono text-[10px] text-muted-foreground">Will use: {extractedName}</p>
				{/if}
			</div>

			<!-- Quality -->
			<div>
				<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Quality
				</span>
				<Select options={qualityOptions} bind:value={quality} />
			</div>

			<!-- Schedule Toggle -->
			<div class="flex items-center justify-between rounded border border-border bg-input p-3">
				<div>
					<p class="font-mono text-sm text-foreground">Enable Schedule</p>
					<p class="font-mono text-[10px] text-muted-foreground">Only record during specific times</p>
				</div>
				<Checkbox bind:checked={scheduleEnabled} />
			</div>

			{#if scheduleEnabled}
				<div class="rounded border border-amber-500/30 bg-amber-500/10 p-3">
					<p class="font-mono text-[10px] text-amber-400">
						Configure schedule after creating channel.
					</p>
				</div>
			{/if}

			<!-- Filters Toggle -->
			<div class="flex items-center justify-between rounded border border-border bg-input p-3">
				<div>
					<p class="font-mono text-sm text-foreground">Enable Filters</p>
					<p class="font-mono text-[10px] text-muted-foreground">Filter by title, game, or viewers</p>
				</div>
				<Checkbox bind:checked={filtersEnabled} />
			</div>

			{#if filtersEnabled}
				<div class="rounded border border-amber-500/30 bg-amber-500/10 p-3">
					<p class="font-mono text-[10px] text-amber-400">
						Configure filters after creating channel.
					</p>
				</div>
			{/if}

			<!-- YouTube Dependencies (only shown when YouTube selected and deps missing) -->
			{#if platform === 'youtube'}
				<DependencyInstaller
					isRemote={isRemoteConnection}
					onStatusChange={(installed) => (youtubeDepsInstalled = installed)}
				/>
			{/if}
		</form>
	{/snippet}

	{#snippet footer()}
		<div class="flex gap-2">
			<Button type="button" intent="primary" fullWidth disabled={!canCreate} onclick={handleCreate}>
				Create Channel
			</Button>
			<Button onclick={onclose}>Cancel</Button>
		</div>
	{/snippet}
</ResponsiveModal>
