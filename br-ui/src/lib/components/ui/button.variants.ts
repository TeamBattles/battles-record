import { tv, type VariantProps } from 'tailwind-variants';

export const button = tv({
	base: [
		'inline-flex items-center justify-center gap-2',
		'font-mono text-sm font-medium select-none',
		'rounded transition-colors',
		'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:ring-offset-background',
		'disabled:pointer-events-none disabled:opacity-50'
	],
	variants: {
		intent: {
			primary: 'bg-emerald-600 text-white hover:bg-emerald-500 focus-visible:ring-emerald-500',
			secondary:
				'border border-border bg-input text-zinc-300 hover:bg-muted focus-visible:ring-zinc-500',
			danger:
				'border border-red-500/30 bg-red-500/10 text-red-400 hover:bg-red-500/20 focus-visible:ring-red-500',
			ghost: 'hover:bg-muted text-zinc-400 focus-visible:ring-zinc-500',
			link: 'text-emerald-400 underline-offset-4 hover:underline focus-visible:ring-emerald-500'
		},
		size: {
			sm: 'h-7 px-2 text-xs',
			md: 'h-9 px-3',
			lg: 'h-11 px-4',
			icon: 'h-9 w-9'
		},
		fullWidth: {
			true: 'w-full'
		}
	},
	compoundVariants: [
		{
			intent: 'ghost',
			size: 'icon',
			class: 'rounded-full'
		}
	],
	defaultVariants: {
		intent: 'secondary',
		size: 'md'
	}
});

export type ButtonVariants = VariantProps<typeof button>;
