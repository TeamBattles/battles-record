/**
 * Svelte actions for common DOM behaviors
 */

/**
 * Autofocus action - focuses the element when it mounts
 * Usage: <input use:autofocus />
 */
export function autofocus(node: HTMLElement) {
	node.focus();
}
