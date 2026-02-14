<script lang="ts">
	import { Menu, Sun, Moon, Monitor } from 'lucide-svelte';
	import { sidebarStore, breakpointStore, themeStore } from '$lib';
	import ServerDropdown from './ServerDropdown.svelte';

	interface Props {
		onAddServer: () => void;
		onManageServers: () => void;
	}

	let { onAddServer, onManageServers }: Props = $props();
</script>

<header class="h-12 border-b border-border bg-card flex items-center px-4">
	<!-- Left: Hamburger (mobile) or Server Selector -->
	<div class="flex items-center gap-2 flex-1">
		{#if breakpointStore.isMobile}
			<button
				class="p-2 hover:bg-muted rounded transition-colors"
				onclick={() => sidebarStore.toggle()}
				aria-label="Toggle menu"
			>
				<Menu size={18} class="text-zinc-500" />
			</button>
		{/if}
		<ServerDropdown {onAddServer} {onManageServers} />
	</div>

	<!-- Center: Title -->
	<div class="flex items-center gap-2">
		<span class="font-mono text-xs uppercase tracking-wider text-zinc-400"> Battles Record </span>
	</div>

	<!-- Right: Theme toggle -->
	<div class="flex-1 flex justify-end">
		<button
			class="flex items-center gap-2 px-3 py-1.5 hover:bg-muted rounded border border-border bg-input transition-colors"
			onclick={() => themeStore.cycle()}
			aria-label="Toggle theme"
		>
			{#if themeStore.mode === 'system'}
				<Monitor size={14} class="text-zinc-500" />
			{:else if themeStore.mode === 'light'}
				<Sun size={14} class="text-amber-500" />
			{:else}
				<Moon size={14} class="text-blue-400" />
			{/if}
			<span class="hidden sm:inline font-mono text-[10px] uppercase tracking-wider text-zinc-500">
				{themeStore.mode}
			</span>
		</button>
	</div>
</header>
