<script lang="ts">
	import {
		ACCENT_OPTIONS,
		ACCENT_SWATCHES,
		DENSITY_OPTIONS,
		RADIUS_OPTIONS,
		THEME_OPTIONS,
		THEME_SWATCHES,
		type EffectiveTheme,
		type Radius,
		type Density
	} from '#lib/appearance.ts';
	import { appearanceStore } from '#lib/stores/appearance.svelte.ts';
	import ChoiceTile from './ui/ChoiceTile.svelte';
	import SegmentedControl from './ui/SegmentedControl.svelte';
	import Switch from './ui/Switch.svelte';
	import Button from './ui/Button.svelte';
	import { cn } from '#lib/utils/cn.ts';

	/**
	 * Every appearance control, shared by the Welcome Screen and Settings > Appearance. Each pick
	 * goes straight through the appearance store, so the whole app re-skins while the user chooses
	 * and the two places can never disagree. `welcome` hides the per-row descriptions and reset.
	 */
	interface Props {
		variant?: 'settings' | 'welcome';
	}

	let { variant = 'settings' }: Props = $props();
	const uid = $props.id();

	const prefs = $derived(appearanceStore.prefs);
	const accentTheme = $derived(appearanceStore.resolvedTheme);
	const showDescriptions = $derived(variant === 'settings');

	const CORNER_PATHS: Record<Radius, string> = {
		sharp: 'M2.5 13V2.5H13',
		soft: 'M2.5 13V7a4.5 4.5 0 0 1 4.5-4.5h6',
		round: 'M2.5 13v-2a8.5 8.5 0 0 1 8.5-8.5h2'
	};
	const DENSITY_LINES: Record<Density, number[]> = {
		comfortable: [3, 8, 13],
		compact: [2.5, 6, 9.5, 13]
	};

	function miniStyle(theme: EffectiveTheme) {
		const s = THEME_SWATCHES[theme];
		return [
			`--m-bg: ${s.bg}`,
			`--m-panel: ${s.panel}`,
			`--m-surface: ${s.surface}`,
			`--m-line: ${s.line}`,
			`--m-muted: ${s.muted}`,
			`--m-accent: ${ACCENT_SWATCHES[theme][prefs.accent]}`
		].join('; ');
	}
</script>

{#snippet miniWindow(theme: EffectiveTheme, clip = false)}
	<span
		class={cn(
			'absolute inset-0 flex bg-[var(--m-bg)]',
			clip && '[clip-path:polygon(100%_0,100%_100%,0_100%)]'
		)}
		style={miniStyle(theme)}
	>
		<span
			class="flex w-[22%] flex-col gap-[10%] border-r border-[var(--m-line)] bg-[var(--m-panel)] p-[6%]"
		>
			<span class="h-[8%] rounded-[1px] bg-[var(--m-accent)]"></span>
			<span class="h-[8%] rounded-[1px] bg-[var(--m-surface)]"></span>
			<span class="h-[8%] rounded-[1px] bg-[var(--m-surface)]"></span>
		</span>
		<span class="grid flex-1 grid-cols-2 gap-[8%] p-[8%]">
			{#each [0, 1, 2, 3] as card (card)}
				<span
					class="flex flex-col justify-end gap-[12%] rounded-[2px] border border-[var(--m-line)] bg-[var(--m-panel)] p-[10%]"
				>
					<span class="h-[14%] w-[70%] rounded-[1px] bg-[var(--m-muted)] opacity-60"></span>
				</span>
			{/each}
		</span>
	</span>
{/snippet}

{#snippet rowLabel(id: string, label: string, description: string)}
	<div class="flex min-w-0 flex-col gap-0.5">
		<span {id} class="font-mono text-[11px] tracking-[0.14em] text-text-2 uppercase">{label}</span>
		{#if showDescriptions}
			<span class="text-xs text-muted-foreground">{description}</span>
		{/if}
	</div>
{/snippet}

<div class={cn('flex flex-col', variant === 'welcome' ? 'gap-5' : 'gap-6')}>
	<!-- Background: sized by its container, not the viewport, because the Welcome Screen card and
	     the Settings panel give it very different widths at the same window size -->
	<div class="@container flex flex-col gap-3">
		{@render rowLabel(
			`${uid}-theme`,
			'Background',
			"The surface palette. System follows your device's light or dark setting."
		)}
		<div role="group" aria-labelledby="{uid}-theme" class="grid grid-cols-3 gap-2 @lg:grid-cols-5">
			{#each THEME_OPTIONS as option (option.value)}
				<ChoiceTile
					selected={prefs.theme === option.value}
					onSelect={() => appearanceStore.set({ theme: option.value })}
					label={option.label}
				>
					{#snippet preview()}
						<span
							class="relative block aspect-[16/10] w-full overflow-hidden rounded border border-border"
						>
							{#if option.value === 'system'}
								{@render miniWindow('black')}
								{@render miniWindow('light', true)}
							{:else}
								{@render miniWindow(option.value)}
							{/if}
						</span>
					{/snippet}
				</ChoiceTile>
			{/each}
		</div>
	</div>

	<!-- Accent -->
	<div class="flex flex-col gap-3">
		{@render rowLabel(
			`${uid}-accent`,
			'Accent',
			'The highlight color for buttons, toggles, focus rings and the honeycomb ripple.'
		)}
		<div role="group" aria-labelledby="{uid}-accent" class="grid grid-cols-2 gap-2 sm:grid-cols-4">
			{#each ACCENT_OPTIONS as option (option.value)}
				<ChoiceTile
					selected={prefs.accent === option.value}
					onSelect={() => appearanceStore.set({ accent: option.value })}
					label={option.label}
				>
					{#snippet preview()}
						<span class="flex h-10 items-center gap-2 rounded border border-border bg-well px-2.5">
							<span
								class="size-5 shrink-0 rounded-full shadow-sm"
								style="background-color: {ACCENT_SWATCHES[accentTheme][option.value]}"
							></span>
							<span
								class="h-1.5 flex-1 rounded-full opacity-70"
								style="background-color: {ACCENT_SWATCHES[accentTheme][option.value]}"
							></span>
						</span>
					{/snippet}
				</ChoiceTile>
			{/each}
		</div>
	</div>

	<!-- Rounding + density -->
	<div class="grid gap-5 sm:grid-cols-2">
		<div class="flex flex-col gap-3">
			{@render rowLabel(`${uid}-radius`, 'Rounding', 'How rounded cards, buttons and menus are.')}
			<SegmentedControl
				ariaLabel="Rounding"
				options={RADIUS_OPTIONS}
				value={prefs.radius}
				onChange={(radius) => appearanceStore.set({ radius })}
				class="self-start"
			>
				{#snippet glyph(value)}
					<svg
						width="15"
						height="15"
						viewBox="0 0 15 15"
						fill="none"
						stroke="currentColor"
						stroke-width="1.6"
						stroke-linecap="round"
						aria-hidden="true"
					>
						<path d={CORNER_PATHS[value]} />
					</svg>
				{/snippet}
			</SegmentedControl>
		</div>
		<div class="flex flex-col gap-3">
			{@render rowLabel(
				`${uid}-density`,
				'Density',
				'Compact tightens spacing and text to fit more on screen.'
			)}
			<SegmentedControl
				ariaLabel="Density"
				options={DENSITY_OPTIONS}
				value={prefs.density}
				onChange={(density) => appearanceStore.set({ density })}
				class="self-start"
			>
				{#snippet glyph(value)}
					<svg
						width="15"
						height="15"
						viewBox="0 0 15 15"
						fill="none"
						stroke="currentColor"
						stroke-width="1.6"
						stroke-linecap="round"
						aria-hidden="true"
					>
						{#each DENSITY_LINES[value] as y (y)}<path d="M2.5 {y}h10" />{/each}
					</svg>
				{/snippet}
			</SegmentedControl>
		</div>
	</div>

	<!-- Accessibility toggles -->
	<div class="grid gap-3 sm:grid-cols-2">
		<div
			class="flex items-center justify-between gap-4 rounded-lg border border-border bg-well px-3 py-3"
		>
			{@render rowLabel(
				`${uid}-contrast`,
				'High contrast',
				'Stronger borders, text and focus rings.'
			)}
			<Switch
				checked={prefs.contrast === 'high'}
				labelledby="{uid}-contrast"
				onCheckedChange={(on) => appearanceStore.set({ contrast: on ? 'high' : 'normal' })}
			/>
		</div>
		<div
			class="flex items-center justify-between gap-4 rounded-lg border border-border bg-well px-3 py-3"
		>
			{@render rowLabel(
				`${uid}-motion`,
				'Reduce motion',
				'Turns off animations, transitions and the honeycomb ripple.'
			)}
			<Switch
				checked={prefs.motion === 'reduced'}
				labelledby="{uid}-motion"
				onCheckedChange={(on) => appearanceStore.set({ motion: on ? 'reduced' : 'on' })}
			/>
		</div>
	</div>

	{#if variant === 'settings'}
		<div class="flex justify-end">
			<Button intent="ghost" size="sm" onclick={() => appearanceStore.reset()}>
				Reset appearance
			</Button>
		</div>
	{/if}
</div>
