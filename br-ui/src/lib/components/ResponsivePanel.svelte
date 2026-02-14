<script lang="ts">
	import { Drawer } from 'vaul-svelte';
	import { breakpointStore } from '$lib';
	import type { Snippet } from 'svelte';

	interface Props {
		open: boolean;
		onClose: () => void;
		children: Snippet;
	}

	let { open, onClose, children }: Props = $props();
</script>

{#if breakpointStore.isMobile}
	<!-- Mobile: Bottom Sheet Drawer -->
	<Drawer.Root {open} onOpenChange={(o) => !o && onClose()}>
		<Drawer.Portal>
			<Drawer.Overlay class="fixed inset-0 bg-black/60 z-40" />
			<Drawer.Content
				class="fixed bottom-0 left-0 right-0 bg-card border-t border-border z-50 rounded-t-lg max-h-[90vh] flex flex-col"
			>
				<!-- Drag Handle -->
				<div class="flex justify-center py-3 flex-shrink-0">
					<div class="w-10 h-1 bg-muted-foreground/50 rounded-full"></div>
				</div>

				<!-- Content - scrollable -->
				<div class="flex-1 overflow-y-auto flex flex-col min-h-0">
					{@render children()}
				</div>
			</Drawer.Content>
		</Drawer.Portal>
	</Drawer.Root>
{:else if open}
	<!-- Desktop/Tablet: Right Slide-Out Panel -->
	<!-- Backdrop -->
	<button class="fixed inset-0 bg-black/60 z-40" onclick={onClose} aria-label="Close panel"
	></button>

	<!-- Panel -->
	<aside
		class="fixed right-0 top-0 bottom-0 w-96 bg-card border-l border-border z-50 flex flex-col overflow-hidden"
	>
		{@render children()}
	</aside>
{/if}
