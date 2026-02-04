export interface Settings {
	theme: 'dark' | 'light' | 'system';
	fontSize: number;
	fontFamily: string;
	defaultShell: string;
	openLastSession: boolean;
	locale: string;
}

const STORAGE_KEY = 'reach-settings';

const defaults: Settings = {
	theme: 'dark',
	fontSize: 14,
	fontFamily: 'monospace',
	defaultShell: '/bin/bash',
	openLastSession: false,
	locale: 'en'
};

let settings = $state<Settings>({ ...defaults });

export function getSettings(): Settings {
	return settings;
}

export function updateSetting<K extends keyof Settings>(key: K, value: Settings[K]): void {
	settings[key] = value;
	saveSettings();
}

export function loadSettings(): void {
	if (typeof localStorage === 'undefined') return;

	try {
		const stored = localStorage.getItem(STORAGE_KEY);
		if (stored) {
			const parsed = JSON.parse(stored) as Partial<Settings>;
			settings.theme = parsed.theme ?? defaults.theme;
			settings.fontSize = parsed.fontSize ?? defaults.fontSize;
			settings.fontFamily = parsed.fontFamily ?? defaults.fontFamily;
			settings.defaultShell = parsed.defaultShell ?? defaults.defaultShell;
			settings.openLastSession = parsed.openLastSession ?? defaults.openLastSession;
			settings.locale = parsed.locale ?? defaults.locale;
		}
	} catch {
		// If parsing fails, keep defaults
	}
}

export function saveSettings(): void {
	if (typeof localStorage === 'undefined') return;

	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
	} catch {
		// Storage might be full or unavailable
	}
}
