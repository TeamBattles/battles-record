<script lang="ts">
	import { Dialog } from 'bits-ui';
	import { Drawer } from 'vaul-svelte';
	import { breakpointStore } from '#lib';
	import { X } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import LuxuryCornerSlivers from '#lib/components/ui/LuxuryCornerSlivers.svelte';
	import HexMesh from '#lib/components/ui/HexMesh.svelte';

	interface Props {
		open: boolean;
		onOpenChange: (open: boolean) => void;
		title: string;
		children: Snippet;
		footer?: Snippet;
		/** Element to focus when modal opens. If provided, prevents default focus behavior. */
		initialFocusEl?: HTMLElement | null;
	}

	let { open, onOpenChange, title, children, footer, initialFocusEl }: Props = $props();

	function handleOpenAutoFocus(e: Event) {
		if (initialFocusEl) {
			e.preventDefault();
			initialFocusEl.focus();
		}
	}
</script>

{#if breakpointStore.isMobile}
	<!-- Mobile: Bottom Sheet Drawer -->
	<Drawer.Root {open} {onOpenChange}>
		<Drawer.Portal>
			<Drawer.Overlay class="fixed inset-0 bg-black/70 z-40" />
			<Drawer.Content
				class="fixed bottom-0 left-0 right-0 bg-popover border-t border-border z-50 rounded-t-xl max-h-[85vh] flex flex-col"
			>
				<!-- Drag Handle -->
				<div class="flex justify-center py-3 flex-shrink-0">
					<div class="w-10 h-1 bg-muted-foreground/50 rounded-full"></div>
				</div>

				<!-- Header -->
				<div
					class="relative overflow-hidden flex items-center justify-between px-4 pb-3 border-b border-border flex-shrink-0"
				>
					<HexMesh />
					<Drawer.Title
						class="relative z-10 font-mono text-[11px] uppercase tracking-[0.14em] text-muted-foreground"
						>{title}</Drawer.Title
					>
					<Drawer.Close class="relative z-10 p-1 hover:bg-hover-wash rounded transition-colors">
						<X size={18} class="text-muted-foreground" />
					</Drawer.Close>
				</div>

				<!-- Content - scrollable -->
				<div class="flex-1 overflow-y-auto p-4 min-h-0">
					{@render children()}
				</div>

				<!-- Footer -->
				{#if footer}
					<div class="border-t border-border p-4 flex-shrink-0">
						{@render footer()}
					</div>
				{/if}
			</Drawer.Content>
		</Drawer.Portal>
	</Drawer.Root>
{:else}
	<!-- Desktop/Tablet: centred dialog with the corner-sliver border -->
	<Dialog.Root {open} {onOpenChange}>
		<Dialog.Portal>
			<Dialog.Overlay class="fixed inset-0 bg-black/70 z-40" />
			<Dialog.Content
				class="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 z-50 w-full max-w-md"
				onOpenAutoFocus={handleOpenAutoFocus}
			>
				<div
					class="animate-luxury-fade-scale relative rounded-xl border border-border bg-popover shadow-2xl flex flex-col max-h-[85vh]"
				>
					<LuxuryCornerSlivers />

					<!-- Header -->
					<div
						class="relative overflow-hidden rounded-t-[inherit] flex items-center justify-between px-4 py-3 border-b border-border flex-shrink-0"
					>
						<HexMesh />
						<Dialog.Title
							class="relative z-10 font-mono text-[11px] uppercase tracking-[0.14em] text-muted-foreground"
							>{title}</Dialog.Title
						>
						<Dialog.Close class="relative z-10 p-1 hover:bg-hover-wash rounded transition-colors">
							<X size={18} class="text-muted-foreground" />
						</Dialog.Close>
					</div>

					<!-- Content - scrollable -->
					<div class="flex-1 overflow-y-auto p-4 min-h-0">
						{@render children()}
					</div>

					<!-- Footer -->
					{#if footer}
						<div class="border-t border-border p-4 flex-shrink-0">
							{@render footer()}
						</div>
					{/if}
				</div>
			</Dialog.Content>
		</Dialog.Portal>
	</Dialog.Root>
{/if}
