<script lang="ts">
	import { onMount } from 'svelte';
	import { browser } from '$app/env';
	import HexMesh from './ui/HexMesh.svelte';
	import LuxuryCornerSlivers from './ui/LuxuryCornerSlivers.svelte';

	interface Props {
		status?: string;
	}

	let { status = 'Initializing...' }: Props = $props();

	onMount(() => {
		// Show the window now that the loading screen is rendered
		showWindow();
	});

	async function showWindow() {
		if (!browser) return;
		try {
			const { getCurrentWindow } = await import('@tauri-apps/api/window');
			const window = getCurrentWindow();
			await window.show();
			await window.setFocus();
		} catch {
			// Not running in Tauri (dev mode), ignore
		}
	}
</script>

<div
	class="relative flex h-screen flex-col items-center justify-center overflow-hidden bg-background"
>
	<div class="pointer-events-none absolute inset-x-0 top-0 h-2/3" aria-hidden="true">
		<HexMesh ripple />
	</div>

	<div class="relative z-10 rounded-xl border border-border bg-card px-12 py-10">
		<LuxuryCornerSlivers />

		<div class="flex flex-col items-center gap-6">
			<h1 class="font-display text-4xl tracking-wide text-foreground">BATTLES RECORD</h1>

			<div class="relative size-8">
				<div class="absolute inset-0 rounded-full border-2 border-line-2"></div>
				<div
					class="absolute inset-0 animate-spin rounded-full border-2 border-transparent border-t-primary"
				></div>
			</div>

			<p class="font-mono text-xs tracking-wider text-muted-foreground uppercase">{status}</p>
		</div>
	</div>
</div>
