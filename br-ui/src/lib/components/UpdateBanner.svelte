<script lang="ts">
	import { AlertTriangle, Download, X } from 'lucide-svelte';
	import { versionStore } from '$lib/stores/version.svelte';

	const RELEASES_URL = 'https://github.com/TeamBattles/battles-record/releases/latest';
</script>

{#if versionStore.activeBanner === 'incompatible'}
	<div
		class="bg-red-500/10 border-b border-red-500/30 px-4 py-2 flex items-center justify-between gap-4"
	>
		<div class="flex items-center gap-2">
			<AlertTriangle size={14} class="text-red-400 flex-shrink-0" />
			<p class="font-mono text-xs text-red-400">
				{versionStore.incompatibleReason}
			</p>
		</div>
		<a
			href={RELEASES_URL}
			target="_blank"
			rel="noopener noreferrer"
			class="flex items-center gap-1.5 px-2 py-1 rounded bg-red-500/20 hover:bg-red-500/30 font-mono text-xs text-red-400 transition-colors whitespace-nowrap"
		>
			<Download size={12} />
			Download Update
		</a>
	</div>
{:else if versionStore.activeBanner === 'daemon-update'}
	<div
		class="bg-blue-500/10 border-b border-blue-500/30 px-4 py-2 flex items-center justify-between gap-4"
	>
		<div class="flex items-center gap-2">
			<Download size={14} class="text-blue-400 flex-shrink-0" />
			<p class="font-mono text-xs text-blue-400">
				Server update available: v{versionStore.daemonLatestVersion}
			</p>
		</div>
		<div class="flex items-center gap-2">
			<a
				href={versionStore.daemonReleaseUrl ?? RELEASES_URL}
				target="_blank"
				rel="noopener noreferrer"
				class="flex items-center gap-1.5 px-2 py-1 rounded bg-blue-500/20 hover:bg-blue-500/30 font-mono text-xs text-blue-400 transition-colors whitespace-nowrap"
			>
				View Release
			</a>
			<button
				class="p-1 rounded hover:bg-blue-500/20 text-blue-400 transition-colors"
				onclick={() => versionStore.dismissBanner(`daemon-${versionStore.daemonLatestVersion}`)}
			>
				<X size={12} />
			</button>
		</div>
	</div>
{:else if versionStore.activeBanner === 'ui-update'}
	<div
		class="bg-emerald-500/10 border-b border-emerald-500/30 px-4 py-2 flex items-center justify-between gap-4"
	>
		<div class="flex items-center gap-2">
			<Download size={14} class="text-emerald-400 flex-shrink-0" />
			<p class="font-mono text-xs text-emerald-400">
				App update available: v{versionStore.latestUIVersion}
			</p>
		</div>
		<div class="flex items-center gap-2">
			<a
				href={versionStore.uiReleaseUrl ?? RELEASES_URL}
				target="_blank"
				rel="noopener noreferrer"
				class="flex items-center gap-1.5 px-2 py-1 rounded bg-emerald-500/20 hover:bg-emerald-500/30 font-mono text-xs text-emerald-400 transition-colors whitespace-nowrap"
			>
				Download
			</a>
			<button
				class="p-1 rounded hover:bg-emerald-500/20 text-emerald-400 transition-colors"
				onclick={() => versionStore.dismissBanner(`ui-${versionStore.latestUIVersion}`)}
			>
				<X size={12} />
			</button>
		</div>
	</div>
{/if}
