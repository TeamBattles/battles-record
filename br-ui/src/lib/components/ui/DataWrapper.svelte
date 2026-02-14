<script lang="ts">
	import type { Snippet } from 'svelte';
	import { connectionStore } from '$lib/stores/connection.svelte';
	import LoadingSpinner from './LoadingSpinner.svelte';
	import ErrorMessage from './ErrorMessage.svelte';

	interface Props {
		isLoading: boolean;
		error: string | null;
		loadingText?: string;
		children: Snippet;
		empty?: Snippet;
		isEmpty?: boolean;
	}

	let {
		isLoading,
		error,
		loadingText = 'Loading...',
		children,
		empty,
		isEmpty = false
	}: Props = $props();
</script>

{#if !connectionStore.isConnected}
	<ErrorMessage message="Not connected to a server." variant="warning" />
{:else if isLoading}
	<LoadingSpinner text={loadingText} />
{:else if error}
	<ErrorMessage message={error} />
{:else if isEmpty && empty}
	{@render empty()}
{:else}
	{@render children()}
{/if}
