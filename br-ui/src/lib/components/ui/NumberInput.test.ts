import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import NumberInput from './NumberInput.svelte';

// Settings saves number fields from `onchange`, so a stepper that updated the value without firing
// `change` would show the new number but never persist it.
describe('NumberInput steppers', () => {
	it('steps the value and fires change so onchange callers save it', async () => {
		const onchange = vi.fn((e: Event) => (e.currentTarget as HTMLInputElement).value);
		render(NumberInput, { props: { value: 2, min: 1, max: 5, onchange } });

		await fireEvent.click(screen.getByRole('button', { name: 'Increase' }));

		expect(screen.getByRole('spinbutton')).toHaveValue(3);
		expect(onchange).toHaveBeenCalledTimes(1);
		expect(onchange.mock.results[0].value).toBe('3');
	});

	it('stops at max', async () => {
		render(NumberInput, { props: { value: 5, min: 1, max: 5 } });

		expect(screen.getByRole('button', { name: 'Increase' })).toBeDisabled();
		await fireEvent.click(screen.getByRole('button', { name: 'Decrease' }));
		expect(screen.getByRole('spinbutton')).toHaveValue(4);
	});
});
