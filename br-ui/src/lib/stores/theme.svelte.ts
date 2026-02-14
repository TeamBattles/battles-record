type ThemeMode = 'system' | 'light' | 'dark';

function createThemeStore() {
	let mode = $state<ThemeMode>('system');
	let resolvedTheme = $state<'light' | 'dark'>('dark');

	function getSystemTheme(): 'light' | 'dark' {
		if (typeof window === 'undefined') return 'dark';
		return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
	}

	function applyTheme(theme: 'light' | 'dark') {
		if (typeof document === 'undefined') return;

		if (theme === 'dark') {
			document.documentElement.classList.add('dark');
		} else {
			document.documentElement.classList.remove('dark');
		}
		resolvedTheme = theme;
	}

	function updateTheme() {
		const theme = mode === 'system' ? getSystemTheme() : mode;
		applyTheme(theme);
	}

	function setMode(newMode: ThemeMode) {
		mode = newMode;
		if (typeof localStorage !== 'undefined') {
			localStorage.setItem('theme', newMode);
		}
		updateTheme();
	}

	function cycle() {
		const modes: ThemeMode[] = ['system', 'light', 'dark'];
		const currentIndex = modes.indexOf(mode);
		const nextIndex = (currentIndex + 1) % modes.length;
		setMode(modes[nextIndex]);
	}

	function init() {
		// Load saved preference
		if (typeof localStorage !== 'undefined') {
			const saved = localStorage.getItem('theme') as ThemeMode | null;
			if (saved && ['system', 'light', 'dark'].includes(saved)) {
				mode = saved;
			}
		}

		// Apply initial theme
		updateTheme();

		// Listen for system theme changes
		if (typeof window !== 'undefined') {
			const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
			const handler = () => {
				if (mode === 'system') {
					updateTheme();
				}
			};
			mediaQuery.addEventListener('change', handler);
			return () => mediaQuery.removeEventListener('change', handler);
		}
	}

	return {
		get mode() {
			return mode;
		},
		get resolvedTheme() {
			return resolvedTheme;
		},
		get isDark() {
			return resolvedTheme === 'dark';
		},
		setMode,
		cycle,
		init
	};
}

export const themeStore = createThemeStore();
