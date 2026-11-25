import { invoke } from '@tauri-apps/api/core';

/**
 * Wrappers for the desktop app's file commands. Paths are on this PC, so callers only offer these
 * while viewing the local server. Both reject with "Not found: <path>" when the path is gone.
 */

/** Opens a file with its default app, or a folder in the file manager. */
export function openPath(path: string): Promise<void> {
	return invoke('open_path', { path });
}

/** Opens the file manager with the file selected (on Linux, the folder that holds it). */
export function showInFolder(path: string): Promise<void> {
	return invoke('show_in_folder', { path });
}
