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
		Terminal,
		Plug,
		Download,
		RefreshCw as UpdateIcon,
		Info,
		ChevronDown,
		Power,
		Film,
		Settings as SettingsIcon
	} from '@lucide/svelte';
	import { Tooltip } from 'bits-ui';
	import {
		settingsStore,
		connectionStore,
		extensionsStore,
		ResponsiveModal,
		toastStore,
		welcomeStore,
		Panel,
		PageHeader,
		AppearanceControls,
		type SavedServer
	} from '#lib';
	import Switch from '#lib/components/ui/Switch.svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import NumberInput from '#lib/components/ui/NumberInput.svelte';
	import Select from '#lib/components/ui/Select.svelte';
	import { api, type PostProcessingConfig, type DownloadsConfig } from '#lib/api/index.ts';
	import { invoke } from '@tauri-apps/api/core';
	import { open } from '@tauri-apps/plugin-dialog';
	import { onMount } from 'svelte';
	import { untrack } from 'svelte';
	import DaemonLogsModal from '#lib/components/DaemonLogsModal.svelte';
	import MessageLogViewer from '#lib/components/MessageLogViewer.svelte';
	import ExtensionGatewayStatus from '#lib/components/ExtensionGatewayStatus.svelte';
	import StatusDot from '#lib/components/ui/StatusDot.svelte';
	import { extensionGatewayStore } from '#lib/stores/extensionGateway.svelte.ts';
	import { formatDate } from '#lib/utils/format.ts';
	import { cn } from '#lib/utils/cn.ts';
	import { extractErrorMessage } from '#lib/utils/errors.ts';

	// Local daemon paths
	interface LocalDaemonPaths {
		config_path: string | null;
		data_dir: string | null;
		library_dir: string | null;
		downloads_dir: string | null;
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

	// Downloads config state
	let dlDirectory = $state<string | null>(null);
	let dlDefaultFormat = $state('bestvideo+bestaudio/best');
	let dlMaxConcurrent = $state(2);
	let dlEmbedThumbnail = $state(true);
	let dlEmbedMetadata = $state(true);
	let dlMaxTotalGb = $state<number | null>(null);
	let dlMaxAgeDays = $state<number | null>(null);

	// Extension state
	let updatingLibrary = $state<string | null>(null);
	let uninstallingLibrary = $state<string | null>(null);
	let installingLibraries = $state<Set<string>>(new Set());
	let expandedLogId = $state<string | null>(null);
	const extensionBrowsers = $derived(extensionGatewayStore.status?.browsers ?? []);

	// Load config when connection is established (handles initial load and tab switches)
	$effect(() => {
		if (connectionStore.isConnected) {
			untrack(() => {
				loadPostProcessingConfig();
				loadDownloadsConfig();
				extensionsStore.load();
			});
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

	async function loadDownloadsConfig() {
		try {
			const dlConfig = await api.getDownloadsConfig();
			dlDirectory = dlConfig.directory;
			dlDefaultFormat = dlConfig.default_format;
			dlMaxConcurrent = dlConfig.max_concurrent;
			dlEmbedThumbnail = dlConfig.embed_thumbnail;
			dlEmbedMetadata = dlConfig.embed_metadata;
			dlMaxTotalGb = dlConfig.max_total_gb ?? null;
			dlMaxAgeDays = dlConfig.retention?.max_age_days ?? null;
		} catch (e) {
			console.error('Failed to load downloads config:', e);
		}
	}

	async function saveDownloadsConfig() {
		if (!connectionStore.isConnected) return;
		try {
			await api.updateDownloadsConfig({
				directory: dlDirectory ?? '',
				default_format: dlDefaultFormat,
				max_concurrent: dlMaxConcurrent,
				embed_thumbnail: dlEmbedThumbnail,
				embed_metadata: dlEmbedMetadata,
				max_total_gb: dlMaxTotalGb ?? undefined,
				retention: {
					max_age_days: dlMaxAgeDays ?? undefined,
					cleanup_interval_hours: 6
				}
			});
			toastStore.success('Downloads settings saved');
		} catch (e) {
			toastStore.error(extractErrorMessage(e, 'Failed to save downloads settings'));
		}
	}

	async function handleUpdateLibrary(name: string) {
		updatingLibrary = name;
		try {
			await extensionsStore.updateLibrary(name);
			toastStore.success(`${name} update started`);
		} catch (e) {
			toastStore.error(extractErrorMessage(e, `Failed to update ${name}`));
		} finally {
			updatingLibrary = null;
		}
	}

	async function handleInstallLibrary(name: string) {
		installingLibraries = new Set([...installingLibraries, name]);
		try {
			await extensionsStore.updateLibrary(name);
		} catch (e) {
			// Install request failed - remove from set so button reappears
			const next = new Set(installingLibraries);
			next.delete(name);
			installingLibraries = next;
			toastStore.error(extractErrorMessage(e, `Failed to install ${name}`));
		}
		// Don't clear on success - the library_status_changed WebSocket event will
		// set installed=true, which hides the install button and shows uninstall
	}

	async function handleUninstallLibrary(name: string) {
		uninstallingLibrary = name;
		try {
			const success = await extensionsStore.uninstallLibrary(name);
			if (success) {
				toastStore.success(`${name === 'ytdlp' ? 'yt-dlp' : name} uninstalled`);
			} else {
				toastStore.error(extensionsStore.error ?? `Failed to uninstall ${name}`);
			}
		} catch (e) {
			toastStore.error(extractErrorMessage(e, `Failed to uninstall ${name}`));
		} finally {
			uninstallingLibrary = null;
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
				undefined,
				true
			);
			// Persist to settings so it survives app restart
			settingsStore.setLocalDaemonLibraryDir(result as string);
		}
	}

	async function handleEditDownloadsDir() {
		const result = await open({
			directory: true,
			title: 'Select Downloads Directory',
			defaultPath: localPaths?.downloads_dir ?? dlDirectory ?? localPaths?.data_dir ?? undefined
		});

		if (result) {
			const dir = result as string;
			dlDirectory = dir;
			settingsStore.setLocalDaemonDownloadsDir(dir);
			await updatePaths(undefined, undefined, undefined, dir);
		}
	}

	async function updatePaths(
		configPath?: string,
		dataDir?: string,
		libraryDir?: string,
		downloadsDir?: string,
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
				libraryDir: libraryDir ?? null,
				downloadsDir: downloadsDir ?? null
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
	const CLOSE_ACTION_OPTIONS: { value: 'ask' | 'background' | 'shutdown'; label: string }[] = [
		{ value: 'ask', label: 'Ask every time' },
		{ value: 'background', label: 'Keep running in background' },
		{ value: 'shutdown', label: 'Shut down service and exit' }
	];
	const OUTPUT_FORMAT_OPTIONS = [
		{ value: 'mp4_reencode', label: 'MP4 (Re-encode)' },
		{ value: 'mp4_copy', label: 'MP4 (Stream Copy)' },
		{ value: 'ts_concat', label: 'TS Concat Only' }
	];
	const SEGMENT_HANDLING_OPTIONS: { value: 'delete' | 'concatenate' | 'keep'; label: string }[] = [
		{ value: 'delete', label: 'Delete original .ts segments' },
		{ value: 'concatenate', label: 'Concatenate into single .ts file' },
		{ value: 'keep', label: 'Keep all .ts segments' }
	];
	const PRESET_OPTIONS = [
		{ value: 'ultrafast', label: 'Ultrafast (lowest quality)' },
		{ value: 'superfast', label: 'Superfast' },
		{ value: 'veryfast', label: 'Veryfast' },
		{ value: 'faster', label: 'Faster' },
		{ value: 'fast', label: 'Fast' },
		{ value: 'medium', label: 'Medium (balanced)' },
		{ value: 'slow', label: 'Slow' },
		{ value: 'slower', label: 'Slower' },
		{ value: 'veryslow', label: 'Veryslow (highest quality)' }
	];
	const VIDEO_CODEC_OPTIONS = [
		{ value: 'libx264', label: 'H.264 (libx264)' },
		{ value: 'libx265', label: 'H.265/HEVC (libx265)' },
		{ value: 'h264_nvenc', label: 'H.264 NVENC (NVIDIA)' },
		{ value: 'h264_qsv', label: 'H.264 QuickSync (Intel)' }
	];
	const AUDIO_CODEC_OPTIONS = [
		{ value: 'aac', label: 'AAC' },
		{ value: 'copy', label: 'Copy Original' }
	];
	const AUDIO_BITRATE_OPTIONS = [
		{ value: '96k', label: '96 kbps' },
		{ value: '128k', label: '128 kbps' },
		{ value: '192k', label: '192 kbps' },
		{ value: '256k', label: '256 kbps' }
	];
	const startupServerOptions = $derived([
		{ value: null, label: 'Ask every time' },
		...settingsStore.settings.servers.map((server) => ({ value: server.id, label: server.name }))
	]);

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
				libraryDir: settingsStore.settings.localDaemonLibraryDir,
				downloadsDir: settingsStore.settings.localDaemonDownloadsDir
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
	<PageHeader
		title="Settings"
		eyebrow="Admin"
		subtitle="Manage servers and app preferences"
		icon={SettingsIcon}
	/>

	<!-- Servers Section -->
	<section>
		<div class="mb-4 flex items-center gap-3">
			<Server size={18} class="text-primary" />
			<h2 class="font-display text-2xl leading-none tracking-wide uppercase">Servers</h2>
			<span class="h-px flex-1 bg-gradient-to-r from-primary/40 to-transparent"></span>
		</div>

		<!-- Local Service -->
		{#if localServer}
			<div class="relative rounded-lg border border-border bg-card p-4 mb-4">
				<div class="flex items-start justify-between gap-4">
					<div class="flex items-center gap-3">
						<div class="p-2 rounded bg-muted">
							<Monitor size={20} class="text-green" />
						</div>
						<div>
							<div class="flex items-center gap-2">
								<h3 class="font-mono text-sm text-foreground">Local Service</h3>
								{#if isServerConnected(localServer)}
									<span
										class="px-1.5 py-0.5 rounded bg-green/20 font-mono text-[10px] text-green"
									>
										Connected
									</span>
								{:else if isDaemonRunning}
									<span
										class="px-1.5 py-0.5 rounded bg-blue/20 font-mono text-[10px] text-blue"
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
								class="p-2 rounded hover:bg-hover-wash transition-colors disabled:opacity-50"
								title="View Logs"
								onclick={handleViewLogs}
							>
								<FileText size={14} class="text-muted-foreground" />
							</button>
							<button
								class="p-2 rounded hover:bg-hover-wash transition-colors disabled:opacity-50"
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
								class="p-2 rounded hover:bg-hover-wash transition-colors disabled:opacity-50"
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
								class="p-2 rounded hover:bg-hover-wash transition-colors disabled:opacity-50"
								title="Start"
								onclick={handleStart}
								disabled={isStarting}
							>
								{#if isStarting}
									<div
										class="size-3.5 animate-spin rounded-full border-2 border-primary border-t-transparent"
									></div>
								{:else}
									<Play size={14} class="text-green" />
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
										class="p-1.5 rounded hover:bg-hover-wash transition-colors shrink-0"
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
											class="p-1.5 rounded hover:bg-hover-wash transition-colors disabled:opacity-50"
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
											class="p-1.5 rounded hover:bg-hover-wash transition-colors"
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
										class="p-1.5 rounded hover:bg-hover-wash transition-colors disabled:opacity-50"
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
										class="p-1.5 rounded hover:bg-hover-wash transition-colors"
										title="Open in folder"
										onclick={() => openInFolder(localPaths!.library_dir ?? localPaths!.data_dir!)}
									>
										<FolderOpen size={12} class="text-muted-foreground" />
									</button>
								</div>
							</div>
							<!-- Downloads Directory -->
							<div class="flex items-center justify-between gap-2">
								<div class="min-w-0 flex-1">
									<p class="font-mono text-[10px] text-muted-foreground mb-0.5">Downloads Directory</p>
									<p
										class="font-mono text-xs text-foreground truncate"
										title={localPaths.downloads_dir ?? dlDirectory ?? 'Loading...'}
									>
										{localPaths.downloads_dir ?? dlDirectory ?? 'Loading...'}
									</p>
									</div>
								<div class="flex items-center gap-1 shrink-0">
									<button
										class="p-1.5 rounded hover:bg-hover-wash transition-colors"
										title="Change downloads directory"
										onclick={handleEditDownloadsDir}
									>
										<Pencil size={12} class="text-muted-foreground" />
									</button>
									{#if localPaths.downloads_dir || dlDirectory}
										<button
											class="p-1.5 rounded hover:bg-hover-wash transition-colors"
											title="Open in folder"
											onclick={() => openInFolder(localPaths!.downloads_dir ?? dlDirectory!)}
										>
											<FolderOpen size={12} class="text-muted-foreground" />
										</button>
									{/if}
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
				<div class="relative rounded-lg border border-border bg-card p-4">
					<div class="flex items-start justify-between gap-4">
						<div class="flex items-center gap-3">
							<div class="p-2 rounded bg-muted">
								<Cloud size={20} class="text-blue" />
							</div>
							<div>
								<div class="flex items-center gap-2">
									<h3 class="font-mono text-sm text-foreground">{server.name}</h3>
									{#if isServerConnected(server)}
										<span
											class="px-1.5 py-0.5 rounded bg-green/20 font-mono text-[10px] text-green"
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
								class="p-2 rounded hover:bg-hover-wash transition-colors"
								title="Edit"
								onclick={() => openEditModal(server)}
							>
								<Pencil size={14} class="text-muted-foreground" />
							</button>
							<button
								class="p-2 rounded hover:bg-hover-wash transition-colors"
								title="Reconnect"
								onclick={() => openReconnectModal(server)}
							>
								<RefreshCw size={14} class="text-muted-foreground" />
							</button>
							<button
								class="p-2 rounded hover:bg-hover-wash transition-colors"
								title="Remove"
								onclick={() => confirmDelete(server)}
							>
								<Trash2 size={14} class="text-destructive" />
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
	<Panel title="Startup Behavior" icon={Play}>
		<label for="settings-startup-server" class="block font-mono text-sm text-foreground mb-2">Connect on launch:</label>
		<Select
			id="settings-startup-server"
			class="max-w-xs"
			value={settingsStore.settings.startupServerId ?? null}
			options={startupServerOptions}
			onValueChange={(id) => settingsStore.setStartupServer(id)}
		/>
	</Panel>

	<!-- Shutdown Behavior -->
	<Panel title="Shutdown Behavior" icon={Power}>
		<label for="settings-close-action" class="block font-mono text-sm text-foreground mb-2"
			>When closing with local service running:</label
		>
		<Select
			id="settings-close-action"
			class="max-w-xs"
			value={settingsStore.settings.closeAction}
			options={CLOSE_ACTION_OPTIONS}
			onValueChange={(action) => settingsStore.setCloseAction(action)}
		/>
	</Panel>

	<!-- Appearance -->
	<Panel
		id="appearance"
		title="Appearance"
		icon={Palette}
		description="Background, accent, rounding, density, contrast and motion"
	>
		<AppearanceControls />
		<div
			class="mt-6 flex flex-col gap-3 border-t border-border pt-4 sm:flex-row sm:items-center sm:justify-between"
		>
			<div>
				<span class="block font-mono text-sm text-foreground">Welcome screen</span>
				<p class="font-mono text-[10px] text-muted-foreground">
					Show the first-run introduction and appearance tour again
				</p>
			</div>
			<Button size="sm" onclick={() => welcomeStore.reset()}>Show welcome screen</Button>
		</div>
	</Panel>

	<!-- Advanced -->
	<Panel title="Advanced" icon={Terminal}>
		<div class="flex items-center justify-between">
			<div>
				<span id="settings-debug-console-label" class="block font-mono text-sm text-foreground">Debug Console</span>
				<p class="font-mono text-[10px] text-muted-foreground">
					Show in-app developer tools for debugging connections and errors
					<kbd class="ml-1 px-1 py-0.5 rounded bg-muted text-[9px]">Ctrl+Shift+D</kbd>
				</p>
			</div>
			<Switch
				checked={settingsStore.settings.debugConsole}
				labelledby="settings-debug-console-label"
				onCheckedChange={() => {
				const newValue = !settingsStore.settings.debugConsole;
				settingsStore.settings.debugConsole = newValue;
				settingsStore.save();
				}}
			/>
		</div>
	</Panel>

	<!-- Downloads -->
	<Panel title="Downloads" icon={Download}>
		<div class="space-y-4">
			<!-- Default Format -->
			<div>
				<div class="flex items-center gap-1.5 mb-2">
					<label
						for="settings-dl-format"
						class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground"
					>
						Default Format
					</label>
					<Tooltip.Provider>
						<Tooltip.Root delayDuration={200}>
							<Tooltip.Trigger
								class="text-muted-foreground hover:text-foreground transition-colors"
							>
								<Info size={12} />
							</Tooltip.Trigger>
							<Tooltip.Portal>
								<Tooltip.Content
									side="bottom"
									align="start"
									sideOffset={8}
									class="z-50 rounded border border-border bg-card p-3 shadow-lg"
								>
									<div class="whitespace-nowrap">
										<p class="text-xs text-muted-foreground mb-2.5">Combine video + audio with <code class="font-mono text-[11px] text-foreground bg-muted px-1 rounded">+</code> &mdash; use <code class="font-mono text-[11px] text-foreground bg-muted px-1 rounded">/</code> for fallback</p>
										<table class="text-xs font-mono">
											<tbody>
												<tr><td class="text-green pr-5 py-0.5">bestvideo+bestaudio/best</td><td class="text-muted-foreground">Best quality</td></tr>
												<tr><td class="text-foreground pr-5 py-0.5">bestvideo[height&lt;=1080]+bestaudio/best</td><td class="text-muted-foreground">Max 1080p</td></tr>
												<tr><td class="text-foreground pr-5 py-0.5">bestvideo[height&lt;=720]+bestaudio/best</td><td class="text-muted-foreground">Max 720p</td></tr>
												<tr><td class="text-foreground pr-5 py-0.5">bestaudio/best</td><td class="text-muted-foreground">Audio only</td></tr>
												<tr><td class="text-foreground pr-5 py-0.5">bestaudio[ext=m4a]</td><td class="text-muted-foreground">M4A audio</td></tr>
											</tbody>
										</table>
										<div class="border-t border-border mt-2 pt-2">
											<table class="text-xs font-mono">
												<tbody>
													<tr><td class="text-foreground pr-4 py-0.5">[height&lt;=N]</td><td class="text-muted-foreground">filter by resolution</td></tr>
													<tr><td class="text-foreground pr-4 py-0.5">[ext=mp4]</td><td class="text-muted-foreground">filter by format</td></tr>
													<tr><td class="text-foreground pr-4 py-0.5">[fps&lt;=30]</td><td class="text-muted-foreground">filter by framerate</td></tr>
												</tbody>
											</table>
										</div>
									</div>
								</Tooltip.Content>
							</Tooltip.Portal>
						</Tooltip.Root>
					</Tooltip.Provider>
				</div>
				<input
					id="settings-dl-format"
					type="text"
					value={dlDefaultFormat}
					onchange={(e) => { dlDefaultFormat = e.currentTarget.value; saveDownloadsConfig(); }}
					class="w-full max-w-xs rounded border border-border bg-btn-bg px-3 py-2 font-mono text-sm text-foreground"
					placeholder="bestvideo[height<=1080]+bestaudio/best"
				/>
			</div>

			<!-- Max Concurrent -->
			<div>
				<label
					for="settings-dl-concurrent"
					class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2"
				>
					Max Concurrent Downloads
				</label>
				<NumberInput
					id="settings-dl-concurrent"
					min="1"
					max="5"
					value={dlMaxConcurrent}
					onchange={(e) => { dlMaxConcurrent = parseInt(e.currentTarget.value) || 2; saveDownloadsConfig(); }}
					class="max-w-xs"
				/>
			</div>

			<!-- Embed Thumbnail Toggle -->
			<div class="flex items-center justify-between">
				<div>
					<span id="settings-dl-thumbnail-label" class="block font-mono text-sm text-foreground">Embed Thumbnail</span>
					<p class="font-mono text-[10px] text-muted-foreground">
						Embed video thumbnail in downloaded files
					</p>
				</div>
				<Switch
					checked={dlEmbedThumbnail}
					labelledby="settings-dl-thumbnail-label"
					onCheckedChange={() => { dlEmbedThumbnail = !dlEmbedThumbnail; saveDownloadsConfig(); }}
				/>
			</div>

			<!-- Embed Metadata Toggle -->
			<div class="flex items-center justify-between">
				<div>
					<span id="settings-dl-metadata-label" class="block font-mono text-sm text-foreground">Embed Metadata</span>
					<p class="font-mono text-[10px] text-muted-foreground">
						Embed video metadata (title, description, etc.) in downloaded files
					</p>
				</div>
				<Switch
					checked={dlEmbedMetadata}
					labelledby="settings-dl-metadata-label"
					onCheckedChange={() => { dlEmbedMetadata = !dlEmbedMetadata; saveDownloadsConfig(); }}
				/>
			</div>

			<!-- Storage & Retention -->
			<div class="border-t border-border pt-4 space-y-4">
				<h4 class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
					Storage & Retention
				</h4>

				<!-- Max Total GB -->
				<div>
					<label
						for="settings-dl-quota"
						class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2"
					>
						Max Total GB
					</label>
					<NumberInput
						id="settings-dl-quota"
						min="0"
						placeholder="Unlimited"
						value={dlMaxTotalGb}
						onchange={(e) => {
							const val = e.currentTarget.value;
							dlMaxTotalGb = val ? parseInt(val) || null : null;
							saveDownloadsConfig();
						}}
						class="max-w-xs"
					/>
					<p class="font-mono text-[9px] text-muted-foreground/70 mt-1">
						Leave empty for unlimited storage
					</p>
				</div>

				<!-- Retention Days -->
				<div>
					<label
						for="settings-dl-retention"
						class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2"
					>
						Retention (Days)
					</label>
					<NumberInput
						id="settings-dl-retention"
						min="0"
						placeholder="Keep forever"
						value={dlMaxAgeDays}
						onchange={(e) => {
							const val = e.currentTarget.value;
							dlMaxAgeDays = val ? parseInt(val) || null : null;
							saveDownloadsConfig();
						}}
						class="max-w-xs"
					/>
					<p class="font-mono text-[9px] text-muted-foreground/70 mt-1">
						Leave empty to keep downloads forever
					</p>
				</div>
			</div>
		</div>
	</Panel>

	<!-- Post-Processing Settings -->
	<Panel title="Post-Processing" icon={Film}>
		<div class="space-y-4">
			<!-- Enable toggle -->
			<div class="flex items-center justify-between">
			<div>
				<span id="settings-postprocess-label" class="block font-mono text-sm text-foreground">Enable Auto Post-Processing</span>
				<p class="font-mono text-[10px] text-muted-foreground">
					Automatically process recordings after they complete
				</p>
			</div>
			<Switch
				checked={postProcessingEnabled}
				labelledby="settings-postprocess-label"
				onCheckedChange={() => togglePostProcessing()}
			/>
		</div>

		<!-- Check Interval -->
		<div>
			<label
				for="check-interval"
				class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2"
			>
				Check Interval (minutes)
			</label>
			<NumberInput
				id="check-interval"
				min="1"
				max="1440"
				value={checkIntervalMinutes}
				onchange={(e) => updateCheckInterval(parseInt(e.currentTarget.value))}
				class="max-w-xs"
			/>
		</div>

		<!-- Output Format -->
		<div>
			<label for="settings-output-format" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
				Output Format
			</label>
			<Select
				id="settings-output-format"
				class="max-w-xs"
				value={outputFormat}
				options={OUTPUT_FORMAT_OPTIONS}
				onValueChange={updateOutputFormat}
			/>
		</div>

		<!-- Segment Handling -->
		<div>
			<label
				for="segmentHandling"
				class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2"
			>
				After Processing
			</label>
			<Select
				id="segmentHandling"
				class="max-w-xs"
				value={segmentHandling}
				options={SEGMENT_HANDLING_OPTIONS}
				onValueChange={updateSegmentHandling}
			/>
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
					<Select
						id="settings-preset"
						class="max-w-xs"
						value={preset}
						options={PRESET_OPTIONS}
						onValueChange={updatePreset}
					/>
				</div>

				<!-- Video Codec -->
				<div>
					<label for="settings-video-codec" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
						Video Codec
					</label>
					<Select
						id="settings-video-codec"
						class="max-w-xs"
						value={videoCodec}
						options={VIDEO_CODEC_OPTIONS}
						onValueChange={updateVideoCodec}
					/>
				</div>

				<!-- Audio Codec -->
				<div>
					<label for="settings-audio-codec" class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
						Audio Codec
					</label>
					<Select
						id="settings-audio-codec"
						class="max-w-xs"
						value={audioCodec}
						options={AUDIO_CODEC_OPTIONS}
						onValueChange={updateAudioCodec}
					/>
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
						<Select
							id="settings-audio-bitrate"
							class="max-w-xs"
							value={audioBitrate}
							options={AUDIO_BITRATE_OPTIONS}
							onValueChange={updateAudioBitrate}
						/>
					</div>
				{/if}
			</div>
		{/if}
		</div>
	</Panel>

	<!-- Library Management -->
	<Panel title="Library Management" icon={FolderOpen}>
		{#if extensionsStore.libraryStatus}
			<div class="space-y-2">
				{#each [
					{ key: 'ytdlp' as const, label: 'yt-dlp', description: 'Video downloader' },
					{ key: 'ffmpeg' as const, label: 'FFmpeg', description: 'Media processing' },
					{ key: 'bun' as const, label: 'Bun', description: 'JavaScript runtime' }
				] as lib}
					{@const info = extensionsStore.libraryStatus[lib.key]}
					{@const isBusy = updatingLibrary === lib.key || uninstallingLibrary === lib.key}
					<div class="flex items-center justify-between gap-3 rounded border border-border bg-muted/30 px-3 py-2">
						<div class="min-w-0">
							<p class="font-mono text-sm text-foreground">
								{lib.label}
								<span class="text-muted-foreground/50 text-[10px] ml-1">{lib.description}</span>
							</p>
							<p class="font-mono text-[10px] text-muted-foreground">
								{#if info.installed}
									v{info.version ?? 'unknown'}
									{#if info.path}
										<span class="text-muted-foreground/50"> &middot; {info.path}</span>
									{/if}
								{:else}
									Not installed
								{/if}
							</p>
						</div>
						<div class="flex items-center gap-2 shrink-0">
							{#if !info.installed && !installingLibraries.has(lib.key)}
								<button
									class="rounded border border-green/30 bg-green/10 px-2 py-1 font-mono text-[10px] text-green hover:bg-green/20 transition-colors disabled:opacity-50"
									onclick={() => handleInstallLibrary(lib.key)}
								>
									Install
								</button>
							{:else if !info.installed && installingLibraries.has(lib.key)}
								<button
									class="rounded border border-green/30 bg-green/10 px-2 py-1 font-mono text-[10px] text-green opacity-50 cursor-not-allowed"
									disabled
								>
									Installing...
								</button>
							{:else if info.installed}
								{#if info.update_available}
									<button
										class="rounded border border-green/30 bg-green/10 px-2 py-1 font-mono text-[10px] text-green hover:bg-green/20 transition-colors disabled:opacity-50"
										disabled={isBusy}
										onclick={() => handleUpdateLibrary(lib.key)}
									>
										{updatingLibrary === lib.key ? 'Updating...' : `Update to ${info.update_available}`}
									</button>
								{/if}
								{#if info.path?.includes('com.battles.record')}
									<button
										class="rounded border border-destructive/30 bg-destructive/10 px-2 py-1 font-mono text-[10px] text-destructive hover:bg-destructive/20 transition-colors disabled:opacity-50"
										disabled={isBusy}
										onclick={() => handleUninstallLibrary(lib.key)}
									>
										{uninstallingLibrary === lib.key ? 'Uninstalling...' : 'Uninstall'}
									</button>
								{:else}
									<Tooltip.Provider>
										<Tooltip.Root delayDuration={200}>
											<Tooltip.Trigger
												class="text-muted-foreground hover:text-foreground transition-colors"
											>
												<Info size={14} />
											</Tooltip.Trigger>
											<Tooltip.Portal>
												<Tooltip.Content
													side="left"
													sideOffset={8}
													class="z-50 max-w-xs rounded border border-border bg-card p-3 shadow-lg"
												>
													<p class="text-xs text-muted-foreground mb-2">This library is installed system-wide, not managed by Battles Record.</p>
													<p class="text-xs text-muted-foreground mb-1.5">To uninstall, use your system's package manager:</p>
													<table class="text-[10px] font-mono">
														<tbody>
															<tr><td class="text-foreground pr-3 py-0.5">Windows</td><td class="text-muted-foreground">Settings > Apps, or delete from install folder</td></tr>
															<tr><td class="text-foreground pr-3 py-0.5">macOS</td><td class="text-muted-foreground">brew uninstall {lib.label.toLowerCase()}</td></tr>
															<tr><td class="text-foreground pr-3 py-0.5">Linux</td><td class="text-muted-foreground">apt/dnf/pacman remove {lib.label.toLowerCase()}</td></tr>
														</tbody>
													</table>
												</Tooltip.Content>
											</Tooltip.Portal>
										</Tooltip.Root>
									</Tooltip.Provider>
								{/if}
							{/if}
						</div>
					</div>
				{/each}
			</div>

		{:else}
			<p class="font-mono text-xs text-muted-foreground/70 py-2">Loading library status...</p>
		{/if}
	</Panel>

	<!-- Browser Extension -->
	<Panel title="Browser Extension" icon={Plug}>
		<div class="space-y-4">
			<ExtensionGatewayStatus showHelp />

			<div class="flex items-center justify-between gap-4 border-t border-border pt-4">
				<div>
					<span id="settings-extension-autostart-label" class="block font-mono text-sm text-foreground">
						Start Automatically
					</span>
					<p class="font-mono text-[10px] text-muted-foreground">
						Start the local service without asking when an approved browser connects
					</p>
				</div>
				<Switch
					checked={settingsStore.settings.autoStartForExtension}
					labelledby="settings-extension-autostart-label"
					onCheckedChange={(checked) => {
						settingsStore.settings.autoStartForExtension = checked;
						settingsStore.save();
					}}
				/>
			</div>

			<div class="border-t border-border pt-4">
				<span class="mb-3 block font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
					Approved Browsers
				</span>

				{#if extensionBrowsers.length === 0}
					<p class="py-2 font-mono text-xs text-muted-foreground">
						{extensionGatewayStore.status?.serviceRunning
							? 'No approved browsers yet'
							: 'Start the local service to see approved browsers'}
					</p>
				{:else}
					<div class="space-y-2">
						{#each extensionBrowsers as browser (browser.clientId)}
							<div class="rounded border border-border bg-well">
								<div class="flex items-center justify-between gap-3 px-3 py-2">
									<div class="flex min-w-0 items-center gap-2">
										<StatusDot status={browser.connected ? 'success' : 'offline'} size="sm" />
										<div class="min-w-0">
											<p class="truncate font-mono text-sm text-foreground">{browser.identifier}</p>
											<p class="font-mono text-[10px] text-muted-foreground">
												{browser.connected ? 'Connected' : 'Not connected'}{#if browser.lastConnected}
													&middot; Last connected {formatDate(browser.lastConnected)}{/if}
											</p>
										</div>
									</div>
									<div class="flex shrink-0 items-center gap-1.5">
										{#if browser.connected}
											<Button
												intent="ghost"
												size="sm"
												aria-label="Message log"
												aria-expanded={expandedLogId === browser.clientId}
												onclick={() =>
													(expandedLogId = expandedLogId === browser.clientId ? null : browser.clientId)}
											>
												<ChevronDown
													size={12}
													class={cn(
														'transition-transform',
														expandedLogId === browser.clientId && 'rotate-180'
													)}
												/>
											</Button>
										{/if}
										<Button
											intent="danger"
											size="sm"
											onclick={() => extensionGatewayStore.removeBrowser(browser.clientId)}
										>
											Remove
										</Button>
									</div>
								</div>
								{#if expandedLogId === browser.clientId && browser.connected}
									<div class="border-t border-border px-3 py-2">
										<MessageLogViewer connectionId={browser.clientId} />
									</div>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			</div>
		</div>
	</Panel>


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
					class="w-full rounded border border-border bg-well px-3 py-2 font-mono text-sm text-foreground"
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
					class="w-full rounded border border-border bg-well px-3 py-2 font-mono text-sm text-foreground"
				/>
				{#if editingServer && editUrl !== editingServer.url && editingServer.type === 'remote'}
					<p class="font-mono text-[10px] text-yellow mt-1">
						Changing URL will require re-authentication
					</p>
				{/if}
			</div>
		</form>
	{/snippet}
	{#snippet footer()}
		<div class="flex gap-2">
			<button
				class="flex-1 rounded bg-primary px-4 py-2 font-mono text-sm text-primary-foreground hover:bg-primary/90 transition-colors"
				onclick={handleEditSave}
			>
				Save
			</button>
			<button
				class="rounded border border-border bg-btn-bg px-4 py-2 font-mono text-sm text-foreground hover:bg-hover-wash transition-colors"
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
				<div class="rounded border border-destructive/30 bg-destructive/10 p-3">
					<p class="font-mono text-xs text-destructive">{reconnectError}</p>
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
					class="w-full rounded border border-border bg-well px-3 py-2 font-mono text-sm text-foreground disabled:opacity-50"
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
					class="w-full rounded border border-border bg-well px-3 py-2 font-mono text-sm text-foreground disabled:opacity-50"
				/>
			</div>
		</form>
	{/snippet}
	{#snippet footer()}
		<div class="flex gap-2">
			<button
				class="flex-1 rounded bg-primary px-4 py-2 font-mono text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50 transition-colors"
				disabled={!reconnectUsername || !reconnectPassword || isReconnecting}
				onclick={handleReconnect}
			>
				{isReconnecting ? 'Connecting...' : 'Connect'}
			</button>
			<button
				class="rounded border border-border bg-btn-bg px-4 py-2 font-mono text-sm text-foreground hover:bg-hover-wash transition-colors"
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
				<div class="rounded border border-destructive/30 bg-destructive/10 p-3">
					<p class="font-mono text-xs text-destructive">{addError}</p>
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
					class="w-full rounded border border-border bg-well px-3 py-2 font-mono text-sm text-foreground placeholder:text-muted-foreground disabled:opacity-50"
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
					class="w-full rounded border border-border bg-well px-3 py-2 font-mono text-sm text-foreground placeholder:text-muted-foreground disabled:opacity-50"
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
					class="w-full rounded border border-border bg-well px-3 py-2 font-mono text-sm text-foreground disabled:opacity-50"
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
					class="w-full rounded border border-border bg-well px-3 py-2 font-mono text-sm text-foreground disabled:opacity-50"
				/>
			</div>
		</form>
	{/snippet}
	{#snippet footer()}
		<div class="flex gap-2">
			<button
				type="submit"
				form="add-server-form"
				class="flex-1 rounded bg-primary px-4 py-2 font-mono text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50 transition-colors"
				disabled={!addUrl || !addUsername || !addPassword || isAdding}
			>
				{isAdding ? 'Connecting...' : 'Connect'}
			</button>
			<button
				type="button"
				class="rounded border border-border bg-btn-bg px-4 py-2 font-mono text-sm text-foreground hover:bg-hover-wash transition-colors"
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
				class="flex-1 rounded bg-destructive px-4 py-2 font-mono text-sm text-destructive-foreground hover:bg-destructive/90 transition-colors"
				onclick={handleDelete}
			>
				Remove
			</button>
			<button
				class="rounded border border-border bg-btn-bg px-4 py-2 font-mono text-sm text-foreground hover:bg-hover-wash transition-colors"
				onclick={() => (deleteServer = null)}
			>
				Cancel
			</button>
		</div>
	{/snippet}
</ResponsiveModal>

<!-- Daemon Logs Modal -->
<DaemonLogsModal open={logsModalOpen} onOpenChange={(v) => (logsModalOpen = v)} logs={daemonLogs} />
