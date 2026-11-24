<script lang="ts">
	import { DropdownMenu } from 'bits-ui';
	import { Check, ChevronDown, Palette } from '@lucide/svelte';
	import { THEME_OPTIONS, THEME_SWATCHES, type Theme } from '#lib/appearance.ts';
	import { appearanceStore } from '#lib/stores/appearance.svelte.ts';
	import { cn } from '#lib/utils/cn.ts';

	/**
	 * Titlebar background picker. A bits-ui menu (like the server menu beside it) rather than a
	 * native select; the radio group supplies arrow-key navigation and the checked state. The full
	 * appearance set lives in Settings > Appearance.
	 */
	interface Props {
		class?: string;
	}

	let { class: className }: Props = $props();

	const current = $derived(appearanceStore.prefs.theme);
	const currentLabel = $derived(THEME_OPTIONS.find((o) => o.value === current)?.label ?? '');

	function swatchStyle(theme: Theme) {
		if (theme === 'system') {
			return `background: linear-gradient(135deg, ${THEME_SWATCHES.black.bg} 50%, ${THEME_SWATCHES.light.bg} 50%)`;
		}
		return `background-color: ${THEME_SWATCHES[theme].bg}`;
	}
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger
		aria-label="Background: {currentLabel}"
		class={cn(
			'flex h-full items-center gap-2 px-3 transition-colors hover:bg-hover-wash data-[state=open]:bg-hover-wash',
			className
		)}
	>
		<Palette size={14} class="text-primary" />
		<span
			class="hidden font-mono text-[10px] tracking-wider text-muted-foreground uppercase md:inline"
		>
			{currentLabel}
		</span>
		<ChevronDown size={12} class="text-muted-foreground" />
	</DropdownMenu.Trigger>

	<DropdownMenu.Portal>
		<DropdownMenu.Content
			class="z-50 min-w-[180px] rounded border border-line-2 bg-well py-1 shadow-lg"
			sideOffset={4}
			align="end"
		>
			<DropdownMenu.RadioGroup
				value={current}
				onValueChange={(value) => appearanceStore.set({ theme: value as Theme })}
			>
				{#each THEME_OPTIONS as option (option.value)}
					<DropdownMenu.RadioItem
						value={option.value}
						class="flex w-full cursor-pointer items-center gap-3 px-3 py-2 text-left outline-none transition-colors data-highlighted:bg-hover-wash"
					>
						{#snippet children({ checked })}
							<span
								class="size-4 shrink-0 rounded-sm border border-line-2"
								style={swatchStyle(option.value)}
								aria-hidden="true"
							></span>
							<span class="flex-1 font-mono text-sm text-text-2">{option.label}</span>
							{#if checked}
								<Check size={14} class="text-primary" />
							{/if}
						{/snippet}
					</DropdownMenu.RadioItem>
				{/each}
			</DropdownMenu.RadioGroup>
		</DropdownMenu.Content>
	</DropdownMenu.Portal>
</DropdownMenu.Root>
