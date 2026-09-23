/**
 * Tell the search engines the site changed, instead of waiting to be crawled.
 *
 * A sitemap is passive: it says what exists and leaves the engine to come
 * looking whenever it feels like it, which for a new domain is days to weeks.
 * IndexNow inverts that — one request names the URLs that changed and the
 * engines fetch them, typically within minutes.
 *
 * Supported by Bing, Yandex, Seznam and Naver, and submitting to any one of
 * them shares the notification with the rest. Bing is the one that matters
 * most here: its index is what ChatGPT's search reads, so this is the
 * shortest path to an assistant being able to find Reach at all.
 *
 * Google does not participate — it has said it is evaluating the protocol and
 * nothing more — so Search Console still has to be set up separately. This
 * complements a sitemap rather than replacing it.
 *
 * Authentication is a shared secret that is public by design: a key file
 * served from the domain root proves whoever is submitting controls the site.
 * There is nothing to leak, which is why the key is committed.
 *
 * Reads the sitemap from the live site rather than from dist/, so it can only
 * ever submit URLs that are actually serving — running it against a local
 * build would announce pages that are still a deploy away, and the engines
 * would fetch a 404 and hold it against the domain.
 *
 * Run after a deploy: node scripts/indexnow.mjs
 */

import { readdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const PUBLIC = fileURLToPath(new URL('../public/', import.meta.url));
/*
 * Every participant, rather than only the shared endpoint.
 *
 * api.indexnow.org is meant to distribute a submission to all of them, but it
 * is operated by Bing and inherits Bing's refusals: a domain Bing has never
 * crawled gets 403 UserForbiddedToAccessSite there, and the notification
 * reaches nobody. Yandex accepts the identical key and payload, which is how
 * that was diagnosed. So each engine is told directly and one refusal costs
 * nothing.
 */
const ENDPOINTS = [
	['IndexNow (Bing)', 'https://api.indexnow.org/IndexNow'],
	['Yandex', 'https://yandex.com/indexnow'],
	['Seznam', 'https://search.seznam.cz/indexnow'],
	['Naver', 'https://searchadvisor.naver.com/indexnow'],
];

const site = (process.env.SITE_URL ?? 'https://reachssh.com').replace(/\/$/, '');
const host = new URL(site).host;

/** The key is whichever bare hex .txt file sits in public/, so rotating it is
 *  a matter of replacing that file and nothing else. */
const keyFile = (await readdir(PUBLIC)).find((f) => /^[0-9a-f]{8,128}\.txt$/.test(f));
if (!keyFile) {
	console.error('No IndexNow key file in public/. Expected something like <hex>.txt');
	process.exit(1);
}
const key = keyFile.replace(/\.txt$/, '');

const locs = (xml) => [...xml.matchAll(/<loc>([^<]+)<\/loc>/g)].map((m) => m[1]);

const get = async (url) => {
	const r = await fetch(url, { headers: { 'User-Agent': 'reach-indexnow/1.0' } });
	if (!r.ok) throw new Error(`${url} returned ${r.status}`);
	return r.text();
};

/** Every page currently published, read from the site's own sitemap. */
async function urlsFromSitemap() {
	const index = await get(`${site}/sitemap-index.xml`);
	const urls = new Set();
	for (const child of locs(index)) {
		for (const url of locs(await get(child))) urls.add(url);
	}
	return [...urls];
}

const urlList = await urlsFromSitemap();
if (urlList.length === 0) {
	console.error('The live sitemap listed no URLs.');
	process.exit(1);
}

const payload = JSON.stringify({
	host,
	key,
	keyLocation: `${site}/${keyFile}`,
	urlList,
});

let accepted = 0;

for (const [name, endpoint] of ENDPOINTS) {
	try {
		const response = await fetch(endpoint, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json; charset=utf-8' },
			body: payload,
		});
		// 200 accepted, 202 accepted while the key is still being verified.
		const ok = response.status === 200 || response.status === 202;
		if (ok) accepted += 1;
		const body = (await response.text().catch(() => '')).trim().replace(/\s+/g, ' ');
		console.log(
			`${name.padEnd(18)} ${response.status} ${response.statusText}` +
				(body ? ` — ${body.slice(0, 120)}` : ''),
		);
	} catch (error) {
		console.log(`${name.padEnd(18)} unreachable — ${String(error).slice(0, 80)}`);
	}
}

console.log(`
${urlList.length} URLs submitted, ${accepted}/${ENDPOINTS.length} engines accepted.`);

// Never fail a deploy over a notification: the pages are live either way and
// the sitemap still does its job.
if (accepted === 0) {
	console.warn('No engine accepted the submission. Check the key file at ' + `${site}/${keyFile}`);
}
