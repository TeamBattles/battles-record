<script lang="ts">
	import { Download, Unplug, Trash2, ArrowRight, Plus } from 'lucide-svelte';
	import { Tween } from 'svelte/motion';
	import { cubicOut } from 'svelte/easing';
	import { extensionsStore, downloadsStore } from '$lib';
	import StatusDot from './ui/StatusDot.svelte';
	import Button from './ui/Button.svelte';
	import { cn } from '$lib/utils/cn';
	import type { ExtensionConnection } from '$lib/api/types';

	interface Props {
		onClose: () => void;
		pairCode: string | null;
		isGeneratingCode: boolean;
		onGeneratePairCode: () => void;
	}

	let { onClose, pairCode, isGeneratingCode, onGeneratePairCode }: Props = $props();

	const connections = $derived(extensionsStore.connections);
	const libraryStatus = $derived(extensionsStore.libraryStatus);
	const config = $derived(extensionsStore.config);

	let activeTab = $state(0);

	// Reset tab if it goes out of bounds
	$effect(() => {
		if (connections.length > 0 && activeTab >= connections.length) {
			activeTab = 0;
		}
	});

	const activeConnection = $derived<ExtensionConnection | undefined>(connections[activeTab]);

	// Library state
	const ytdlpInstalled = $derived(libraryStatus?.ytdlp?.installed ?? false);
	const ffmpegInstalled = $derived(libraryStatus?.ffmpeg?.installed ?? false);
	const bothInstalled = $derived(ytdlpInstalled && ffmpegInstalled);
	const ytdlpUpdate = $derived(libraryStatus?.ytdlp?.update_available);
	const ffmpegUpdate = $derived(libraryStatus?.ffmpeg?.update_available);
	const hasUpdate = $derived(!!ytdlpUpdate || !!ffmpegUpdate);
	const showLibraries = $derived(!bothInstalled || hasUpdate);

	// Downloads summary
	const pendingCount = $derived(downloadsStore.queuedCount);
	const activeCount = $derived(downloadsStore.activeCount);
	const latestActive = $derived(
		downloadsStore.downloads.find(
			(d) =>
				d.status === 'downloading' ||
				d.status === 'extracting_info' ||
				d.status === 'processing'
		)
	);

	const miniProgress = new Tween(0, { duration: 400, easing: cubicOut });
	$effect(() => {
		if (latestActive) {
			miniProgress.set(latestActive.percent);
		} else {
			miniProgress.set(0);
		}
	});

	async function handleDisconnect(clientId: string) {
		await extensionsStore.disconnect(clientId);
		onClose();
	}

	async function handleInstallLibraries() {
		await extensionsStore.installLibraries();
	}

	async function handleUpdateLibrary(name: string) {
		await extensionsStore.updateLibrary(name);
	}
</script>

<div class="w-full">
	{#if connections.length > 1}
		<div class="flex border-b border-border">
			{#each connections as conn, i (conn.client_id)}
				<button
					class={cn(
						'flex-1 px-3 py-2 font-mono text-[10px] uppercase tracking-wider transition-colors truncate',
						i === activeTab
							? 'text-zinc-200 border-b-2 border-emerald-500 bg-zinc-800/50'
							: 'text-zinc-500 hover:text-zinc-400 hover:bg-zinc-800/30'
					)}
					onclick={() => (activeTab = i)}
				>
					{conn.identifier}
				</button>
			{/each}
		</div>
	{/if}

	{#if activeConnection}
		<div class="p-3 space-y-3">
			<!-- Connection Info -->
			<div class="space-y-1.5">
				<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500"
					>Connection</span
				>
				<div class="flex items-center justify-between">
					<div class="flex items-center gap-2">
						<StatusDot
							status={activeConnection.connected ? 'success' : 'error'}
							size="sm"
						/>
						<span class="font-mono text-xs text-zinc-300">
							{activeConnection.identifier}
						</span>
					</div>
					{#if config?.actual_port}
						<span class="font-mono text-[10px] text-zinc-500">
							Port {config.actual_port}
						</span>
					{/if}
				</div>
			</div>

			<!-- Libraries (hidden if both installed and no updates) -->
			{#if showLibraries}
				<div class="space-y-1.5">
					<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500"
						>Libraries</span
					>
					<div class="space-y-1">
						{#if libraryStatus?.ytdlp}
							<div class="flex items-center justify-between">
								<span class="font-mono text-xs text-zinc-400">
									yt-dlp
									{#if ytdlpInstalled && libraryStatus.ytdlp.version}
										<span class="text-zinc-500">v{libraryStatus.ytdlp.version}</span>
									{:else if !ytdlpInstalled}
										<span class="text-red-400">not installed</span>
									{/if}
								</span>
								{#if ytdlpUpdate}
									<Button
										intent="ghost"
										size="sm"
										class="text-amber-400 hover:text-amber-300"
										onclick={() => handleUpdateLibrary('ytdlp')}
									>
										{#snippet children()}Update{/snippet}
									</Button>
								{/if}
							</div>
						{/if}
						{#if libraryStatus?.ffmpeg}
							<div class="flex items-center justify-between">
								<span class="font-mono text-xs text-zinc-400">
									FFmpeg
									{#if ffmpegInstalled && libraryStatus.ffmpeg.version}
										<span class="text-zinc-500">v{libraryStatus.ffmpeg.version}</span>
									{:else if !ffmpegInstalled}
										<span class="text-red-400">not installed</span>
									{/if}
								</span>
								{#if ffmpegUpdate}
									<Button
										intent="ghost"
										size="sm"
										class="text-amber-400 hover:text-amber-300"
										onclick={() => handleUpdateLibrary('ffmpeg')}
									>
										{#snippet children()}Update{/snippet}
									</Button>
								{/if}
							</div>
						{/if}
					</div>
					{#if !bothInstalled}
						<Button
							intent="primary"
							size="sm"
							fullWidth={true}
							class="mt-1 font-mono text-[10px] uppercase tracking-wider"
							onclick={handleInstallLibraries}
						>
							{#snippet children()}Install Libraries{/snippet}
						</Button>
					{/if}
				</div>
			{/if}

			<!-- Downloads Summary -->
			<div class="space-y-1.5">
				<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500"
					>Downloads</span
				>
				<div class="space-y-1.5">
					<div class="flex items-center gap-3 text-xs font-mono text-zinc-400">
						<Download size={12} class="text-zinc-500" />
						<span>
							{#if activeCount > 0 && pendingCount > 0}
								{activeCount} active, {pendingCount} queued
							{:else if activeCount > 0}
								{activeCount} active
							{:else if pendingCount > 0}
								{pendingCount} queued
							{:else}
								No active downloads
							{/if}
						</span>
					</div>

					{#if latestActive}
						<div class="space-y-1">
							<span class="font-mono text-[10px] text-zinc-400 truncate block">
								{latestActive.title ?? latestActive.url}
							</span>
							<div class="w-full h-1 bg-zinc-700 rounded-full overflow-hidden">
								<div
									class="h-full bg-emerald-500 rounded-full"
									style:width="{miniProgress.current}%"
								></div>
							</div>
							<span class="font-mono text-[10px] text-zinc-500">
								{miniProgress.current.toFixed(0)}%
								{#if latestActive.speed}
									- {latestActive.speed}
								{/if}
							</span>
						</div>
					{/if}

					<a
						href="/downloads"
						class="flex items-center gap-1 font-mono text-[10px] text-zinc-400 hover:text-zinc-300 transition-colors"
						onclick={onClose}
					>
						View Downloads
						<ArrowRight size={10} />
					</a>
				</div>
			</div>

			<!-- Disconnect / Delete -->
			<div>
				<Button
					intent="danger"
					size="sm"
					fullWidth={true}
					class="font-mono text-[10px] uppercase tracking-wider"
					onclick={() => handleDisconnect(activeConnection.client_id)}
				>
					{#snippet children()}
						{#if activeConnection.connected}
							<Unplug size={12} />
							Disconnect
						{:else}
							<Trash2 size={12} />
							Delete
						{/if}
					{/snippet}
				</Button>
			</div>

			<!-- Pair New Extension -->
			<div class="pt-2 border-t border-border space-y-1.5">
				<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500"
					>Pair New Extension</span
				>
				{#if pairCode}
					<div class="rounded border border-zinc-700 bg-zinc-800 px-3 py-2 text-center">
						<p class="font-mono text-[10px] text-zinc-500 mb-1">PAIR CODE</p>
						<p class="font-mono text-lg tracking-widest text-emerald-400">{pairCode}</p>
						<p class="font-mono text-[9px] text-zinc-500 mt-1">Enter this code in the browser extension</p>
					</div>
					<Button
						intent="secondary"
						size="sm"
						fullWidth={true}
						class="font-mono"
						disabled={isGeneratingCode}
						onclick={onGeneratePairCode}
					>
						{#snippet children()}
							{isGeneratingCode ? 'Generating...' : 'Regenerate Code'}
						{/snippet}
					</Button>
				{:else}
					<Button
						intent="secondary"
						size="sm"
						fullWidth={true}
						class="font-mono"
						disabled={isGeneratingCode}
						onclick={onGeneratePairCode}
					>
						{#snippet children()}
							<Plus size={12} />
							{isGeneratingCode ? 'Generating...' : 'Generate Pair Code'}
						{/snippet}
					</Button>
				{/if}
			</div>
		</div>
	{:else}
		<div class="p-3">
			<span class="font-mono text-xs text-zinc-500">No connections</span>
		</div>
	{/if}
</div>
