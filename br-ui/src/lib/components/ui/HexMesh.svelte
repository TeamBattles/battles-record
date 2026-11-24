<script lang="ts">
	import { cn } from '#lib/utils/cn.ts';

	/**
	 * HexMesh, ported from teambattles.gg: a faint blue honeycomb that pools in the top-right corner
	 * of headers and heroes, with an optional accent ripple expanding from that corner.
	 *
	 * Host contract: the host is `relative overflow-hidden` and its real content sits above this
	 * layer (`relative z-10`). The layer is absolutely inset, pointer-events-none and aria-hidden.
	 * Resting opacity is theme-driven through `--hexmesh-opacity` (app.css). The ripple stops under
	 * both the OS reduced-motion setting and Appearance > Reduce motion.
	 *
	 * The hairline color is `--hexmesh-line`: the cool blue #96bae8 shared by all three apps, which
	 * Bone re-points to a deeper blue because the shared one disappears on cream. The tile SVG is a
	 * mask (like the ripple's) so that color can come from a token.
	 */
	interface Props {
		ripple?: boolean;
		class?: string;
	}

	let { ripple = false, class: className }: Props = $props();
</script>

<div class={cn('hexmesh pointer-events-none absolute inset-0 z-0', className)} aria-hidden="true">
	<div class="hexmesh-tiles"></div>
	{#if ripple}
		<div class="hexmesh-ripple"></div>
	{/if}
</div>

<style>
	@property --hexr {
		syntax: '<number>';
		inherits: false;
		initial-value: 0;
	}

	/* The 55% mix matches the fill-opacity the tile SVG carried before it became a mask. */
	.hexmesh-tiles {
		position: absolute;
		inset: 0;
		opacity: var(--hexmesh-opacity, 0.42);
		background-color: color-mix(in srgb, var(--hexmesh-line, #96bae8) 55%, transparent);
		-webkit-mask-image:
			url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='28' height='49' viewBox='0 0 28 49'%3E%3Cg fill='%23ffffff' fill-rule='evenodd'%3E%3Cpath d='M13.99 9.25l13 7.5v15l-13 7.5L1 31.75v-15l12.99-7.5zM3 17.9v11.7l10.99 6.34 11-6.35V17.9l-11-6.34L3 17.9zM0 15l12.98-7.5V0h-2v6.35L0 12.69v2.3zm0 18.5L12.98 41v8h-2v-6.85L0 35.81v-2.3zM15 0v7.5L27.99 15H28v-2.31h-.01L17 6.35V0h-2zm0 49v-8l12.99-7.5H28v2.31h-.01L17 42.15V49h-2z'/%3E%3C/g%3E%3C/svg%3E"),
			radial-gradient(95% 120% at 100% -5%, #000 0%, rgba(0, 0, 0, 0.4) 42%, transparent 70%);
		mask-image:
			url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='28' height='49' viewBox='0 0 28 49'%3E%3Cg fill='%23ffffff' fill-rule='evenodd'%3E%3Cpath d='M13.99 9.25l13 7.5v15l-13 7.5L1 31.75v-15l12.99-7.5zM3 17.9v11.7l10.99 6.34 11-6.35V17.9l-11-6.34L3 17.9zM0 15l12.98-7.5V0h-2v6.35L0 12.69v2.3zm0 18.5L12.98 41v8h-2v-6.85L0 35.81v-2.3zM15 0v7.5L27.99 15H28v-2.31h-.01L17 6.35V0h-2zm0 49v-8l12.99-7.5H28v2.31h-.01L17 42.15V49h-2z'/%3E%3C/g%3E%3C/svg%3E"),
			radial-gradient(95% 120% at 100% -5%, #000 0%, rgba(0, 0, 0, 0.4) 42%, transparent 70%);
		-webkit-mask-size:
			34px 60px,
			100% 100%;
		mask-size:
			34px 60px,
			100% 100%;
		-webkit-mask-composite: source-in;
		mask-composite: intersect;
	}

	/* Accent fill masked by (honeycomb) ∩ (expanding ring) ∩ (corner pool). `--hexr` only
	   interpolates because it is registered as a <number> above; unregistered it would jump. */
	.hexmesh-ripple {
		position: absolute;
		inset: 0;
		--hexr: 0;
		background: var(--primary);
		-webkit-mask-image:
			url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='28' height='49' viewBox='0 0 28 49'%3E%3Cg fill='%23ffffff' fill-rule='evenodd'%3E%3Cpath d='M13.99 9.25l13 7.5v15l-13 7.5L1 31.75v-15l12.99-7.5zM3 17.9v11.7l10.99 6.34 11-6.35V17.9l-11-6.34L3 17.9zM0 15l12.98-7.5V0h-2v6.35L0 12.69v2.3zm0 18.5L12.98 41v8h-2v-6.85L0 35.81v-2.3zM15 0v7.5L27.99 15H28v-2.31h-.01L17 6.35V0h-2zm0 49v-8l12.99-7.5H28v2.31h-.01L17 42.15V49h-2z'/%3E%3C/g%3E%3C/svg%3E"),
			radial-gradient(
				circle at 100% -5%,
				transparent calc(var(--hexr) * 1% - 16%),
				#000 calc(var(--hexr) * 1% - 3%),
				#000 calc(var(--hexr) * 1% + 3%),
				transparent calc(var(--hexr) * 1% + 18%)
			),
			radial-gradient(95% 120% at 100% -5%, #000 0%, rgba(0, 0, 0, 0.5) 46%, transparent 72%);
		mask-image:
			url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='28' height='49' viewBox='0 0 28 49'%3E%3Cg fill='%23ffffff' fill-rule='evenodd'%3E%3Cpath d='M13.99 9.25l13 7.5v15l-13 7.5L1 31.75v-15l12.99-7.5zM3 17.9v11.7l10.99 6.34 11-6.35V17.9l-11-6.34L3 17.9zM0 15l12.98-7.5V0h-2v6.35L0 12.69v2.3zm0 18.5L12.98 41v8h-2v-6.85L0 35.81v-2.3zM15 0v7.5L27.99 15H28v-2.31h-.01L17 6.35V0h-2zm0 49v-8l12.99-7.5H28v2.31h-.01L17 42.15V49h-2z'/%3E%3C/g%3E%3C/svg%3E"),
			radial-gradient(
				circle at 100% -5%,
				transparent calc(var(--hexr) * 1% - 16%),
				#000 calc(var(--hexr) * 1% - 3%),
				#000 calc(var(--hexr) * 1% + 3%),
				transparent calc(var(--hexr) * 1% + 18%)
			),
			radial-gradient(95% 120% at 100% -5%, #000 0%, rgba(0, 0, 0, 0.5) 46%, transparent 72%);
		-webkit-mask-size:
			34px 60px,
			100% 100%,
			100% 100%;
		mask-size:
			34px 60px,
			100% 100%,
			100% 100%;
		-webkit-mask-composite: source-in, source-in;
		mask-composite: intersect, intersect;
		animation: br-hex-ripple 6s cubic-bezier(0.4, 0, 0.2, 1) infinite;
	}

	@keyframes br-hex-ripple {
		0% {
			--hexr: 4;
			opacity: 0;
		}
		8% {
			opacity: 0.95;
		}
		60% {
			--hexr: 150;
			opacity: 0.95;
		}
		78% {
			--hexr: 175;
			opacity: 0;
		}
		100% {
			--hexr: 175;
			opacity: 0;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.hexmesh-ripple {
			display: none;
		}
	}
	:global([data-motion='reduced']) .hexmesh-ripple {
		display: none;
	}
</style>
