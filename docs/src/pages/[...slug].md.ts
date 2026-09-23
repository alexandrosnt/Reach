import type { APIRoute, GetStaticPaths } from 'astro';
import { getCollection } from 'astro:content';
import { pageAsMarkdown, type DocEntry } from '../lib/markdown';

/**
 * Every documentation page, also served as markdown.
 *
 * `/features/tunnels/` renders HTML for people; `/features/tunnels.md` hands
 * the same page to anything that would rather have prose than a parsed
 * document. It is the convention the larger documentation sites settled on —
 * Anthropic's and Stripe's docs both answer on `.md` — and it is what makes a
 * site genuinely readable by an assistant rather than merely crawlable.
 *
 * Generated from the same content collection the pages are built from, so a
 * new page appears here the moment it is written and neither copy can drift.
 *
 * This complements llms.txt rather than repeating it: that file is a map of
 * the site, llms-full.txt is everything in one fetch, and this is a single
 * page for a reader that already knows which one it wants.
 */

export const getStaticPaths: GetStaticPaths = async () => {
	const docs = await getCollection('docs');
	return (
		docs
			// The landing page is a component, not prose. It would contribute
			// markup and nothing readable.
			.filter((entry) => entry.id !== '' && entry.id !== 'index')
			.map((entry) => ({ params: { slug: entry.id }, props: { entry } }))
	);
};

export const GET: APIRoute = ({ props, site }) => {
	const entry = props.entry as DocEntry;
	const base = import.meta.env.BASE_URL.replace(/\/?$/, '/');
	const canonical = site
		? new URL(`${base}${entry.id}/`, site).href
		: `/${entry.id}/`;

	return new Response(pageAsMarkdown(entry, canonical), {
		headers: {
			'Content-Type': 'text/markdown; charset=utf-8',
			// Safe to cache: it changes only when the site is rebuilt.
			'Cache-Control': 'public, max-age=3600',
		},
	});
};
