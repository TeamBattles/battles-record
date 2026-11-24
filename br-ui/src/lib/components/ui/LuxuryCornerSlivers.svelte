<script lang="ts">
	import { cn } from '#lib/utils/cn.ts';

	/**
	 * The corner-accent border shared with teambattles.gg and Battles Replay: a corner-anchored
	 * accent gradient painted into a 1px ring that follows the host's border-radius, so it wraps
	 * rounded corners cleanly at every Appearance > Rounding setting.
	 *
	 * Host contract:
	 * - The host is `relative` and normally a rounded 1px-`border` box; the ring overlays that
	 *   border (inset -1px) where `overflow-clip-margin` is supported, otherwise it sits inside it.
	 * - A host that must clip full-bleed children uses `overflow-clip [overflow-clip-margin:1px]`,
	 *   never `overflow-hidden`, which clips at the padding edge and swallows the ring.
	 * - Borderless or partially bordered hosts pass `flush` so the ring stays inside the surface.
	 */
	type Corners = 'tl' | 'tr' | 'bl' | 'br' | 'tl-br' | 'all';
	type Intensity = 'subtle' | 'default' | 'strong';
	type Length = 'xs' | 'sm' | 'md' | 'lg' | 'xl' | '2xl';

	interface Props {
		corners?: Corners;
		intensity?: Intensity;
		length?: Length;
		/** Fade in on the parent's `group-hover` (the parent must carry `group`). */
		hoverFadeIn?: boolean;
		flush?: boolean;
		class?: string;
	}

	let {
		corners = 'tl-br',
		intensity = 'default',
		length = 'xl',
		hoverFadeIn = false,
		flush = false,
		class: className
	}: Props = $props();

	const RADII: Record<Length, number> = { xs: 40, sm: 48, md: 64, lg: 96, xl: 128, '2xl': 176 };
	const UNIFORM: Record<Intensity, number> = { subtle: 0.3, default: 0.4, strong: 0.6 };
	// tl-br leads with a stronger top-left and trails with a softer bottom-right.
	const PAIRED: Record<Intensity, [number, number]> = {
		subtle: [0.4, 0.2],
		default: [0.6, 0.4],
		strong: [0.6, 0.3]
	};
	const POS = { tl: '0 0', tr: '100% 0', bl: '0 100%', br: '100% 100%' } as const;
	type CornerKey = keyof typeof POS;

	const layer = (corner: CornerKey, alpha: number, radius: number) =>
		`radial-gradient(circle ${radius}px at ${POS[corner]}, color-mix(in oklab, var(--primary) ${Math.round(alpha * 100)}%, transparent), transparent)`;

	const backgroundImage = $derived.by(() => {
		const radius = RADII[length];
		if (corners === 'tl-br') {
			const [lead, trail] = PAIRED[intensity];
			return `${layer('tl', lead, radius)}, ${layer('br', trail, radius)}`;
		}
		const alpha = UNIFORM[intensity];
		const list: CornerKey[] = corners === 'all' ? ['tl', 'tr', 'bl', 'br'] : [corners];
		return list.map((c) => layer(c, alpha, radius)).join(', ');
	});
</script>

<!-- m-0 keeps a parent's space-y-* margins off the absolutely positioned ring; z-10 keeps later
     positioned siblings from painting over it. -->
<div
	class={cn(
		'luxury-corner-rim pointer-events-none absolute z-10 m-0 rounded-[inherit] border border-transparent',
		hoverFadeIn && 'opacity-0 transition-opacity duration-300 group-hover:opacity-100',
		className
	)}
	data-flush={flush ? '' : undefined}
	style:background-image={backgroundImage}
	aria-hidden="true"
></div>

<style>
	.luxury-corner-rim {
		inset: 0;
		background-origin: border-box;
		background-clip: border-box;
		/* Keep only the 1px border ring: border-box layer minus padding-box layer. */
		-webkit-mask:
			linear-gradient(#fff 0 0) padding-box,
			linear-gradient(#fff 0 0);
		-webkit-mask-composite: xor;
		mask:
			linear-gradient(#fff 0 0) padding-box,
			linear-gradient(#fff 0 0);
		mask-composite: exclude;
	}

	@supports (overflow-clip-margin: 1px) {
		.luxury-corner-rim:not([data-flush]) {
			inset: -1px;
		}
	}
</style>
