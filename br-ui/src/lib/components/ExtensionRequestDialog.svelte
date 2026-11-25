<script lang="ts">
	import { Plug } from '@lucide/svelte';
	import ResponsiveModal from './ResponsiveModal.svelte';
	import Button from './ui/Button.svelte';
	import Checkbox from './ui/Checkbox.svelte';
	import { extensionGatewayStore } from '#lib/stores/extensionGateway.svelte.ts';

	let alwaysStart = $state(false);

	const request = $derived(extensionGatewayStore.pending);
	const needsService = $derived(request?.kind === 'start' || request?.kind === 'start_and_allow');
	const browserName = $derived(request?.identifier ?? null);

	const heading = $derived.by(() => {
		switch (request?.kind) {
			case 'start':
				return 'Start the local service?';
			case 'allow':
				return `Allow ${browserName ?? 'this browser'} to connect?`;
			default:
				return `${browserName ?? 'A browser'} wants to connect`;
		}
	});

	const primaryLabel = $derived.by(() => {
		if (extensionGatewayStore.isResponding) return needsService ? 'Starting...' : 'Allowing...';
		switch (request?.kind) {
			case 'start':
				return 'Start';
			case 'allow':
				return 'Allow';
			default:
				return 'Start & connect';
		}
	});

	function allow() {
		const remember = needsService && alwaysStart;
		alwaysStart = false;
		void extensionGatewayStore.respond('allow', remember);
	}

	function deny() {
		if (extensionGatewayStore.isResponding) return;
		alwaysStart = false;
		void extensionGatewayStore.respond('deny');
	}
</script>

<ResponsiveModal
	open={request !== null}
	onOpenChange={(open) => {
		if (!open) deny();
	}}
	title="Battles Replay"
>
	<div class="space-y-4">
		<div class="flex items-start gap-3">
			<div class="rounded-full bg-primary/15 p-2">
				<Plug class="size-5 text-primary" />
			</div>
			<div class="min-w-0 space-y-2">
				<h2 class="font-display text-xl uppercase tracking-wide text-foreground">{heading}</h2>
				{#if needsService}
					<p class="font-mono text-xs leading-relaxed text-muted-foreground">
						Battles Replay needs the local service on this PC. Downloads and channels you add from
						the browser are saved here.
					</p>
				{:else}
					<p class="font-mono text-xs leading-relaxed text-muted-foreground">
						Allowed browsers can download videos and add channels to this PC's local service. You
						can remove a browser any time in Settings.
					</p>
				{/if}
				{#if request?.origin}
					<p class="font-mono text-[10px] break-all text-dim">{request.origin}</p>
				{/if}
			</div>
		</div>

		{#if needsService}
			<Checkbox
				bind:checked={alwaysStart}
				label="Always start it when Battles Replay connects"
				labelClass="font-mono text-xs text-text-2"
			/>
		{/if}
	</div>

	{#snippet footer()}
		<div class="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
			<Button intent="secondary" onclick={deny} disabled={extensionGatewayStore.isResponding}>
				Not now
			</Button>
			<Button intent="primary" onclick={allow} disabled={extensionGatewayStore.isResponding}>
				{primaryLabel}
			</Button>
		</div>
	{/snippet}
</ResponsiveModal>
