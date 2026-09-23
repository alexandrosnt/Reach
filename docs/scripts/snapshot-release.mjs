/**
 * Refresh docs/src/data/release.json from the current GitHub release.
 *
 * The site resolves download links while it builds. That normally goes
 * straight to the API, but the API is rate limited per IP address and is not
 * always reachable, so a snapshot of the last release is committed and used
 * whenever the call fails. This keeps that snapshot current.
 *
 * Run by the docs workflow after a release is published, and safe to run by
 * hand: it rewrites nothing unless the fetch succeeded.
 */

import { writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const REPO = 'alexandrosnt/Reach';
const OUT = fileURLToPath(new URL('../src/data/release.json', import.meta.url));

const token = process.env.GITHUB_TOKEN ?? process.env.GH_TOKEN;

const response = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
	headers: {
		Accept: 'application/vnd.github+json',
		...(token ? { Authorization: `Bearer ${token}` } : {}),
	},
});

if (!response.ok) {
	// Not a build failure: the committed snapshot is still perfectly usable.
	console.warn(`GitHub returned ${response.status}; leaving the snapshot as it is.`);
	process.exit(0);
}

const release = await response.json();

// Only the three fields the site reads. Storing the whole payload would put a
// few hundred kilobytes of unrelated API detail into every diff.
const snapshot = {
	tag_name: release.tag_name,
	published_at: release.published_at,
	assets: (release.assets ?? []).map(({ name, size, browser_download_url }) => ({
		name,
		size,
		browser_download_url,
	})),
};

await writeFile(OUT, `${JSON.stringify(snapshot, null, '\t')}\n`);
console.log(`Snapshot updated to ${snapshot.tag_name} (${snapshot.assets.length} assets).`);
