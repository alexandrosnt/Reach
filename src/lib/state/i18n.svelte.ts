import enLocale from '../i18n/locales/en.json';

let locale: string = $state('en');
let translations: Map<string, string> = $state(new Map());

function loadEntries(entries: Record<string, string>): void {
	const newMap = new Map<string, string>();
	for (const key of Object.keys(entries)) {
		newMap.set(key, entries[key]);
	}
	translations = newMap;
}

/**
 * Translate a key with optional template interpolation.
 * Reads reactively from the translations Map (Svelte 5 $state).
 * Returns the key itself if no translation is found.
 */
export function t(key: string, params?: Record<string, string | number>): string {
	let value = translations.get(key);
	if (value === undefined) {
		return key;
	}

	if (params) {
		for (const param of Object.keys(params)) {
			value = value.replaceAll(`{{${param}}}`, String(params[param]));
		}
	}

	return value;
}

/**
 * Change the active locale. Uses static import for English,
 * dynamic import for other locales. Replaces the translations Map,
 * triggering reactive updates across all components that call t().
 */
export async function changeLocale(newLocale: string): Promise<void> {
	if (newLocale === 'en') {
		loadEntries(enLocale);
	} else {
		const module = await import(`../i18n/locales/${newLocale}.json`);
		const entries: Record<string, string> = module.default ?? module;
		loadEntries(entries);
	}
	locale = newLocale;
}

/**
 * Get the current locale value (reactive via $state).
 */
export function getLocale(): string {
	return locale;
}

// Load English on startup
loadEntries(enLocale);
