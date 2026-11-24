<script lang="ts">
	import { ResponsiveModal, connectionStore } from '#lib';
	import { KeyRound, LogOut, AlertTriangle } from '@lucide/svelte';

	let username = $state('');
	let password = $state('');
	let isAuthenticating = $state(false);

	// Pre-fill username from stored value when modal opens
	$effect(() => {
		if (connectionStore.showSessionExpiredModal && connectionStore.username) {
			username = connectionStore.username;
		}
	});

	// Reset form when modal closes
	$effect(() => {
		if (!connectionStore.showSessionExpiredModal) {
			password = '';
			isAuthenticating = false;
		}
	});

	const canSubmit = $derived(username.trim().length > 0 && password.trim().length > 0);

	async function handleSubmit(e: Event) {
		e.preventDefault();
		if (!canSubmit || isAuthenticating) return;

		isAuthenticating = true;
		const success = await connectionStore.reauthenticate(username, password);
		if (!success) {
			// Keep authenticating state for error display
			isAuthenticating = false;
		}
	}

	function handleOpenChange(open: boolean) {
		if (!open) {
			connectionStore.dismissSessionExpiredModal();
		}
	}
</script>

<ResponsiveModal
	open={connectionStore.showSessionExpiredModal}
	onOpenChange={handleOpenChange}
	title="Session Expired"
>
	<!-- Warning Banner -->
	<div
		class="mb-4 flex items-start gap-3 rounded border border-orange/30 bg-orange/10 p-3"
	>
		<AlertTriangle size={20} class="text-orange flex-shrink-0 mt-0.5" />
		<div>
			<p class="font-mono text-sm text-orange">Your session has expired</p>
			<p class="font-mono text-xs text-muted-foreground mt-1">
				Please sign in again to continue using {connectionStore.activeServer?.name ?? 'the server'}.
			</p>
		</div>
	</div>

	<!-- Error Display -->
	{#if connectionStore.error}
		<div
			class="mb-4 rounded border border-destructive/30 bg-destructive/10 px-3 py-2 font-mono text-xs text-destructive"
		>
			{connectionStore.error}
		</div>
	{/if}

	<!-- Login Form -->
	<form onsubmit={handleSubmit} class="space-y-4">
		<div>
			<label for="session-username" class="font-mono text-xs uppercase tracking-wider text-muted-foreground mb-1.5 block">
				Username
			</label>
			<input
				id="session-username"
				type="text"
				bind:value={username}
				disabled={isAuthenticating}
				autocomplete="username"
				class="w-full rounded border border-line-2 bg-chip px-3 py-2 font-mono text-sm text-foreground placeholder:text-dim disabled:opacity-50 focus:border-primary focus:outline-none focus:ring-1 focus:ring-ring"
				placeholder="Enter username"
			/>
		</div>

		<div>
			<label for="session-password" class="font-mono text-xs uppercase tracking-wider text-muted-foreground mb-1.5 block">
				Password
			</label>
			<input
				id="session-password"
				type="password"
				bind:value={password}
				disabled={isAuthenticating}
				autocomplete="current-password"
				class="w-full rounded border border-line-2 bg-chip px-3 py-2 font-mono text-sm text-foreground placeholder:text-dim disabled:opacity-50 focus:border-primary focus:outline-none focus:ring-1 focus:ring-ring"
				placeholder="Enter password"
			/>
		</div>

		<!-- Actions -->
		<div class="flex gap-3 pt-2">
			<button
				type="submit"
				disabled={!canSubmit || isAuthenticating}
				class="flex-1 flex items-center justify-center gap-2 rounded bg-primary px-4 py-2.5 font-mono text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
			>
				<KeyRound size={16} />
				{isAuthenticating ? 'Signing in...' : 'Sign In'}
			</button>
			<button
				type="button"
				onclick={() => connectionStore.dismissSessionExpiredModal()}
				disabled={isAuthenticating}
				class="flex items-center justify-center gap-2 rounded border border-line-3 bg-btn-bg px-4 py-2.5 font-mono text-sm text-text-2 hover:bg-btn-bg-hover disabled:opacity-50 transition-colors"
			>
				<LogOut size={16} />
				Disconnect
			</button>
		</div>
	</form>

	{#snippet footer()}
		<p class="font-mono text-xs text-muted-foreground text-center">
			Having trouble? Check your credentials or contact your server administrator.
		</p>
	{/snippet}
</ResponsiveModal>
