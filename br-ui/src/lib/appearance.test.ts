import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';
import {
	APPEARANCE_STORAGE_KEY,
	DEFAULT_APPEARANCE,
	LEGACY_THEME_STORAGE_KEY,
	appearanceDom,
	applyAppearanceDom,
	parseStoredAppearance,
	sanitizeAppearance
} from './appearance';

describe('parseStoredAppearance', () => {
	it('fills gaps and drops invalid values from stored preferences', () => {
		const prefs = parseStoredAppearance(
			JSON.stringify({ theme: 'graphite', accent: 'purple', radius: 'round', density: 42 }),
			null
		);
		expect(prefs).toEqual({ ...DEFAULT_APPEARANCE, theme: 'graphite', radius: 'round' });
	});

	it('migrates the pre-redesign theme key when nothing new is stored', () => {
		expect(parseStoredAppearance(null, 'dark').theme).toBe('black');
		expect(parseStoredAppearance(null, 'light').theme).toBe('light');
		expect(parseStoredAppearance('{not json', 'system').theme).toBe('system');
	});

	it('prefers stored preferences over the legacy key', () => {
		expect(parseStoredAppearance(JSON.stringify({ theme: 'dark' }), 'light').theme).toBe('dark');
	});

	it('falls back to defaults for junk', () => {
		expect(sanitizeAppearance('nope')).toEqual(DEFAULT_APPEARANCE);
		expect(parseStoredAppearance(null, 'sepia')).toEqual(DEFAULT_APPEARANCE);
	});
});

describe('appearanceDom', () => {
	it('omits every optional axis at its default, since app.css has no selector for defaults', () => {
		const dom = appearanceDom({ ...DEFAULT_APPEARANCE }, false);
		expect(dom.theme).toBe('black');
		expect(dom.dark).toBe(true);
		expect(Object.values(dom.attrs).every((value) => value === null)).toBe(true);
	});

	it('resolves system against the OS scheme and drops the dark gate on bone', () => {
		const dom = appearanceDom({ ...DEFAULT_APPEARANCE, theme: 'system' }, true);
		expect(dom.theme).toBe('light');
		expect(dom.dark).toBe(false);
	});
});

/**
 * app.html applies the stored appearance before the bundle loads with a hand-written copy of
 * parseStoredAppearance + appearanceDom. If the two drift, startup flashes (or keeps) the wrong
 * theme until the store re-applies, so run the real script against the same inputs.
 */
describe('app.html pre-paint script', () => {
	const html = readFileSync(resolve(process.cwd(), 'src/app.html'), 'utf8');
	const script = html.match(/<script>([\s\S]*?)<\/script>/)?.[1] ?? '';
	const ATTRS = [
		'data-theme',
		'data-accent',
		'data-radius',
		'data-density',
		'data-contrast',
		'data-motion'
	];

	function snapshot(el: Element) {
		return {
			dark: el.classList.contains('dark'),
			attrs: Object.fromEntries(ATTRS.map((name) => [name, el.getAttribute(name)]))
		};
	}

	function runPrePaint(stored: string | null, legacy: string | null, prefersLight: boolean) {
		localStorage.clear();
		if (stored !== null) localStorage.setItem(APPEARANCE_STORAGE_KEY, stored);
		if (legacy !== null) localStorage.setItem(LEGACY_THEME_STORAGE_KEY, legacy);
		const root = document.documentElement;
		for (const name of ATTRS) root.removeAttribute(name);
		root.classList.remove('dark');
		window.matchMedia = ((query: string) => ({
			matches: query.includes('light') ? prefersLight : !prefersLight
		})) as unknown as typeof window.matchMedia;
		new Function(script)();
		return snapshot(root);
	}

	function expected(stored: string | null, legacy: string | null, prefersLight: boolean) {
		const el = document.createElement('div');
		applyAppearanceDom(appearanceDom(parseStoredAppearance(stored, legacy), prefersLight), el);
		return snapshot(el);
	}

	// happy-dom exposes no localStorage without an origin; the script only needs get/set.
	beforeAll(() => {
		const data = new Map<string, string>();
		vi.stubGlobal('localStorage', {
			getItem: (key: string) => data.get(key) ?? null,
			setItem: (key: string, value: string) => void data.set(key, String(value)),
			removeItem: (key: string) => void data.delete(key),
			clear: () => data.clear()
		});
	});
	afterAll(() => vi.unstubAllGlobals());

	const cases: [string, string | null, string | null, boolean][] = [
		['fresh install', null, null, false],
		['legacy dark', null, 'dark', false],
		['legacy light', null, 'light', false],
		['legacy system on a light OS', null, 'system', true],
		[
			'every axis changed',
			JSON.stringify({
				theme: 'graphite',
				accent: 'lime',
				radius: 'sharp',
				density: 'compact',
				contrast: 'high',
				motion: 'reduced'
			}),
			null,
			false
		],
		['system on a light OS', JSON.stringify({ theme: 'system', accent: 'cyan' }), null, true],
		[
			'invalid values',
			JSON.stringify({ theme: 'sepia', accent: 'red', radius: 'huge' }),
			'light',
			false
		],
		['corrupt JSON with a legacy key', '{oops', 'light', false],
		['non-object JSON with a legacy key', 'null', 'light', false],
		[
			'stored object without a theme beside a legacy key',
			JSON.stringify({ accent: 'amber' }),
			'light',
			false
		]
	];

	it.each(cases)('matches appearanceDom: %s', (_label, stored, legacy, prefersLight) => {
		expect(runPrePaint(stored, legacy, prefersLight)).toEqual(
			expected(stored, legacy, prefersLight)
		);
	});
});
