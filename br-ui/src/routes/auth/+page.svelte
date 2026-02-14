<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Key, Shield, Tv, Info } from 'lucide-svelte';
	import { open } from '@tauri-apps/plugin-shell';
	import { start, cancel, onUrl } from '@fabianlars/tauri-plugin-oauth';
	import { connectionStore } from '$lib/stores/connection.svelte';
	import { platformAuthStore } from '$lib/stores/platformAuth.svelte';
	import { toastStore } from '$lib/stores/toast.svelte';
	import Panel from '$lib/components/Panel.svelte';
	import PlatformAuthCard from '$lib/components/PlatformAuthCard.svelte';
	import ManualTokenInput from '$lib/components/ManualTokenInput.svelte';
	import OAuthPendingOverlay from '$lib/components/OAuthPendingOverlay.svelte';
	import AdvancedOAuthModal from '$lib/components/AdvancedOAuthModal.svelte';
	import { untrack } from 'svelte';
	import type { Platform } from '$lib/api/types';
	import { OAUTH_RESPONSE_HTML } from '$lib/utils/oauth-response';

	let selectedPlatformForInput = $state<Platform>('twitch');
	let advancedModalPlatform = $state<Platform | null>(null);
	let oauthServerPort = $state<number | null>(null);
	let oauthUnlisten: (() => void) | null = null;

	// Fixed ports for OAuth callbacks - these must be registered with OAuth providers
	const OAUTH_PORTS = [17927, 17928, 17929, 17930];

	// Cleanup OAuth server on unmount
	onDestroy(() => {
		cleanupOAuthServer();
	});

	// Reload platform auth when server changes or connection is established
	$effect(() => {
		const serverId = connectionStore.activeServerId;
		if (connectionStore.isConnected && serverId) {
			untrack(() => {
				platformAuthStore.load(serverId);
			});
		}
	});

	async function cleanupOAuthServer() {
		if (oauthUnlisten) {
			oauthUnlisten();
			oauthUnlisten = null;
		}
		if (oauthServerPort !== null) {
			try {
				await cancel(oauthServerPort);
			} catch (e) {
				console.warn('[OAuth] Failed to cancel OAuth server:', e);
			}
			oauthServerPort = null;
		}
	}

	// Open URL in system browser with error handling
	async function openInBrowser(url: string): Promise<boolean> {
		console.log('[OAuth] Opening URL in browser:', url);

		// Try Tauri's shell open first (works in Tauri app)
		try {
			await open(url);
			console.log('[OAuth] Browser opened successfully (Tauri)');
			return true;
		} catch (tauriError) {
			console.warn('[OAuth] Tauri open() failed, trying window.open():', tauriError);
		}

		// Fallback to window.open() for web mode
		try {
			const popup = window.open(url, '_blank');
			if (popup) {
				console.log('[OAuth] Browser opened successfully (Web)');
				return true;
			} else {
				console.warn('[OAuth] Popup was blocked');
				return false;
			}
		} catch (webError) {
			console.error('[OAuth] window.open() also failed:', webError);
			return false;
		}
	}

	function handleConnectPlatform(platform: Platform) {
		selectedPlatformForInput = platform;
		// Scroll to manual input section
		const inputSection = document.getElementById('manual-token-section');
		if (inputSection) {
			inputSection.scrollIntoView({ behavior: 'smooth' });
		}
	}

	async function handleStartOAuth(
		platform: Platform,
		options?: { clientId?: string; clientSecret?: string }
	) {
		try {
			// Clean up any previous OAuth server before starting a new one
			// This prevents port conflicts and stale state issues
			await cleanupOAuthServer();

			// Start the OAuth localhost server on a fixed port
			// These ports must be registered with OAuth providers
			console.log('[OAuth] Starting OAuth server on fixed ports:', OAUTH_PORTS);
			const port = await start({
				ports: OAUTH_PORTS,
				response: OAUTH_RESPONSE_HTML
			});
			console.log('[OAuth] OAuth server started on port:', port);
			oauthServerPort = port;

			// Set up listener for OAuth callback
			oauthUnlisten = await onUrl(async (callbackUrl) => {
				console.log('[OAuth] Received callback URL:', callbackUrl);
				await handleOAuthCallback(callbackUrl);
			});

			// Small delay to ensure server is fully ready to accept connections
			await new Promise((resolve) => setTimeout(resolve, 100));

			// Construct redirect URI using the localhost server
			const redirectUri = `http://localhost:${port}`;
			console.log('[OAuth] Using redirect URI:', redirectUri);

			// Start OAuth flow with backend
			const authUrl = await platformAuthStore.startOAuth(platform, redirectUri, options);
			if (authUrl) {
				console.log('[OAuth] Received auth URL:', authUrl);
				const success = await openInBrowser(authUrl);
				if (!success) {
					platformAuthStore.setBrowserFailed(true);
					toastStore.error('Failed to open browser. Use the link below to authenticate.');
				}
			} else {
				console.error('[OAuth] No auth URL received');
				await cleanupOAuthServer();
			}
		} catch (e) {
			console.error('[OAuth] Failed to start OAuth:', e);
			toastStore.error('Failed to start OAuth flow');
			await cleanupOAuthServer();
		}
	}

	async function handleOAuthCallback(callbackUrl: string) {
		try {
			const url = new URL(callbackUrl);
			const code = url.searchParams.get('code');
			const state = url.searchParams.get('state');
			const error = url.searchParams.get('error');
			const errorDescription = url.searchParams.get('error_description');

			// Handle OAuth error from provider
			if (error) {
				console.error('[OAuth] Provider returned error:', error, errorDescription);
				toastStore.error(errorDescription || error);
				await cleanupOAuthServer();
				platformAuthStore.cancelOAuth();
				return;
			}

			const platform = platformAuthStore.oauthPending;

			if (code && state && platform) {
				console.log('[OAuth] Completing OAuth flow...');
				await platformAuthStore.completeOAuth(platform, code, state);
			} else {
				console.error('[OAuth] Missing code, state, or platform');
				toastStore.error('Invalid OAuth callback - missing required parameters');
			}
		} catch (e) {
			console.error('[OAuth] Failed to handle callback:', e);
			toastStore.error('Failed to process OAuth callback');
		} finally {
			// Delay cleanup to ensure HTTP response is fully sent to browser
			await new Promise((resolve) => setTimeout(resolve, 1500));
			await cleanupOAuthServer();
		}
	}

	async function handleRetryOpenBrowser() {
		const url = platformAuthStore.oauthUrl;
		if (url) {
			await openInBrowser(url);
		}
	}

	async function handleCancelOAuth() {
		await cleanupOAuthServer();
		platformAuthStore.cancelOAuth();
	}

	function handleShowAdvanced(platform: Platform) {
		advancedModalPlatform = platform;
	}

	function handleCloseAdvanced() {
		advancedModalPlatform = null;
	}

	function handleAdvancedConnect(options: { clientId: string; clientSecret?: string }) {
		if (advancedModalPlatform) {
			handleStartOAuth(advancedModalPlatform, options);
			advancedModalPlatform = null;
		}
	}
</script>

<div class="space-y-6">
	<!-- Page Header -->
	<div class="space-y-2">
		<div class="flex items-center gap-3">
			<Key class="size-5 text-zinc-500" />
			<h1 class="font-display text-3xl sm:text-4xl tracking-tight uppercase">
				Platform Authentication
			</h1>
		</div>
	</div>

	<!-- Description Panel -->
	<Panel title="About Platform Authentication" icon={Shield}>
		<p class="font-mono text-xs text-zinc-500 leading-relaxed">
			Authenticate with streaming platforms to record subscriber-only content. Once connected, the
			daemon will use your credentials to access restricted streams that require a subscription or
			membership.
		</p>
	</Panel>

	<!-- Connection Warning -->
	{#if !connectionStore.isConnected}
		<div class="rounded-lg border border-amber-500/30 bg-amber-500/5 p-4">
			<p class="font-mono text-xs text-amber-400">
				Not connected to a server. Connect to manage platform authentication.
			</p>
		</div>
	{:else if platformAuthStore.isLoading}
		<div class="flex items-center gap-2 text-zinc-500">
			<div
				class="size-4 animate-spin rounded-full border-2 border-zinc-500 border-t-transparent"
			></div>
			<span class="font-mono text-xs">Loading platform authentication...</span>
		</div>
	{:else if platformAuthStore.error}
		<div class="rounded-lg border border-red-500/30 bg-red-500/5 p-4">
			<p class="font-mono text-xs text-red-400">{platformAuthStore.error}</p>
		</div>
	{:else}
		<!-- Platform Cards Grid -->
		<div class="space-y-4">
			<div class="flex items-center gap-2">
				<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">Platforms</span>
				{#if platformAuthStore.connectedCount > 0}
					<span
						class="rounded bg-emerald-500/20 px-1.5 py-0.5 font-mono text-[10px] text-emerald-400"
					>
						{platformAuthStore.connectedCount} connected
					</span>
				{/if}
			</div>

			<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
				<PlatformAuthCard
					platform="twitch"
					auth={platformAuthStore.twitch}
					onconnect={() => handleConnectPlatform('twitch')}
					onstartOAuth={() => handleStartOAuth('twitch')}
					onadvanced={() => handleShowAdvanced('twitch')}
				/>
				<PlatformAuthCard
					platform="youtube"
					auth={platformAuthStore.youtube}
					onconnect={() => handleConnectPlatform('youtube')}
					onstartOAuth={() => handleStartOAuth('youtube')}
					onadvanced={() => handleShowAdvanced('youtube')}
				/>
				<PlatformAuthCard
					platform="kick"
					auth={platformAuthStore.kick}
					onconnect={() => handleConnectPlatform('kick')}
					onstartOAuth={() => handleStartOAuth('kick')}
					onadvanced={() => handleShowAdvanced('kick')}
				/>
			</div>
		</div>

		<!-- Manual Token Input Section -->
		<div id="manual-token-section">
			<ManualTokenInput initialPlatform={selectedPlatformForInput} />
		</div>

		<!-- What This Enables Section -->
		<Panel title="What This Enables" icon={Info}>
			<div class="space-y-6">
				<!-- Twitch -->
				<div class="space-y-2">
					<div class="flex items-center gap-2">
						<Tv class="size-4 text-purple-500" />
						<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">Twitch</span>
					</div>
					<ul class="list-disc list-inside space-y-1 text-zinc-500 ml-6">
						<li class="font-mono text-xs">Record subscriber-only streams</li>
						<li class="font-mono text-xs">Access sub-only VOD archives</li>
						<li class="font-mono text-xs">Higher quality options for some streams</li>
					</ul>
				</div>

				<!-- YouTube -->
				<div class="space-y-2">
					<div class="flex items-center gap-2">
						<Tv class="size-4 text-red-500" />
						<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">YouTube</span>
					</div>
					<ul class="list-disc list-inside space-y-1 text-zinc-500 ml-6">
						<li class="font-mono text-xs">Record member-only livestreams</li>
						<li class="font-mono text-xs">Access member-only premieres</li>
						<li class="font-mono text-xs">Download membership-restricted content</li>
					</ul>
				</div>

				<!-- Kick -->
				<div class="space-y-2">
					<div class="flex items-center gap-2">
						<Tv class="size-4 text-green-500" />
						<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">Kick</span>
					</div>
					<ul class="list-disc list-inside space-y-1 text-zinc-500 ml-6">
						<li class="font-mono text-xs">Record subscriber-only streams</li>
						<li class="font-mono text-xs">Access subscriber-exclusive content</li>
					</ul>
				</div>
			</div>
		</Panel>

		<!-- Security Note -->
		<div class="rounded-lg border border-zinc-700 bg-zinc-800/30 p-4">
			<div class="flex items-start gap-3">
				<Shield class="size-5 text-zinc-500 mt-0.5 flex-shrink-0" />
				<div class="space-y-1">
					<p class="font-mono text-xs uppercase tracking-wider text-zinc-400">Security Note</p>
					<p class="font-mono text-xs text-zinc-500 leading-relaxed">
						Your tokens are stored in the daemon's configuration file. Never share your access
						tokens with anyone. Tokens may expire and require re-authentication. Use refresh tokens
						when available for automatic renewal.
					</p>
				</div>
			</div>
		</div>
	{/if}
</div>

{#if platformAuthStore.oauthPending}
	<OAuthPendingOverlay
		platform={platformAuthStore.oauthPending}
		authUrl={platformAuthStore.oauthUrl}
		browserFailed={platformAuthStore.oauthBrowserFailed}
		oncancel={handleCancelOAuth}
		onRetry={handleRetryOpenBrowser}
	/>
{/if}

{#if advancedModalPlatform}
	<AdvancedOAuthModal
		platform={advancedModalPlatform}
		onclose={handleCloseAdvanced}
		onconnect={handleAdvancedConnect}
	/>
{/if}
