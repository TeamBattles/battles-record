<script lang="ts">
	import { toastStore } from '$lib/stores/toast.svelte';

	// Expose store state for testing via data attributes
</script>

<div data-testid="toast-wrapper">
	<span data-testid="toast-count">{toastStore.toasts.length}</span>

	{#each toastStore.toasts as toast, i}
		<div data-testid="toast-{i}">
			<span data-testid="toast-{i}-id">{toast.id}</span>
			<span data-testid="toast-{i}-type">{toast.type}</span>
			<span data-testid="toast-{i}-message">{toast.message}</span>
			<span data-testid="toast-{i}-duration">{toast.duration}</span>
		</div>
	{/each}
</div>
