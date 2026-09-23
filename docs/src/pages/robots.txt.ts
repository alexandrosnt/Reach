import type { APIRoute } from 'astro';

/**
 * robots.txt, generated so the sitemap line follows the configured domain
 * rather than being hard-coded to whatever the site was called last week.
 *
 * Everything is allowed, including the crawlers that train and retrieve for
 * language models. For an open source tool that is the point: being quotable
 * by an assistant is how a lot of people will hear about it at all, and there
 * is nothing here that is not already public in the repository. The model
 * crawlers are named explicitly rather than left to the wildcard because
 * several of them look for their own block first and stop there.
 */

const AI_CRAWLERS = [
	'GPTBot',
	'OAI-SearchBot',
	'ChatGPT-User',
	'ClaudeBot',
	'Claude-User',
	'Claude-SearchBot',
	'PerplexityBot',
	'Perplexity-User',
	'Google-Extended',
	'Applebot-Extended',
	'CCBot',
	'Bytespider',
	'meta-externalagent',
	'Amazonbot',
	'cohere-ai',
	'Diffbot',
	'DuckAssistBot',
	'MistralAI-User',
	'YouBot',
];

export const GET: APIRoute = ({ site }) => {
	// BASE_URL comes back exactly as configured — '/Reach', with no trailing
	// slash — so joining onto it directly yields '/Reachllms.txt'.
	const base = import.meta.env.BASE_URL.replace(/\/?$/, '/');
	const at = (file: string) => (site ? new URL(`${base}${file}`, site).href : `/${file}`);

	const lines = [
		'# Everything here is public, and mirrors the repository.',
		'User-agent: *',
		'Allow: /',
		'',
		'# Named explicitly: several of these read their own block before the',
		'# wildcard, and being readable by an assistant is how people find this.',
		...AI_CRAWLERS.map((agent) => `User-agent: ${agent}`),
		'Allow: /',
		'',
		`# The documentation as plain markdown: ${at('llms.txt')}`,
		`# All of it in a single file: ${at('llms-full.txt')}`,
		'',
		`Sitemap: ${at('sitemap-index.xml')}`,
		'',
	];

	return new Response(lines.join('\n'), {
		headers: { 'Content-Type': 'text/plain; charset=utf-8' },
	});
};
