/**
 * The languages Reach ships.
 *
 * One list, because there were two: the setup wizard and Settings > General
 * each kept their own copy, so a locale added to one was silently missing from
 * the other. A new translation is now a locale JSON plus one line here.
 *
 * `name` is the language in itself — someone who cannot read the current UI
 * language still has to find their own. `english` is the endonym's English
 * name, shown alongside so the list stays scannable for everyone else.
 */
export interface Language {
	/** Locale code; must match a file in `./locales/<code>.json`. */
	code: string;
	/** Endonym — the language's name in that language. */
	name: string;
	/** English name, shown as a subtitle. */
	english: string;
}

export const LANGUAGES: Language[] = [
	{ code: 'en', name: 'English', english: 'English' },
	{ code: 'de', name: 'Deutsch', english: 'German' },
	{ code: 'es', name: 'Español', english: 'Spanish' },
	{ code: 'fr', name: 'Français', english: 'French' },
	{ code: 'el', name: 'Ελληνικά', english: 'Greek' },
	{ code: 'it', name: 'Italiano', english: 'Italian' },
	{ code: 'bg', name: 'Български', english: 'Bulgarian' },
	{ code: 'ru', name: 'Русский', english: 'Russian' },
	{ code: 'zh', name: '中文', english: 'Chinese' }
];

/** Look up a language, falling back to English rather than returning undefined. */
export function findLanguage(code: string): Language {
	return LANGUAGES.find((l) => l.code === code) ?? LANGUAGES[0];
}
