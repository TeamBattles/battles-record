<script lang="ts">
	import { Icon, Tv, type IconNode } from '@lucide/svelte';

	interface Props {
		platform: 'twitch' | 'youtube' | 'kick';
		class?: string;
	}

	let { platform, class: className = 'w-5 h-5' }: Props = $props();

	// Lucide 1.0 dropped its brand icons; these are the last Lucide-published paths (ISC)
	const twitchNode: IconNode = [['path', { d: 'M21 2H3v16h5v4l4-4h5l4-4V2zm-10 9V7m5 4V7' }]];
	const youtubeNode: IconNode = [
		[
			'path',
			{
				d: 'M2.5 17a24.12 24.12 0 0 1 0-10 2 2 0 0 1 1.4-1.4 49.56 49.56 0 0 1 16.2 0A2 2 0 0 1 21.5 7a24.12 24.12 0 0 1 0 10 2 2 0 0 1-1.4 1.4 49.55 49.55 0 0 1-16.2 0A2 2 0 0 1 2.5 17'
			}
		],
		['path', { d: 'm10 15 5-3-5-3z' }]
	];

	const platformColors = {
		twitch: 'text-purple',
		youtube: 'text-destructive',
		kick: 'text-green'
	};
</script>

{#if platform === 'twitch'}
	<Icon iconNode={twitchNode} class="{className} {platformColors.twitch}" />
{:else if platform === 'youtube'}
	<Icon iconNode={youtubeNode} class="{className} {platformColors.youtube}" />
{:else}
	<Tv class="{className} {platformColors.kick}" />
{/if}
