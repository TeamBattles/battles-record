<script lang="ts">
	import { AlertTriangle, RefreshCw, Loader2 } from 'lucide-svelte';
	import { connectionStore } from '$lib';

	let isRetrying = $state(false);

	async function handleRetry() {
		isRetrying = true;
		connectionStore.retryNow();
		// Brief delay for visual feedback
		setTimeout(() => {
			isRetrying = false;
		}, 1000);
	}
</script>

{#if connectionStore.shouldShowReconnectBanner}
	<div
		class="bg-amber-500/10 border-b border-amber-500/30 px-4 py-2 flex items-center justify-between gap-4"
	>
		<div class="flex items-center gap-2">
			<AlertTriangle size={14} class="text-amber-400 flex-shrink-0" />
			<p class="font-mono text-xs text-amber-400">
				Connection to "{connectionStore.activeServer?.name}" lost. Reconnecting...
			</p>
		</div>
		<button
			class="flex items-center gap-1.5 px-2 py-1 rounded bg-amber-500/20 hover:bg-amber-500/30 font-mono text-xs text-amber-400 transition-colors disabled:opacity-50"
			onclick={handleRetry}
			disabled={isRetrying}
		>
			{#if isRetrying}
				<Loader2 size={12} class="animate-spin" />
			{:else}
				<RefreshCw size={12} />
			{/if}
			Retry Now
		</button>
	</div>
{/if}
