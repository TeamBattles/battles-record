<script lang="ts">
	import { X, Folder, FileVideo } from 'lucide-svelte';
	import CornerBrackets from './ui/CornerBrackets.svelte';

	interface Props {
		open: boolean;
		recordingPath: string;
		outputFile?: string;
		onClose: () => void;
		onOpenRecording: () => void;
		onOpenLibrary: () => void;
	}

	let { open, recordingPath, outputFile, onClose, onOpenRecording, onOpenLibrary }: Props =
		$props();

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onClose();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div class="absolute inset-0" onclick={onClose}></div>

		<div class="relative w-full max-w-md border border-border bg-card shadow-xl">
			<CornerBrackets />

			<!-- Header -->
			<div class="flex items-center justify-between border-b border-border px-4 py-3">
				<h3 class="font-mono text-sm uppercase tracking-wider text-foreground">Open Location</h3>
				<button class="p-1 hover:bg-muted rounded transition-colors" onclick={onClose}>
					<X class="w-4 h-4 text-muted-foreground" />
				</button>
			</div>

			<!-- Content -->
			<div class="p-4 space-y-3">
				<p class="font-mono text-xs text-muted-foreground mb-4">
					Choose which folder to open for this recording:
				</p>

				<!-- Recording folder option -->
				<button
					class="w-full flex items-start gap-3 p-3 rounded border border-border bg-muted/50 hover:bg-muted hover:border-border transition-colors text-left"
					onclick={() => {
						onOpenRecording();
						onClose();
					}}
				>
					<div class="p-2 rounded bg-muted">
						<Folder class="w-5 h-5 text-amber-400" />
					</div>
					<div class="flex-1 min-w-0">
						<div class="font-mono text-sm text-foreground">Recording Folder</div>
						<div class="font-mono text-[10px] text-muted-foreground mt-1">
							Raw segment files (.ts) from the original recording
						</div>
						<div class="font-mono text-[10px] text-muted-foreground/70 mt-1 truncate" title={recordingPath}>
							{recordingPath}
						</div>
					</div>
				</button>

				<!-- Library folder option -->
				{#if outputFile}
					<button
						class="w-full flex items-start gap-3 p-3 rounded border border-border bg-muted/50 hover:bg-muted hover:border-border transition-colors text-left"
						onclick={() => {
							onOpenLibrary();
							onClose();
						}}
					>
						<div class="p-2 rounded bg-muted">
							<FileVideo class="w-5 h-5 text-emerald-400" />
						</div>
						<div class="flex-1 min-w-0">
							<div class="font-mono text-sm text-foreground">Library Folder</div>
							<div class="font-mono text-[10px] text-muted-foreground mt-1">
								Post-processed output file ready for playback
							</div>
							<div class="font-mono text-[10px] text-muted-foreground/70 mt-1 truncate" title={outputFile}>
								{outputFile}
							</div>
						</div>
					</button>
				{/if}
			</div>
		</div>
	</div>
{/if}
