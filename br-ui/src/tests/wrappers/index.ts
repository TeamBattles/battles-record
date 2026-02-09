/**
 * Store Wrapper Components for Testing
 *
 * These components mount stores and expose their state via data-testid attributes.
 * This allows testing Svelte 5 $state reactivity through real component rendering.
 *
 * Usage:
 * ```typescript
 * import { render, screen } from '@testing-library/svelte';
 * import ChannelsStoreWrapper from './wrappers/ChannelsStoreWrapper.svelte';
 *
 * test('channels load correctly', async () => {
 *   render(ChannelsStoreWrapper);
 *
 *   // Store is mounted, check initial state
 *   expect(screen.getByTestId('is-loading')).toHaveTextContent('false');
 *
 *   // Trigger load and verify
 *   channelsStore.load();
 *   await waitFor(() => {
 *     expect(screen.getByTestId('channel-count')).toHaveTextContent('3');
 *   });
 * });
 * ```
 */

export { default as ChannelsStoreWrapper } from './ChannelsStoreWrapper.svelte';
export { default as RecordingsStoreWrapper } from './RecordingsStoreWrapper.svelte';
export { default as StorageStoreWrapper } from './StorageStoreWrapper.svelte';
export { default as ToastStoreWrapper } from './ToastStoreWrapper.svelte';
