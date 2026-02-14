<script lang="ts">
	import { WifiOff, RefreshCw, ArrowLeftRight } from 'lucide-svelte';
	import { connectionStore } from '$lib';

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
		<div class="p-4 rounded-full bg-zinc-800 inline-block mb-4">
			<WifiOff size={48} class="text-zinc-400" />
		</div>

		<h2 class="font-display text-2xl uppercase tracking-tight mb-2">
			Connection to "{connectionStore.activeServer?.name}" lost
		</h2>

		<ul class="font-mono text-xs text-zinc-500 space-y-1 mb-6">
			<li>Server may be offline</li>
			<li>Network connection interrupted</li>
			<li>Session expired</li>
		</ul>

		{#if showAuthForm}
			<!-- Re-authentication form for expired tokens -->
			<div class="bg-zinc-900 rounded border border-zinc-700 p-4 mb-4 text-left">
				<p class="font-mono text-xs text-zinc-400 mb-4">
					Session expired. Please re-enter your credentials:
				</p>

				<div class="space-y-3">
					<input
						type="text"
						placeholder="Username"
						bind:value={username}
						disabled={isAuthenticating}
						class="w-full rounded border border-zinc-700 bg-zinc-800 px-3 py-2 font-mono text-sm text-zinc-100 placeholder:text-zinc-600 disabled:opacity-50"
					/>
					<input
						type="password"
						placeholder="Password"
						bind:value={password}
						disabled={isAuthenticating}
						class="w-full rounded border border-zinc-700 bg-zinc-800 px-3 py-2 font-mono text-sm text-zinc-100 placeholder:text-zinc-600 disabled:opacity-50"
					/>
				</div>
			</div>

			<div class="flex gap-3 justify-center">
				<button
					class="flex items-center gap-2 rounded bg-emerald-600 px-4 py-2 font-mono text-sm text-white hover:bg-emerald-500 disabled:opacity-50 transition-colors"
					disabled={!canSubmitAuth || isAuthenticating}
					onclick={handleReauthenticate}
				>
					<RefreshCw size={14} />
					Reconnect
				</button>
				<button
					class="flex items-center gap-2 rounded border border-zinc-600 bg-zinc-800 px-4 py-2 font-mono text-sm text-zinc-300 hover:bg-zinc-700 transition-colors"
					onclick={onSwitchServer}
				>
					<ArrowLeftRight size={14} />
					Switch Server
				</button>
			</div>
		{:else}
			<div class="flex gap-3 justify-center">
				<button
					class="flex items-center gap-2 rounded bg-emerald-600 px-4 py-2 font-mono text-sm text-white hover:bg-emerald-500 transition-colors"
					onclick={onReconnect}
				>
					<RefreshCw size={14} />
					Reconnect
				</button>
				<button
					class="flex items-center gap-2 rounded border border-zinc-600 bg-zinc-800 px-4 py-2 font-mono text-sm text-zinc-300 hover:bg-zinc-700 transition-colors"
					onclick={onSwitchServer}
				>
					<ArrowLeftRight size={14} />
					Switch Server
				</button>
			</div>
		{/if}

		{#if connectionStore.error}
			<p class="font-mono text-xs text-red-400 mt-4">{connectionStore.error}</p>
		{/if}
	</div>
</div>
