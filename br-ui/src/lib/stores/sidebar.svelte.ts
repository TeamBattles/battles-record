import { breakpointStore } from './breakpoint.svelte';

type SidebarState = 'expanded' | 'collapsed' | 'hidden';

function createSidebarStore() {
	let isOpen = $state(false); // For mobile overlay
	let userPreference = $state<'expanded' | 'collapsed' | null>(null);

	let state = $derived.by((): SidebarState => {
		if (breakpointStore.isMobile) {
			return 'hidden';
		}
		if (breakpointStore.isTablet) {
			return userPreference === 'expanded' ? 'expanded' : 'collapsed';
		}
		// Desktop
		return userPreference === 'collapsed' ? 'collapsed' : 'expanded';
	});

	function toggle() {
		if (breakpointStore.isMobile) {
			isOpen = !isOpen;
		} else {
			userPreference = state === 'expanded' ? 'collapsed' : 'expanded';
		}
	}

	function open() {
		isOpen = true;
	}

	function close() {
		isOpen = false;
	}

	return {
		get state() {
			return state;
		},
		get isOpen() {
			return isOpen;
		},
		toggle,
		open,
		close
	};
}

export const sidebarStore = createSidebarStore();
