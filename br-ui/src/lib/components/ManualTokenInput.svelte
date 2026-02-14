<script lang="ts">
	import { ChevronDown, ChevronUp, Save, ExternalLink, Eye, EyeOff } from 'lucide-svelte';
	import type { Platform, SetPlatformAuthRequest } from '$lib/api/types';
	import { platformAuthStore } from '$lib/stores/platformAuth.svelte';
	import { api } from '$lib/api';
	import CornerBrackets from './ui/CornerBrackets.svelte';

	interface Props {
		initialPlatform?: Platform;
	}

	let { initialPlatform = 'twitch' }: Props = $props();

	let isExpanded = $state(false);
	let selectedPlatform = $state<Platform>('twitch');
	let accessToken = $state('');
	let refreshToken = $state('');
	let username = $state('');
	let cookieContent = $state('');
	let showAccessToken = $state(false);
	let showRefreshToken = $state(false);
	let isSaving = $state(false);
	let saveError = $state<string | null>(null);

	const platforms: { value: Platform; label: string }[] = [
		{ value: 'twitch', label: 'Twitch' },
		{ value: 'youtube', label: 'YouTube' },
		{ value: 'kick', label: 'Kick' }
	];

	// Check if the selected platform uses cookie auth instead of token auth
	const isCookieAuth = $derived(selectedPlatform === 'youtube');

	const tokenInstructions: Record<
		Platform,
		{ title: string; steps: string[]; link?: string; isCookie?: boolean; script?: string }
	> = {
		twitch: {
			title: 'How to get your Twitch auth token',
			steps: [
				'Log in to Twitch.tv in your browser',
				'Open browser developer tools (F12)',
				'Go to Application tab → Cookies → https://www.twitch.tv',
				'Find the "auth-token" cookie and copy its Value'
			]
		},
		youtube: {
			title: 'How to export your YouTube cookies',
			steps: [
				'Log in to YouTube in your browser',
				'Open browser developer tools (F12)',
				'Go to Console tab and paste the script below',
				'If prompted, type "allow pasting" first, then paste again',
				'Press Enter, then copy the output',
				'Paste it in the textarea below'
			],
			isCookie: true,
			script: `(function(){const c=document.cookie.split(';').map(x=>{const[n,...v]=x.trim().split('=');return '.youtube.com\\tTRUE\\t/\\tFALSE\\t0\\t'+n+'\\t'+v.join('=')}).join('\\n');console.log('# Netscape HTTP Cookie File\\n'+c)})();`
		},
		kick: {
			title: 'How to get your Kick authentication token',
			steps: [
				'Log in to Kick.com in your browser',
				'Open browser developer tools (F12)',
				'Go to Network tab, filter by XHR requests',
				'Look for requests to api.kick.com and find the Authorization: Bearer header',
				'Copy the token value (without "Bearer " prefix)'
			]
		}
	};

	$effect(() => {
		selectedPlatform = initialPlatform;
		// Clear form when platform changes
		accessToken = '';
		refreshToken = '';
		username = '';
		cookieContent = '';
		saveError = null;
	});

	async function handleSave() {
		saveError = null;

		// Validate input based on auth type
		if (isCookieAuth) {
			if (!cookieContent.trim()) {
				return;
			}
		} else {
			if (!accessToken.trim()) {
				return;
			}
		}

		isSaving = true;

		try {
			if (isCookieAuth && selectedPlatform === 'youtube') {
				// Use YouTube-specific cookie API
				await api.setYouTubeCookies(cookieContent.trim());
				// Refresh the platform auth store to show updated status
				await platformAuthStore.load();
				// Clear form
				cookieContent = '';
				isExpanded = false;
			} else {
				// Standard token auth for other platforms
				const credentials: SetPlatformAuthRequest = {
					access_token: accessToken.trim(),
					refresh_token: refreshToken.trim() || undefined,
					username: username.trim() || undefined
				};

				const success = await platformAuthStore.setCredentials(selectedPlatform, credentials);

				if (success) {
					// Clear form
					accessToken = '';
					refreshToken = '';
					username = '';
					isExpanded = false;
				}
			}
		} catch (e) {
			saveError = e instanceof Error ? e.message : 'Failed to save credentials';
		}

		isSaving = false;
	}

	function handleToggle() {
		isExpanded = !isExpanded;
	}
</script>

<div class="relative border border-border bg-card">
	<CornerBrackets />

	<!-- Toggle header -->
	<button
		class="w-full flex items-center justify-between px-4 py-3 hover:bg-muted/30 transition-colors"
		onclick={handleToggle}
	>
		<span class="font-mono text-xs uppercase tracking-wider text-zinc-400">
			Advanced: Manual Token Entry
		</span>
		{#if isExpanded}
			<ChevronUp class="size-4 text-zinc-500" />
		{:else}
			<ChevronDown class="size-4 text-zinc-500" />
		{/if}
	</button>

	<!-- Expanded content -->
	{#if isExpanded}
		<div class="border-t border-border/60 p-4 space-y-4">
			<!-- Platform selector -->
			<div class="space-y-1.5">
				<label
					for="platform-select"
					class="font-mono text-[10px] uppercase tracking-wider text-zinc-500"
				>
					Platform
				</label>
				<select
					id="platform-select"
					class="w-full rounded border border-border bg-input px-3 py-2 font-mono text-xs"
					bind:value={selectedPlatform}
				>
					{#each platforms as platform}
						<option value={platform.value}>{platform.label}</option>
					{/each}
				</select>
			</div>

			{#if isCookieAuth}
				<!-- Cookie Content Textarea (for YouTube) -->
				<div class="space-y-1.5">
					<label
						for="cookie-content"
						class="font-mono text-[10px] uppercase tracking-wider text-zinc-500"
					>
						Cookie File Content <span class="text-red-400">*</span>
					</label>
					<textarea
						id="cookie-content"
						class="w-full rounded border border-border bg-input px-3 py-2 font-mono text-xs min-h-[120px] resize-y"
						placeholder="# Netscape HTTP Cookie File&#10;# Paste the entire cookie file content here..."
						bind:value={cookieContent}
					></textarea>
					<p class="text-[10px] text-zinc-500">
						Run the script in DevTools Console on YouTube.com, then paste the output above.
					</p>
				</div>
			{:else}
				<!-- Access Token -->
				<div class="space-y-1.5">
					<label
						for="access-token"
						class="font-mono text-[10px] uppercase tracking-wider text-zinc-500"
					>
						Access Token <span class="text-red-400">*</span>
					</label>
					<div class="relative">
						<input
							id="access-token"
							type={showAccessToken ? 'text' : 'password'}
							class="w-full rounded border border-border bg-input px-3 py-2 pr-10 font-mono text-xs"
							placeholder="Enter your access token"
							bind:value={accessToken}
						/>
						<button
							type="button"
							class="absolute right-2 top-1/2 -translate-y-1/2 p-1 hover:bg-muted rounded transition-colors"
							onclick={() => (showAccessToken = !showAccessToken)}
						>
							{#if showAccessToken}
								<EyeOff class="size-3.5 text-zinc-500" />
							{:else}
								<Eye class="size-3.5 text-zinc-500" />
							{/if}
						</button>
					</div>
				</div>

				<!-- Refresh Token -->
				<div class="space-y-1.5">
					<label
						for="refresh-token"
						class="font-mono text-[10px] uppercase tracking-wider text-zinc-500"
					>
						Refresh Token <span class="text-zinc-600">(optional)</span>
					</label>
					<div class="relative">
						<input
							id="refresh-token"
							type={showRefreshToken ? 'text' : 'password'}
							class="w-full rounded border border-border bg-input px-3 py-2 pr-10 font-mono text-xs"
							placeholder="Enter refresh token for auto-renewal"
							bind:value={refreshToken}
						/>
						<button
							type="button"
							class="absolute right-2 top-1/2 -translate-y-1/2 p-1 hover:bg-muted rounded transition-colors"
							onclick={() => (showRefreshToken = !showRefreshToken)}
						>
							{#if showRefreshToken}
								<EyeOff class="size-3.5 text-zinc-500" />
							{:else}
								<Eye class="size-3.5 text-zinc-500" />
							{/if}
						</button>
					</div>
				</div>

				<!-- Username -->
				<div class="space-y-1.5">
					<label
						for="username"
						class="font-mono text-[10px] uppercase tracking-wider text-zinc-500"
					>
						Username <span class="text-zinc-600">(optional)</span>
					</label>
					<input
						id="username"
						type="text"
						class="w-full rounded border border-border bg-input px-3 py-2 font-mono text-xs"
						placeholder="Your username on the platform"
						bind:value={username}
					/>
				</div>
			{/if}

			<!-- Instructions -->
			<div class="rounded border border-border/60 bg-muted/30 p-3 space-y-2">
				<p class="font-mono text-[10px] uppercase tracking-wider text-zinc-400">
					{tokenInstructions[selectedPlatform].title}
				</p>
				<ol class="list-decimal list-inside space-y-1">
					{#each tokenInstructions[selectedPlatform].steps as step}
						<li class="font-mono text-xs text-zinc-500">{step}</li>
					{/each}
				</ol>
				{#if tokenInstructions[selectedPlatform].link}
					<a
						href={tokenInstructions[selectedPlatform].link}
						target="_blank"
						rel="noopener noreferrer"
						class="inline-flex items-center gap-1 font-mono text-xs text-blue-400 hover:text-blue-300 transition-colors"
					>
						<ExternalLink class="size-3" />
						Open {selectedPlatform === 'twitch'
							? 'Token Generator'
							: selectedPlatform === 'youtube'
								? 'Cookie Extension'
								: 'Link'}
					</a>
				{/if}
				{#if tokenInstructions[selectedPlatform].script}
					<div class="mt-2 space-y-1">
						<p class="font-mono text-[10px] uppercase tracking-wider text-zinc-400">
							Script to paste in Console:
						</p>
						<div class="relative">
							<pre
								class="rounded border border-border/60 bg-zinc-900 p-2 font-mono text-[10px] text-zinc-300 overflow-x-auto whitespace-pre-wrap break-all">{tokenInstructions[selectedPlatform].script}</pre>
							<button
								type="button"
								class="absolute right-1 top-1 rounded border border-border bg-input px-2 py-0.5 font-mono text-[10px] hover:bg-muted transition-colors"
								onclick={() =>
									navigator.clipboard.writeText(
										tokenInstructions[selectedPlatform].script || ''
									)}
							>
								Copy
							</button>
						</div>
					</div>
				{/if}
			</div>

			<!-- Error message -->
			{#if saveError}
				<div class="rounded border border-red-500/30 bg-red-500/10 p-3">
					<p class="font-mono text-xs text-red-400">{saveError}</p>
				</div>
			{/if}

			<!-- Save button -->
			<div class="flex justify-end">
				<button
					class="flex items-center gap-2 rounded border border-border bg-input px-4 py-2 font-mono text-xs hover:bg-muted transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
					onclick={handleSave}
					disabled={(isCookieAuth ? !cookieContent.trim() : !accessToken.trim()) || isSaving}
				>
					<Save class="size-3.5" />
					{isSaving ? 'Saving...' : isCookieAuth ? 'Save Cookies' : 'Save Token'}
				</button>
			</div>
		</div>
	{/if}
</div>
