/**
 * Which text in the terminal is a link that a click opens.
 *
 * The link addon's own pattern ends a link at `!`, `*`, `'`, `(` and `)`,
 * all of which are legal in a URL and common in long ones: a maps link
 * (`data=!3m1!4b1`), a Wikipedia page with a parenthesis in its title, a
 * sign-in or tracking link. A click then opened only the part before it.
 *
 * Here those characters are part of the link, with two limits that keep the
 * text around a link out of it, as GitHub and VS Code do:
 *  - a parenthesis belongs to the link only when it is closed inside it, so
 *    `(see https://example.com/a)` and a Markdown `[x](https://…)` stop
 *    before their closing `)`;
 *  - the last character cannot be punctuation that usually ends a sentence
 *    or a quote: `. , : ; ! ? ' * ~ ]`.
 *
 * The addon follows a link onto the next row when the terminal wrapped it.
 */

/** Characters a link may contain, other than parentheses. */
const MID = `[^\\s"<>\`{}|\\\\^()]`;
/** A parenthesised part closed inside the link, as in `Mercury_(planet)`. */
const GROUP = `\\(${MID}*\\)`;
/** Characters a link may end with. */
const LAST = `[^\\s"'<>\`{}|\\\\^():;,.!?*~\\[\\]]`;

/** The pattern handed to the link addon. */
export const URL_PATTERN = new RegExp(`https?://(?:${MID}|${GROUP})*(?:${LAST}|${GROUP})`, 'i');

/** The links found in `line`, in order. */
export function findLinks(line: string): string[] {
	return [...line.matchAll(new RegExp(URL_PATTERN.source, 'gi'))].map((m) => m[0]);
}
