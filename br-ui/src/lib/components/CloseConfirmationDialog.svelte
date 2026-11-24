<script lang="ts">
	import { AlertTriangle } from '@lucide/svelte';
	import { listen } from '@tauri-apps/api/event';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { exit } from '@tauri-apps/plugin-process';
	import { connectionStore, settingsStore } from '#lib';
	import { onMount } from 'svelte';
	import LuxuryCornerSlivers from '#lib/components/ui/LuxuryCornerSlivers.svelte';
	import Checkbox from '#lib/components/ui/Checkbox.svelte';

	let showDialog = $state(false);
	let choice = $state<'background' | 'shutdown'>('background');
	let rememberChoice = $state(false);

	onMount(() => {
		const unlisten = listen('close-requested', async () => {
			// Wait for settings to be loaded if not already
			if (!settingsStore.isLoaded) {
				console.warn('[CloseDialog] Settings not loaded yet, showing dialog');
				showDialog = true;
				return;
			}

			const savedAction = settingsStore.settings.closeAction;
			console.log('[CloseDialog] closeAction:', savedAction);

			if (savedAction && savedAction !== 'ask') {
				// Auto-apply saved preference
				if (savedAction === 'background') {
					await getCurrentWindow().hide();
				} else {
					// Hide window immediately for responsive feel
					await getCurrentWindow().hide();
					// Then stop daemon in background before exiting
					await connectionStore.stopLocalDaemon();
					await exit(0);
				}
			} else {
				// Show dialog to ask user
				showDialog = true;
			}
		});

		return () => {
			unlisten.then((fn) => fn());
		};
	});

	async function confirm() {
		if (rememberChoice) {
			// Wait for settings to be saved before proceeding
			await settingsStore.setCloseActionAsync(choice);
		}

		if (choice === 'background') {
			await getCurrentWindow().hide();
		} else {
			// Hide window immediately for responsive feel
			await getCurrentWindow().hide();
			// Then stop daemon in background before exiting
			await connectionStore.stopLocalDaemon();
			await exit(0);
		}

		showDialog = false;
	}

	function cancel() {
		showDialog = false;
	}
</script>

{#if showDialog}
	<div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/70 p-4">
		<div class="animate-luxury-fade-scale relative rounded-xl w-full max-w-md border border-border bg-popover p-6 shadow-2xl">
			<LuxuryCornerSlivers />

			<div class="mb-4 flex items-center gap-3">
				<div class="rounded-full bg-yellow/20 p-2">
					<AlertTriangle class="size-6 text-yellow" />
				</div>
				<h2 class="font-display text-xl uppercase tracking-tight">Local Service Running</h2>
			</div>

			<p class="mb-6 font-mono text-sm text-muted-foreground">
				The local service is currently running. What would you like to do?
			</p>

			<div class="mb-6 space-y-3">
				<label
					class="flex cursor-pointer items-start gap-3 rounded border border-border p-3 transition-colors hover:border-muted-foreground"
					class:border-primary={choice === 'background'}
				>
					<input
						type="radio"
						name="close-choice"
						value="background"
						bind:group={choice}
						class="mt-1"
					/>
					<div>
						<p class="font-mono text-sm text-foreground">Keep running in background</p>
						<p class="font-mono text-xs text-muted-foreground">Service continues, app minimizes to tray</p>
					</div>
				</label>

				<label
					class="flex cursor-pointer items-start gap-3 rounded border border-border p-3 transition-colors hover:border-muted-foreground"
					class:border-primary={choice === 'shutdown'}
				>
					<input
						type="radio"
						name="close-choice"
						value="shutdown"
						bind:group={choice}
						class="mt-1"
					/>
					<div>
						<p class="font-mono text-sm text-foreground">Shut down service and exit</p>
						<p class="font-mono text-xs text-muted-foreground">Stops all recordings</p>
					</div>
				</label>
			</div>

			<div class="mb-6">
				<Checkbox
					bind:checked={rememberChoice}
					label="Remember my choice"
					labelClass="text-muted-foreground"
				/>
			</div>

			<div class="flex justify-end gap-3">
				<button
					class="rounded border border-border bg-btn-bg px-4 py-2 font-mono text-sm text-foreground transition-colors hover:bg-hover-wash"
					onclick={cancel}
				>
					Cancel
				</button>
				<button
					class="rounded bg-primary px-4 py-2 font-mono text-sm text-primary-foreground transition-colors hover:bg-primary/90"
					onclick={confirm}
				>
					Confirm
				</button>
			</div>
		</div>
	</div>
{/if}
