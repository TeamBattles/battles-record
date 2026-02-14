/**
 * ScheduleRulesEditor Component Tests
 *
 * Tests for the schedule rules editor that allows users to define
 * recording windows with day selection and time inputs.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/svelte';
import ScheduleRulesEditor from './ScheduleRulesEditor.svelte';
import type { ScheduleRule } from '$lib/api/types';

describe('ScheduleRulesEditor', () => {
	const mockOnchange = vi.fn();

	beforeEach(() => {
		vi.clearAllMocks();
	});

	afterEach(() => {
		cleanup();
	});

	function renderEditor(rules: ScheduleRule[] = []) {
		return render(ScheduleRulesEditor, {
			props: {
				rules,
				onchange: mockOnchange
			}
		});
	}

	describe('initial state', () => {
		it('shows Add Rule button when no rules', () => {
			renderEditor();

			expect(screen.getByRole('button', { name: /add rule/i })).toBeInTheDocument();
		});

		it('shows empty state with no rule cards', () => {
			renderEditor();

			expect(screen.queryByText('Rule 1')).not.toBeInTheDocument();
		});

		it('shows existing rules', () => {
			const rules: ScheduleRule[] = [{ days: [1, 2, 3], start_time: '18:00', end_time: '22:00' }];
			renderEditor(rules);

			expect(screen.getByText('Rule 1')).toBeInTheDocument();
		});

		it('shows multiple rules with correct numbering', () => {
			const rules: ScheduleRule[] = [
				{ days: [1, 2, 3], start_time: '18:00', end_time: '22:00' },
				{ days: [0, 6], start_time: '10:00', end_time: '23:00' }
			];
			renderEditor(rules);

			expect(screen.getByText('Rule 1')).toBeInTheDocument();
			expect(screen.getByText('Rule 2')).toBeInTheDocument();
		});
	});

	describe('rule management', () => {
		it('addRule creates new rule with weekday defaults', async () => {
			renderEditor();

			const addButton = screen.getByRole('button', { name: /add rule/i });
			await fireEvent.click(addButton);

			expect(mockOnchange).toHaveBeenCalledWith([
				{ days: [1, 2, 3, 4, 5], start_time: '18:00', end_time: '23:00' }
			]);
		});

		it('addRule appends to existing rules', async () => {
			const existingRules: ScheduleRule[] = [
				{ days: [0, 6], start_time: '10:00', end_time: '18:00' }
			];
			renderEditor(existingRules);

			const addButton = screen.getByRole('button', { name: /add rule/i });
			await fireEvent.click(addButton);

			expect(mockOnchange).toHaveBeenCalledWith([
				{ days: [0, 6], start_time: '10:00', end_time: '18:00' },
				{ days: [1, 2, 3, 4, 5], start_time: '18:00', end_time: '23:00' }
			]);
		});

		it('removeRule deletes correct rule', async () => {
			const rules: ScheduleRule[] = [
				{ days: [1, 2, 3], start_time: '18:00', end_time: '22:00' },
				{ days: [0, 6], start_time: '10:00', end_time: '23:00' }
			];
			renderEditor(rules);

			// Find trash buttons
			const trashButtons = screen
				.getAllByRole('button')
				.filter(
					(btn) =>
						btn.querySelector('svg')?.classList.contains('lucide-trash-2') ||
						btn.querySelector('.lucide-trash-2')
				);
			// Click the first rule's trash button
			await fireEvent.click(trashButtons[0]);

			expect(mockOnchange).toHaveBeenCalledWith([
				{ days: [0, 6], start_time: '10:00', end_time: '23:00' }
			]);
		});
	});

	describe('day selection', () => {
		it('toggleDay adds day if missing', async () => {
			const rules: ScheduleRule[] = [{ days: [1, 2, 3], start_time: '18:00', end_time: '22:00' }];
			renderEditor(rules);

			// Click Friday (index 5) - 'F' button
			const dayButtons = screen
				.getAllByRole('button')
				.filter((btn) => ['S', 'M', 'T', 'W', 'F'].includes(btn.textContent?.trim() || ''));
			// Find one of the Friday buttons (F is at index 5 in dayLabels)
			// dayLabels = ['S', 'M', 'T', 'W', 'T', 'F', 'S']
			// We need to click the 6th button (index 5) which is F
			const fridayButton = dayButtons[5];
			await fireEvent.click(fridayButton);

			expect(mockOnchange).toHaveBeenCalledWith([
				{ days: [1, 2, 3, 5], start_time: '18:00', end_time: '22:00' }
			]);
		});

		it('toggleDay removes day if present', async () => {
			const rules: ScheduleRule[] = [{ days: [1, 2, 3], start_time: '18:00', end_time: '22:00' }];
			renderEditor(rules);

			// Click Tuesday (index 2) which is already selected
			// dayLabels = ['S', 'M', 'T', 'W', 'T', 'F', 'S']
			// Find Tuesday (first T at index 2)
			const dayButtons = screen
				.getAllByRole('button')
				.filter((btn) => btn.textContent?.trim() && btn.textContent.trim().length === 1);
			// Tuesday is index 2
			await fireEvent.click(dayButtons[2]);

			expect(mockOnchange).toHaveBeenCalledWith([
				{ days: [1, 3], start_time: '18:00', end_time: '22:00' }
			]);
		});

		it('days maintain sorted order after toggle', async () => {
			const rules: ScheduleRule[] = [{ days: [1, 5], start_time: '18:00', end_time: '22:00' }];
			renderEditor(rules);

			// Add Wednesday (index 3)
			const dayButtons = screen
				.getAllByRole('button')
				.filter((btn) => btn.textContent?.trim() && btn.textContent.trim().length === 1);
			await fireEvent.click(dayButtons[3]); // W

			expect(mockOnchange).toHaveBeenCalledWith([
				{ days: [1, 3, 5], start_time: '18:00', end_time: '22:00' }
			]);
		});

		it('selected days show emerald styling', () => {
			const rules: ScheduleRule[] = [{ days: [1, 2, 3], start_time: '18:00', end_time: '22:00' }];
			renderEditor(rules);

			// Find day buttons with emerald styling
			const dayButtons = screen
				.getAllByRole('button')
				.filter((btn) => btn.textContent?.trim() && btn.textContent.trim().length === 1);

			// Monday (index 1), Tuesday (2), Wednesday (3) should be selected
			expect(dayButtons[1]).toHaveClass('bg-emerald-600');
			expect(dayButtons[2]).toHaveClass('bg-emerald-600');
			expect(dayButtons[3]).toHaveClass('bg-emerald-600');
		});

		it('unselected days show zinc styling', () => {
			const rules: ScheduleRule[] = [{ days: [1, 2, 3], start_time: '18:00', end_time: '22:00' }];
			renderEditor(rules);

			const dayButtons = screen
				.getAllByRole('button')
				.filter((btn) => btn.textContent?.trim() && btn.textContent.trim().length === 1);

			// Sunday (0), Thursday (4), Friday (5), Saturday (6) should NOT be selected
			expect(dayButtons[0]).toHaveClass('bg-zinc-700');
			expect(dayButtons[4]).toHaveClass('bg-zinc-700');
		});
	});

	describe('presets', () => {
		it('Weekdays preset sets [1,2,3,4,5]', async () => {
			const rules: ScheduleRule[] = [{ days: [0, 6], start_time: '10:00', end_time: '18:00' }];
			renderEditor(rules);

			const weekdaysButton = screen.getByRole('button', { name: 'Weekdays' });
			await fireEvent.click(weekdaysButton);

			expect(mockOnchange).toHaveBeenCalledWith([
				{ days: [1, 2, 3, 4, 5], start_time: '10:00', end_time: '18:00' }
			]);
		});

		it('Weekends preset sets [0,6]', async () => {
			const rules: ScheduleRule[] = [
				{ days: [1, 2, 3, 4, 5], start_time: '18:00', end_time: '22:00' }
			];
			renderEditor(rules);

			const weekendsButton = screen.getByRole('button', { name: 'Weekends' });
			await fireEvent.click(weekendsButton);

			expect(mockOnchange).toHaveBeenCalledWith([
				{ days: [0, 6], start_time: '18:00', end_time: '22:00' }
			]);
		});

		it('Every day preset sets [0,1,2,3,4,5,6]', async () => {
			const rules: ScheduleRule[] = [{ days: [1], start_time: '18:00', end_time: '22:00' }];
			renderEditor(rules);

			const everydayButton = screen.getByRole('button', { name: 'Every day' });
			await fireEvent.click(everydayButton);

			expect(mockOnchange).toHaveBeenCalledWith([
				{ days: [0, 1, 2, 3, 4, 5, 6], start_time: '18:00', end_time: '22:00' }
			]);
		});
	});

	describe('time input', () => {
		it('start_time updates individually', async () => {
			const rules: ScheduleRule[] = [{ days: [1, 2, 3], start_time: '18:00', end_time: '22:00' }];
			renderEditor(rules);

			const timeInputs = screen.getAllByDisplayValue(/:/);
			const startInput = timeInputs[0];

			await fireEvent.change(startInput, { target: { value: '19:30' } });

			expect(mockOnchange).toHaveBeenCalledWith([
				{ days: [1, 2, 3], start_time: '19:30', end_time: '22:00' }
			]);
		});

		it('end_time updates individually', async () => {
			const rules: ScheduleRule[] = [{ days: [1, 2, 3], start_time: '18:00', end_time: '22:00' }];
			renderEditor(rules);

			const timeInputs = screen.getAllByDisplayValue(/:/);
			const endInput = timeInputs[1];

			await fireEvent.change(endInput, { target: { value: '23:30' } });

			expect(mockOnchange).toHaveBeenCalledWith([
				{ days: [1, 2, 3], start_time: '18:00', end_time: '23:30' }
			]);
		});

		it('shows Start and End labels', () => {
			const rules: ScheduleRule[] = [{ days: [1, 2, 3], start_time: '18:00', end_time: '22:00' }];
			renderEditor(rules);

			expect(screen.getByText('Start')).toBeInTheDocument();
			expect(screen.getByText('End')).toBeInTheDocument();
		});
	});

	describe('multiple rules', () => {
		it('preset on one rule does not affect other rules', async () => {
			const rules: ScheduleRule[] = [
				{ days: [1, 2, 3], start_time: '18:00', end_time: '22:00' },
				{ days: [4, 5], start_time: '10:00', end_time: '18:00' }
			];
			renderEditor(rules);

			// Click Weekends on the first rule
			const weekendsButtons = screen.getAllByRole('button', { name: 'Weekends' });
			await fireEvent.click(weekendsButtons[0]);

			expect(mockOnchange).toHaveBeenCalledWith([
				{ days: [0, 6], start_time: '18:00', end_time: '22:00' },
				{ days: [4, 5], start_time: '10:00', end_time: '18:00' }
			]);
		});

		it('removing middle rule preserves other rules', async () => {
			const rules: ScheduleRule[] = [
				{ days: [1], start_time: '09:00', end_time: '12:00' },
				{ days: [2], start_time: '13:00', end_time: '16:00' },
				{ days: [3], start_time: '17:00', end_time: '20:00' }
			];
			renderEditor(rules);

			// Find and click the trash button for Rule 2 (middle rule)
			// We need to find buttons that look like trash buttons
			const allButtons = screen.getAllByRole('button');
			// The trash buttons are small buttons with just an icon
			const trashButtons = allButtons.filter((btn) => {
				// Trash buttons have the Trash2 icon and no text
				const hasTrashIcon = btn.innerHTML.includes('trash') || btn.innerHTML.includes('Trash');
				return hasTrashIcon;
			});

			// If we can't find trash buttons by innerHTML, fall back to finding by class/structure
			if (trashButtons.length === 0) {
				// The trash buttons are inside the rule headers, one per rule
				const ruleHeaders = document.querySelectorAll('.font-mono.text-\\[10px\\]');
				// Actually just click the second remove button
				const removeButtons = allButtons.filter(
					(btn) => btn.querySelector('svg') !== null && btn.classList.contains('p-1')
				);
				if (removeButtons.length >= 2) {
					await fireEvent.click(removeButtons[1]);
				}
			} else {
				await fireEvent.click(trashButtons[1]);
			}

			expect(mockOnchange).toHaveBeenCalledWith([
				{ days: [1], start_time: '09:00', end_time: '12:00' },
				{ days: [3], start_time: '17:00', end_time: '20:00' }
			]);
		});
	});
});
