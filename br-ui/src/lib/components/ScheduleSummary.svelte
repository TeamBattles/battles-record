<script lang="ts">
	import type { ScheduleRule } from '$lib/api/types';

	interface Props {
		rules: ScheduleRule[];
		class?: string;
	}

	let { rules, class: className = '' }: Props = $props();

	function formatTime(time: string): string {
		const [hours, minutes] = time.split(':').map(Number);
		const period = hours >= 12 ? 'PM' : 'AM';
		const displayHours = hours % 12 || 12;
		return `${displayHours}:${minutes.toString().padStart(2, '0')} ${period}`;
	}

	function formatDays(days: number[]): string {
		if (days.length === 7) return 'Every day';
		if (days.length === 5 && !days.includes(0) && !days.includes(6)) return 'Weekdays';
		if (days.length === 2 && days.includes(0) && days.includes(6)) return 'Weekends';

		const dayNames = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
		const sortedDays = [...days].sort((a, b) => a - b);

		// Check for consecutive days
		let ranges: string[] = [];
		let rangeStart = sortedDays[0];
		let rangeEnd = sortedDays[0];

		for (let i = 1; i <= sortedDays.length; i++) {
			if (i < sortedDays.length && sortedDays[i] === rangeEnd + 1) {
				rangeEnd = sortedDays[i];
			} else {
				if (rangeEnd - rangeStart >= 2) {
					ranges.push(`${dayNames[rangeStart]}-${dayNames[rangeEnd]}`);
				} else if (rangeEnd - rangeStart === 1) {
					ranges.push(dayNames[rangeStart], dayNames[rangeEnd]);
				} else {
					ranges.push(dayNames[rangeStart]);
				}
				if (i < sortedDays.length) {
					rangeStart = sortedDays[i];
					rangeEnd = sortedDays[i];
				}
			}
		}

		return ranges.join(', ');
	}

	function formatRule(rule: ScheduleRule): string {
		return `${formatDays(rule.days)} ${formatTime(rule.start_time)} - ${formatTime(rule.end_time)}`;
	}
</script>

{#if rules.length === 0}
	<span class="font-mono text-xs text-zinc-500 {className}">No schedule</span>
{:else}
	<div class="space-y-1 {className}">
		{#each rules as rule}
			<p class="font-mono text-xs text-zinc-400">{formatRule(rule)}</p>
		{/each}
	</div>
{/if}
