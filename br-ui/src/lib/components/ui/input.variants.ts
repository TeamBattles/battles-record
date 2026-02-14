import { tv, type VariantProps } from 'tailwind-variants';

export const input = tv({
	base: [
		'w-full rounded border px-3 py-2',
		'font-mono text-sm',
		'transition-colors',
		'placeholder:text-muted-foreground',
		'focus:outline-none focus:ring-2 focus:ring-offset-1 focus:ring-offset-background',
		'disabled:cursor-not-allowed disabled:opacity-50'
	],
	variants: {
		variant: {
			default: 'border-border bg-input text-foreground focus:ring-ring',
			error: 'border-red-500/50 bg-input text-foreground focus:ring-red-500'
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
