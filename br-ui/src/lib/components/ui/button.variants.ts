import { tv, type VariantProps } from 'tailwind-variants';

/** Arena button recipes; the focus triad matches teambattles.gg's canonical contract. */
export const button = tv({
	base: [
		'inline-flex items-center justify-center gap-2',
		'font-mono text-sm font-medium select-none',
		'rounded border border-transparent transition-colors',
		'focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none',
		'disabled:pointer-events-none disabled:opacity-50'
	],
	variants: {
		intent: {
			primary: 'bg-primary text-primary-foreground hover:bg-primary/90',
			secondary: 'border-border bg-btn-bg text-text-2 hover:bg-btn-bg-hover hover:text-foreground',
			danger: 'border-destructive/30 bg-destructive/10 text-destructive hover:bg-destructive/20',
			ghost: 'text-muted-foreground hover:bg-hover-wash hover:text-foreground',
			link: 'text-primary underline-offset-4 hover:underline'
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
