<script lang="ts">
	import { ResponsiveModal } from '$lib';
	import { channelsStore } from '$lib/stores/channels.svelte';
	import { toastStore } from '$lib/stores/toast.svelte';
	import { HardDrive, Clock, Loader2 } from 'lucide-svelte';
	import type { Channel } from '$lib/api/types';

	interface Props {
		channel: Channel;
		onclose: () => void;
	}

	let { channel, onclose }: Props = $props();

	let quotaGb = $state<number | undefined>(undefined);
	let retentionDays = $state<number | undefined>(undefined);
	let isSubmitting = $state(false);

	// Sync form state from channel prop
	$effect(() => {
		quotaGb = channel.quota_gb;
		retentionDays = channel.retention_days;
	});

	const quotaPresets = [
		{ label: 'Unlimited', value: undefined },
		{ label: '10 GB', value: 10 },
		{ label: '25 GB', value: 25 },
		{ label: '50 GB', value: 50 },
		{ label: '100 GB', value: 100 },
		{ label: '250 GB', value: 250 }
	];

	const retentionPresets = [
		{ label: 'Unlimited', value: undefined },
		{ label: '7 days', value: 7 },
		{ label: '14 days', value: 14 },
		{ label: '30 days', value: 30 },
		{ label: '60 days', value: 60 },
		{ label: '90 days', value: 90 }
	];

	async function handleSave() {
		isSubmitting = true;
		try {
			const success = await channelsStore.updateChannel(channel.id, {
				// Send null for 0/undefined to clear the value (unlimited)
				quota_gb: quotaGb || undefined,
				retention_days: retentionDays || undefined
			});
			if (success) {
				toastStore.success(`Quota settings updated for ${channel.name}`);
				onclose();
			} else {
				toastStore.error(channelsStore.error ?? 'Failed to update quota');
			}
		} catch (e) {
			toastStore.error('Failed to update quota settings');
		} finally {
			isSubmitting = false;
		}
	}
</script>

<ResponsiveModal open={true} onOpenChange={(open) => !open && onclose()} title="Channel Quota">
	<div class="space-y-6">
		<!-- Channel info -->
		<div class="flex items-center gap-2">
			<span class="font-mono text-sm text-zinc-100">{channel.name}</span>
			<span
				class="rounded bg-zinc-200 px-1.5 py-0.5 font-mono text-[9px] uppercase text-zinc-600 dark:bg-zinc-800 dark:text-zinc-500"
			>
				{channel.platform}
			</span>
		</div>

		<!-- Storage quota -->
		<div>
			<label
				class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 flex items-center gap-1.5 mb-2"
			>
				<HardDrive class="size-3" />
				Max Storage
			</label>
			<div class="grid grid-cols-3 gap-2">
				{#each quotaPresets as preset (preset.value)}
					<button
						type="button"
						class="rounded border px-3 py-2 font-mono text-xs transition-colors {quotaGb ===
						preset.value
							? 'border-emerald-500 bg-emerald-500/10 text-emerald-400'
							: 'border-border bg-input hover:bg-muted text-zinc-300'}"
						onclick={() => (quotaGb = preset.value)}
					>
						{preset.label}
					</button>
				{/each}
			</div>
			<p class="font-mono text-[10px] text-zinc-500 mt-2">
				Auto-delete oldest recordings when this limit is exceeded.
			</p>
		</div>

		<!-- Retention days -->
		<div>
			<label
				class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 flex items-center gap-1.5 mb-2"
			>
				<Clock class="size-3" />
				Retention Period
			</label>
			<div class="grid grid-cols-3 gap-2">
				{#each retentionPresets as preset (preset.value)}
					<button
						type="button"
						class="rounded border px-3 py-2 font-mono text-xs transition-colors {retentionDays ===
						preset.value
							? 'border-emerald-500 bg-emerald-500/10 text-emerald-400'
							: 'border-border bg-input hover:bg-muted text-zinc-300'}"
						onclick={() => (retentionDays = preset.value)}
					>
						{preset.label}
					</button>
				{/each}
			</div>
			<p class="font-mono text-[10px] text-zinc-500 mt-2">
				Auto-delete recordings older than this period.
			</p>
		</div>
	</div>

	{#snippet footer()}
		<div class="flex items-center gap-2">
			<button
				class="flex-1 rounded border border-border bg-input px-3 py-2 font-mono text-xs hover:bg-muted transition-colors"
				onclick={onclose}
			>
				Cancel
			</button>
			<button
				class="flex-1 rounded border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 font-mono text-xs text-emerald-400 hover:bg-emerald-500/20 transition-colors flex items-center justify-center gap-2"
				onclick={handleSave}
				disabled={isSubmitting}
			>
				{#if isSubmitting}
					<Loader2 class="size-3.5 animate-spin" />
				{/if}
				Save
			</button>
		</div>
	{/snippet}
</ResponsiveModal>
