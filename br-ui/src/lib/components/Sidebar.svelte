<script lang="ts">
	import { page } from '$app/stores';
	import { sidebarStore, breakpointStore } from '$lib';
	import {
		LayoutDashboard,
		Tv,
		Video,
		Calendar,
		Activity,
		HardDrive,
		Key,
		Users,
		Settings,
		X
	} from 'lucide-svelte';

	const mainNav = [
		{ href: '/', icon: LayoutDashboard, label: 'Dashboard' },
		{ href: '/channels', icon: Tv, label: 'Channels' },
		{ href: '/recordings', icon: Video, label: 'Recordings' },
		{ href: '/schedules', icon: Calendar, label: 'Schedules' },
		{ href: '/activity', icon: Activity, label: 'Activity' },
		{ href: '/storage', icon: HardDrive, label: 'Storage' }
	];

	const adminNav = [
		{ href: '/auth', icon: Key, label: 'Auth' },
		{ href: '/users', icon: Users, label: 'Users' },
		{ href: '/settings', icon: Settings, label: 'Settings' }
	];

	function isActive(href: string): boolean {
		if (href === '/') return $page.url.pathname === '/';
		return $page.url.pathname.startsWith(href);
	}

	function handleNavClick() {
		if (breakpointStore.isMobile) {
			sidebarStore.close();
		}
	}
</script>

<!-- Mobile Overlay Backdrop -->
{#if breakpointStore.isMobile && sidebarStore.isOpen}
	<button
		class="fixed inset-0 bg-black/50 z-40"
		onclick={() => sidebarStore.close()}
		aria-label="Close sidebar"
	></button>
{/if}

<!-- Sidebar -->
<aside
	class="bg-card border-r border-border flex flex-col transition-all duration-200
		{sidebarStore.state === 'hidden' && !sidebarStore.isOpen ? 'hidden' : ''}
		{sidebarStore.state === 'hidden' && sidebarStore.isOpen
		? 'fixed left-0 top-0 bottom-0 w-48 z-50'
		: ''}
		{sidebarStore.state === 'collapsed' ? 'w-14' : ''}
		{sidebarStore.state === 'expanded' ? 'w-48' : ''}"
>
	<!-- Mobile Close Button -->
	{#if breakpointStore.isMobile && sidebarStore.isOpen}
		<div class="flex justify-end p-2 border-b border-border">
			<button
				class="p-2 hover:bg-muted rounded transition-colors"
				onclick={() => sidebarStore.close()}
				aria-label="Close sidebar"
			>
				<X size={18} class="text-zinc-500" />
			</button>
		</div>
	{/if}

	<!-- Navigation -->
	<nav class="flex-1 py-2">
		{#each mainNav as item (item.href)}
			{@const active = isActive(item.href)}
			{#if sidebarStore.state === 'collapsed'}
				<a
					href={item.href}
					onclick={handleNavClick}
					class="flex items-center justify-center py-3 mx-1 my-0.5 rounded relative group transition-colors
						{active ? 'bg-muted text-foreground' : 'text-zinc-500 hover:text-foreground hover:bg-muted'}"
					title={item.label}
				>
					<item.icon size={18} />
					<!-- Tooltip -->
					<span
						class="absolute left-full ml-2 px-2 py-1 rounded bg-card border border-border font-mono text-xs whitespace-nowrap opacity-0 group-hover:opacity-100 pointer-events-none z-50 shadow-lg"
					>
						{item.label}
					</span>
				</a>
			{:else}
				<a
					href={item.href}
					onclick={handleNavClick}
					class="flex items-center gap-3 mx-2 my-0.5 px-3 py-2 rounded transition-colors
						{active ? 'bg-muted text-foreground' : 'text-zinc-500 hover:text-foreground hover:bg-muted'}"
				>
					<item.icon size={18} />
					<span class="font-mono text-xs">{item.label}</span>
				</a>
			{/if}
		{/each}

		<div class="border-t border-border mx-2 my-2"></div>

		{#each adminNav as item (item.href)}
			{@const active = isActive(item.href)}
			{#if sidebarStore.state === 'collapsed'}
				<a
					href={item.href}
					onclick={handleNavClick}
					class="flex items-center justify-center py-3 mx-1 my-0.5 rounded relative group transition-colors
						{active ? 'bg-muted text-foreground' : 'text-zinc-500 hover:text-foreground hover:bg-muted'}"
					title={item.label}
				>
					<item.icon size={18} />
					<!-- Tooltip -->
					<span
						class="absolute left-full ml-2 px-2 py-1 rounded bg-card border border-border font-mono text-xs whitespace-nowrap opacity-0 group-hover:opacity-100 pointer-events-none z-50 shadow-lg"
					>
						{item.label}
					</span>
				</a>
			{:else}
				<a
					href={item.href}
					onclick={handleNavClick}
					class="flex items-center gap-3 mx-2 my-0.5 px-3 py-2 rounded transition-colors
						{active ? 'bg-muted text-foreground' : 'text-zinc-500 hover:text-foreground hover:bg-muted'}"
				>
					<item.icon size={18} />
					<span class="font-mono text-xs">{item.label}</span>
				</a>
			{/if}
		{/each}
	</nav>
</aside>
