<script lang="ts">
	import { ArrowLeft, ArrowRight, CalendarClock, Download, Radio, Settings } from '@lucide/svelte';
	import HexMesh from './ui/HexMesh.svelte';
	import LuxuryCornerSlivers from './ui/LuxuryCornerSlivers.svelte';
	import Button from './ui/Button.svelte';
	import AppearanceControls from './AppearanceControls.svelte';

	/**
	 * First-run Welcome Screen: an introduction, then the appearance controls so a new user picks
	 * a background, accent, rounding, density, contrast and motion before setting up a server.
	 * Every pick applies live; finishing or skipping both call `onComplete`.
	 *
	 * It covers the titlebar, so the whole screen drags the window like a splash screen. Tauri's
	 * drag script still lets buttons and other controls take clicks.
	 */
	interface Props {
		onComplete: () => void;
	}

	let { onComplete }: Props = $props();

	let step = $state<0 | 1>(0);

	const FEATURES = [
		{
			icon: Radio,
			title: 'Record live',
			body: 'Twitch, YouTube and Kick streams saved the moment a channel goes live.'
		},
		{
			icon: CalendarClock,
			title: 'Schedules',
			body: 'Record only the hours, games or titles you care about.'
		},
		{
			icon: Download,
			title: 'Library',
			body: 'Browse, download and tidy every recording from one place.'
		}
	];
</script>

<div
	class="fixed inset-0 z-[70] flex flex-col overflow-y-auto bg-background"
	role="dialog"
	aria-modal="true"
	aria-labelledby="welcome-title"
>
	<div
		class="pointer-events-none fixed inset-x-0 top-0 h-[60vh] overflow-hidden"
		aria-hidden="true"
	>
		<HexMesh ripple />
	</div>

	<!-- The drag region sits inside the scroll container, not on it, so the scrollbar stays usable -->
	<div class="relative z-10 flex flex-1 flex-col" data-tauri-drag-region="deep">
		<div class="mx-auto flex w-full max-w-4xl flex-1 flex-col justify-center px-4 py-8 sm:px-6">
			<section
				class="relative overflow-clip rounded-xl border border-border bg-card [overflow-clip-margin:1px]"
			>
				<LuxuryCornerSlivers intensity="strong" length="2xl" />

				{#if step === 0}
					<div class="animate-luxury-fade-up relative flex flex-col gap-8 p-5 sm:p-8">
						<div class="flex flex-col gap-4">
							<div class="flex items-center gap-3">
								<img src="/favicon.svg" alt="" class="size-10" />
								<span
									class="font-mono text-[11px] tracking-[0.18em] text-muted-foreground uppercase"
								>
									Welcome to
								</span>
							</div>
							<div class="flex items-center gap-4">
								<h1
									id="welcome-title"
									class="font-display text-5xl leading-none tracking-wide text-foreground uppercase sm:text-6xl"
								>
									Battles Record
								</h1>
								<span
									class="hidden h-px flex-1 bg-gradient-to-r from-primary/50 to-transparent sm:block"
								></span>
							</div>
							<p class="max-w-xl text-sm text-muted-foreground sm:text-base">
								Your stream recorder. Add the channels you follow and it captures their broadcasts
								automatically, ready to watch, download or clip later.
							</p>
						</div>

						<div class="grid gap-3 sm:grid-cols-3">
							{#each FEATURES as feature (feature.title)}
								<div class="group relative rounded-lg border border-border bg-well p-4">
									<LuxuryCornerSlivers corners="tl" length="md" hoverFadeIn />
									<span
										class="mb-3 flex size-8 items-center justify-center rounded-md bg-primary/10 text-primary"
									>
										<feature.icon class="size-4" />
									</span>
									<h2 class="font-mono text-[11px] tracking-[0.14em] text-text-2 uppercase">
										{feature.title}
									</h2>
									<p class="mt-1 text-xs text-muted-foreground">{feature.body}</p>
								</div>
							{/each}
						</div>

						<div class="flex flex-col-reverse gap-2 sm:flex-row sm:items-center sm:justify-between">
							<Button intent="ghost" onclick={onComplete}>Skip for now</Button>
							<Button intent="primary" size="lg" onclick={() => (step = 1)}>
								Personalize
								<ArrowRight class="size-4" />
							</Button>
						</div>
					</div>
				{:else}
					<div class="animate-luxury-fade-up relative flex flex-col gap-6 p-5 sm:p-8">
						<div class="flex flex-col gap-1.5">
							<span class="font-mono text-[11px] tracking-[0.18em] text-muted-foreground uppercase">
								Appearance
							</span>
							<h1
								id="welcome-title"
								class="font-display text-4xl leading-none tracking-wide text-foreground uppercase"
							>
								Make it yours
							</h1>
							<p class="text-sm text-muted-foreground">
								Every change applies straight away. You can revisit these any time in
								<span class="inline-flex items-center gap-1 text-text-2">
									<Settings class="size-3.5" /> Settings
								</span>.
							</p>
						</div>

						<AppearanceControls variant="welcome" />

						<div
							class="flex flex-col-reverse gap-2 border-t border-border pt-5 sm:flex-row sm:items-center sm:justify-between"
						>
							<Button intent="ghost" onclick={() => (step = 0)}>
								<ArrowLeft class="size-4" />
								Back
							</Button>
							<Button intent="primary" size="lg" onclick={onComplete}>
								Get started
								<ArrowRight class="size-4" />
							</Button>
						</div>
					</div>
				{/if}
			</section>
		</div>
	</div>
</div>
