/**
 * Turning a documentation entry back into plain markdown.
 *
 * The pages are authored in markdown, but what a reader fetches is rendered
 * HTML wrapped in navigation, a sidebar, a search widget and a pile of script
 * tags. A machine asking what Reach does has to wade through all of it to
 * reach three paragraphs of prose.
 *
 * So every page is also served as markdown at its own path with `.md` on the
 * end, the way Anthropic's and Stripe's documentation do it. Same source as
 * the HTML, so the two cannot drift, and the same source as llms-full.txt,
 * which is why this lives here rather than being written twice.
 */

/** The shape both callers need from a content collection entry. */
export interface DocEntry {
	id: string;
	body?: string;
	data: { title: string; description?: string };
}

/**
 * Strip what only exists for the renderer.
 *
 * The import lines and component tags in an .mdx file mean nothing without
 * Starlight to render them, and left in they are noise in every fetch. The
 * prose around them is what carries the answer.
 */
export function toPlainMarkdown(body: string): string {
	return (
		body
			// `import X from '...'` at the top of an .mdx file.
			.replace(/^import\s+.+?from\s+['"].+?['"];?\s*$/gm, '')
			// Component tags left on their own line, paired or self-closing.
			.replace(/^<\/?[A-Z][\w.]*(\s[^>]*)?\/?>\s*$/gm, '')
			.replace(/\n{3,}/g, '\n\n')
			.trim()
	);
}

/**
 * One page as a standalone markdown document.
 *
 * The title and description are repeated above the body because a fetcher
 * that lands here has no page around it to read them from, and the canonical
 * link is included so anything quoting this can cite the real URL rather than
 * the .md one.
 */
export function pageAsMarkdown(entry: DocEntry, canonical: string): string {
	const head = [
		`# ${entry.data.title}`,
		'',
		entry.data.description ? `> ${entry.data.description}` : '',
		'',
		`Source: ${canonical}`,
		'',
	].filter((line, i, all) => !(line === '' && all[i - 1] === ''));

	return `${head.join('\n')}\n${toPlainMarkdown(entry.body ?? '')}\n`;
}
