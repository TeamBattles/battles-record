<script lang="ts">
	import { page } from '$app/state';
	import { sidebarStore, breakpointStore } from '#lib';
	import { cn } from '#lib/utils/cn.ts';
	import { Tooltip } from 'bits-ui';
	import {
		LayoutDashboard,
		Tv,
		Video,
		Download,
		Calendar,
		Activity,
		HardDrive,
		Key,
		Users,
		Settings,
		X
	} from '@lucide/svelte';

	const sections = [
		{
			label: 'Record',
			items: [
				{ href: '/', icon: LayoutDashboard, label: 'Dashboard' },
				{ href: '/channels', icon: Tv, label: 'Channels' },
				{ href: '/recordings', icon: Video, label: 'Recordings' },
				{ href: '/downloads', icon: Download, label: 'Downloads' },
				{ href: '/schedules', icon: Calendar, label: 'Schedules' },
				{ href: '/activity', icon: Activity, label: 'Activity' },
				{ href: '/storage', icon: HardDrive, label: 'Storage' }
			]
		},
		{
			label: 'Admin',
			items: [
				{ href: '/auth', icon: Key, label: 'Auth' },
				{ href: '/users', icon: Users, label: 'Users' },
				{ href: '/settings', icon: Settings, label: 'Settings' }
			]
		}
	];

	const collapsed = $derived(sidebarStore.state === 'collapsed');

	function isActive(href: string): boolean {
		if (href === '/') return page.url.pathname === '/';
		return page.url.pathname.startsWith(href);
	}

	function handleNavClick() {
		if (breakpointStore.isMobile) {
			sidebarStore.close();
		}
	}
</script>

<!--
	The icon-only rail labels its links with a portalled tooltip. An in-place absolute label would
	sit inside the nav's scroll box: overflow-y forces overflow-x to scroll too, so the label both
	added a horizontal scrollbar and got clipped when it appeared.
-->
{#snippet navItem(item: (typeof sections)[number]['items'][number])}
	{@const active = isActive(item.href)}
	<Tooltip.Root disabled={!collapsed}>
		<Tooltip.Trigger onclick={handleNavClick}>
			{#snippet child({ props })}
				<!-- The trigger adds type="button" for its default <button>; it means nothing on a link -->
				<a
					{...props}
					type={undefined}
					href={item.href}
					aria-current={active ? 'page' : undefined}
					aria-label={collapsed ? item.label : undefined}
					class={cn(
						'relative my-0.5 flex items-center rounded transition-colors',
						collapsed ? 'mx-1.5 justify-center py-2.5' : 'mx-2 gap-3 px-3 py-2',
						active
							? 'bg-hover-wash text-foreground'
							: 'text-muted-foreground hover:bg-hover-wash hover:text-foreground'
					)}
				>
					{#if active}
						<span
							class={cn(
								'absolute top-1.5 bottom-1.5 w-0.5 rounded-full bg-primary',
								collapsed ? '-left-1.5' : '-left-2'
							)}
							aria-hidden="true"
						></span>
					{/if}
					<item.icon size={18} class={active ? 'text-primary' : ''} />
					{#if !collapsed}
						<span class="font-mono text-xs">{item.label}</span>
					{/if}
				</a>
			{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Portal>
			<Tooltip.Content
				side="right"
				sideOffset={8}
				class="z-50 rounded border border-border bg-popover px-2 py-1 font-mono text-xs whitespace-nowrap text-popover-foreground shadow-lg"
			>
				{item.label}
			</Tooltip.Content>
		</Tooltip.Portal>
	</Tooltip.Root>
{/snippet}

<!-- Mobile Overlay Backdrop -->
{#if breakpointStore.isMobile && sidebarStore.isOpen}
	<button
		class="fixed inset-0 z-40 bg-black/50"
		onclick={() => sidebarStore.close()}
		aria-label="Close sidebar"
	></button>
{/if}

<aside
	class={cn(
		'flex flex-col border-r border-border bg-rail transition-all duration-200',
		sidebarStore.state === 'hidden' && !sidebarStore.isOpen && 'hidden',
		sidebarStore.state === 'hidden' &&
			sidebarStore.isOpen &&
			'fixed top-0 bottom-0 left-0 z-50 w-52',
		collapsed && 'w-14',
		sidebarStore.state === 'expanded' && 'w-52'
	)}
>
	{#if breakpointStore.isMobile && sidebarStore.isOpen}
		<div class="flex justify-end border-b border-border p-2">
			<button
				class="rounded p-2 transition-colors hover:bg-hover-wash"
				onclick={() => sidebarStore.close()}
				aria-label="Close sidebar"
			>
				<X size={18} class="text-muted-foreground" />
			</button>
		</div>
	{/if}

	<nav class="flex-1 overflow-x-hidden overflow-y-auto py-3">
		<Tooltip.Provider delayDuration={200}>
			{#each sections as section, index (section.label)}
				{#if index > 0}
					<div class="mx-3 my-3 border-t border-border"></div>
				{/if}
				{#if !collapsed}
					<p class="mb-1 px-5 font-mono text-[10px] tracking-[0.18em] text-dim uppercase">
						{section.label}
					</p>
				{/if}
				{#each section.items as item (item.href)}
					{@render navItem(item)}
				{/each}
			{/each}
		</Tooltip.Provider>
	</nav>
</aside>
