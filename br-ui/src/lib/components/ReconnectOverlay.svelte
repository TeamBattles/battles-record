<script lang="ts">
	import { WifiOff, RefreshCw, ArrowLeftRight } from '@lucide/svelte';
	import { connectionStore } from '#lib';

	interface Props {
		onSwitchServer: () => void;
		onReconnect: () => void;
		showAuthForm?: boolean;
	}

	let { onSwitchServer, onReconnect, showAuthForm = false }: Props = $props();

	let username = $state('');
	let password = $state('');
	let isAuthenticating = $state(false);

	const canSubmitAuth = $derived(username.trim().length > 0 && password.trim().length > 0);

	async function handleReauthenticate() {
		if (!canSubmitAuth || !connectionStore.activeServerId) return;

		isAuthenticating = true;
		const success = await connectionStore.authenticateRemote(
			connectionStore.activeServerId,
			username,
			password
		);

		if (success) {
			username = '';
			password = '';
		}
		isAuthenticating = false;
	}
</script>

<div
	class="absolute inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center z-30 p-4"
>
	<div class="max-w-md w-full text-center">
		<div class="p-4 rounded-full bg-chip inline-block mb-4">
			<WifiOff size={48} class="text-muted-foreground" />
		</div>

		<h2 class="font-display text-2xl uppercase tracking-tight mb-2">
			Connection to "{connectionStore.activeServer?.name}" lost
		</h2>

		<ul class="font-mono text-xs text-muted-foreground space-y-1 mb-6">
			<li>Server may be offline</li>
			<li>Network connection interrupted</li>
			<li>Session expired</li>
		</ul>

		{#if showAuthForm}
			<!-- Re-authentication form for expired tokens -->
			<div class="bg-well rounded border border-line-2 p-4 mb-4 text-left">
				<p class="font-mono text-xs text-muted-foreground mb-4">
					Session expired. Please re-enter your credentials:
				</p>

				<div class="space-y-3">
					<input
						type="text"
						placeholder="Username"
						bind:value={username}
						disabled={isAuthenticating}
						class="w-full rounded border border-line-2 bg-chip px-3 py-2 font-mono text-sm text-foreground placeholder:text-dim disabled:opacity-50"
					/>
					<input
						type="password"
						placeholder="Password"
						bind:value={password}
						disabled={isAuthenticating}
						class="w-full rounded border border-line-2 bg-chip px-3 py-2 font-mono text-sm text-foreground placeholder:text-dim disabled:opacity-50"
					/>
				</div>
			</div>

			<div class="flex gap-3 justify-center">
				<button
					class="flex items-center gap-2 rounded bg-primary px-4 py-2 font-mono text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50 transition-colors"
					disabled={!canSubmitAuth || isAuthenticating}
					onclick={handleReauthenticate}
				>
					<RefreshCw size={14} />
					Reconnect
				</button>
				<button
					class="flex items-center gap-2 rounded border border-line-3 bg-btn-bg px-4 py-2 font-mono text-sm text-text-2 hover:bg-btn-bg-hover transition-colors"
					onclick={onSwitchServer}
				>
					<ArrowLeftRight size={14} />
					Switch Server
				</button>
			</div>
		{:else}
			<div class="flex gap-3 justify-center">
				<button
					class="flex items-center gap-2 rounded bg-primary px-4 py-2 font-mono text-sm text-primary-foreground hover:bg-primary/90 transition-colors"
					onclick={onReconnect}
				>
					<RefreshCw size={14} />
					Reconnect
				</button>
				<button
					class="flex items-center gap-2 rounded border border-line-3 bg-btn-bg px-4 py-2 font-mono text-sm text-text-2 hover:bg-btn-bg-hover transition-colors"
					onclick={onSwitchServer}
				>
					<ArrowLeftRight size={14} />
					Switch Server
				</button>
			</div>
		{/if}

		{#if connectionStore.error}
			<p class="font-mono text-xs text-destructive mt-4">{connectionStore.error}</p>
		{/if}
	</div>
</div>
