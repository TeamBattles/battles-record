<script lang="ts">
	import { toastStore } from '$lib/stores/toast.svelte';
	import Toast from './Toast.svelte';

	function handleDismiss(id: string) {
		toastStore.dismiss(id);
	}
</script>

{#if toastStore.toasts.length > 0}
	<div
		class="fixed bottom-4 right-4 left-4 sm:left-auto sm:w-80 z-50 flex flex-col gap-2"
		aria-live="polite"
	>
		{#each toastStore.toasts as toast (toast.id)}
			<Toast id={toast.id} type={toast.type} message={toast.message} onDismiss={handleDismiss} />
		{/each}
	</div>
{/if}
