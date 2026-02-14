<script lang="ts">
	import { Twitch, Youtube, Tv } from 'lucide-svelte';

	interface Props {
		platform: 'twitch' | 'youtube' | 'kick';
		class?: string;
	}

	let { platform, class: className = 'w-5 h-5' }: Props = $props();

	const platformColors = {
		twitch: 'text-purple-500',
		youtube: 'text-red-500',
		kick: 'text-green-500'
	};
</script>

{#if platform === 'twitch'}
	<Twitch class="{className} {platformColors.twitch}" />
{:else if platform === 'youtube'}
	<Youtube class="{className} {platformColors.youtube}" />
{:else}
	<Tv class="{className} {platformColors.kick}" />
{/if}
