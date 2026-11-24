<script lang="ts">
	import { Menu, Minus, Square, X } from '@lucide/svelte';
	import { sidebarStore, breakpointStore } from '#lib';
	import ServerDropdown from './ServerDropdown.svelte';
	import ThemeDropdown from './ThemeDropdown.svelte';
	import { browser } from '$app/env';
	import { onMount } from 'svelte';

	interface Props {
		onAddServer: () => void;
		onManageServers: () => void;
	}

	let { onAddServer, onManageServers }: Props = $props();

	// Check if running in Tauri - must be reactive since __TAURI__ loads after hydration
	let isTauri = $state(false);
	let isMaximized = $state(false);
	let appWindow: Awaited<ReturnType<typeof import('@tauri-apps/api/window').getCurrentWindow>> | null = null;

	onMount(async () => {
		// Try to setup Tauri - if it works, we're in Tauri
		try {
			await setupTauriWindow();
			isTauri = true;
		} catch {
			isTauri = false;
		}
	});

	async function setupTauriWindow() {
		const { getCurrentWindow } = await import('@tauri-apps/api/window');
		appWindow = getCurrentWindow();
		isMaximized = await appWindow.isMaximized();

		// Listen for resize to track maximized state
		appWindow.onResized(async () => {
			if (appWindow) {
				isMaximized = await appWindow.isMaximized();
			}
		});
	}

	async function minimize() {
		if (appWindow) {
			await appWindow.minimize();
		}
	}

	async function toggleMaximize() {
		if (appWindow) {
			await appWindow.toggleMaximize();
		}
	}

	async function close() {
		if (appWindow) {
			await appWindow.close();
		}
	}

	const isMac = $derived(browser && navigator.platform.includes('Mac'));
</script>

<header
	class="h-10 border-b border-border bg-rail flex items-center select-none"
	class:pl-[70px]={isTauri && isMac}
>
	<!-- Left: App Icon + Title + Server Dropdown -->
	<div class="flex items-center h-full">
		{#if breakpointStore.isMobile}
			<button
				class="h-full px-3 hover:bg-hover-wash transition-colors flex items-center"
				onclick={() => sidebarStore.toggle()}
				aria-label="Toggle menu"
			>
				<Menu size={16} class="text-muted-foreground" />
			</button>
		{:else}
			<!-- App Icon + Title -->
			<div
				class="h-full flex items-center gap-2.5 px-3 border-r border-border"
				data-tauri-drag-region="deep"
			>
				<img src="/favicon.svg" alt="" class="size-6" />
				<span class="font-mono text-xs uppercase tracking-wider text-muted-foreground">
					Battles Record
				</span>
			</div>
		{/if}
		<div class="h-full flex items-center px-2 border-r border-border">
			<ServerDropdown {onAddServer} {onManageServers} compact />
		</div>
	</div>

	<!-- Center: Drag Region -->
	<div
		class="flex-1 h-full"
		data-tauri-drag-region
	></div>

	<!-- Right: Theme + Window Controls -->
	<div class="flex items-center h-full">
		<ThemeDropdown class="border-l border-border" />

		<!-- Window Controls (Tauri only) -->
		{#if isTauri}
			<button
				class="h-full px-3 hover:bg-hover-wash transition-colors flex items-center border-l border-border"
				onclick={minimize}
				aria-label="Minimize"
			>
				<Minus size={14} class="text-muted-foreground" />
			</button>
			<button
				class="h-full px-3 hover:bg-hover-wash transition-colors flex items-center"
				onclick={toggleMaximize}
				aria-label={isMaximized ? 'Restore' : 'Maximize'}
			>
				{#if isMaximized}
					<svg viewBox="0 0 14 14" width="14" height="14" class="text-muted-foreground">
						<path
							fill="none"
							stroke="currentColor"
							stroke-width="1.5"
							d="M4 1.5h8.5V10M1.5 4v8.5H10V4z"
						/>
					</svg>
				{:else}
					<Square size={12} class="text-muted-foreground" />
				{/if}
			</button>
			<button
				class="h-full px-3 hover:bg-destructive transition-colors flex items-center group"
				onclick={close}
				aria-label="Close"
			>
				<X size={14} class="text-muted-foreground group-hover:text-white" />
			</button>
		{/if}
	</div>
</header>
