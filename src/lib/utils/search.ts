/**
 * Text matching for pickers.
 *
 * Shared because the language picker and the theme marketplace both need the
 * same thing, and both need it to survive a registry that grows: filtering is
 * what keeps a list usable once it is longer than a screen.
 */

/**
 * Fold case and strip diacritics.
 *
 * So "francais" finds "Français", "espanol" finds "Español", and "nord" finds
 * "Nörd". Someone reaching for their own language is often on a keyboard that
 * cannot produce its accents, which is most of the point.
 */
export function fold(value: string): string {
	return value.normalize('NFD').replace(/\p{Diacritic}/gu, '').toLowerCase();
}

/**
 * True when every whitespace-separated term in `query` appears in at least one
 * of `fields`. Term-wise rather than substring-wise so "solar light" finds
 * "Solarized Light" — word order is not something a user should have to guess.
 *
 * An empty query matches everything; callers decide whether that means "show
 * all" or "show nothing".
 */
export function matchesQuery(query: string, fields: (string | undefined)[]): boolean {
	const terms = fold(query).split(/\s+/).filter(Boolean);
	if (terms.length === 0) return true;
	const haystack = fields.filter(Boolean).map((f) => fold(f as string));
	return terms.every((term) => haystack.some((f) => f.includes(term)));
}
