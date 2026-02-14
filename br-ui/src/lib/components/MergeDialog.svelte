<script lang="ts">
	import { ResponsiveModal } from '$lib';
	import { toastStore } from '$lib/stores/toast.svelte';
	import { api } from '$lib/api/client';
	import { extractErrorMessage } from '$lib/utils/errors';
	import { Loader2, GitMerge } from 'lucide-svelte';
	import Input from './ui/Input.svelte';

	interface Props {
		open: boolean;
		sourcePlatform: string;
		sourceChannel: string;
		onMerged?: () => void;
	}

	let {
		open = $bindable(false),
		sourcePlatform,
		sourceChannel,
		onMerged
	}: Props = $props();

	let targetChannel = $state('');
	let isMerging = $state(false);

	function handleClose() {
		if (!isMerging) {
			open = false;
			targetChannel = '';
		}
	}

	async function handleMerge() {
		const target = targetChannel.trim();
		if (!target) return;

		isMerging = true;
		try {
			const result = await api.mergeDownloads(sourcePlatform, sourceChannel, target);
			toastStore.success(`Merged ${result.files_moved} file(s) from "${sourceChannel}" into "${target}"`);
			open = false;
			targetChannel = '';
			onMerged?.();
		} catch (e) {
			toastStore.error(extractErrorMessage(e, 'Failed to merge downloads'));
		} finally {
			isMerging = false;
		}
	}
</script>

<ResponsiveModal {open} onOpenChange={(v) => { if (!v) handleClose(); }} title="Merge Channel">
	<div class="space-y-4">
		<p class="font-mono text-xs text-zinc-400">
			Move all downloads from <span class="text-zinc-100">{sourceChannel}</span> into another channel
			on <span class="text-zinc-100">{sourcePlatform}</span>.
		</p>

		<!-- Source (read-only) -->
		<div>
			<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-1.5 block">
				Source
			</span>
			<div class="flex items-center gap-2 rounded border border-border bg-zinc-800/50 px-3 py-2">
				<GitMerge class="size-3.5 text-zinc-500" />
				<span class="font-mono text-xs text-zinc-300">{sourceChannel}</span>
				<span class="rounded bg-zinc-700 px-1.5 py-0.5 font-mono text-[9px] uppercase text-zinc-400">
					{sourcePlatform}
				</span>
			</div>
		</div>

		<!-- Target channel input -->
		<div>
			<label
				for="merge-target"
				class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-1.5 block"
			>
				Target Channel
			</label>
			<Input
				id="merge-target"
				placeholder="Enter target channel name..."
				bind:value={targetChannel}
				disabled={isMerging}
			/>
			<p class="font-mono text-[10px] text-zinc-600 mt-1.5">
				Downloads will be moved to the target channel's directory.
			</p>
		</div>
	</div>

	{#snippet footer()}
		<div class="flex items-center gap-2">
			<button
				class="flex-1 rounded border border-border bg-input px-3 py-2 font-mono text-xs hover:bg-muted transition-colors"
				onclick={handleClose}
				disabled={isMerging}
			>
				Cancel
			</button>
			<button
				class="flex-1 rounded border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 font-mono text-xs text-emerald-400 hover:bg-emerald-500/20 transition-colors flex items-center justify-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
				onclick={handleMerge}
				disabled={isMerging || !targetChannel.trim()}
			>
				{#if isMerging}
					<Loader2 class="size-3.5 animate-spin" />
				{/if}
				Merge
			</button>
		</div>
	{/snippet}
</ResponsiveModal>
