<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';
	import { onMount, onDestroy } from 'svelte';
	import Button from './ui/Button.svelte';

	interface DependencyStatus {
		bun_available: boolean;
		bun_version: string | null;
		ytdlp_available: boolean;
		ytdlp_version: string | null;
	}

	interface InstallProgress {
		dependency: string;
		status: string;
		progress: number | null;
		error: string | null;
	}

	interface Props {
		isRemote: boolean;
		onStatusChange?: (allInstalled: boolean) => void;
	}

	let { isRemote, onStatusChange }: Props = $props();

	let status = $state<DependencyStatus | null>(null);
	let loading = $state(true);
	let installing = $state(false);
	let installError = $state<string | null>(null);
	let currentProgress = $state<InstallProgress | null>(null);

	const allInstalled = $derived(status?.bun_available && status?.ytdlp_available);

	$effect(() => {
		onStatusChange?.(allInstalled ?? false);
	});

	let unlistenProgress: (() => void) | null = null;

	onMount(async () => {
		unlistenProgress = await listen<InstallProgress>('dependency-progress', (event) => {
			currentProgress = event.payload;
			if (event.payload.status === 'error') {
				installError = event.payload.error;
			}
		});

		await checkDependencies();
	});

	onDestroy(() => {
		unlistenProgress?.();
	});

	async function checkDependencies() {
		loading = true;
		try {
			if (isRemote) {
				status = null;
			} else {
				status = await invoke<DependencyStatus>('check_youtube_dependencies');
			}
		} catch (e) {
			console.error('Failed to check dependencies:', e);
		} finally {
			loading = false;
		}
	}

	async function installDependencies() {
		if (!status) return;

		installing = true;
		installError = null;
		currentProgress = null;

		try {
			const result = await invoke<{
				bun_installed: boolean;
				ytdlp_installed: boolean;
				errors: string[];
			}>('install_youtube_dependencies', {
				installBun: !status.bun_available,
				installYtdlp: !status.ytdlp_available
			});

			if (result.errors.length > 0) {
				installError = result.errors.join(', ');
			}

			await checkDependencies();
		} catch (e) {
			installError = e instanceof Error ? e.message : String(e);
		} finally {
			installing = false;
			currentProgress = null;
		}
	}
</script>

{#if loading}
	<div class="rounded border border-zinc-700 bg-zinc-800 p-3">
		<p class="font-mono text-xs text-zinc-400">Checking dependencies...</p>
	</div>
{:else if isRemote && !allInstalled}
	<div class="rounded border border-amber-500/30 bg-amber-500/10 p-3">
		<p class="mb-2 font-mono text-[10px] uppercase tracking-wider text-amber-400">
			Missing Dependencies
		</p>
		<p class="mb-2 font-mono text-xs text-zinc-300">Remote server missing Bun and/or yt-dlp.</p>
		<p class="font-mono text-[10px] text-zinc-500">
			Use the bundled Docker image or install manually on the server.
		</p>
	</div>
{:else if status && !allInstalled}
	<div class="rounded border border-amber-500/30 bg-amber-500/10 p-3">
		<p class="mb-2 font-mono text-[10px] uppercase tracking-wider text-amber-400">
			Required Dependencies
		</p>
		<p class="mb-2 font-mono text-xs text-zinc-300">YouTube recording requires Bun and yt-dlp.</p>

		<div class="mb-3 flex gap-4 font-mono text-xs">
			{#if status.bun_available}
				<span class="text-emerald-400">✓ Bun {status.bun_version ?? ''}</span>
			{:else}
				<span class="text-red-400">✗ Bun missing</span>
			{/if}
			{#if status.ytdlp_available}
				<span class="text-emerald-400">✓ yt-dlp {status.ytdlp_version ?? ''}</span>
			{:else}
				<span class="text-red-400">✗ yt-dlp missing</span>
			{/if}
		</div>

		{#if installing}
			<div class="mb-2">
				<div class="flex items-center gap-2">
					<div
						class="size-3 animate-spin rounded-full border-2 border-amber-400 border-t-transparent"
					></div>
					<span class="font-mono text-xs text-amber-400">
						{#if currentProgress}
							Installing {currentProgress.dependency}...
						{:else}
							Installing dependencies...
						{/if}
					</span>
				</div>
			</div>
		{:else}
			<Button intent="primary" onclick={installDependencies} disabled={installing}>
				Install Missing Dependencies
			</Button>
		{/if}

		{#if installError}
			<p class="mt-2 font-mono text-[10px] text-red-400">{installError}</p>
		{/if}

		<p class="mt-2 font-mono text-[10px] text-zinc-500">Or download the bundled app version.</p>
	</div>
{/if}
