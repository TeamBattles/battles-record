<script lang="ts">
	import {
		CheckCircle,
		XCircle,
		AlertTriangle,
		RefreshCw,
		Link2Off,
		Loader2,
		ExternalLink,
		Settings
	} from '@lucide/svelte';
	import type { Platform, PlatformAuth } from '#lib/api/types.ts';
	import { platformAuthStore } from '#lib/stores/platformAuth.svelte.ts';
	import PlatformIcon from './PlatformIcon.svelte';

	interface Props {
		platform: Platform;
		auth: PlatformAuth | null;
		onconnect?: () => void;
		onstartOAuth?: () => void;
		onadvanced?: () => void;
	}

	let { platform, auth, onconnect, onstartOAuth, onadvanced }: Props = $props();

	const platformNames: Record<Platform, string> = {
		twitch: 'Twitch',
		youtube: 'YouTube',
		kick: 'Kick'
	};

	const platformDescriptions: Record<Platform, string> = {
		twitch: 'Connect to record subscriber-only streams and access sub-only VODs',
		youtube: 'Connect to record member-only livestreams and premieres',
		kick: 'Connect to record subscriber-only streams'
	};

	let isTesting = $derived(platformAuthStore.testingPlatform === platform);
	let expiryInfo = $derived(platformAuthStore.getExpiryInfo(auth));
	let isConnected = $derived(auth?.status === 'connected');
	let isExpired = $derived(auth?.status === 'expired');
	let isNotConnected = $derived(!auth || auth.status === 'not_connected');
	let oauthAvailable = $derived(platformAuthStore.isOAuthAvailable(platform));

	async function handleTest() {
		await platformAuthStore.testConnection(platform);
	}

	async function handleDisconnect() {
		await platformAuthStore.disconnect(platform);
	}
</script>

<div class="relative rounded-lg border border-border bg-card h-full">

	<div class="p-4 h-full flex flex-col">
		<!-- Header -->
		<div class="flex items-center justify-between mb-3">
			<div class="flex items-center gap-3">
				<PlatformIcon {platform} class="size-6" />
				<span class="font-mono text-sm uppercase tracking-wider">
					{platformNames[platform]}
				</span>
			</div>

			<!-- Status indicator -->
			{#if isConnected}
				<div class="flex items-center gap-1.5 text-green">
					<span class="size-2 rounded-full bg-green"></span>
					<span class="font-mono text-[10px] uppercase tracking-wider">Connected</span>
				</div>
			{:else if isExpired}
				<div class="flex items-center gap-1.5 text-yellow">
					<AlertTriangle class="size-3" />
					<span class="font-mono text-[10px] uppercase tracking-wider">Expired</span>
				</div>
			{:else}
				<div class="flex items-center gap-1.5 text-muted-foreground">
					<span class="size-2 rounded-full bg-dim"></span>
					<span class="font-mono text-[10px] uppercase tracking-wider">Not Connected</span>
				</div>
			{/if}
		</div>

		<!-- Content -->
		<div class="flex-1 flex flex-col">
			{#if isConnected}
				<!-- Connected state -->
				<div class="space-y-1">
					{#if auth?.username}
						<p class="font-mono text-xs text-muted-foreground">
							Logged in as: <span class="text-foreground">{auth.username}</span>
						</p>
					{/if}

					{#if expiryInfo.text}
						<p
							class="font-mono text-xs {expiryInfo.isExpiringSoon
								? 'text-yellow'
								: 'text-muted-foreground'}"
						>
							{expiryInfo.text}
						</p>
					{/if}
				</div>

				<!-- Actions for connected state - pushed to bottom -->
				<div class="flex flex-col gap-2 sm:flex-row sm:items-center mt-auto pt-3">
					<button
						class="flex items-center justify-center gap-2 rounded border border-border bg-btn-bg px-3 py-1.5 font-mono text-xs hover:bg-hover-wash transition-colors disabled:opacity-50"
						onclick={handleTest}
						disabled={isTesting}
					>
						{#if isTesting}
							<Loader2 class="size-3.5 animate-spin" />
							Testing...
						{:else}
							<CheckCircle class="size-3.5 text-muted-foreground" />
							Test Connection
						{/if}
					</button>
					<button
						class="flex items-center justify-center gap-2 rounded border border-destructive/30 bg-destructive/5 px-3 py-1.5 font-mono text-xs text-destructive hover:bg-destructive/10 transition-colors"
						onclick={handleDisconnect}
					>
						<Link2Off class="size-3.5" />
						Disconnect
					</button>
				</div>
			{:else if isExpired}
				<!-- Expired state -->
				<div class="space-y-1">
					{#if auth?.username}
						<p class="font-mono text-xs text-muted-foreground">
							Was connected as: <span class="text-muted-foreground">{auth.username}</span>
						</p>
					{/if}
					<p class="font-mono text-xs text-yellow">Token has expired. Please reconnect.</p>
				</div>

				<!-- Actions pushed to bottom -->
				<div class="mt-auto pt-3 flex flex-col items-center gap-2">
					{#if oauthAvailable}
						<!-- Primary: OAuth reconnect button (centered) -->
						<button
							class="flex items-center justify-center gap-2 rounded bg-primary px-4 py-2 font-mono text-xs text-primary-foreground transition-colors hover:bg-primary/90"
							onclick={() => onstartOAuth?.()}
						>
							<ExternalLink class="size-3.5" />
							Reconnect with {platformNames[platform]}
						</button>

						<!-- Secondary options -->
						<div class="flex items-center justify-center gap-3">
							<button
								class="font-mono text-[10px] text-muted-foreground underline hover:text-text-2"
								onclick={() => onconnect?.()}
							>
								Enter token manually
							</button>
							<span class="text-dim">|</span>
							<button
								class="flex items-center gap-1 font-mono text-[10px] text-muted-foreground hover:text-text-2"
								onclick={() => onadvanced?.()}
							>
								<Settings class="size-3" />
								Advanced
							</button>
							<span class="text-dim">|</span>
							<button
								class="flex items-center gap-1 font-mono text-[10px] text-muted-foreground hover:text-text-2"
								onclick={handleDisconnect}
							>
								<Link2Off class="size-3" />
								Remove
							</button>
						</div>
					{:else}
						<button
							class="flex items-center justify-center gap-2 rounded border border-yellow/30 bg-yellow/10 px-3 py-1.5 font-mono text-xs text-yellow hover:bg-yellow/20 transition-colors"
							onclick={() => onconnect?.()}
						>
							<RefreshCw class="size-3.5" />
							Reconnect
						</button>
						<button
							class="font-mono text-[10px] text-muted-foreground underline hover:text-text-2"
							onclick={handleDisconnect}
						>
							Remove credentials
						</button>
					{/if}
				</div>
			{:else}
				<!-- Not connected state -->
				<p class="font-mono text-xs text-muted-foreground">
					{platformDescriptions[platform]}
				</p>

				<!-- Connect button and secondary options pushed to bottom -->
				<div class="mt-auto pt-3 flex flex-col items-center gap-2">
					{#if oauthAvailable}
						<!-- Primary: OAuth button (centered) -->
						<button
							class="flex items-center justify-center gap-2 rounded bg-primary px-4 py-2 font-mono text-xs text-primary-foreground transition-colors hover:bg-primary/90"
							onclick={() => onstartOAuth?.()}
						>
							<ExternalLink class="size-3.5" />
							Connect with {platformNames[platform]}
						</button>

						<!-- Secondary options -->
						<div class="flex items-center justify-center gap-3">
							<button
								class="font-mono text-[10px] text-muted-foreground underline hover:text-text-2"
								onclick={() => onconnect?.()}
							>
								Enter token manually
							</button>
							<span class="text-dim">|</span>
							<button
								class="flex items-center gap-1 font-mono text-[10px] text-muted-foreground hover:text-text-2"
								onclick={() => onadvanced?.()}
							>
								<Settings class="size-3" />
								Advanced
							</button>
						</div>
					{:else}
						<!-- Only manual entry available -->
						<button
							class="flex items-center justify-center gap-2 rounded border border-border bg-btn-bg px-4 py-2 font-mono text-xs transition-colors hover:bg-hover-wash"
							onclick={() => onconnect?.()}
						>
							Connect
						</button>
					{/if}
				</div>
			{/if}
		</div>
	</div>
</div>
