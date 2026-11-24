<script lang="ts">
	import { AlertTriangle, Download, X } from '@lucide/svelte';
	import { versionStore } from '#lib/stores/version.svelte.ts';

	const RELEASES_URL = 'https://github.com/TeamBattles/battles-record/releases/latest';
</script>

{#if versionStore.activeBanner === 'incompatible'}
	<div
		class="bg-destructive/10 border-b border-destructive/30 px-4 py-2 flex items-center justify-between gap-4"
	>
		<div class="flex items-center gap-2">
			<AlertTriangle size={14} class="text-destructive flex-shrink-0" />
			<p class="font-mono text-xs text-destructive">
				{versionStore.incompatibleReason}
			</p>
		</div>
		<a
			href={RELEASES_URL}
			target="_blank"
			rel="noopener noreferrer"
			class="flex items-center gap-1.5 px-2 py-1 rounded bg-destructive/20 hover:bg-destructive/30 font-mono text-xs text-destructive transition-colors whitespace-nowrap"
		>
			<Download size={12} />
			Download Update
		</a>
	</div>
{:else if versionStore.activeBanner === 'daemon-update'}
	<div
		class="bg-blue/10 border-b border-blue/30 px-4 py-2 flex items-center justify-between gap-4"
	>
		<div class="flex items-center gap-2">
			<Download size={14} class="text-blue flex-shrink-0" />
			<p class="font-mono text-xs text-blue">
				Server update available: v{versionStore.daemonLatestVersion}
			</p>
		</div>
		<div class="flex items-center gap-2">
			<a
				href={versionStore.daemonReleaseUrl ?? RELEASES_URL}
				target="_blank"
				rel="noopener noreferrer"
				class="flex items-center gap-1.5 px-2 py-1 rounded bg-blue/20 hover:bg-blue/30 font-mono text-xs text-blue transition-colors whitespace-nowrap"
			>
				View Release
			</a>
			<button
				class="p-1 rounded hover:bg-blue/20 text-blue transition-colors"
				onclick={() => versionStore.dismissBanner(`daemon-${versionStore.daemonLatestVersion}`)}
			>
				<X size={12} />
			</button>
		</div>
	</div>
{:else if versionStore.activeBanner === 'ui-update'}
	<div
		class="bg-green/10 border-b border-green/30 px-4 py-2 flex items-center justify-between gap-4"
	>
		<div class="flex items-center gap-2">
			<Download size={14} class="text-green flex-shrink-0" />
			<p class="font-mono text-xs text-green">
				App update available: v{versionStore.latestUIVersion}
			</p>
		</div>
		<div class="flex items-center gap-2">
			<a
				href={versionStore.uiReleaseUrl ?? RELEASES_URL}
				target="_blank"
				rel="noopener noreferrer"
				class="flex items-center gap-1.5 px-2 py-1 rounded bg-green/20 hover:bg-green/30 font-mono text-xs text-green transition-colors whitespace-nowrap"
			>
				Download
			</a>
			<button
				class="p-1 rounded hover:bg-green/20 text-green transition-colors"
				onclick={() => versionStore.dismissBanner(`ui-${versionStore.latestUIVersion}`)}
			>
				<X size={12} />
			</button>
		</div>
	</div>
{/if}
