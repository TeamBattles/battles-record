<script lang="ts">
	import { Menu, Sun, Moon, Monitor, Minus, Square, X } from 'lucide-svelte';
	import { sidebarStore, breakpointStore, themeStore } from '$lib';
	import ServerDropdown from './ServerDropdown.svelte';
	import { browser } from '$app/environment';
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

	async function startDrag(e: MouseEvent) {
		if (appWindow && e.button === 0) {
			e.preventDefault();
			await appWindow.startDragging();
		}
	}

	const isMac = $derived(browser && navigator.platform.includes('Mac'));
</script>

<header
	class="h-9 border-b border-border bg-card flex items-center select-none"
	class:pl-[70px]={isTauri && isMac}
>
	<!-- Left: App Icon + Title + Server Dropdown -->
	<div class="flex items-center h-full">
		{#if breakpointStore.isMobile}
			<button
				class="h-full px-3 hover:bg-muted transition-colors flex items-center"
				onclick={() => sidebarStore.toggle()}
				aria-label="Toggle menu"
			>
				<Menu size={16} class="text-zinc-500" />
			</button>
		{:else}
			<!-- App Icon + Title -->
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="h-full flex items-center gap-2 px-3 border-r border-border"
				onmousedown={startDrag}
			>
				<img src="/favicon.svg" alt="" class="size-4" />
				<span class="font-mono text-xs uppercase tracking-wider text-zinc-500">
					Battles Record
				</span>
			</div>
		{/if}
		<div class="h-full flex items-center px-2 border-r border-border">
			<ServerDropdown {onAddServer} {onManageServers} compact />
		</div>
	</div>

	<!-- Center: Drag Region -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="flex-1 h-full"
		data-tauri-drag-region
		onmousedown={startDrag}
	></div>

	<!-- Right: Theme + Window Controls -->
	<div class="flex items-center h-full">
		<!-- Theme Toggle -->
		<button
			class="h-full px-3 hover:bg-muted transition-colors flex items-center border-l border-border"
			onclick={() => themeStore.cycle()}
			aria-label="Toggle theme"
			title="Theme: {themeStore.mode}"
		>
			{#if themeStore.mode === 'system'}
				<Monitor size={14} class="text-zinc-500" />
			{:else if themeStore.mode === 'light'}
				<Sun size={14} class="text-amber-500" />
			{:else}
				<Moon size={14} class="text-blue-400" />
			{/if}
		</button>

		<!-- Window Controls (Tauri only) -->
		{#if isTauri}
			<button
				class="h-full px-3 hover:bg-muted transition-colors flex items-center border-l border-border"
				onclick={minimize}
				aria-label="Minimize"
			>
				<Minus size={14} class="text-zinc-500" />
			</button>
			<button
				class="h-full px-3 hover:bg-muted transition-colors flex items-center"
				onclick={toggleMaximize}
				aria-label={isMaximized ? 'Restore' : 'Maximize'}
			>
				{#if isMaximized}
					<svg viewBox="0 0 14 14" width="14" height="14" class="text-zinc-500">
						<path
							fill="none"
							stroke="currentColor"
							stroke-width="1.5"
							d="M4 1.5h8.5V10M1.5 4v8.5H10V4z"
						/>
					</svg>
				{:else}
					<Square size={12} class="text-zinc-500" />
				{/if}
			</button>
			<button
				class="h-full px-3 hover:bg-red-500 transition-colors flex items-center group"
				onclick={close}
				aria-label="Close"
			>
				<X size={14} class="text-zinc-500 group-hover:text-white" />
			</button>
		{/if}
	</div>
</header>
