import {
	APPEARANCE_STORAGE_KEY,
	DEFAULT_APPEARANCE,
	LEGACY_THEME_STORAGE_KEY,
	THEME_OPTIONS,
	appearanceDom,
	applyAppearanceDom,
	effectiveTheme,
	parseStoredAppearance,
	type AppearancePrefs,
	type EffectiveTheme
} from '#lib/appearance.ts';

/**
 * Live appearance controller. Every change is written to localStorage (read synchronously by the
 * pre-paint script in app.html, so a restart never flashes the old theme) and applied to <html>
 * straight away, so the Welcome Screen and Settings > Appearance re-skin the app while the user is
 * choosing.
 */
class AppearanceStore {
	prefs = $state<AppearancePrefs>({ ...DEFAULT_APPEARANCE });
	prefersLight = $state(false);

	resolvedTheme = $derived<EffectiveTheme>(effectiveTheme(this.prefs.theme, this.prefersLight));

	/** Loads stored preferences, applies them and follows the OS scheme. Returns a cleanup. */
	init(): () => void {
		if (typeof window === 'undefined') return () => {};

		this.prefs = parseStoredAppearance(
			readStorage(APPEARANCE_STORAGE_KEY),
			readStorage(LEGACY_THEME_STORAGE_KEY)
		);

		const media = window.matchMedia?.('(prefers-color-scheme: light)');
		this.prefersLight = !!media?.matches;
		this.apply();

		const onSchemeChange = (event: MediaQueryListEvent) => {
			this.prefersLight = event.matches;
			this.apply();
		};
		media?.addEventListener?.('change', onSchemeChange);
		return () => media?.removeEventListener?.('change', onSchemeChange);
	}

	set(patch: Partial<AppearancePrefs>) {
		this.prefs = { ...this.prefs, ...patch };
		this.persist();
		this.apply();
	}

	reset() {
		this.prefs = { ...DEFAULT_APPEARANCE };
		this.persist();
		this.apply();
	}

	/** Steps through the themes in picker order (the titlebar quick switch). */
	cycleTheme() {
		const order = THEME_OPTIONS.map((option) => option.value);
		const next = order[(order.indexOf(this.prefs.theme) + 1) % order.length];
		this.set({ theme: next });
	}

	private apply() {
		if (typeof document === 'undefined') return;
		applyAppearanceDom(
			appearanceDom(this.prefs, this.prefersLight),
			document.documentElement,
			document.body
		);
	}

	private persist() {
		try {
			localStorage.setItem(APPEARANCE_STORAGE_KEY, JSON.stringify(this.prefs));
			localStorage.removeItem(LEGACY_THEME_STORAGE_KEY);
		} catch {
			// Storage can be unavailable (private mode, quota); the choice still applies this session.
		}
	}
}

function readStorage(key: string): string | null {
	try {
		return localStorage.getItem(key);
	} catch {
		return null;
	}
}

export const appearanceStore = new AppearanceStore();
