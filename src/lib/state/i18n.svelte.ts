let locale: string = $state('en');
let translations: Map<string, string> = $state(new Map());

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
 * Change the active locale. Dynamically imports the locale JSON file
 * and replaces the translations Map, triggering reactive updates
 * across all components that call t().
 */
export async function changeLocale(newLocale: string): Promise<void> {
	if (newLocale === locale) {
		return;
	}

	const module = await import(`../i18n/locales/${newLocale}.json`);
	const entries: Record<string, string> = module.default ?? module;

	const newMap = new Map<string, string>();
	for (const key of Object.keys(entries)) {
		newMap.set(key, entries[key]);
	}

	translations = newMap;
	locale = newLocale;
}

/**
 * Get the current locale value (reactive via $state).
 */
export function getLocale(): string {
	return locale;
}

/**
 * Eagerly load the default English locale at module initialization.
 */
async function init(): Promise<void> {
	const module = await import('../i18n/locales/en.json');
	const entries: Record<string, string> = module.default ?? module;

	const newMap = new Map<string, string>();
	for (const key of Object.keys(entries)) {
		newMap.set(key, entries[key]);
	}

	translations = newMap;
}

init();
