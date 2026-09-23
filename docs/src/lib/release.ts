/**
 * The current release, resolved while the site is built.
 *
 * Asset filenames carry the version, so no fixed URL can name them and they
 * have to be looked up. This used to happen in the browser, which was wrong in
 * a way that only shows up in the field: GitHub allows sixty anonymous API
 * calls per hour per IP address, so a reader behind a company NAT, a VPN or a
 * university network very often gets a 403 and no direct link at all. The
 * download button then did nothing useful for exactly the people least able to
 * work out why.
 *
 * So the lookup moved to build time. Every download link in the built HTML is
 * already the real asset URL, for everyone, with no JavaScript and no API call
 * from the reader's machine. The cost is staleness — a release published after
 * the last build would not be linked — and that is paid for in the workflow:
 * publishing a release triggers a docs rebuild.
 *
 * Two sources, in order. A snapshot of the last release is committed at
 * src/data/release.json, so a build works offline, behind a rate limit, or on
 * a machine with no token, and still emits real download URLs. The API is
 * asked first and wins when it answers, because it is the one that can be
 * newer.
 *
 * Nothing here throws. With both unavailable every link falls back to the
 * releases page, which is correct, just one click longer.
 */

import snapshot from '../data/release.json';

export const REPO = 'alexandrosnt/Reach';
export const RELEASES_URL = `https://github.com/${REPO}/releases/latest`;

export interface Asset {
	name: string;
	size: number;
	browser_download_url: string;
}

export interface Release {
	tag_name?: string;
	assets?: Asset[];
}

let pending: Promise<Release | null> | undefined;

/**
 * Looked up once per build, not once per component: several components ask,
 * and a token-less build has a small allowance to spend.
 */
export function latestRelease(): Promise<Release | null> {
	pending ??= load();
	return pending;
}

async function load(): Promise<Release | null> {
	// Present in Actions, absent locally. Without it the build still works and
	// just shares the anonymous allowance.
	const token = process.env.GITHUB_TOKEN ?? process.env.GH_TOKEN;

	try {
		const response = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
			headers: {
				Accept: 'application/vnd.github+json',
				...(token ? { Authorization: `Bearer ${token}` } : {}),
			},
		});

		if (!response.ok) {
			console.warn(
				`[downloads] GitHub returned ${response.status}; using the committed snapshot (${snapshot.tag_name}).`,
			);
			return snapshot as Release;
		}

		return (await response.json()) as Release;
	} catch (error) {
		console.warn('[downloads] could not reach GitHub, using the committed snapshot:', error);
		return snapshot as Release;
	}
}

/** The first asset whose name ends with one of `patterns`. */
export function findAsset(release: Release | null, patterns: string[]): Asset | undefined {
	if (!release?.assets) return undefined;
	return release.assets.find((a) => patterns.some((p) => p && a.name.endsWith(p)));
}

export function megabytes(bytes: number): string {
	return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

/** What a template needs to render one download link. */
export interface ResolvedDownload {
	href: string;
	size: string;
	/** False when the release did not carry this platform's build. */
	resolved: boolean;
}

export function resolve(release: Release | null, patterns: string[]): ResolvedDownload {
	const asset = findAsset(release, patterns);
	if (!asset) return { href: RELEASES_URL, size: '', resolved: false };
	return { href: asset.browser_download_url, size: megabytes(asset.size), resolved: true };
}
