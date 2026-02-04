const translations: Map<string, string> = new Map();
let currentLocaleValue = 'en';

/**
 * Load a locale's translations from its JSON file into the translations Map.
 * The English locale is loaded eagerly at module init; others are lazy-loaded.
 */
export async function loadLocale(locale: string): Promise<void> {
	const module = await import(`./locales/${locale}.json`);
	const entries: Record<string, string> = module.default ?? module;

	translations.clear();
	for (const key of Object.keys(entries)) {
		translations.set(key, entries[key]);
	}

	currentLocaleValue = locale;
}

/**
 * Translate a key with optional template interpolation.
 * Performs O(1) Map lookup. Replaces {{param}} patterns with provided values.
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
 * Get the current locale string.
 */
export function currentLocale(): string {
	return currentLocaleValue;
}

/**
 * Set and load a new locale. If loading fails, the previous translations remain.
 */
export async function setLocale(locale: string): Promise<void> {
	if (locale === currentLocaleValue) {
		return;
	}
	await loadLocale(locale);
}

// Eagerly load the default English locale
loadLocale('en');
