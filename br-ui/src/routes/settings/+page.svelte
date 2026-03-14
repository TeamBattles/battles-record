<script lang="ts">
	import {
		Server,
		Monitor,
		Cloud,
		Play,
		Square,
		RotateCcw,
		FileText,
		Pencil,
		RefreshCw,
		Trash2,
		Plus,
		Check,
		FolderOpen,
		HardDrive,
		Palette,
		Terminal
	} from 'lucide-svelte';
	import {
		settingsStore,
		connectionStore,
		ResponsiveModal,
		toastStore,
		type SavedServer
	} from '$lib';
	import CornerBrackets from '$lib/components/ui/CornerBrackets.svelte';
	import { api, type PostProcessingConfig } from '$lib/api';
	import { invoke } from '@tauri-apps/api/core';
	import { open } from '@tauri-apps/plugin-dialog';
	import { onMount } from 'svelte';
	import DaemonLogsModal from '$lib/components/DaemonLogsModal.svelte';

	// Local daemon paths
	interface LocalDaemonPaths {
		config_path: string | null;
		data_dir: string | null;
		library_dir: string | null;
	}
	let localPaths = $state<LocalDaemonPaths | null>(null);
	let isUpdatingPaths = $state(false);
	let isUpdatingLibraryDir = $state(false);

	onMount(async () => {
		try {
			localPaths = await invoke<LocalDaemonPaths>('get_local_daemon_paths');
		} catch (e) {
			console.error('Failed to get local daemon paths:', e);
		}
	});

	// Load config when connection is established (handles initial load and tab switches)
	$effect(() => {
		if (connectionStore.isConnected) {
			loadPostProcessingConfig();
		}
	});

	async function loadPostProcessingConfig() {
		try {
			const ppConfig = await api.getPostProcessingConfig();
			postProcessingEnabled = ppConfig.enabled;
			checkIntervalMinutes = ppConfig.check_interval_minutes;
			outputFormat = ppConfig.output_format;
			segmentHandling = ppConfig.segment_handling;
			crf = ppConfig.encoding.crf;
			preset = ppConfig.encoding.preset;
			videoCodec = ppConfig.encoding.video_codec;
			audioCodec = ppConfig.encoding.audio_codec;
			audioBitrate = ppConfig.encoding.audio_bitrate;
		} catch (e) {
			console.error('Failed to load post-processing config:', e);
		}
	}

	async function openInFolder(path: string) {
		try {
			await invoke('show_in_folder', { path });
		} catch (e) {
			console.error('Failed to open folder:', e);
		}
	}

	async function handleEditDataDir() {
		const result = await open({
			directory: true,
			title: 'Select Recordings Directory',
			defaultPath: localPaths?.data_dir ?? undefined
		});

		if (result) {
			await updatePaths(
				localPaths?.config_path ?? undefined,
				result as string,
				localPaths?.library_dir ?? undefined
			);
			// Persist to settings so it survives app restart
			settingsStore.setLocalDaemonDataDir(result as string);
		}
	}

	async function handleEditLibraryDir() {
		const result = await open({
			directory: true,
			title: 'Select Library Directory',
			defaultPath: localPaths?.library_dir ?? localPaths?.data_dir ?? undefined
		});

		if (result) {
			await updatePaths(
				localPaths?.config_path ?? undefined,
				localPaths?.data_dir ?? undefined,
				result as string,
				true
			);
			// Persist to settings so it survives app restart
			settingsStore.setLocalDaemonLibraryDir(result as string);
		}
	}

	async function updatePaths(
		configPath?: string,
		dataDir?: string,
		libraryDir?: string,
		isLibraryUpdate = false
	) {
		if (isLibraryUpdate) {
			isUpdatingLibraryDir = true;
		} else {
			isUpdatingPaths = true;
		}
		try {
			await invoke('set_local_daemon_paths', {
				configPath: configPath ?? null,
				dataDir: dataDir ?? null,
				libraryDir: libraryDir ?? null
			});
			// Refresh paths
			localPaths = await invoke<LocalDaemonPaths>('get_local_daemon_paths');
			toastStore.success('Paths updated, daemon restarted');
		} catch (e) {
			toastStore.error(`Failed to update paths: ${e}`);
		} finally {
			isUpdatingPaths = false;
			isUpdatingLibraryDir = false;
		}
	}

	// Edit server modal state
	let editModalOpen = $state(false);
	let editingServer = $state<SavedServer | null>(null);
	let editName = $state('');
	let editUrl = $state('');

	// Reconnect modal state (for re-authentication)
	let reconnectModalOpen = $state(false);
	let reconnectServer = $state<SavedServer | null>(null);
	let reconnectUsername = $state('');
	let reconnectPassword = $state('');
	let isReconnecting = $state(false);
	let reconnectError = $state('');

	// Add server modal state
	let addModalOpen = $state(false);
	let addName = $state('');
	let addUrl = $state('');
	let addUsername = $state('');
	let addPassword = $state('');
	let isAdding = $state(false);
	let addError = $state('');
	let addUrlInput = $state<HTMLInputElement | null>(null);

	// Delete confirmation
	let deleteServer = $state<SavedServer | null>(null);

	// Daemon control state
	let isDaemonRunning = $state(false);
	let isStarting = $state(false);
	let isRestarting = $state(false);
	let isStopping = $state(false);
	let logsModalOpen = $state(false);
	let daemonLogs = $state<string[]>([]);

	// Check daemon status on mount and periodically
	$effect(() => {
		checkDaemonStatus();
		const interval = setInterval(checkDaemonStatus, 5000);
		return () => clearInterval(interval);
	});

	async function checkDaemonStatus() {
		try {
			isDaemonRunning = await connectionStore.isDaemonRunning();
		} catch {
			isDaemonRunning = false;
		}
	}

	// Post-processing settings state
	let postProcessingEnabled = $state(true);
	let checkIntervalMinutes = $state(15);
	let outputFormat = $state('mp4_reencode');
	let segmentHandling = $state<'delete' | 'concatenate' | 'keep'>('delete');
	let crf = $state(20);
	let preset = $state('medium');
	let videoCodec = $state('libx264');
	let audioCodec = $state('aac');
	let audioBitrate = $state('128k');

	function openEditModal(server: SavedServer) {
		editingServer = server;
		editName = server.name;
		editUrl = server.url;
		editModalOpen = true;
	}

	function handleEditSave() {
		if (!editingServer) return;
		const urlChanged = editUrl !== editingServer.url;
		settingsStore.updateServer(editingServer.id, {
			name: editName,
			url: editUrl,
			// Clear token if URL changed (need to re-auth)
			...(urlChanged && editingServer.type === 'remote'
				? { token: undefined, tokenExpiry: undefined }
				: {})
		});
		editModalOpen = false;
		editingServer = null;
	}

	function openReconnectModal(server: SavedServer) {
		reconnectServer = server;
		reconnectUsername = '';
		reconnectPassword = '';
		reconnectError = '';
		reconnectModalOpen = true;
	}

	async function handleReconnect() {
		if (!reconnectServer || !reconnectUsername || !reconnectPassword) return;

		isReconnecting = true;
		reconnectError = '';

		const success = await connectionStore.authenticateRemote(
			reconnectServer.id,
			reconnectUsername,
			reconnectPassword
		);

		if (success) {
			reconnectModalOpen = false;
			reconnectServer = null;
		} else {
			reconnectError = connectionStore.error ?? 'Authentication failed';
		}
		isReconnecting = false;
	}

	function confirmDelete(server: SavedServer) {
		deleteServer = server;
	}

	async function handleDelete() {
		if (!deleteServer) return;
		const wasActive = connectionStore.activeServerId === deleteServer.id;
		if (wasActive) {
			connectionStore.disconnect();
		}
		settingsStore.removeServer(deleteServer.id);
		deleteServer = null;
		if (wasActive) {
			await connectionStore.connectToLocal();
		}
	}

	async function handleAddServer() {
		if (!addUrl || !addUsername || !addPassword) return;

		isAdding = true;
		addError = '';

		let url = addUrl.trim();
		if (!url.startsWith('http://') && !url.startsWith('https://')) {
			url = `http://${url}`;
		}

		const serverId = crypto.randomUUID();
		const name = addName.trim() || new URL(url).hostname;

		settingsStore.addServer({
			id: serverId,
			name,
			type: 'remote',
			url
		});

		const success = await connectionStore.authenticateRemote(serverId, addUsername, addPassword);

		if (success) {
			addModalOpen = false;
			addName = '';
			addUrl = '';
			addUsername = '';
			addPassword = '';
		} else {
			settingsStore.removeServer(serverId);
			addError = connectionStore.error ?? 'Connection failed';
		}
		isAdding = false;
	}

	function getTokenStatus(server: SavedServer): string {
		if (server.type !== 'remote') return '';
		if (!server.token) return 'No token';
		if (!server.tokenExpiry) return 'Token saved';
		const daysLeft = Math.ceil((server.tokenExpiry - Date.now()) / (1000 * 60 * 60 * 24));
		if (daysLeft < 0) return 'Token expired';
		if (daysLeft === 0) return 'Expires today';
		if (daysLeft === 1) return 'Expires tomorrow';
		return `Expires in ${daysLeft} days`;
	}

	function isServerConnected(server: SavedServer): boolean {
		return connectionStore.activeServerId === server.id && connectionStore.isConnected;
	}

	async function handleViewLogs() {
		try {
			daemonLogs = await invoke<string[]>('get_daemon_logs');
		} catch (e) {
			daemonLogs = ['Failed to fetch logs'];
		}
		logsModalOpen = true;
	}

	async function handleStart() {
		isStarting = true;
		try {
			await invoke<number>('start_local_daemon', {
				dataDir: settingsStore.settings.localDaemonDataDir,
				libraryDir: settingsStore.settings.localDaemonLibraryDir
			});
			await checkDaemonStatus();
			// Refresh paths to show actual daemon paths
			localPaths = await invoke<LocalDaemonPaths>('get_local_daemon_paths');
			toastStore.success('Local service started');
		} catch (e) {
			console.error('Failed to start daemon:', e);
			toastStore.error('Failed to start local service');
		} finally {
			isStarting = false;
		}
	}

	async function handleRestart() {
		isRestarting = true;
		try {
			await invoke('restart_local_daemon');
			// Refresh paths to show actual daemon paths
			localPaths = await invoke<LocalDaemonPaths>('get_local_daemon_paths');
			toastStore.success('Local service restarted');
		} catch (e) {
			console.error('Failed to restart daemon:', e);
			toastStore.error('Failed to restart local service');
		} finally {
			isRestarting = false;
		}
	}

	async function handleStop() {
		isStopping = true;
		try {
			await invoke('stop_local_daemon');
			// Disconnect from local server if connected
			if (connectionStore.activeServerId === localServer?.id) {
				connectionStore.disconnect();
			}
			await checkDaemonStatus();
			toastStore.success('Local service stopped');
		} catch (e) {
			console.error('Failed to stop daemon:', e);
			toastStore.error('Failed to stop local service');
		} finally {
			isStopping = false;
		}
	}

	// Post-processing save function
	async function savePostProcessingConfig() {
		if (!connectionStore.isConnected) return;
		try {
			await api.updatePostProcessingConfig({
				enabled: postProcessingEnabled,
				check_interval_minutes: checkIntervalMinutes,
				output_format: outputFormat as 'mp4_reencode' | 'mp4_copy' | 'ts_concat',
				segment_handling: segmentHandling,
				encoding: {
					crf,
					preset,
					video_codec: videoCodec,
					audio_codec: audioCodec,
					audio_bitrate: audioBitrate
				}
			});
			toastStore.success('Post-processing settings saved');
		} catch (e) {
			toastStore.error('Failed to save settings');
			console.error('Failed to save post-processing config:', e);
		}
	}

	// Post-processing handler functions
	async function togglePostProcessing() {
		postProcessingEnabled = !postProcessingEnabled;
		await savePostProcessingConfig();
	}

	async function updateCheckInterval(value: number) {
		checkIntervalMinutes = value;
		await savePostProcessingConfig();
	}

	async function updateOutputFormat(value: string) {
		outputFormat = value;
		await savePostProcessingConfig();
	}

	async function updateSegmentHandling(value: 'delete' | 'concatenate' | 'keep') {
		segmentHandling = value;
		await savePostProcessingConfig();
	}

	async function updateCrf(value: number) {
		crf = value;
		await savePostProcessingConfig();
	}

	async function updatePreset(value: string) {
		preset = value;
		await savePostProcessingConfig();
	}

	async function updateVideoCodec(value: string) {
		videoCodec = value;
		await savePostProcessingConfig();
	}

	async function updateAudioCodec(value: string) {
		audioCodec = value;
		await savePostProcessingConfig();
	}

	async function updateAudioBitrate(value: string) {
		audioBitrate = value;
		await savePostProcessingConfig();
	}

	const localServer = $derived(settingsStore.settings.servers.find((s) => s.type === 'local'));
	const remoteServers = $derived(settingsStore.settings.servers.filter((s) => s.type === 'remote'));
</script>

<div class="space-y-8">
	<div>
		<h1 class="font-display text-4xl tracking-tight uppercase">Settings</h1>
		<p class="text-muted-foreground mt-2">Manage servers and app preferences</p>
	</div>

	<!-- Servers Section -->
	<section>
		<h2 class="font-display text-2xl uppercase tracking-tight mb-4 flex items-center gap-2">
			<Server size={20} />
			Servers
		</h2>

		<!-- Local Service -->
		{#if localServer}
			<div class="relative border border-border bg-card p-4 mb-4">
				<CornerBrackets />
				<div class="flex items-start justify-between gap-4">
					<div class="flex items-center gap-3">
						<div class="p-2 rounded bg-muted">
							<Monitor size={20} class="text-emerald-400" />
						</div>
						<div>
							<div class="flex items-center gap-2">
								<h3 class="font-mono text-sm text-foreground">Local Service</h3>
								{#if isServerConnected(localServer)}
									<span
										class="px-1.5 py-0.5 rounded bg-emerald-500/20 font-mono text-[10px] text-emerald-400"
									>
										Connected
									</span>
								{:else if isDaemonRunning}
									<span
										class="px-1.5 py-0.5 rounded bg-blue-500/20 font-mono text-[10px] text-blue-400"
									>
										Running
									</span>
								{:else}
									<span
										class="px-1.5 py-0.5 rounded bg-muted font-mono text-[10px] text-muted-foreground"
									>
										Stopped
									</span>
								{/if}
							</div>
							<p class="font-mono text-xs text-muted-foreground">{localServer.url}</p>
						</div>
					</div>

					<div class="flex items-center gap-2">
						{#if isDaemonRunning}
							<button
								class="p-2 rounded hover:bg-muted transition-colors disabled:opacity-50"
								title="View Logs"
								onclick={handleViewLogs}
							>
								<FileText size={14} class="text-muted-foreground" />
							</button>
							<button
								class="p-2 rounded hover:bg-muted transition-colors disabled:opacity-50"
								title="Restart"
								onclick={handleRestart}
								disabled={isRestarting || isStopping}
							>
								{#if isRestarting}
									<RotateCcw size={14} class="text-muted-foreground animate-spin" />
								{:else}
									<RotateCcw size={14} class="text-muted-foreground" />
								{/if}
							</button>
							<button
								class="p-2 rounded hover:bg-muted transition-colors disabled:opacity-50"
								title="Stop"
								onclick={handleStop}
								disabled={isRestarting || isStopping}
							>
								{#if isStopping}
									<div
										class="size-3.5 animate-spin rounded-full border-2 border-muted-foreground border-t-transparent"
									></div>
								{:else}
									<Square size={14} class="text-muted-foreground" />
								{/if}
							</button>
						{:else}
							<button
								class="p-2 rounded hover:bg-muted transition-colors disabled:opacity-50"
								title="Start"
								onclick={handleStart}
								disabled={isStarting}
							>
								{#if isStarting}
									<div
										class="size-3.5 animate-spin rounded-full border-2 border-emerald-500 border-t-transparent"
									></div>
								{:else}
									<Play size={14} class="text-emerald-400" />
								{/if}
							</button>
						{/if}
					</div>
				</div>

				<!-- Local Storage Paths (only show when daemon is running) -->
				{#if localPaths && isDaemonRunning}
					<div class="mt-4 pt-4 border-t border-border">
						<div class="flex items-center gap-2 mb-3">
							<HardDrive size={14} class="text-muted-foreground" />
							<span class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground"
								>Local Storage</span
							>
						</div>
						<div class="space-y-2">
							{#if localPaths.config_path}
								<div class="flex items-center justify-between gap-2">
									<div class="min-w-0 flex-1">
										<p class="font-mono text-[10px] text-muted-foreground mb-0.5">Config File</p>
										<p
											class="font-mono text-xs text-foreground truncate"
											title={localPaths.config_path}
										>
											{localPaths.config_path}
										</p>
									</div>
									<button
										class="p-1.5 rounded hover:bg-muted transition-colors shrink-0"
										title="Open in folder"
										onclick={() => openInFolder(localPaths!.config_path!)}
									>
										<FolderOpen size={12} class="text-muted-foreground" />
									</button>
								</div>
							{/if}
							{#if localPaths.data_dir}
								<div class="flex items-center justify-between gap-2">
									<div class="min-w-0 flex-1">
										<p class="font-mono text-[10px] text-muted-foreground mb-0.5">Recordings Directory</p>
										<p class="font-mono text-xs text-foreground truncate" title={localPaths.data_dir}>
											{localPaths.data_dir}
										</p>
									</div>
									<div class="flex items-center gap-1 shrink-0">
										<button
											class="p-1.5 rounded hover:bg-muted transition-colors disabled:opacity-50"
											title="Change directory"
											onclick={handleEditDataDir}
											disabled={isUpdatingPaths}
										>
											{#if isUpdatingPaths}
												<div
													class="size-3 animate-spin rounded-full border border-muted-foreground border-t-transparent"
												></div>
											{:else}
												<Pencil size={12} class="text-muted-foreground" />
											{/if}
										</button>
										<button
											class="p-1.5 rounded hover:bg-muted transition-colors"
											title="Open in folder"
											onclick={() => openInFolder(localPaths!.data_dir!)}
										>
											<FolderOpen size={12} class="text-muted-foreground" />
										</button>
									</div>
								</div>
							{/if}
							<!-- Library Directory -->
							<div class="flex items-center justify-between gap-2">
								<div class="min-w-0 flex-1">
									<p class="font-mono text-[10px] text-muted-foreground mb-0.5">Library Directory</p>
									<p
										class="font-mono text-xs text-foreground truncate"
										title={localPaths.library_dir ?? localPaths.data_dir ?? ''}
									>
										{localPaths.library_dir ?? localPaths.data_dir ?? 'Same as recordings'}
									</p>
									{#if !localPaths.library_dir}
										<p class="font-mono text-[9px] text-muted-foreground/70">
											Processed files saved with recordings
										</p>
									{/if}
								</div>
								<div class="flex items-center gap-1 shrink-0">
									<button
										class="p-1.5 rounded hover:bg-muted transition-colors disabled:opacity-50"
										title="Change library directory"
										onclick={handleEditLibraryDir}
										disabled={isUpdatingLibraryDir}
									>
										{#if isUpdatingLibraryDir}
											<div
												class="size-3 animate-spin rounded-full border border-muted-foreground border-t-transparent"
											></div>
										{:else}
											<Pencil size={12} class="text-muted-foreground" />
										{/if}
									</button>
									<button
										class="p-1.5 rounded hover:bg-muted transition-colors"
										title="Open in folder"
										onclick={() => openInFolder(localPaths!.library_dir ?? localPaths!.data_dir!)}
									>
										<FolderOpen size={12} class="text-muted-foreground" />
									</button>
								</div>
							</div>
						</div>
					</div>
				{/if}
			</div>
		{/if}

		<!-- Remote Servers -->
		<div class="space-y-3">
			<h3 class="font-mono text-xs uppercase tracking-wider text-muted-foreground">Remote Servers</h3>

			{#each remoteServers as server}
				<div class="relative border border-border bg-card p-4">
					<CornerBrackets />
					<div class="flex items-start justify-between gap-4">
						<div class="flex items-center gap-3">
							<div class="p-2 rounded bg-muted">
								<Cloud size={20} class="text-blue-400" />
							</div>
							<div>
								<div class="flex items-center gap-2">
									<h3 class="font-mono text-sm text-foreground">{server.name}</h3>
									{#if isServerConnected(server)}
										<span
											class="px-1.5 py-0.5 rounded bg-emerald-500/20 font-mono text-[10px] text-emerald-400"
										>
											Connected
										</span>
									{/if}
								</div>
								<p class="font-mono text-xs text-muted-foreground">{server.url}</p>
								<p class="font-mono text-[10px] text-muted-foreground/70">{getTokenStatus(server)}</p>
							</div>
						</div>

						<div class="flex items-center gap-2">
							<button
								class="p-2 rounded hover:bg-muted transition-colors"
								title="Edit"
								onclick={() => openEditModal(server)}
							>
								<Pencil size={14} class="text-muted-foreground" />
							</button>
							<button
								class="p-2 rounded hover:bg-muted transition-colors"
								title="Reconnect"
								onclick={() => openReconnectModal(server)}
							>
								<RefreshCw size={14} class="text-muted-foreground" />
							</button>
							<button
								class="p-2 rounded hover:bg-muted transition-colors"
								title="Remove"
								onclick={() => confirmDelete(server)}
							>
								<Trash2 size={14} class="text-red-500" />
							</button>
						</div>
					</div>
				</div>
			{/each}

			{#if remoteServers.length === 0}
				<p class="font-mono text-xs text-muted-foreground/70 py-4">No remote servers configured</p>
			{/if}

			<button
				class="flex items-center gap-2 px-4 py-2 rounded border border-dashed border-border hover:border-muted-foreground font-mono text-sm text-muted-foreground hover:text-foreground transition-colors"
				onclick={() => (addModalOpen = true)}
			>
				<Plus size={14} />
				Add Remote Server
			</button>
		</div>
	</section>

	<!-- Startup Behavior -->
	<section>
		<h3 class="font-mono text-xs uppercase tracking-wider text-muted-foreground mb-3">Startup Behavior</h3>
		<div class="relative border border-border bg-card p-4">
			<CornerBrackets />
			<label for="settings-startup-server" class="block font-mono text-sm text-foreground mb-2">Connect on launch:</label>
			<select
				id="settings-startup-server"
				class="w-full max-w-xs rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground"
				value={settingsStore.settings.startupServerId ?? ''}
				onchange={(e) => settingsStore.setStartupServer(e.currentTarget.value || null)}
			>
				<option value="">Ask every time</option>
				{#each settingsStore.settings.servers as server}
					<option value={server.id}>{server.name}</option>
				{/each}
			</select>
		</div>
	</section>

	<!-- Shutdown Behavior -->
	<section>
		<h3 class="font-mono text-xs uppercase tracking-wider text-muted-foreground mb-3">Shutdown Behavior</h3>
		<div class="relative border border-border bg-card p-4">
			<CornerBrackets />
			<label for="settings-close-action" class="block font-mono text-sm text-foreground mb-2"
				>When closing with local service running:</label
			>
			<select
				id="settings-close-action"
				class="w-full max-w-xs rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground"
				value={settingsStore.settings.closeAction}
				onchange={(e) =>
					settingsStore.setCloseAction(e.currentTarget.value as 'ask' | 'background' | 'shutdown')}
			>
				<option value="ask">Ask every time</option>
				<option value="background">Keep running in background</option>
				<option value="shutdown">Shut down service and exit</option>
			</select>
		</div>
	</section>

	<!-- Appearance -->
	<section>
		<h3 class="font-mono text-xs uppercase tracking-wider text-muted-foreground mb-3 flex items-center gap-2">
			<Palette size={14} />
			Appearance
		</h3>
		<div class="relative border border-border bg-card p-4">
			<CornerBrackets />
			<!-- Corner Brackets Toggle -->
			<div class="flex items-center justify-between">
				<div>
					<span id="settings-corner-brackets-label" class="block font-mono text-sm text-foreground">Show Corner Brackets</span>
					<p class="font-mono text-[10px] text-muted-foreground">
						Display decorative corner brackets on cards and panels
					</p>
				</div>
				<button
					role="switch"
					aria-checked={settingsStore.settings.showCornerBrackets}
					aria-labelledby="settings-corner-brackets-label"
					class="relative w-11 h-6 rounded-full transition-colors {settingsStore.settings.showCornerBrackets
						? 'bg-emerald-600'
						: 'bg-muted'}"
					onclick={() => settingsStore.setShowCornerBrackets(!settingsStore.settings.showCornerBrackets)}
				>
					<span
						class="absolute left-1 top-1 w-4 h-4 rounded-full bg-white transition-transform {settingsStore.settings.showCornerBrackets
							? 'translate-x-5'
							: ''}"
					></span>
				</button>
			</div>
		</div>
	</section>

	<!-- Advanced -->
	<section>
		<h3 class="font-mono text-xs uppercase tracking-wider text-muted-foreground mb-3 flex items-center gap-2">
			<Terminal size={14} />
			Advanced
		</h3>
		<div class="relative border border-border bg-card p-4">
			<CornerBrackets />
			<div class="flex items-center justify-between">
				<div>
					<span id="settings-debug-console-label" class="block font-mono text-sm text-foreground">Debug Console</span>
					<p class="font-mono text-[10px] text-muted-foreground">
						Show in-app developer tools for debugging connections and errors
						<kbd class="ml-1 px-1 py-0.5 rounded bg-muted text-[9px]">Ctrl+Shift+D</kbd>
					</p>
				</div>
				<button
					role="switch"
					aria-checked={settingsStore.settings.debugConsole}
					aria-labelledby="settings-debug-console-label"
					class="relative w-11 h-6 rounded-full transition-colors {settingsStore.settings.debugConsole
						? 'bg-emerald-600'
						: 'bg-muted'}"
					onclick={() => {
						const newValue = !settingsStore.settings.debugConsole;
						settingsStore.settings.debugConsole = newValue;
						settingsStore.save();
					}}
				>
					<span
						class="absolute left-1 top-1 w-4 h-4 rounded-full bg-white transition-transform {settingsStore.settings.debugConsole
							? 'translate-x-5'
							: ''}"
					></span>
				</button>
			</div>
		</div>
	</section>

	<!-- Post-Processing Settings -->
	<section>
		<h3 class="font-mono text-xs uppercase tracking-wider text-muted-foreground mb-3">Post-Processing</h3>
		<div class="relative border border-border bg-card p-4">
			<CornerBrackets />
			<div class="space-y-4">
				<!-- Enable toggle -->
				<div class="flex items-center justify-between">
				<div>
					<span id="settings-postprocess-label" class="block font-mono text-sm text-foreground">Enable Auto Post-Processing</span>
					<p class="font-mono text-[10px] text-muted-foreground">
						Automatically process recordings after they complete
					</p>
				</div>
				<button
					role="switch"
					aria-checked={postProcessingEnabled}
					aria-labelledby="settings-postprocess-label"
					class="relative w-11 h-6 rounded-full transition-colors {postProcessingEnabled
						? 'bg-emerald-600'
						: 'bg-muted'}"
					onclick={() => togglePostProcessing()}
				>
					<span
						class="absolute left-1 top-1 w-4 h-4 rounded-full bg-white transition-transform {postProcessingEnabled
							? 'translate-x-5'
							: ''}"
					></span>
				</button>
			</div>

			<!-- Check Interval -->
			<div>
				<label
					for="check-interval"
					class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2"
				>
					Check Interval (minutes)
				</label>
				<input
					id="check-interval"
					type="number"
					min="1"
					max="1440"
					value={checkIntervalMinutes}
					onchange={(e) => updateCheckInterval(parseInt(e.currentTarget.value))}
					class="w-full max-w-xs rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground"
				/>
			</div>

			<!-- Output Format -->
			<div>
				<label for="settings-output-format" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Output Format
				</label>
				<select
					id="settings-output-format"
					class="w-full max-w-xs rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground"
					value={outputFormat}
					onchange={(e) => updateOutputFormat(e.currentTarget.value)}
				>
					<option value="mp4_reencode">MP4 (Re-encode)</option>
					<option value="mp4_copy">MP4 (Stream Copy)</option>
					<option value="ts_concat">TS Concat Only</option>
				</select>
			</div>

			<!-- Segment Handling -->
			<div>
				<label
					for="segmentHandling"
					class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2"
				>
					After Processing
				</label>
				<select
					id="segmentHandling"
					value={segmentHandling}
					onchange={(e) =>
						updateSegmentHandling(e.currentTarget.value as 'delete' | 'concatenate' | 'keep')}
					class="w-full max-w-xs rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground"
				>
					<option value="delete">Delete original .ts segments</option>
					<option value="concatenate">Concatenate into single .ts file</option>
					<option value="keep">Keep all .ts segments</option>
				</select>
			</div>

			<!-- Encoding Settings (only when mp4_reencode) -->
			{#if outputFormat === 'mp4_reencode'}
				<div class="border-t border-border pt-4 space-y-4">
					<h4 class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
						Encoding Settings
					</h4>

					<!-- CRF Slider -->
					<div>
						<div class="flex items-center justify-between mb-2 max-w-xs">
							<label
								for="crf-slider"
								class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground"
							>
								Quality (CRF)
							</label>
							<span class="font-mono text-xs text-muted-foreground">{crf}</span>
						</div>
						<input
							id="crf-slider"
							type="range"
							min="0"
							max="51"
							value={crf}
							onchange={(e) => updateCrf(parseInt(e.currentTarget.value))}
							class="w-full max-w-xs"
						/>
						<div class="flex justify-between max-w-xs">
							<span class="font-mono text-[9px] text-muted-foreground/70">Best Quality</span>
							<span class="font-mono text-[9px] text-muted-foreground/70">Smallest Size</span>
						</div>
					</div>

					<!-- Preset -->
					<div>
						<label for="settings-preset" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
							Encoding Preset
						</label>
						<select
							id="settings-preset"
							class="w-full max-w-xs rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground"
							value={preset}
							onchange={(e) => updatePreset(e.currentTarget.value)}
						>
							<option value="ultrafast">Ultrafast (lowest quality)</option>
							<option value="superfast">Superfast</option>
							<option value="veryfast">Veryfast</option>
							<option value="faster">Faster</option>
							<option value="fast">Fast</option>
							<option value="medium">Medium (balanced)</option>
							<option value="slow">Slow</option>
							<option value="slower">Slower</option>
							<option value="veryslow">Veryslow (highest quality)</option>
						</select>
					</div>

					<!-- Video Codec -->
					<div>
						<label for="settings-video-codec" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
							Video Codec
						</label>
						<select
							id="settings-video-codec"
							class="w-full max-w-xs rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground"
							value={videoCodec}
							onchange={(e) => updateVideoCodec(e.currentTarget.value)}
						>
							<option value="libx264">H.264 (libx264)</option>
							<option value="libx265">H.265/HEVC (libx265)</option>
							<option value="h264_nvenc">H.264 NVENC (NVIDIA)</option>
							<option value="h264_qsv">H.264 QuickSync (Intel)</option>
						</select>
					</div>

					<!-- Audio Codec -->
					<div>
						<label for="settings-audio-codec" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
							Audio Codec
						</label>
						<select
							id="settings-audio-codec"
							class="w-full max-w-xs rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground"
							value={audioCodec}
							onchange={(e) => updateAudioCodec(e.currentTarget.value)}
						>
							<option value="aac">AAC</option>
							<option value="copy">Copy Original</option>
						</select>
					</div>

					<!-- Audio Bitrate (only when aac) -->
					{#if audioCodec === 'aac'}
						<div>
							<label
								for="settings-audio-bitrate"
								class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2"
							>
								Audio Bitrate
							</label>
							<select
								id="settings-audio-bitrate"
								class="w-full max-w-xs rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground"
								value={audioBitrate}
								onchange={(e) => updateAudioBitrate(e.currentTarget.value)}
							>
								<option value="96k">96 kbps</option>
								<option value="128k">128 kbps</option>
								<option value="192k">192 kbps</option>
								<option value="256k">256 kbps</option>
							</select>
						</div>
					{/if}
				</div>
			{/if}
			</div>
		</div>
	</section>
</div>

<!-- Edit Server Modal -->
<ResponsiveModal open={editModalOpen} onOpenChange={(v) => (editModalOpen = v)} title="Edit Server">
	{#snippet children()}
		<form
			class="space-y-4"
			onsubmit={(e) => {
				e.preventDefault();
				handleEditSave();
			}}
		>
			<div>
				<label for="edit-server-name" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Server Name
				</label>
				<input
					id="edit-server-name"
					type="text"
					bind:value={editName}
					class="w-full rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground"
				/>
			</div>
			<div>
				<label for="edit-server-url" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Server URL
				</label>
				<input
					id="edit-server-url"
					type="text"
					bind:value={editUrl}
					class="w-full rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground"
				/>
				{#if editingServer && editUrl !== editingServer.url && editingServer.type === 'remote'}
					<p class="font-mono text-[10px] text-amber-400 mt-1">
						Changing URL will require re-authentication
					</p>
				{/if}
			</div>
		</form>
	{/snippet}
	{#snippet footer()}
		<div class="flex gap-2">
			<button
				class="flex-1 rounded bg-emerald-600 px-4 py-2 font-mono text-sm text-white hover:bg-emerald-500 transition-colors"
				onclick={handleEditSave}
			>
				Save
			</button>
			<button
				class="rounded border border-border bg-input px-4 py-2 font-mono text-sm text-foreground hover:bg-muted transition-colors"
				onclick={() => (editModalOpen = false)}
			>
				Cancel
			</button>
		</div>
	{/snippet}
</ResponsiveModal>

<!-- Reconnect Modal -->
<ResponsiveModal
	open={reconnectModalOpen}
	onOpenChange={(v) => (reconnectModalOpen = v)}
	title="Reconnect"
>
	{#snippet children()}
		<form
			class="space-y-4"
			onsubmit={(e) => {
				e.preventDefault();
				handleReconnect();
			}}
		>
			{#if reconnectError}
				<div class="rounded border border-red-500/30 bg-red-500/10 p-3">
					<p class="font-mono text-xs text-red-400">{reconnectError}</p>
				</div>
			{/if}
			<p class="font-mono text-sm text-muted-foreground">
				Enter credentials for {reconnectServer?.name}
			</p>
			<div>
				<label for="reconnect-username" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Username
				</label>
				<input
					id="reconnect-username"
					type="text"
					bind:value={reconnectUsername}
					disabled={isReconnecting}
					class="w-full rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground disabled:opacity-50"
				/>
			</div>
			<div>
				<label for="reconnect-password" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Password
				</label>
				<input
					id="reconnect-password"
					type="password"
					bind:value={reconnectPassword}
					disabled={isReconnecting}
					class="w-full rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground disabled:opacity-50"
				/>
			</div>
		</form>
	{/snippet}
	{#snippet footer()}
		<div class="flex gap-2">
			<button
				class="flex-1 rounded bg-emerald-600 px-4 py-2 font-mono text-sm text-white hover:bg-emerald-500 disabled:opacity-50 transition-colors"
				disabled={!reconnectUsername || !reconnectPassword || isReconnecting}
				onclick={handleReconnect}
			>
				{isReconnecting ? 'Connecting...' : 'Connect'}
			</button>
			<button
				class="rounded border border-border bg-input px-4 py-2 font-mono text-sm text-foreground hover:bg-muted transition-colors"
				onclick={() => (reconnectModalOpen = false)}
			>
				Cancel
			</button>
		</div>
	{/snippet}
</ResponsiveModal>

<!-- Add Server Modal -->
<ResponsiveModal
	open={addModalOpen}
	onOpenChange={(v) => (addModalOpen = v)}
	title="Add Remote Server"
	initialFocusEl={addUrlInput}
>
	{#snippet children()}
		<form
			id="add-server-form"
			class="space-y-4"
			onsubmit={(e) => {
				e.preventDefault();
				handleAddServer();
			}}
		>
			{#if addError}
				<div class="rounded border border-red-500/30 bg-red-500/10 p-3">
					<p class="font-mono text-xs text-red-400">{addError}</p>
				</div>
			{/if}
			<div>
				<label for="settings-add-name" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Server Name (optional)
				</label>
				<input
					id="settings-add-name"
					type="text"
					placeholder="My Server"
					bind:value={addName}
					disabled={isAdding}
					class="w-full rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground placeholder:text-muted-foreground disabled:opacity-50"
				/>
			</div>
			<div>
				<label for="settings-add-url" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Server URL
				</label>
				<input
					id="settings-add-url"
					type="text"
					placeholder="http://192.168.1.100:8080"
					bind:value={addUrl}
					bind:this={addUrlInput}
					disabled={isAdding}
					class="w-full rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground placeholder:text-muted-foreground disabled:opacity-50"
				/>
			</div>
			<div>
				<label for="settings-add-username" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Username
				</label>
				<input
					id="settings-add-username"
					type="text"
					bind:value={addUsername}
					disabled={isAdding}
					class="w-full rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground disabled:opacity-50"
				/>
			</div>
			<div>
				<label for="settings-add-password" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Password
				</label>
				<input
					id="settings-add-password"
					type="password"
					bind:value={addPassword}
					disabled={isAdding}
					class="w-full rounded border border-border bg-input px-3 py-2 font-mono text-sm text-foreground disabled:opacity-50"
				/>
			</div>
		</form>
	{/snippet}
	{#snippet footer()}
		<div class="flex gap-2">
			<button
				type="submit"
				form="add-server-form"
				class="flex-1 rounded bg-emerald-600 px-4 py-2 font-mono text-sm text-white hover:bg-emerald-500 disabled:opacity-50 transition-colors"
				disabled={!addUrl || !addUsername || !addPassword || isAdding}
			>
				{isAdding ? 'Connecting...' : 'Connect'}
			</button>
			<button
				type="button"
				class="rounded border border-border bg-input px-4 py-2 font-mono text-sm text-foreground hover:bg-muted transition-colors"
				onclick={() => (addModalOpen = false)}
			>
				Cancel
			</button>
		</div>
	{/snippet}
</ResponsiveModal>

<!-- Delete Confirmation Modal -->
<ResponsiveModal
	open={deleteServer !== null}
	onOpenChange={(v) => !v && (deleteServer = null)}
	title="Remove Server"
>
	{#snippet children()}
		<p class="font-mono text-sm text-muted-foreground">
			Are you sure you want to remove "{deleteServer?.name}"? This cannot be undone.
		</p>
	{/snippet}
	{#snippet footer()}
		<div class="flex gap-2">
			<button
				class="flex-1 rounded bg-red-600 px-4 py-2 font-mono text-sm text-white hover:bg-red-500 transition-colors"
				onclick={handleDelete}
			>
				Remove
			</button>
			<button
				class="rounded border border-border bg-input px-4 py-2 font-mono text-sm text-foreground hover:bg-muted transition-colors"
				onclick={() => (deleteServer = null)}
			>
				Cancel
			</button>
		</div>
	{/snippet}
</ResponsiveModal>

<!-- Daemon Logs Modal -->
<DaemonLogsModal open={logsModalOpen} onOpenChange={(v) => (logsModalOpen = v)} logs={daemonLogs} />
