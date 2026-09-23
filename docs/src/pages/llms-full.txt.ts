import type { APIRoute } from 'astro';
import { getCollection } from 'astro:content';
import { pageAsMarkdown, toPlainMarkdown } from '../lib/markdown';
import { PRODUCT } from '../site';

/**
 * llms-full.txt — every documentation page as one markdown file.
 *
 * The companion to llms.txt: that one is a map to follow, this one is the
 * whole thing in a single fetch, for a reader that would rather pay one
 * request than thirty. Same source as the rendered pages, so it cannot drift.
 *
 * The bodies are the raw markdown, minus the frontmatter Astro has already
 * parsed off and minus the import lines and component tags that only mean
 * something once Starlight has rendered them. That cleanup lives in
 * lib/markdown.ts because the per-page .md routes need exactly the same
 * thing, and two copies would drift.
 */

/** Sidebar order, so a linear read follows the intended path. */
const ORDER = ['download', 'getting-started', 'features', 'vault', 'sync', 'settings', 'development'];

const rank = (slug: string) => {
	const at = ORDER.findIndex((prefix) => slug === prefix || slug.startsWith(`${prefix}/`));
	return at === -1 ? ORDER.length : at;
};

/** BASE_URL comes back exactly as configured, which may be '/Reach' with no
 *  trailing slash. Joining paths onto that silently produces '/Reachdownload'. */
const base = import.meta.env.BASE_URL.replace(/\/?$/, '/');

export const GET: APIRoute = async ({ site }) => {
	const docs = await getCollection('docs');
	const url = (slug: string) =>
		site ? new URL(`${base}${slug}/`.replace('//', '/'), site).href : `/${slug}/`;

	const pages = docs
		// The landing page is a component, not prose: it would contribute
		// nothing here but markup.
		.filter((entry) => entry.id !== '')
		.sort((a, b) => rank(a.id) - rank(b.id) || a.id.localeCompare(b.id))
		.map((entry) => {
			const head = [
				`# ${entry.data.title}`,
				'',
				entry.data.description ? `> ${entry.data.description}` : '',
				'',
				`Source: ${url(entry.id)}`,
				'',
			]
				.filter((l, i, all) => !(l === '' && all[i - 1] === ''))
				.join('\n');
			return `${head}\n${toPlainMarkdown(entry.body)}`;
		});

	const body = `# ${PRODUCT.name} — complete documentation

> ${PRODUCT.description}

${PRODUCT.licence} licensed. Source: ${PRODUCT.repo}
Platforms: ${PRODUCT.platforms.join(', ')}
Generated from the documentation source, so it matches the site exactly.

---

${pages.join('\n\n---\n\n')}
`;

	return new Response(body, {
		headers: { 'Content-Type': 'text/plain; charset=utf-8' },
	});
};
