<script lang="ts">
	import { ResponsivePanel } from '$lib';
	import { userSessionsStore } from '$lib/stores/users.svelte';
	import type { User } from '$lib/api/types';
	import { X, Monitor, Smartphone, Globe, Clock, Trash2, XCircle } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import CornerBrackets from './ui/CornerBrackets.svelte';

	interface Props {
		user: User;
		onclose: () => void;
	}

	let { user, onclose }: Props = $props();

	onMount(() => {
		userSessionsStore.load(user.id);
		return () => userSessionsStore.clear();
	});

	function formatTimeAgo(dateStr: string): string {
		const date = new Date(dateStr);
		const now = new Date();
		const diffMs = now.getTime() - date.getTime();
		const diffMins = Math.floor(diffMs / (1000 * 60));
		const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
		const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

		if (diffMins < 1) return 'Just now';
		if (diffMins < 60) return `${diffMins} min ago`;
		if (diffHours < 24) return `${diffHours} hour${diffHours !== 1 ? 's' : ''} ago`;
		return `${diffDays} day${diffDays !== 1 ? 's' : ''} ago`;
	}

	function parseUserAgent(ua?: string): { icon: typeof Monitor; label: string } {
		if (!ua) return { icon: Globe, label: 'Unknown device' };

		// Very simple parsing - in production you'd use a proper UA parser
		const isMobile = /mobile|android|iphone|ipad/i.test(ua);
		if (isMobile) {
			return { icon: Smartphone, label: 'Mobile device' };
		}

		if (/chrome/i.test(ua)) return { icon: Monitor, label: 'Chrome' };
		if (/firefox/i.test(ua)) return { icon: Monitor, label: 'Firefox' };
		if (/safari/i.test(ua)) return { icon: Monitor, label: 'Safari' };
		if (/edge/i.test(ua)) return { icon: Monitor, label: 'Edge' };

		return { icon: Monitor, label: 'Desktop' };
	}

	async function handleRevokeSession(sessionId: string) {
		await userSessionsStore.revokeSession(sessionId);
	}

	async function handleRevokeAll() {
		await userSessionsStore.revokeAllSessions();
	}
</script>

<ResponsivePanel open={true} onClose={onclose}>
	<!-- Header -->
	<div
		class="flex items-center justify-between px-4 py-3 border-b border-zinc-700/60 bg-zinc-800/50 flex-shrink-0"
	>
		<div class="flex items-center gap-2">
			<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">
				Sessions: {user.username}
			</span>
			{#if userSessionsStore.sessions.length > 0}
				<span class="rounded bg-zinc-700 px-1.5 py-0.5 font-mono text-[10px] text-zinc-400">
					{userSessionsStore.sessions.length}
				</span>
			{/if}
		</div>
		<button class="p-1 hover:bg-zinc-700 rounded transition-colors" onclick={onclose}>
			<X size={18} class="text-zinc-500" />
		</button>
	</div>

	<!-- Content -->
	<div class="flex-1 overflow-y-auto p-4 min-h-0">
		{#if userSessionsStore.isLoading}
			<div class="flex items-center justify-center py-8">
				<div
					class="size-5 animate-spin rounded-full border-2 border-zinc-500 border-t-transparent"
				></div>
			</div>
		{:else if userSessionsStore.error}
			<div class="rounded border border-red-500/30 bg-red-500/5 p-4">
				<p class="font-mono text-xs text-red-400">{userSessionsStore.error}</p>
			</div>
		{:else if userSessionsStore.sessions.length === 0}
			<div class="flex flex-col items-center justify-center py-8 text-zinc-500">
				<Globe class="size-8 opacity-30 mb-2" />
				<p class="font-mono text-xs">No active sessions</p>
			</div>
		{:else}
			<div class="space-y-3">
				{#each userSessionsStore.sessions as session (session.id)}
					{@const device = parseUserAgent(session.user_agent)}
					{@const DeviceIcon = device.icon}
					<div class="relative border border-border bg-card p-4">
						<CornerBrackets size="sm" />

						<div class="flex items-start justify-between gap-3">
							<div class="flex-1 min-w-0">
								<!-- IP Address -->
								<div class="flex items-center gap-2 mb-2">
									<Globe size={14} class="text-zinc-500 flex-shrink-0" />
									<span class="font-mono text-sm text-zinc-200">
										{session.ip_address || 'Unknown IP'}
									</span>
								</div>

								<!-- Device -->
								<div class="flex items-center gap-2 mb-2">
									<DeviceIcon size={14} class="text-zinc-500 flex-shrink-0" />
									<span class="font-mono text-xs text-zinc-400">
										{device.label}
									</span>
								</div>

								<!-- Last Active -->
								<div class="flex items-center gap-2">
									<Clock size={14} class="text-zinc-500 flex-shrink-0" />
									<span class="font-mono text-xs text-zinc-500">
										Active: {formatTimeAgo(session.last_active)}
									</span>
								</div>
							</div>

							<!-- Revoke Button -->
							<button
								class="p-2 rounded hover:bg-red-500/10 transition-colors group"
								title="Revoke session"
								onclick={() => handleRevokeSession(session.id)}
							>
								<Trash2
									size={14}
									class="text-zinc-500 group-hover:text-red-400 transition-colors"
								/>
							</button>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>

	<!-- Footer -->
	{#if userSessionsStore.sessions.length > 0}
		<div class="border-t border-zinc-700 p-4 bg-zinc-900 flex-shrink-0">
			<button
				class="w-full flex items-center justify-center gap-2 rounded border border-red-500/30 bg-red-500/5 px-4 py-2 font-mono text-sm text-red-400 hover:bg-red-500/10 transition-colors"
				onclick={handleRevokeAll}
			>
				<XCircle size={14} />
				Revoke All Sessions
			</button>
		</div>
	{/if}
</ResponsivePanel>
