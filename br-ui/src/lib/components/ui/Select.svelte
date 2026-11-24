<script lang="ts" generics="T extends string | number | null">
	import { Select } from 'bits-ui';
	import { Check, ChevronDown } from '@lucide/svelte';
	import { input } from './input.variants';
	import { cn } from '#lib/utils/cn.ts';

	/**
	 * App-drawn select (bits-ui) in place of the native <select>. WebKitGTK paints a native select
	 * with its own GTK frame inside our border, opens a GTK popup menu, and treats a focused select
	 * like a text field, so the focus ring returned after every mouse pick.
	 *
	 * Values can be any string, number or null, matched by equality like a bound <select>. bits-ui
	 * only deals in strings and treats '' as "nothing selected", so items are keyed by their index
	 * internally and `''` or `null` work as ordinary option values ("All …" filters rely on it).
	 * `id` lands on the trigger, so an existing `<label for>` still points at it.
	 */
	interface Option {
		value: T;
		label: string;
		disabled?: boolean;
	}

	interface Props {
		value?: T;
		options: readonly Option[];
		placeholder?: string;
		onValueChange?: (value: T) => void;
		id?: string;
		disabled?: boolean;
		ariaLabel?: string;
		/** `sm` is the compact size used by toolbar filters. */
		size?: 'sm' | 'md';
		class?: string;
	}

	let {
		value = $bindable(),
		options,
		placeholder,
		onValueChange,
		id,
		disabled = false,
		ariaLabel,
		size = 'md',
		class: className
	}: Props = $props();

	const selectedIndex = $derived(options.findIndex((option) => option.value === value));
	const selectedLabel = $derived(selectedIndex >= 0 ? options[selectedIndex].label : undefined);

	function selectKey(key: string) {
		const option = options[Number(key)];
		if (!option || option.value === value) return;
		value = option.value;
		onValueChange?.(option.value);
	}
</script>

<Select.Root
	type="single"
	bind:value={() => (selectedIndex >= 0 ? String(selectedIndex) : ''), selectKey}
	{disabled}
>
	<Select.Trigger
		{id}
		aria-label={ariaLabel}
		class={cn(
			input(),
			'flex items-center justify-between gap-2 text-left data-[state=open]:border-ring',
			size === 'sm' && 'py-1.5 text-xs',
			className
		)}
	>
		<span class={cn('truncate', !selectedLabel && 'text-dim')}>{selectedLabel ?? placeholder}</span>
		<ChevronDown class="size-3.5 shrink-0 text-muted-foreground" />
	</Select.Trigger>

	<Select.Portal>
		<Select.Content
			sideOffset={4}
			class="z-50 max-h-72 min-w-(--bits-select-anchor-width) overflow-y-auto rounded border border-line-2 bg-well py-1 shadow-lg"
		>
			{#each options as option, index (index)}
				<Select.Item
					value={String(index)}
					label={option.label}
					disabled={option.disabled}
					class={cn(
						'flex cursor-pointer items-center gap-3 px-3 py-2 font-mono text-text-2 outline-none transition-colors',
						'data-disabled:cursor-not-allowed data-disabled:opacity-50 data-highlighted:bg-hover-wash',
						size === 'sm' ? 'text-xs' : 'text-sm'
					)}
				>
					{#snippet children({ selected })}
						<span class="flex-1 truncate">{option.label}</span>
						{#if selected}
							<Check size={14} class="shrink-0 text-primary" />
						{/if}
					{/snippet}
				</Select.Item>
			{/each}
		</Select.Content>
	</Select.Portal>
</Select.Root>
