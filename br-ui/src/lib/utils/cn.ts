import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

/**
 * Combines clsx for conditional classes with tailwind-merge for conflict resolution.
 * Use this for any component that accepts a `class` prop or has conditional styling.
 *
 * @example
 * // Basic usage
 * cn('px-4 py-2', 'px-6')  // => 'py-2 px-6' (px-6 wins)
 *
 * // Conditional classes
 * cn('base', isActive && 'bg-blue-500', isDisabled && 'opacity-50')
 *
 * // Object syntax
 * cn('base', { 'bg-blue-500': isActive, 'opacity-50': isDisabled })
 *
 * // Component with overridable classes
 * cn('px-4 py-2 bg-blue-500', props.class)  // props.class can override any default
 */
export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}
