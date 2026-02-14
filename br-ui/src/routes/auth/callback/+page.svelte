<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { platformAuthStore } from '$lib/stores/platformAuth.svelte';
	import { Loader2, CheckCircle, XCircle } from 'lucide-svelte';

	let status = $state<'processing' | 'success' | 'error'>('processing');
	let errorMessage = $state<string | null>(null);

	onMount(async () => {
		const searchParams = $page.url.searchParams;
		const code = searchParams.get('code');
		const state = searchParams.get('state');
		const error = searchParams.get('error');
		const errorDescription = searchParams.get('error_description');

		// Handle OAuth error from provider
		if (error) {
			status = 'error';
			errorMessage = errorDescription || error;
			return;
		}

		// Validate required params
		if (!code || !state) {
			status = 'error';
			errorMessage = 'Missing authorization code or state';
			return;
		}

		// Get the platform from stored state
		const platform = platformAuthStore.oauthPending;
		if (!platform) {
			status = 'error';
			errorMessage = 'No OAuth flow in progress';
			return;
		}

		// Complete the OAuth flow
		const success = await platformAuthStore.completeOAuth(platform, code, state);

		if (success) {
			status = 'success';
			// Redirect back to auth page after short delay
			setTimeout(() => {
				goto('/auth');
			}, 2000);
		} else {
			status = 'error';
			errorMessage = 'Failed to complete authentication';
		}
	});
</script>

<div class="flex min-h-screen items-center justify-center p-4">
	<div class="w-full max-w-md rounded-lg border border-border bg-card p-6 text-center">
		{#if status === 'processing'}
			<Loader2 class="mx-auto size-12 animate-spin text-zinc-400" />
			<h1 class="mt-4 font-mono text-lg uppercase tracking-wider">Completing Authentication</h1>
			<p class="mt-2 text-sm text-zinc-500">Please wait...</p>
		{:else if status === 'success'}
			<CheckCircle class="mx-auto size-12 text-emerald-400" />
			<h1 class="mt-4 font-mono text-lg uppercase tracking-wider">Connected Successfully</h1>
			<p class="mt-2 text-sm text-zinc-500">Redirecting back to settings...</p>
		{:else}
			<XCircle class="mx-auto size-12 text-red-400" />
			<h1 class="mt-4 font-mono text-lg uppercase tracking-wider">Authentication Failed</h1>
			<p class="mt-2 text-sm text-red-400">{errorMessage}</p>
			<a
				href="/auth"
				class="mt-4 inline-block rounded border border-border bg-input px-4 py-2 font-mono text-xs transition-colors hover:bg-muted"
			>
				Return to Settings
			</a>
		{/if}
	</div>
</div>
