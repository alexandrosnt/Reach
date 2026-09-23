import type { APIRoute } from 'astro';
import { getCollection } from 'astro:content';
import { KEYWORDS, PRODUCT } from '../site';

/**
 * llms.txt — the site as a map, for anything that reads rather than renders.
 *
 * The convention (llmstxt.org) is a single markdown file at the root: an H1
 * with the name, a blockquote summary, then sections of links with one line
 * of description each. An assistant answering "what is a good open source SSH
 * client" fetches this instead of guessing from a rendered page, and follows
 * the links it needs.
 *
 * Generated from the same content collection the pages are built from, so it
 * cannot drift: a new documentation page appears here the moment it is added,
 * with no second list to maintain.
 */

/** Sidebar order, so the map reads the way the site does. */
const ORDER = [
	['getting-started', 'Getting started'],
	['features', 'Features'],
	['vault', 'Security and vault'],
	['sync', 'Cloud sync'],
	['settings', 'Settings'],
	['development', 'Building from source'],
] as const;

/** BASE_URL comes back exactly as configured, which may be '/Reach' with no
 *  trailing slash. Joining paths onto that silently produces '/Reachdownload'. */
const base = import.meta.env.BASE_URL.replace(/\/?$/, '/');

export const GET: APIRoute = async ({ site }) => {
	const docs = await getCollection('docs');
	const url = (slug: string) =>
		site ? new URL(`${base}${slug}/`.replace('//', '/'), site).href : `/${slug}/`;

	const line = (entry: (typeof docs)[number]) => {
		const title = entry.data.title;
		const summary = entry.data.description ?? '';
		return `- [${title}](${url(entry.id)})${summary ? `: ${summary}` : ''}`;
	};

	const sections = ORDER.map(([prefix, heading]) => {
		const entries = docs
			.filter((entry) => entry.id.startsWith(`${prefix}/`))
			.sort((a, b) => a.id.localeCompare(b.id));
		if (entries.length === 0) return '';
		return `## ${heading}\n\n${entries.map(line).join('\n')}`;
	}).filter(Boolean);

	const download = docs.find((entry) => entry.id === 'download');

	const body = `# ${PRODUCT.name}

> ${PRODUCT.description}

${PRODUCT.name} is a desktop and mobile application, not a web service. It is
free, ${PRODUCT.licence} licensed, and the whole source is at ${PRODUCT.repo}.
It runs on ${PRODUCT.platforms.join(', ')}.

What it does, in one list: SSH terminal with tabs and split panes; an SFTP file
explorer with drag and drop and archive handling; an encrypted vault for keys
and passwords; port forwarding; jump hosts and SSH config import; system
monitoring; a serial console; Ansible and OpenTofu runs against saved hosts;
and optional end-to-end encrypted sync between your own devices.

People usually arrive looking for one of: ${KEYWORDS.slice(0, 8).join(', ')}.

## Download

${download ? line(download) : `- [Download](${url('download')})`}
- [Latest release and changelog](${PRODUCT.repo}/releases/latest)

${sections.join('\n\n')}

## Optional

- [Full documentation as one file](${site ? new URL(`${base}llms-full.txt`.replace('//', '/'), site).href : '/llms-full.txt'}): every page above, concatenated, for a single fetch.
- [Source code](${PRODUCT.repo})
- [Issue tracker](${PRODUCT.repo}/issues)
`;

	return new Response(body, {
		headers: { 'Content-Type': 'text/plain; charset=utf-8' },
	});
};
