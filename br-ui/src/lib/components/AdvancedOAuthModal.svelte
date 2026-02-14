<script lang="ts">
	import { ResponsiveModal } from '$lib';
	import type { Platform } from '$lib/api/types';
	import Button from './ui/Button.svelte';
	import Input from './ui/Input.svelte';
	import { Eye, Settings, Info, ExternalLink } from 'lucide-svelte';
	import PlatformIcon from './PlatformIcon.svelte';

	interface Props {
		platform: Platform;
		onclose: () => void;
		onconnect: (options: { clientId: string; clientSecret?: string }) => void;
	}

	let { platform, onclose, onconnect }: Props = $props();

	let clientId = $state('');
	let clientSecret = $state('');
	let showSecret = $state(false);

	const platformNames: Record<Platform, string> = {
		twitch: 'Twitch',
		youtube: 'YouTube',
		kick: 'Kick'
	};

	const platformDocs: Record<Platform, string> = {
		twitch: 'https://dev.twitch.tv/console/apps',
		youtube: 'https://console.cloud.google.com/apis/credentials',
		kick: 'https://kick.com/settings/developer'
	};

	const platformInfo: Record<Platform, { needsSecret: boolean; note: string }> = {
		twitch: {
			needsSecret: false,
			note: 'Client secret is optional for public PKCE clients. Leave empty to use PKCE flow.'
		},
		youtube: {
			needsSecret: false,
			note: 'Client secret is optional for public PKCE clients. Leave empty to use PKCE flow.'
		},
		kick: {
			needsSecret: true,
			note: 'Client secret is required for Kick OAuth. Create your app at kick.com/settings/developer.'
		}
	};

	const canConnect = $derived(clientId.trim().length > 0);

	function handleConnect() {
		if (!canConnect) return;
		onconnect({
			clientId: clientId.trim(),
			clientSecret: clientSecret.trim() || undefined
		});
	}

	function handleOpenChange(open: boolean) {
		if (!open) onclose();
	}
</script>

<ResponsiveModal open={true} onOpenChange={handleOpenChange} title="Advanced OAuth Settings">
	{#snippet children()}
		<div class="space-y-4">
			<!-- Platform indicator -->
			<div class="flex items-center gap-3 pb-2 border-b border-border">
				<PlatformIcon {platform} class="size-5" />
				<span class="font-mono text-sm uppercase tracking-wider">{platformNames[platform]}</span>
			</div>

			<!-- Info box -->
			<div class="rounded border border-border bg-muted/30 p-3">
				<div class="flex items-start gap-2">
					<Info class="size-4 text-muted-foreground mt-0.5 flex-shrink-0" />
					<div class="space-y-2">
						<p class="font-mono text-xs text-muted-foreground leading-relaxed">
							Use your own OAuth application credentials. This is for advanced users who want to use
							their own registered application instead of the bundled one.
						</p>
						<a
							href={platformDocs[platform]}
							target="_blank"
							rel="noopener noreferrer"
							class="inline-flex items-center gap-1 font-mono text-[10px] text-emerald-400 hover:text-emerald-300"
						>
							<ExternalLink class="size-3" />
							Open {platformNames[platform]} Developer Console
						</a>
					</div>
				</div>
			</div>

			<!-- Client ID -->
			<div>
				<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Client ID <span class="text-red-400">*</span>
				</span>
				<Input
					type="text"
					placeholder="Enter your OAuth client ID"
					bind:value={clientId}
					autocomplete="off"
				/>
			</div>

			<!-- Client Secret -->
			<div>
				<span class="block font-mono text-[10px] uppercase tracking-wider text-muted-foreground mb-2">
					Client Secret
					{#if platformInfo[platform].needsSecret}
						<span class="text-red-400">*</span>
					{:else}
						<span class="text-muted-foreground/70">(optional)</span>
					{/if}
				</span>
				<div class="relative">
					<Input
						type={showSecret ? 'text' : 'password'}
						class="pr-10"
						placeholder={platformInfo[platform].needsSecret
							? 'Enter your client secret'
							: 'Leave empty for PKCE flow'}
						bind:value={clientSecret}
						autocomplete="off"
					/>
					<button
						type="button"
						class="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-muted-foreground hover:text-foreground"
						onclick={() => (showSecret = !showSecret)}
					>
						<Eye size={16} class={showSecret ? '' : 'opacity-50'} />
					</button>
				</div>
				<p class="font-mono text-[10px] text-muted-foreground mt-1.5 leading-relaxed">
					{platformInfo[platform].note}
				</p>
			</div>
		</div>
	{/snippet}

	{#snippet footer()}
		<div class="flex gap-2">
			<Button
				type="button"
				intent="primary"
				fullWidth
				disabled={!canConnect}
				onclick={handleConnect}
			>
				<ExternalLink class="size-3.5 mr-2" />
				Connect with Custom Credentials
			</Button>
			<Button type="button" onclick={onclose}>Cancel</Button>
		</div>
	{/snippet}
</ResponsiveModal>
