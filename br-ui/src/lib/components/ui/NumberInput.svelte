<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';
	import { Minus, Plus } from '@lucide/svelte';
	import { input } from './input.variants';
	import { cn } from '#lib/utils/cn.ts';

	/**
	 * Number field with app-drawn − / + steppers. WebKitGTK's native spin buttons are a tiny GTK
	 * widget CSS cannot resize, so app.css hides them on every number input and these replace them.
	 *
	 * The steppers call the input's own stepUp/stepDown, so min, max and step still apply, then fire
	 * `input` and `change` like a typed edit: `bind:value` and `onchange` callers both see it. They
	 * keep focus in the field (no tab stop, no focus on click); the arrow keys step from the keyboard.
	 * `class` sizes the wrapper; the field itself always uses the shared input recipe.
	 */
	interface Props extends Omit<HTMLInputAttributes, 'type' | 'value' | 'class'> {
		value?: number | null;
		class?: string;
	}

	let {
		value = $bindable(null),
		class: className,
		readonly,
		disabled,
		min,
		max,
		...restProps
	}: Props = $props();

	let field = $state<HTMLInputElement>();

	const canStep = $derived(!readonly && !disabled);
	const atMin = $derived(min != null && value != null && value <= Number(min));
	const atMax = $derived(max != null && value != null && value >= Number(max));

	function step(direction: 1 | -1) {
		if (!field) return;
		if (direction > 0) field.stepUp();
		else field.stepDown();
		field.dispatchEvent(new Event('input', { bubbles: true }));
		field.dispatchEvent(new Event('change', { bubbles: true }));
	}
</script>

{#snippet stepper(direction: 1 | -1)}
	<button
		type="button"
		tabindex={-1}
		aria-label={direction > 0 ? 'Increase' : 'Decrease'}
		disabled={direction > 0 ? atMax : atMin}
		onmousedown={(e) => e.preventDefault()}
		onclick={() => step(direction)}
		class="flex size-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-hover-wash hover:text-foreground disabled:pointer-events-none disabled:opacity-40"
	>
		{#if direction > 0}
			<Plus class="size-3.5" />
		{:else}
			<Minus class="size-3.5" />
		{/if}
	</button>
{/snippet}

<div class={cn('relative w-full', className)}>
	<input
		bind:this={field}
		bind:value
		type="number"
		{readonly}
		{disabled}
		{min}
		{max}
		class={cn(input(), canStep && 'pr-16')}
		{...restProps}
	/>
	{#if canStep}
		<div class="absolute inset-y-0 right-0 flex items-center gap-0.5 pr-1.5">
			{@render stepper(-1)}
			{@render stepper(1)}
		</div>
	{/if}
</div>
