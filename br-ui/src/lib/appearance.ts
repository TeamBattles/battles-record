/**
 * Appearance vocabulary shared with teambattles.gg and Battles Replay: the same axes, ids and
 * defaults, so the three apps can be skinned identically. Framework-agnostic on purpose - the
 * pre-paint script in app.html mirrors `appearanceDom()` by hand, and the reactive controller in
 * stores/appearance.svelte.ts builds on these helpers.
 *
 * A default axis is expressed by OMITTING its data-* attribute: app.css has no selector for
 * accent=red, radius=soft, density=comfortable, contrast=normal or motion=on. Theme is the
 * exception and is always written, because :root carries the navy fallback while the shipped
 * default is Onyx.
 */

export type Theme = 'black' | 'dark' | 'graphite' | 'light' | 'system';
export type EffectiveTheme = Exclude<Theme, 'system'>;
export type Accent = 'red' | 'amber' | 'cyan' | 'lime';
export type Radius = 'sharp' | 'soft' | 'round';
export type Density = 'comfortable' | 'compact';
export type Contrast = 'normal' | 'high';
export type Motion = 'on' | 'reduced';

export interface AppearancePrefs {
	theme: Theme;
	accent: Accent;
	radius: Radius;
	density: Density;
	contrast: Contrast;
	motion: Motion;
}

export const APPEARANCE_STORAGE_KEY = 'br-appearance';
/** The pre-redesign light/dark/system preference, migrated on first load. */
export const LEGACY_THEME_STORAGE_KEY = 'theme';

export const DEFAULT_APPEARANCE: Readonly<AppearancePrefs> = Object.freeze({
	theme: 'black',
	accent: 'red',
	radius: 'soft',
	density: 'comfortable',
	contrast: 'normal',
	motion: 'on'
});

export const THEME_OPTIONS: readonly { value: Theme; label: string }[] = [
	{ value: 'black', label: 'Onyx' },
	{ value: 'dark', label: 'Navy' },
	{ value: 'graphite', label: 'Graphite' },
	{ value: 'light', label: 'Bone' },
	{ value: 'system', label: 'System' }
];

export const ACCENT_OPTIONS: readonly { value: Accent; label: string }[] = [
	{ value: 'red', label: 'Red' },
	{ value: 'amber', label: 'Amber' },
	{ value: 'cyan', label: 'Cyan' },
	{ value: 'lime', label: 'Lime' }
];

export const RADIUS_OPTIONS: readonly { value: Radius; label: string }[] = [
	{ value: 'sharp', label: 'Sharp' },
	{ value: 'soft', label: 'Soft' },
	{ value: 'round', label: 'Round' }
];

export const DENSITY_OPTIONS: readonly { value: Density; label: string }[] = [
	{ value: 'comfortable', label: 'Comfortable' },
	{ value: 'compact', label: 'Compact' }
];

/**
 * Preview colors for the theme and accent pickers, copied from the matching app.css blocks.
 * The pickers paint every theme at once, so they cannot read the live tokens; keep in step.
 */
export const THEME_SWATCHES: Record<
	EffectiveTheme,
	{ bg: string; panel: string; surface: string; line: string; text: string; muted: string }
> = {
	black: {
		bg: '#0a0a0a',
		panel: '#0f0f0f',
		surface: '#171717',
		line: '#1f1f1f',
		text: '#fafafa',
		muted: '#888888'
	},
	dark: {
		bg: '#04080f',
		panel: '#0e1b2c',
		surface: '#182c44',
		line: 'rgba(150, 186, 232, 0.12)',
		text: '#eef4fc',
		muted: '#7589a2'
	},
	graphite: {
		bg: '#16181c',
		panel: '#1d2027',
		surface: '#2b303a',
		line: 'rgba(180, 188, 200, 0.18)',
		text: '#f2f3f5',
		muted: '#8a909b'
	},
	light: {
		bg: '#f4efe4',
		panel: '#fffdf7',
		surface: '#ece6d8',
		line: 'rgba(40, 36, 27, 0.12)',
		text: '#1a1814',
		muted: '#6b665c'
	}
};

/** Each accent as tuned for a theme: brighter on graphite, deeper on bone for AA. */
export const ACCENT_SWATCHES: Record<EffectiveTheme, Record<Accent, string>> = {
	black: { red: '#ef4444', amber: '#f59e0b', cyan: '#06b6d4', lime: '#a3e635' },
	dark: { red: '#ef4444', amber: '#f59e0b', cyan: '#06b6d4', lime: '#a3e635' },
	graphite: { red: '#f87171', amber: '#fbbf24', cyan: '#22d3ee', lime: '#bef264' },
	light: { red: '#dc2626', amber: '#b45309', cyan: '#0e7490', lime: '#4d7c0f' }
};

const ALLOWED: { [K in keyof AppearancePrefs]: readonly AppearancePrefs[K][] } = {
	theme: THEME_OPTIONS.map((o) => o.value),
	accent: ACCENT_OPTIONS.map((o) => o.value),
	radius: RADIUS_OPTIONS.map((o) => o.value),
	density: DENSITY_OPTIONS.map((o) => o.value),
	contrast: ['normal', 'high'],
	motion: ['on', 'reduced']
};

/** Complete, valid preferences from whatever storage held (missing, partial or corrupt). */
export function sanitizeAppearance(raw: unknown): AppearancePrefs {
	const source = raw && typeof raw === 'object' ? (raw as Record<string, unknown>) : {};
	const prefs: AppearancePrefs = { ...DEFAULT_APPEARANCE };
	for (const key of Object.keys(ALLOWED) as (keyof AppearancePrefs)[]) {
		const value = source[key];
		if ((ALLOWED[key] as readonly unknown[]).includes(value)) {
			(prefs as unknown as Record<string, unknown>)[key] = value;
		}
	}
	return prefs;
}

/**
 * Maps the old three-way theme switch onto the new themes: its dark mode becomes Onyx (the closest
 * surface), light becomes Bone, and system stays system.
 */
export function migrateLegacyTheme(legacy: string | null): Theme | null {
	if (legacy === 'dark') return 'black';
	if (legacy === 'light') return 'light';
	if (legacy === 'system') return 'system';
	return null;
}

/**
 * Stored JSON -> preferences. The legacy theme key is consulted only when no stored object exists
 * (missing, corrupt or non-object JSON) - the same rule the pre-paint script in app.html applies.
 */
export function parseStoredAppearance(
	stored: string | null,
	legacy: string | null
): AppearancePrefs {
	if (stored) {
		try {
			const parsed: unknown = JSON.parse(stored);
			if (parsed && typeof parsed === 'object') return sanitizeAppearance(parsed);
		} catch {
			// Corrupt JSON falls through to the legacy key / defaults.
		}
	}
	const migrated = migrateLegacyTheme(legacy);
	return migrated ? { ...DEFAULT_APPEARANCE, theme: migrated } : { ...DEFAULT_APPEARANCE };
}

/** "System" follows the OS, choosing between Onyx and Bone as teambattles.gg does. */
export function effectiveTheme(theme: Theme, prefersLight: boolean): EffectiveTheme {
	if (theme === 'system') return prefersLight ? 'light' : 'black';
	return theme;
}

export interface AppearanceDom {
	theme: EffectiveTheme;
	/** Whether the `.dark` Tailwind variant gate belongs on <html>. */
	dark: boolean;
	/** Optional axes keyed by attribute name; null means remove (the axis is at its default). */
	attrs: Record<
		'data-accent' | 'data-radius' | 'data-density' | 'data-contrast' | 'data-motion',
		string | null
	>;
}

export function appearanceDom(prefs: AppearancePrefs, prefersLight: boolean): AppearanceDom {
	const theme = effectiveTheme(prefs.theme, prefersLight);
	const value = <K extends keyof AppearancePrefs>(key: K) =>
		prefs[key] === DEFAULT_APPEARANCE[key] ? null : prefs[key];
	return {
		theme,
		dark: theme !== 'light',
		attrs: {
			'data-accent': value('accent'),
			'data-radius': value('radius'),
			'data-density': value('density'),
			'data-contrast': value('contrast'),
			'data-motion': value('motion')
		}
	};
}

/**
 * Writes the derived attributes onto <html> and <body>. Body carries data-theme too so anything
 * portalled outside the app root still re-skins.
 */
export function applyAppearanceDom(
	dom: AppearanceDom,
	root: HTMLElement,
	body?: HTMLElement | null
) {
	root.setAttribute('data-theme', dom.theme);
	body?.setAttribute('data-theme', dom.theme);
	root.classList.toggle('dark', dom.dark);
	for (const [name, value] of Object.entries(dom.attrs)) {
		if (value === null) root.removeAttribute(name);
		else root.setAttribute(name, value);
	}
}
