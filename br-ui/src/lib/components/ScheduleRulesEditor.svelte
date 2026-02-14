<script lang="ts">
	import { Plus, Trash2 } from 'lucide-svelte';
	import type { ScheduleRule } from '$lib/api/types';

	interface Props {
		rules: ScheduleRule[];
		onchange: (rules: ScheduleRule[]) => void;
	}

	let { rules, onchange }: Props = $props();

	const dayLabels = ['S', 'M', 'T', 'W', 'T', 'F', 'S'];
	const dayNames = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

	function addRule() {
		onchange([...rules, { days: [1, 2, 3, 4, 5], start_time: '18:00', end_time: '23:00' }]);
	}

	function removeRule(index: number) {
		onchange(rules.filter((_, i) => i !== index));
	}

	function updateRule(index: number, updates: Partial<ScheduleRule>) {
		const newRules = [...rules];
		newRules[index] = { ...newRules[index], ...updates };
		onchange(newRules);
	}

	function toggleDay(ruleIndex: number, day: number) {
		const rule = rules[ruleIndex];
		const days = rule.days.includes(day)
			? rule.days.filter((d) => d !== day)
			: [...rule.days, day].sort((a, b) => a - b);
		updateRule(ruleIndex, { days });
	}

	function setPreset(ruleIndex: number, preset: 'weekdays' | 'weekends' | 'everyday') {
		const days =
			preset === 'weekdays'
				? [1, 2, 3, 4, 5]
				: preset === 'weekends'
					? [0, 6]
					: [0, 1, 2, 3, 4, 5, 6];
		updateRule(ruleIndex, { days });
	}
</script>

<div class="space-y-3">
	{#each rules as rule, index (index)}
		<div class="rounded border border-zinc-700 bg-zinc-800 p-3">
			<div class="flex items-center justify-between mb-3">
				<span class="font-mono text-[10px] uppercase tracking-wider text-zinc-500"
					>Rule {index + 1}</span
				>
				<button
					class="p-1 hover:bg-zinc-700 rounded transition-colors"
					onclick={() => removeRule(index)}
				>
					<Trash2 size={14} class="text-red-400" />
				</button>
			</div>

			<!-- Day selector -->
			<div class="mb-3">
				<span class="block font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-2"
					>Days</span
				>
				<div class="flex gap-1 mb-2">
					{#each dayLabels as label, dayIndex (dayIndex)}
						<button
							class="w-8 h-8 rounded font-mono text-xs transition-colors {rule.days.includes(
								dayIndex
							)
								? 'bg-emerald-600 text-white'
								: 'bg-zinc-700 text-zinc-400 hover:bg-zinc-600'}"
							onclick={() => toggleDay(index, dayIndex)}
							title={dayNames[dayIndex]}
						>
							{label}
						</button>
					{/each}
				</div>
				<div class="flex gap-2">
					<button
						class="px-2 py-1 rounded font-mono text-[10px] bg-zinc-700 text-zinc-400 hover:bg-zinc-600 transition-colors"
						onclick={() => setPreset(index, 'weekdays')}
					>
						Weekdays
					</button>
					<button
						class="px-2 py-1 rounded font-mono text-[10px] bg-zinc-700 text-zinc-400 hover:bg-zinc-600 transition-colors"
						onclick={() => setPreset(index, 'weekends')}
					>
						Weekends
					</button>
					<button
						class="px-2 py-1 rounded font-mono text-[10px] bg-zinc-700 text-zinc-400 hover:bg-zinc-600 transition-colors"
						onclick={() => setPreset(index, 'everyday')}
					>
						Every day
					</button>
				</div>
			</div>

			<!-- Time inputs -->
			<div class="flex gap-3">
				<div class="flex-1">
					<span class="block font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-1"
						>Start</span
					>
					<input
						type="time"
						class="w-full bg-zinc-900 border border-zinc-700 px-2 py-1.5 rounded font-mono text-sm text-zinc-100"
						value={rule.start_time}
						onchange={(e) => updateRule(index, { start_time: e.currentTarget.value })}
					/>
				</div>
				<div class="flex-1">
					<span class="block font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-1"
						>End</span
					>
					<input
						type="time"
						class="w-full bg-zinc-900 border border-zinc-700 px-2 py-1.5 rounded font-mono text-sm text-zinc-100"
						value={rule.end_time}
						onchange={(e) => updateRule(index, { end_time: e.currentTarget.value })}
					/>
				</div>
			</div>
		</div>
	{/each}

	<button
		class="w-full flex items-center justify-center gap-2 px-3 py-2 rounded border border-dashed border-zinc-700 hover:border-zinc-500 font-mono text-xs text-zinc-400 hover:text-zinc-300 transition-colors"
		onclick={addRule}
	>
		<Plus size={14} />
		Add Rule
	</button>
</div>
