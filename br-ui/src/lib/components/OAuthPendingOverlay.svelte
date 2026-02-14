<script lang="ts">
	import { Loader2, ExternalLink, X, Copy } from 'lucide-svelte';
	import type { Platform } from '$lib/api/types';
	import { toastStore } from '$lib/stores/toast.svelte';
	import CornerBrackets from './ui/CornerBrackets.svelte';

	interface Props {
		platform: Platform;
		authUrl?: string | null;
		browserFailed?: boolean;
		oncancel: () => void;
		onRetry?: () => void;
	}

	let { platform, authUrl, browserFailed = false, oncancel, onRetry }: Props = $props();

	const platformNames: Record<Platform, string> = {
		twitch: 'Twitch',
		youtube: 'YouTube',
		kick: 'Kick'
	};

	let elapsedSeconds = $state(0);
	let showTimeout = $state(false);
	let showTrouble = $state(false);

	// Track elapsed time
	$effect(() => {
		const interval = setInterval(() => {
			elapsedSeconds++;
			if (elapsedSeconds >= 300) {
				// 5 minutes
				showTimeout = true;
			}
			if (elapsedSeconds >= 10 && !showTrouble) {
				showTrouble = true;
			}
		}, 1000);

		return () => clearInterval(interval);
	});

	function handleCopyUrl() {
		if (authUrl) {
			navigator.clipboard.writeText(authUrl).then(() => {
				toastStore.success('URL copied to clipboard');
			});
		}
	}

	function formatTime(seconds: number): string {
		const mins = Math.floor(seconds / 60);
		const secs = seconds % 60;
		return `${mins}:${secs.toString().padStart(2, '0')}`;
	}
</script>

<div
	class="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm"
	role="dialog"
	aria-modal="true"
	aria-labelledby="oauth-title"
>
	<div class="relative mx-4 w-full max-w-md border border-border bg-card p-6">
		<CornerBrackets size="lg" />

		<!-- Close button -->
		<button
			class="absolute right-4 top-4 rounded p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground z-10"
			onclick={oncancel}
			aria-label="Cancel"
		>
			<X class="size-5" />
		</button>

		<div class="flex flex-col items-center space-y-6 text-center">
			<!-- Platform indicator -->
			<div class="rounded-full bg-muted p-4">
				<Loader2 class="size-12 animate-spin text-emerald-400" />
			</div>

			<!-- Title -->
			<div>
				<h2 id="oauth-title" class="font-mono text-lg uppercase tracking-wider">
					Connecting to {platformNames[platform]}
				</h2>
				<p class="mt-2 text-sm text-muted-foreground">
					Please complete the authentication in your browser
				</p>
			</div>

			<!-- Browser hint -->
			<div
				class="flex items-center gap-2 rounded bg-muted px-4 py-2 font-mono text-xs text-muted-foreground"
			>
				<ExternalLink class="size-4" />
				<span>A browser window should have opened</span>
			</div>

			<!-- Elapsed time -->
			<p class="font-mono text-xs text-muted-foreground">
				Elapsed: {formatTime(elapsedSeconds)}
			</p>

			<!-- Trouble opening fallback - shows immediately if browser failed, otherwise after 10 seconds -->
			{#if (browserFailed || showTrouble) && authUrl}
				<div class="w-full space-y-2 rounded border border-border bg-muted p-3">
					<p class="font-mono text-xs text-muted-foreground">Browser didn't open?</p>
					<div class="flex justify-center gap-2">
						<button
							class="flex items-center gap-1.5 rounded border border-border bg-input px-3 py-1.5 font-mono text-xs transition-colors hover:bg-muted"
							onclick={handleCopyUrl}
						>
							<Copy class="size-3" />
							Copy URL
						</button>
						{#if onRetry}
							<button
								class="flex items-center gap-1.5 rounded border border-border bg-input px-3 py-1.5 font-mono text-xs transition-colors hover:bg-muted"
								onclick={onRetry}
							>
								<ExternalLink class="size-3" />
								Open Again
							</button>
						{/if}
					</div>
				</div>
			{/if}

			{#if showTimeout}
				<!-- Timeout message -->
				<div class="rounded border border-amber-500/30 bg-amber-500/10 p-4">
					<p class="text-sm text-amber-400">
						Taking longer than expected? Make sure you complete the login in your browser.
					</p>
					<div class="mt-3 flex justify-center gap-2">
						<button
							class="rounded border border-border bg-input px-3 py-1.5 font-mono text-xs transition-colors hover:bg-muted"
							onclick={() => (showTimeout = false)}
						>
							Keep waiting
						</button>
						<button
							class="rounded border border-red-500/30 bg-red-500/10 px-3 py-1.5 font-mono text-xs text-red-400 transition-colors hover:bg-red-500/20"
							onclick={oncancel}
						>
							Cancel
						</button>
					</div>
				</div>
			{:else}
				<!-- Cancel button -->
				<button
					class="rounded border border-border bg-input px-4 py-2 font-mono text-xs transition-colors hover:bg-muted"
					onclick={oncancel}
				>
					Cancel
				</button>
			{/if}
		</div>
	</div>
</div>
