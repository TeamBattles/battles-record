<script lang="ts">
	import StatusDot from './ui/StatusDot.svelte';
	import { extensionGatewayStore } from '#lib/stores/extensionGateway.svelte.ts';
	import { FRONT_DOOR_PORT_RANGE } from '#lib/utils/constants.ts';
	import { cn } from '#lib/utils/cn.ts';

	interface Props {
		showHelp?: boolean;
		class?: string;
	}

	let { showHelp = false, class: className }: Props = $props();

	const status = $derived(extensionGatewayStore.status);
</script>

<div class={cn('space-y-2', className)}>
	<div class="flex items-center gap-2">
		<StatusDot status={status?.listening ? 'success' : 'warning'} size="sm" />
		<span class="font-mono text-xs text-foreground">
			{#if status?.listening}
				Listening for Battles Replay on port <span class="tabular-nums">{status.port}</span>
			{:else}
				Ports {FRONT_DOOR_PORT_RANGE} are in use
			{/if}
		</span>
	</div>
	<div class="flex items-center gap-2">
		<StatusDot status={status?.serviceRunning ? 'success' : 'offline'} size="sm" />
		<span class="font-mono text-xs text-text-2">
			Local service: {status?.serviceRunning ? 'Running' : 'Stopped'}
		</span>
	</div>
	{#if showHelp}
		<p class="font-mono text-[10px] leading-relaxed text-muted-foreground">
			Install Battles Replay, open its Battles Record settings, click Connect, then approve the
			request here.
		</p>
	{/if}
</div>
