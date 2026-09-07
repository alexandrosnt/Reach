/**
 * Theme engine.
 *
 * A theme is data, not code: a map of design tokens plus a terminal palette.
 * Applying one writes CSS custom properties onto the document root, so every
 * component styled with var(--color-...) re-themes instantly, and pushes the
 * terminal palette into xterm, which keeps its colours in JS rather than CSS.
 *
 * Because themes carry no executable code, installing one from the marketplace
 * is far lower risk than installing a plugin — the worst a bad theme can do is
 * look wrong.
 */

/** The 16 ANSI colours plus the terminal chrome colours xterm needs. */
export interface TerminalPalette {
	background: string;
	foreground: string;
	cursor: string;
	cursorAccent: string;
	selectionBackground: string;
	selectionForeground: string;
	black: string;
	red: string;
	green: string;
	yellow: string;
	blue: string;
	magenta: string;
	cyan: string;
	white: string;
	brightBlack: string;
	brightRed: string;
	brightGreen: string;
	brightYellow: string;
	brightBlue: string;
	brightMagenta: string;
	brightCyan: string;
	brightWhite: string;
}

/** UI tokens. Each key maps to a --color-<key> custom property. */
export interface ThemeColors {
	'bg-primary': string;
	'bg-secondary': string;
	'bg-elevated': string;
	border: string;
	'text-primary': string;
	'text-secondary': string;
	'text-tertiary': string;
	accent: string;
	'accent-hover': string;
	success: string;
	warning: string;
	danger: string;
	'surface-hover': string;
	'surface-active': string;
	'surface-sunken': string;
}

export interface Theme {
	id: string;
	name: string;
	author?: string;
	version?: string;
	/** Drives the light/dark class so native scrollbars and controls match. */
	appearance: 'dark' | 'light';
	colors: ThemeColors;
	terminal: TerminalPalette;
}

/** Every key a complete theme must define. Used to validate imported themes. */
export const COLOR_KEYS: (keyof ThemeColors)[] = [
	'bg-primary',
	'bg-secondary',
	'bg-elevated',
	'border',
	'text-primary',
	'text-secondary',
	'text-tertiary',
	'accent',
	'accent-hover',
	'success',
	'warning',
	'danger',
	'surface-hover',
	'surface-active',
	'surface-sunken'
];

export const TERMINAL_KEYS: (keyof TerminalPalette)[] = [
	'background',
	'foreground',
	'cursor',
	'cursorAccent',
	'selectionBackground',
	'selectionForeground',
	'black',
	'red',
	'green',
	'yellow',
	'blue',
	'magenta',
	'cyan',
	'white',
	'brightBlack',
	'brightRed',
	'brightGreen',
	'brightYellow',
	'brightBlue',
	'brightMagenta',
	'brightCyan',
	'brightWhite'
];

export const DARK: Theme = {
	id: 'reach-dark',
	name: 'Reach Dark',
	appearance: 'dark',
	colors: {
		'bg-primary': '#0a0a0a',
		'bg-secondary': '#141414',
		'bg-elevated': '#1c1c1e',
		border: 'rgba(255, 255, 255, 0.08)',
		'text-primary': '#f5f5f7',
		'text-secondary': '#86868b',
		'text-tertiary': '#6e6e73',
		accent: '#0a84ff',
		'accent-hover': '#409cff',
		success: '#30d158',
		warning: '#ffd60a',
		danger: '#ff453a',
		'surface-hover': 'rgba(255, 255, 255, 0.06)',
		'surface-active': 'rgba(255, 255, 255, 0.10)',
		'surface-sunken': 'rgba(0, 0, 0, 0.20)'
	},
	terminal: {
		background: '#0a0a0a',
		foreground: '#f5f5f7',
		cursor: '#0a84ff',
		cursorAccent: '#0a0a0a',
		selectionBackground: 'rgba(10, 132, 255, 0.3)',
		selectionForeground: '#f5f5f7',
		black: '#1d1f21',
		red: '#ff453a',
		green: '#30d158',
		yellow: '#ffd60a',
		blue: '#0a84ff',
		magenta: '#bf5af2',
		cyan: '#64d2ff',
		white: '#f5f5f7',
		brightBlack: '#6e6e73',
		brightRed: '#ff6961',
		brightGreen: '#4ae06a',
		brightYellow: '#ffe566',
		brightBlue: '#409cff',
		brightMagenta: '#da8fff',
		brightCyan: '#8be8ff',
		brightWhite: '#ffffff'
	}
};

export const LIGHT: Theme = {
	id: 'reach-light',
	name: 'Reach Light',
	appearance: 'light',
	colors: {
		'bg-primary': '#ffffff',
		'bg-secondary': '#f5f5f7',
		'bg-elevated': '#ffffff',
		border: 'rgba(0, 0, 0, 0.06)',
		'text-primary': '#1d1d1f',
		'text-secondary': '#86868b',
		'text-tertiary': '#8e8e93',
		accent: '#007aff',
		'accent-hover': '#0071e3',
		success: '#34c759',
		warning: '#ff9f0a',
		danger: '#ff3b30',
		'surface-hover': 'rgba(0, 0, 0, 0.04)',
		'surface-active': 'rgba(0, 0, 0, 0.08)',
		'surface-sunken': 'rgba(0, 0, 0, 0.03)'
	},
	terminal: {
		// The terminal stayed #0a0a0a in light mode before themes existed — a
		// black rectangle filling most of an otherwise white app.
		background: '#ffffff',
		foreground: '#1d1d1f',
		cursor: '#007aff',
		cursorAccent: '#ffffff',
		selectionBackground: 'rgba(0, 122, 255, 0.25)',
		selectionForeground: '#1d1d1f',
		black: '#1d1d1f',
		red: '#d70015',
		green: '#248a3d',
		yellow: '#a05a00',
		blue: '#0040dd',
		magenta: '#9f0d91',
		cyan: '#0071a4',
		white: '#8e8e93',
		brightBlack: '#6e6e73',
		brightRed: '#ff3b30',
		brightGreen: '#34c759',
		brightYellow: '#ff9f0a',
		brightBlue: '#007aff',
		brightMagenta: '#af52de',
		brightCyan: '#55bef0',
		brightWhite: '#1d1d1f'
	}
};

export const BUILT_IN: Theme[] = [DARK, LIGHT];

class ThemeState {
	current = $state<Theme>(DARK);
	/** Themes beyond the built-ins, e.g. installed from the marketplace. */
	installed = $state<Theme[]>([]);

	get all(): Theme[] {
		return [...BUILT_IN, ...this.installed];
	}
}

export const themeState = new ThemeState();

/**
 * Write a theme onto the document.
 *
 * Each token becomes an inline custom property on :root, which beats the
 * stylesheet defaults without needing a class per theme, and the dark/light
 * class is swapped so native chrome stays consistent.
 */
export function applyTheme(theme: Theme): void {
	const root = document.documentElement;
	for (const key of COLOR_KEYS) {
		const value = theme.colors[key];
		if (value) root.style.setProperty('--color-' + key, value);
	}
	root.classList.remove('dark', 'light');
	root.classList.add(theme.appearance);
	themeState.current = theme;
}

/** Reject a theme that would leave part of the app unstyled. */
export function validateTheme(
	input: unknown
): { ok: true; theme: Theme } | { ok: false; error: string } {
	if (typeof input !== 'object' || input === null) {
		return { ok: false, error: 'Theme must be an object' };
	}
	const t = input as Partial<Theme>;
	if (typeof t.id !== 'string' || !t.id) return { ok: false, error: 'Missing id' };
	if (typeof t.name !== 'string' || !t.name) return { ok: false, error: 'Missing name' };
	if (t.appearance !== 'dark' && t.appearance !== 'light') {
		return { ok: false, error: 'appearance must be "dark" or "light"' };
	}
	if (typeof t.colors !== 'object' || t.colors === null) {
		return { ok: false, error: 'Missing colors' };
	}
	if (typeof t.terminal !== 'object' || t.terminal === null) {
		return { ok: false, error: 'Missing terminal palette' };
	}

	const colors = t.colors as ThemeColors;
	const missingColor = COLOR_KEYS.find((k) => typeof colors[k] !== 'string');
	if (missingColor) return { ok: false, error: 'Missing colors.' + missingColor };

	const terminal = t.terminal as TerminalPalette;
	const missingTerm = TERMINAL_KEYS.find((k) => typeof terminal[k] !== 'string');
	if (missingTerm) return { ok: false, error: 'Missing terminal.' + missingTerm };

	return { ok: true, theme: t as Theme };
}
