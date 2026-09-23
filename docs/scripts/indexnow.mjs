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
const ENDPOINT = 'https://api.indexnow.org/IndexNow';

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

const response = await fetch(ENDPOINT, {
	method: 'POST',
	headers: { 'Content-Type': 'application/json; charset=utf-8' },
	body: JSON.stringify({
		host,
		key,
		keyLocation: `${site}/${keyFile}`,
		urlList,
	}),
});

// 200 accepted, 202 accepted while the key is still being verified. Anything
// else is worth seeing, but none of it should fail a deploy: the sitemap
// still works and the pages are already live.
const body = await response.text().catch(() => '');
console.log(`IndexNow: ${response.status} ${response.statusText} for ${urlList.length} URLs`);
if (body.trim()) console.log(body.trim().slice(0, 300));

if (response.status >= 400) {
	console.warn('Submission rejected. The pages are live regardless; check the key file.');
}
