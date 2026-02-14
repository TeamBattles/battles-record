<script lang="ts">
	import { onMount } from 'svelte';
	import { browser } from '$app/environment';
	import CornerBrackets from './ui/CornerBrackets.svelte';

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
		} catch (e) {
			// Not running in Tauri (dev mode), ignore
		}
	}
</script>

<div class="h-screen flex flex-col items-center justify-center bg-background-deep">
	<!-- Main container with corner brackets -->
	<div class="relative px-12 py-10">
		<CornerBrackets size="lg" />

		<!-- Content -->
		<div class="flex flex-col items-center gap-6">
			<!-- App name -->
			<h1 class="font-display text-4xl tracking-wide text-foreground">BATTLES RECORD</h1>

			<!-- Animated spinner -->
			<div class="relative size-8">
				<div
					class="absolute inset-0 rounded-full border-2 border-zinc-700 dark:border-zinc-600"
				></div>
				<div
					class="absolute inset-0 animate-spin rounded-full border-2 border-transparent border-t-orange-400"
				></div>
			</div>

			<!-- Status text -->
			<p class="font-mono text-xs uppercase tracking-wider text-zinc-500">{status}</p>
		</div>
	</div>
</div>
