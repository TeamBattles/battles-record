import { browser } from '$app/environment';

type Breakpoint = 'mobile' | 'tablet' | 'desktop';

function createBreakpointStore() {
	let breakpoint = $state<Breakpoint>('desktop');
	let isMobile = $derived(breakpoint === 'mobile');
	let isTablet = $derived(breakpoint === 'tablet');
	let isDesktop = $derived(breakpoint === 'desktop');

	function init() {
		if (!browser) return;

		const mqMobile = window.matchMedia('(max-width: 639px)');
		const mqTablet = window.matchMedia('(min-width: 640px) and (max-width: 1023px)');

		function update() {
			if (mqMobile.matches) {
				breakpoint = 'mobile';
			} else if (mqTablet.matches) {
				breakpoint = 'tablet';
			} else {
				breakpoint = 'desktop';
			}
		}

		update();
		mqMobile.addEventListener('change', update);
		mqTablet.addEventListener('change', update);

		return () => {
			mqMobile.removeEventListener('change', update);
			mqTablet.removeEventListener('change', update);
		};
	}

	return {
		get breakpoint() {
			return breakpoint;
		},
		get isMobile() {
			return isMobile;
		},
		get isTablet() {
			return isTablet;
		},
		get isDesktop() {
			return isDesktop;
		},
		init
	};
}

export const breakpointStore = createBreakpointStore();
