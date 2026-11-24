<script lang="ts">
	import { Monitor, Cloud, AlertTriangle, Check, Loader2, ArrowLeft, Plus } from '@lucide/svelte';
	import { settingsStore, connectionStore, autofocus } from '#lib';
	import type { SavedServer } from '#lib';
	import HexMesh from './ui/HexMesh.svelte';
	import LuxuryCornerSlivers from './ui/LuxuryCornerSlivers.svelte';
	import Checkbox from './ui/Checkbox.svelte';

	interface Props {
		onComplete: () => void;
		showCancel?: boolean;
		onCancel?: () => void;
	}

	let { onComplete, showCancel = false, onCancel }: Props = $props();

	type Step =
		| 'choose'
		| 'local-connecting'
		| 'remote-select'
		| 'remote-form'
		| 'remote-connecting'
		| 'success'
		| 'error';

	let step = $state<Step>('choose');
	let rememberChoice = $state(true);
	let errorMessage = $state('');
	let errorSource = $state<'local' | 'remote'>('local'); // Track what caused the error

	// Remote form fields
	let serverName = $state('');
	let serverUrl = $state('');
	let username = $state('');
	let password = $state('');

	// State for editing existing vs creating new
	let selectedServerId = $state<string | null>(null); // null = new server, string = editing existing

	// Duplicate detection
	let duplicateServer = $state<SavedServer | null>(null);
	let showDuplicateWarning = $state(false);

	// Health check state for remote-select step
	let serverHealth = $state<Record<string, 'unknown' | 'checking' | 'healthy' | 'unreachable'>>({});

	// Derived: existing saved remote servers
	const savedRemoteServers = $derived(
		settingsStore.settings.servers.filter((s) => s.type === 'remote')
	);

	const canSubmitRemote = $derived(
		serverUrl.trim().length > 0 && username.trim().length > 0 && password.trim().length > 0
	);

	// Health check functions
	async function checkServerHealth(server: SavedServer): Promise<boolean> {
		try {
			const response = await fetch(`${server.url}/health`, {
				method: 'GET',
				signal: AbortSignal.timeout(3000)
			});
			return response.ok;
		} catch {
			return false;
		}
	}

	async function checkAllServersHealth() {
		for (const server of savedRemoteServers) {
			serverHealth[server.id] = 'checking';
			const isHealthy = await checkServerHealth(server);
			serverHealth[server.id] = isHealthy ? 'healthy' : 'unreachable';
		}
	}

	function getServerHealthColor(serverId: string): string {
		const health = serverHealth[serverId];
		switch (health) {
			case 'healthy':
				return 'bg-green';
			case 'unreachable':
				return 'bg-destructive';
			case 'checking':
				return 'bg-yellow';
			default:
				return 'bg-faint';
		}
	}

	function isServerChecking(serverId: string): boolean {
		return serverHealth[serverId] === 'checking';
	}

	async function selectLocal() {
		step = 'local-connecting';
		errorSource = 'local';

		const success = await connectionStore.connectToLocal();

		if (success) {
			if (rememberChoice) {
				settingsStore.setStartupServer('local');
			}
			step = 'success';
			setTimeout(onComplete, 1000);
		} else {
			errorMessage = connectionStore.error ?? 'Failed to start local service';
			step = 'error';
		}
	}

	function selectRemote() {
		if (savedRemoteServers.length > 0) {
			step = 'remote-select';
			// Start health checks
			checkAllServersHealth();
		} else {
			// No existing servers, go directly to form
			resetFormFields();
			step = 'remote-form';
		}
	}

	function resetFormFields() {
		selectedServerId = null;
		serverName = '';
		serverUrl = '';
		username = '';
		password = '';
	}

	function goToNewServerForm() {
		resetFormFields();
		step = 'remote-form';
	}

	async function connectToExistingServer(server: SavedServer) {
		step = 'remote-connecting';

		// If token exists and not expired, try direct connect
		if (server.token && server.tokenExpiry && Date.now() < server.tokenExpiry) {
			const success = await connectionStore.connectToServer(server.id);
			if (success) {
				if (rememberChoice) {
					settingsStore.setStartupServer(server.id);
				}
				step = 'success';
				setTimeout(onComplete, 1000);
				return;
			}
		}

		// Token missing/expired - need re-authentication
		// Pre-fill form with server details and go to form step
		selectedServerId = server.id;
		serverName = server.name;
		serverUrl = server.url;
		username = server.username ?? '';
		password = '';
		step = 'remote-form';
	}

	async function submitRemote() {
		// Normalize URL
		let url = serverUrl.trim();
		if (!url.startsWith('http://') && !url.startsWith('https://')) {
			url = `http://${url}`;
		}

		// Check for duplicate host (only if adding new, not editing existing)
		if (!selectedServerId) {
			try {
				const urlObj = new URL(url);
				const existingWithSameHost = savedRemoteServers.find((s) => {
					try {
						const existingUrl = new URL(s.url);
						return existingUrl.host === urlObj.host;
					} catch {
						return false;
					}
				});

				if (existingWithSameHost) {
					duplicateServer = existingWithSameHost;
					showDuplicateWarning = true;
					return;
				}
			} catch {
				// Invalid URL, let the connection fail naturally
			}
		}

		step = 'remote-connecting';

		let serverId: string;
		const name = serverName.trim() || new URL(url).hostname;

		if (selectedServerId) {
			// Editing existing server - update URL/name if changed
			serverId = selectedServerId;
			settingsStore.updateServer(serverId, { name, url });
		} else {
			// Create new server entry
			serverId = crypto.randomUUID();
			settingsStore.addServer({
				id: serverId,
				name,
				type: 'remote',
				url
			});
		}

		const success = await connectionStore.authenticateRemote(serverId, username, password);

		if (success) {
			if (rememberChoice) {
				settingsStore.setStartupServer(serverId);
			}
			step = 'success';
			setTimeout(onComplete, 1000);
		} else {
			// Only remove the server if we just created it (not editing existing)
			if (!selectedServerId) {
				settingsStore.removeServer(serverId);
			}
			errorMessage = connectionStore.error ?? 'Authentication failed';
			errorSource = 'remote';
			step = 'error';
		}
	}

	function handleEditExisting() {
		if (!duplicateServer) return;
		// Pre-fill form with existing server details
		selectedServerId = duplicateServer.id;
		serverName = duplicateServer.name;
		serverUrl = duplicateServer.url;
		username = duplicateServer.username ?? '';
		password = '';
		duplicateServer = null;
		showDuplicateWarning = false;
		step = 'remote-form';
	}

	function cancelDuplicateWarning() {
		duplicateServer = null;
		showDuplicateWarning = false;
	}

	function retryConnection() {
		errorMessage = '';
		if (errorSource === 'remote') {
			// Retry the remote connection with the same form values
			submitRemote();
		} else {
			// Retry local connection
			selectLocal();
		}
	}

	function goBackToForm() {
		errorMessage = '';
		// Go back to the remote form step - form values are already preserved in state
		step = 'remote-form';
	}

	function goBack() {
		if (step === 'remote-form' && savedRemoteServers.length > 0) {
			step = 'remote-select';
		} else {
			step = 'choose';
		}
	}
</script>

<div class="fixed inset-0 bg-background flex justify-center overflow-y-auto p-4 z-50">
	<div class="pointer-events-none fixed inset-x-0 top-0 h-[60vh] overflow-hidden" aria-hidden="true">
		<HexMesh ripple />
	</div>
	<div class="relative z-10 w-full max-w-2xl my-auto">
		<!-- Header -->
		<div class="mb-8 flex flex-col gap-1.5">
			<span class="font-mono text-[11px] tracking-[0.18em] text-muted-foreground uppercase">
				Connect a server
			</span>
			<div class="flex items-center gap-4">
				<h1 class="font-display text-4xl leading-none tracking-wide uppercase text-foreground">
					Battles Record
				</h1>
				<span class="h-px flex-1 bg-gradient-to-r from-primary/50 to-transparent"></span>
			</div>
			<p class="text-sm text-muted-foreground">Choose how you want to connect</p>
		</div>

		{#if step === 'choose'}
			<!-- Connection Type Selection -->
			<div class="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
				<!-- Local Option -->
				<button
					class="group relative flex flex-col text-left p-6 rounded-lg border border-border bg-card hover:border-line-3 transition-colors"
					onclick={selectLocal}
				>
					<LuxuryCornerSlivers hoverFadeIn length="lg" />
					<div class="flex items-center gap-3 mb-4">
						<div class="p-2 rounded bg-chip">
							<Monitor size={24} class="text-green" />
						</div>
						<h2 class="font-display text-xl uppercase tracking-tight">Local Service</h2>
					</div>

					<div class="space-y-2 mb-4">
						<p class="font-mono text-xs text-green">+ No network setup required</p>
						<p class="font-mono text-xs text-green">+ Full control over your data</p>
						<p class="font-mono text-xs text-green">+ Works offline</p>
					</div>

					<div class="space-y-2 mb-4">
						<p class="font-mono text-xs text-muted-foreground">- Must keep app running for recordings</p>
						<p class="font-mono text-xs text-muted-foreground">- Only accessible from this device</p>
					</div>

					<div
						class="flex items-center gap-2 p-2 rounded bg-yellow/10 border border-yellow/30 self-start"
					>
						<AlertTriangle size={14} class="text-yellow flex-shrink-0" />
						<p class="font-mono text-[10px] text-yellow">Closing the app stops all recordings</p>
					</div>
				</button>

				<!-- Remote Option -->
				<button
					class="group relative flex flex-col text-left p-6 rounded-lg border border-border bg-card hover:border-line-3 transition-colors"
					onclick={selectRemote}
				>
					<LuxuryCornerSlivers hoverFadeIn length="lg" />
					<div class="flex items-center gap-3 mb-4">
						<div class="p-2 rounded bg-chip">
							<Cloud size={24} class="text-blue" />
						</div>
						<div class="flex items-center gap-2">
							<h2 class="font-display text-xl uppercase tracking-tight">Remote Server</h2>
							{#if savedRemoteServers.length > 0}
								<span
									class="px-1.5 py-0.5 rounded bg-blue/20 font-mono text-[10px] text-blue"
								>
									{savedRemoteServers.length} saved
								</span>
							{/if}
						</div>
					</div>

					<div class="space-y-2 mb-4">
						<p class="font-mono text-xs text-green">+ Runs independently of this app</p>
						<p class="font-mono text-xs text-green">+ Access from multiple devices</p>
						<p class="font-mono text-xs text-green">+ Recordings continue if app closes</p>
					</div>

					<div class="space-y-2">
						<p class="font-mono text-xs text-muted-foreground">- Requires server setup</p>
						<p class="font-mono text-xs text-muted-foreground">- Network configuration needed</p>
					</div>
				</button>
			</div>

			<!-- Remember Choice -->
			<div class="flex justify-center mb-4">
				<Checkbox
					bind:checked={rememberChoice}
					label="Use this choice for future launches"
					labelClass="text-muted-foreground"
				/>
			</div>

			{#if showCancel && onCancel}
				<div class="text-center">
					<button
						class="font-mono text-sm text-muted-foreground hover:text-foreground transition-colors"
						onclick={onCancel}
					>
						Cancel
					</button>
				</div>
			{/if}
		{:else if step === 'remote-select'}
			<!-- Remote Server Selection -->
			<div class="max-w-md mx-auto">
				<button
					class="flex items-center gap-2 font-mono text-sm text-muted-foreground hover:text-foreground mb-6 transition-colors"
					onclick={() => (step = 'choose')}
				>
					<ArrowLeft size={16} />
					Back
				</button>

				<div class="p-6 rounded border border-line-2 bg-well">
					<h2 class="font-mono text-xs uppercase tracking-wider text-muted-foreground mb-4">
						Saved Servers
					</h2>

					<div class="space-y-2 mb-4">
						{#each savedRemoteServers as server (server.id)}
							<button
								class="w-full flex items-center gap-3 p-3 rounded border border-line-2 bg-chip hover:border-line-3 transition-colors text-left"
								onclick={() => connectToExistingServer(server)}
							>
								<span
									class="size-2 rounded-full {getServerHealthColor(server.id)}"
									class:animate-pulse={isServerChecking(server.id)}
								></span>
								<div class="flex-1 min-w-0">
									<p class="font-mono text-sm text-foreground truncate">{server.name}</p>
									<p class="font-mono text-[10px] text-muted-foreground truncate">{server.url}</p>
								</div>
							</button>
						{/each}
					</div>

					<button
						class="w-full flex items-center justify-center gap-2 p-3 rounded border border-dashed border-line-3 hover:border-line-3 text-muted-foreground hover:text-foreground transition-colors"
						onclick={goToNewServerForm}
					>
						<Plus size={16} />
						<span class="font-mono text-sm">Add New Server</span>
					</button>

					<!-- Remember Choice -->
					<div class="mt-4 pt-4 border-t border-line">
						<Checkbox
							bind:checked={rememberChoice}
							label="Use this choice for future launches"
							labelClass="text-xs text-muted-foreground"
						/>
					</div>
				</div>
			</div>
		{:else if step === 'remote-form'}
			<!-- Remote Server Form -->
			<div class="max-w-md mx-auto">
				<button
					class="flex items-center gap-2 font-mono text-sm text-muted-foreground hover:text-foreground mb-6 transition-colors"
					onclick={goBack}
				>
					<ArrowLeft size={16} />
					Back
				</button>

				<form
					class="p-6 rounded border border-line-2 bg-well space-y-4"
					onsubmit={(e) => {
						e.preventDefault();
						if (canSubmitRemote) submitRemote();
					}}
				>
					{#if selectedServerId}
						<div
							class="flex items-center gap-2 p-2 rounded bg-blue/10 border border-blue/30"
						>
							<Cloud size={14} class="text-blue flex-shrink-0" />
							<p class="font-mono text-[10px] text-blue">
								Re-authenticating to existing server
							</p>
						</div>
					{/if}

					<div>
						<label for="wizard-server-name" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
							Server Name (optional)
						</label>
						<input
							id="wizard-server-name"
							type="text"
							placeholder="My Server"
							bind:value={serverName}
							class="w-full rounded border border-line-2 bg-chip px-3 py-2 font-mono text-sm text-foreground placeholder:text-dim"
						/>
					</div>

					<div>
						<label for="wizard-server-url" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
							Server URL
						</label>
						<input
							id="wizard-server-url"
							type="text"
							placeholder="http://192.168.1.100:8080"
							bind:value={serverUrl}
							use:autofocus
							disabled={!!selectedServerId}
							class="w-full rounded border border-line-2 bg-chip px-3 py-2 font-mono text-sm text-foreground placeholder:text-dim disabled:opacity-50 disabled:cursor-not-allowed"
						/>
						{#if selectedServerId}
							<p class="font-mono text-[10px] text-muted-foreground mt-1">
								URL cannot be changed when re-authenticating
							</p>
						{/if}
					</div>

					<div>
						<label for="wizard-username" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
							Username
						</label>
						<input
							id="wizard-username"
							type="text"
							bind:value={username}
							class="w-full rounded border border-line-2 bg-chip px-3 py-2 font-mono text-sm text-foreground"
						/>
					</div>

					<div>
						<label for="wizard-password" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
							Password
						</label>
						<input
							id="wizard-password"
							type="password"
							bind:value={password}
							class="w-full rounded border border-line-2 bg-chip px-3 py-2 font-mono text-sm text-foreground"
						/>
					</div>

					<!-- Remember Choice -->
					<div class="pt-2">
						<Checkbox
							bind:checked={rememberChoice}
							label="Use this choice for future launches"
							labelClass="text-muted-foreground"
						/>
					</div>

					<button
						type="submit"
						class="w-full rounded bg-primary px-4 py-2 font-mono text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
						disabled={!canSubmitRemote}
					>
						{selectedServerId ? 'Reconnect' : 'Connect'}
					</button>
				</form>
			</div>
		{:else if step === 'local-connecting' || step === 'remote-connecting'}
			<!-- Connecting State -->
			<div class="flex flex-col items-center justify-center py-12">
				<Loader2 size={48} class="text-green animate-spin mb-4" />
				<p class="font-mono text-sm text-muted-foreground">
					{step === 'local-connecting' ? 'Starting local service...' : 'Connecting to server...'}
				</p>
			</div>
		{:else if step === 'success'}
			<!-- Success State -->
			<div class="flex flex-col items-center justify-center py-12">
				<div class="p-4 rounded-full bg-green/20 mb-4">
					<Check size={48} class="text-green" />
				</div>
				<p class="font-mono text-sm text-muted-foreground">Connected successfully!</p>
			</div>
		{:else if step === 'error'}
			<!-- Error State -->
			<div class="max-w-md mx-auto text-center py-12">
				<div class="p-4 rounded-full bg-destructive/20 mb-4 inline-block">
					<AlertTriangle size={48} class="text-destructive" />
				</div>
				<p class="font-mono text-sm text-destructive mb-6">{errorMessage}</p>
				<div class="flex items-center justify-center gap-3">
					{#if errorSource === 'remote'}
						<button
							class="rounded border border-line-2 bg-btn-bg px-4 py-2 font-mono text-sm text-text-2 hover:bg-btn-bg-hover transition-colors"
							onclick={goBackToForm}
						>
							<span class="flex items-center gap-2">
								<ArrowLeft size={14} />
								Go Back
							</span>
						</button>
					{/if}
					<button
						class="rounded bg-primary px-6 py-2 font-mono text-sm text-primary-foreground hover:bg-primary/90 transition-colors"
						onclick={retryConnection}
					>
						Try Again
					</button>
				</div>
			</div>
		{/if}
	</div>
</div>

<!-- Duplicate Server Warning Modal -->
{#if showDuplicateWarning && duplicateServer}
	<div class="fixed inset-0 bg-black/70 flex items-center justify-center z-[60] p-4">
		<div class="animate-luxury-fade-scale bg-popover border border-border rounded-xl max-w-sm w-full p-6 shadow-2xl">
			<div class="flex items-center gap-3 mb-4">
				<div class="p-2 rounded-full bg-yellow/20">
					<AlertTriangle size={20} class="text-yellow" />
				</div>
				<h3 class="font-display text-lg uppercase tracking-tight">Server Already Exists</h3>
			</div>

			<p class="font-mono text-sm text-muted-foreground mb-6">
				A connection to this host already exists: <span class="text-foreground"
					>"{duplicateServer.name}"</span
				>
			</p>

			<p class="font-mono text-xs text-muted-foreground mb-6">
				Would you like to edit the existing connection instead?
			</p>

			<div class="flex gap-3">
				<button
					class="flex-1 rounded bg-primary px-4 py-2 font-mono text-sm text-primary-foreground hover:bg-primary/90 transition-colors"
					onclick={handleEditExisting}
				>
					Edit Existing
				</button>
				<button
					class="rounded border border-line-2 bg-btn-bg px-4 py-2 font-mono text-sm text-text-2 hover:bg-btn-bg-hover transition-colors"
					onclick={cancelDuplicateWarning}
				>
					Cancel
				</button>
			</div>
		</div>
	</div>
{/if}
