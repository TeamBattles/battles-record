<script lang="ts">
	import ResponsiveModal from './ResponsiveModal.svelte';
	import Button from './ui/Button.svelte';
	import { ServerOff, Power, Loader2 } from 'lucide-svelte';
	import { connectionStore } from '$lib';

	interface Props {
		open: boolean;
		onOpenChange: (open: boolean) => void;
	}

	let { open, onOpenChange }: Props = $props();
	let isStarting = $state(false);
	let error = $state<string | null>(null);

	// Reset error when dialog opens
	$effect(() => {
		if (open) {
			error = null;
		}
	});

	async function handleStart() {
		isStarting = true;
		error = null;

		const success = await connectionStore.connectToLocal();

		if (success) {
			onOpenChange(false);
		} else {
			error = connectionStore.error ?? 'Failed to start service';
		}

		isStarting = false;
	}

	function handleCancel() {
		if (!isStarting) {
			onOpenChange(false);
		}
	}
</script>

<ResponsiveModal {open} {onOpenChange} title="Local Service Offline">
	<div class="flex flex-col items-center text-center">
		<div class="mb-4 rounded-full bg-zinc-800 p-4">
			<ServerOff size={32} class="text-zinc-400" />
		</div>
		<p class="mb-2 font-mono text-sm text-zinc-300">
			The local recording service is not running.
		</p>
		<p class="mb-4 font-mono text-xs text-zinc-500">
			Start the service to enable recording functionality.
		</p>
		{#if error}
			<p class="font-mono text-xs text-red-400">{error}</p>
		{/if}
	</div>

	{#snippet footer()}
		<div class="flex justify-end gap-3">
			<Button intent="secondary" onclick={handleCancel} disabled={isStarting}>
				Cancel
			</Button>
			<Button intent="primary" onclick={handleStart} disabled={isStarting}>
				{#if isStarting}
					<Loader2 size={14} class="mr-1 animate-spin" />
					Starting...
				{:else}
					<Power size={14} class="mr-1" />
					Start Service
				{/if}
			</Button>
		</div>
	{/snippet}
</ResponsiveModal>
