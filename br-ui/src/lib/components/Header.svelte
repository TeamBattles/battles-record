<script lang="ts">
	import { Menu, Palette } from '@lucide/svelte';
	import { sidebarStore, breakpointStore, appearanceStore } from '#lib';
	import { THEME_OPTIONS } from '#lib/appearance.ts';
	import ServerDropdown from './ServerDropdown.svelte';

	interface Props {
		onAddServer: () => void;
		onManageServers: () => void;
	}

	let { onAddServer, onManageServers }: Props = $props();

	const themeLabel = $derived(
		THEME_OPTIONS.find((option) => option.value === appearanceStore.prefs.theme)?.label ?? ''
	);
</script>

<header class="h-12 border-b border-border bg-card flex items-center px-4">
	<!-- Left: Hamburger (mobile) or Server Selector -->
	<div class="flex items-center gap-2 flex-1">
		{#if breakpointStore.isMobile}
			<button
				class="p-2 hover:bg-hover-wash rounded transition-colors"
				onclick={() => sidebarStore.toggle()}
				aria-label="Toggle menu"
			>
				<Menu size={18} class="text-muted-foreground" />
			</button>
		{/if}
		<ServerDropdown {onAddServer} {onManageServers} />
	</div>

	<!-- Center: Title -->
	<div class="flex items-center gap-2">
		<span class="font-mono text-xs uppercase tracking-wider text-muted-foreground"> Battles Record </span>
	</div>

	<!-- Right: Theme toggle -->
	<div class="flex-1 flex justify-end">
		<button
			class="flex items-center gap-2 px-3 py-1.5 hover:bg-hover-wash rounded border border-border bg-btn-bg transition-colors"
			onclick={() => appearanceStore.cycleTheme()}
			aria-label="Switch background, currently {themeLabel}"
		>
			<Palette size={14} class="text-primary" />
			<span class="hidden sm:inline font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
				{themeLabel}
			</span>
		</button>
	</div>
</header>
