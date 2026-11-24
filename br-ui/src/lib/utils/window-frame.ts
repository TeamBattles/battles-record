import { isTauri } from '@tauri-apps/api/core';

/**
 * Rounds the corners of the undecorated desktop window. The Tauri window is created transparent,
 * and `data-window="rounded"` on <html> (styled in app.css) clips the page to a rounded rectangle.
 *
 * The attribute is set synchronously so the first visible frame is already rounded: the window is
 * shown from LoadingScreen's onMount, which runs before the root layout's. Corners go square while
 * the window is maximized or fullscreen. macOS is skipped because a transparent window there needs
 * Tauri's private-API opt-in, which blocks App Store distribution.
 *
 * Returns a cleanup function that stops tracking and restores square corners.
 */
export function trackWindowFrame(): () => void {
	if (!isTauri() || navigator.platform.includes('Mac')) return () => {};

	const root = document.documentElement;
	let disposed = false;
	let stopListening: (() => void) | undefined;

	const setRounded = (rounded: boolean) => {
		if (rounded && !disposed) root.dataset.window = 'rounded';
		else delete root.dataset.window;
	};
	setRounded(true);

	(async () => {
		const { getCurrentWindow } = await import('@tauri-apps/api/window');
		const appWindow = getCurrentWindow();
		const sync = async () => {
			const [maximized, fullscreen] = await Promise.all([
				appWindow.isMaximized(),
				appWindow.isFullscreen()
			]);
			setRounded(!maximized && !fullscreen);
		};

		const unlisten = await appWindow.onResized(sync);
		if (disposed) {
			unlisten();
			return;
		}
		stopListening = unlisten;
		await sync();
	})().catch((e) => console.warn('Window frame tracking failed', e));

	return () => {
		disposed = true;
		stopListening?.();
		setRounded(false);
	};
}
