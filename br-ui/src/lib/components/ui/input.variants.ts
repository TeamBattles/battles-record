import { tv, type VariantProps } from 'tailwind-variants';

/** Inputs sit in a well with an inset shadow and share the Arena focus triad. */
export const input = tv({
	base: [
		'w-full rounded border bg-well px-3 py-2',
		'font-mono text-sm text-foreground shadow-[inset_0_1px_2px_rgba(0,0,0,0.25)]',
		'transition-colors',
		'placeholder:text-dim',
		'focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none',
		'disabled:cursor-not-allowed disabled:opacity-50'
	],
	variants: {
		variant: {
			default: 'border-line-2',
			error:
				'border-destructive/50 focus-visible:border-destructive focus-visible:ring-destructive/40'
		},
		inputSize: {
			sm: 'px-2 py-1 text-xs',
			md: 'px-3 py-2 text-sm',
			lg: 'px-4 py-3'
		}
	},
	defaultVariants: {
		variant: 'default',
		inputSize: 'md'
	}
});

export type InputVariants = VariantProps<typeof input>;
