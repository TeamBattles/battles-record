<script lang="ts">
	import { tv, type VariantProps } from 'tailwind-variants';
	import PlatformIcon from './PlatformIcon.svelte';
	import { cn } from '$lib/utils/cn';

	const avatar = tv({
		base: 'relative rounded-full bg-zinc-800 flex items-center justify-center border border-border',
		variants: {
			size: {
				sm: 'size-8',
				md: 'size-12',
				lg: 'size-16',
				xl: 'size-24'
			}
		},
		defaultVariants: {
			size: 'md'
		}
	});

	const iconSizes = {
		sm: 'w-4 h-4',
		md: 'w-6 h-6',
		lg: 'w-8 h-8',
		xl: 'w-12 h-12'
	} as const;

	// Badge sizes relative to avatar size
	const badgeSizes = {
		sm: 'size-4',
		md: 'size-5',
		lg: 'size-6',
		xl: 'size-8'
	} as const;

	const badgeIconSizes = {
		sm: 'w-2.5 h-2.5',
		md: 'w-3 h-3',
		lg: 'w-3.5 h-3.5',
		xl: 'w-5 h-5'
	} as const;

	type AvatarVariants = VariantProps<typeof avatar>;

	interface Props extends AvatarVariants {
		/** Image source URL */
		src?: string | null;
		/** Alt text for the image */
		alt: string;
		/** Platform for fallback icon */
		platform: 'twitch' | 'youtube' | 'kick';
		/** Show platform badge overlay (only when image is shown) */
		showBadge?: boolean;
		/** Additional classes */
		class?: string;
	}

	let { src, alt, platform, size = 'md', showBadge = true, class: className }: Props = $props();

	let imageError = $state(false);

	// Show fallback if no src or if image failed to load
	const showFallback = $derived(!src || imageError);

	function handleImageError() {
		imageError = true;
	}

	// Reset error state when src changes
	$effect(() => {
		if (src) {
			imageError = false;
		}
	});
</script>

<div class={cn(avatar({ size }), className)} title={alt}>
	{#if showFallback}
		<PlatformIcon {platform} class={iconSizes[size ?? 'md']} />
	{:else}
		<img
			{src}
			{alt}
			class="size-full object-cover rounded-full"
			onerror={handleImageError}
			loading="lazy"
		/>
		<!-- Platform badge overlay -->
		{#if showBadge}
			<div
				class={cn(
					'absolute -bottom-0.5 -right-0.5 rounded-full bg-zinc-900 border border-zinc-700 flex items-center justify-center',
					badgeSizes[size ?? 'md']
				)}
			>
				<PlatformIcon {platform} class={badgeIconSizes[size ?? 'md']} />
			</div>
		{/if}
	{/if}
</div>
