<script lang="ts">
	import { ResponsiveModal, settingsStore, connectionStore } from '$lib';
	import { Loader2 } from 'lucide-svelte';

	interface Props {
		open: boolean;
		onOpenChange: (open: boolean) => void;
	}

	let { open, onOpenChange }: Props = $props();

	let serverName = $state('');
	let serverUrl = $state('');
	let username = $state('');
	let password = $state('');
	let isConnecting = $state(false);
	let error = $state('');
	let serverUrlInput = $state<HTMLInputElement | null>(null);
	const canSubmit = $derived(
		serverUrl.trim().length > 0 && username.trim().length > 0 && password.trim().length > 0
	);

	function reset() {
		serverName = '';
		serverUrl = '';
		username = '';
		password = '';
		isConnecting = false;
		error = '';
	}

	function handleOpenChange(newOpen: boolean) {
		if (!newOpen) {
			reset();
		}
		onOpenChange(newOpen);
	}

	async function handleSubmit() {
		if (!canSubmit || isConnecting) return;

		isConnecting = true;
		error = '';

		// Normalize URL
		let url = serverUrl.trim();
		if (!url.startsWith('http://') && !url.startsWith('https://')) {
			url = `http://${url}`;
		}

		// Create server entry
		const serverId = crypto.randomUUID();
		const name = serverName.trim() || new URL(url).hostname;

		settingsStore.addServer({
			id: serverId,
			name,
			type: 'remote',
			url
		});

		const success = await connectionStore.authenticateRemote(serverId, username, password);

		if (success) {
			reset();
			onOpenChange(false);
		} else {
			// Remove the server we just added since auth failed
			settingsStore.removeServer(serverId);
			error = connectionStore.error ?? 'Connection failed';
			isConnecting = false;
		}
	}
</script>

<ResponsiveModal
	{open}
	onOpenChange={handleOpenChange}
	title="Add Remote Server"
	initialFocusEl={serverUrlInput}
>
	{#snippet children()}
		<form
			id="add-server-form"
			class="space-y-4"
			onsubmit={(e) => {
				e.preventDefault();
				handleSubmit();
			}}
		>
			{#if error}
				<div class="rounded border border-red-500/30 bg-red-500/10 p-3">
					<p class="font-mono text-xs text-red-400">{error}</p>
				</div>
			{/if}

			<div>
				<label for="add-server-name" class="block font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-2">
					Server Name (optional)
				</label>
				<input
					id="add-server-name"
					type="text"
					placeholder="My Server"
					bind:value={serverName}
					disabled={isConnecting}
					class="w-full rounded border border-zinc-700 bg-zinc-800 px-3 py-2 font-mono text-sm text-zinc-100 placeholder:text-zinc-600 disabled:opacity-50"
				/>
			</div>

			<div>
				<label for="add-server-url" class="block font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-2">
					Server URL
				</label>
				<input
					id="add-server-url"
					type="text"
					placeholder="http://192.168.1.100:8080"
					bind:value={serverUrl}
					bind:this={serverUrlInput}
					disabled={isConnecting}
					class="w-full rounded border border-zinc-700 bg-zinc-800 px-3 py-2 font-mono text-sm text-zinc-100 placeholder:text-zinc-600 disabled:opacity-50"
				/>
			</div>

			<div>
				<label for="add-server-username" class="block font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-2">
					Username
				</label>
				<input
					id="add-server-username"
					type="text"
					bind:value={username}
					disabled={isConnecting}
					class="w-full rounded border border-zinc-700 bg-zinc-800 px-3 py-2 font-mono text-sm text-zinc-100 disabled:opacity-50"
				/>
			</div>

			<div>
				<label for="add-server-password" class="block font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-2">
					Password
				</label>
				<input
					id="add-server-password"
					type="password"
					bind:value={password}
					disabled={isConnecting}
					class="w-full rounded border border-zinc-700 bg-zinc-800 px-3 py-2 font-mono text-sm text-zinc-100 disabled:opacity-50"
				/>
			</div>
		</form>
	{/snippet}

	{#snippet footer()}
		<div class="flex gap-2">
			<button
				type="submit"
				form="add-server-form"
				class="flex-1 flex items-center justify-center gap-2 rounded bg-emerald-600 px-4 py-2 font-mono text-sm text-white hover:bg-emerald-500 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
				disabled={!canSubmit || isConnecting}
			>
				{#if isConnecting}
					<Loader2 size={14} class="animate-spin" />
					Connecting...
				{:else}
					Connect
				{/if}
			</button>
			<button
				type="button"
				class="rounded border border-zinc-700 bg-zinc-800 px-4 py-2 font-mono text-sm text-zinc-300 hover:bg-zinc-700 transition-colors"
				onclick={() => handleOpenChange(false)}
				disabled={isConnecting}
			>
				Cancel
			</button>
		</div>
	{/snippet}
</ResponsiveModal>
