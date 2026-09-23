/**
 * Who this site is, in one place.
 *
 * The site is moving from a project's documentation to a software company's
 * site that happens to document its products, and the company name and domain
 * are not settled yet. Everything that will change on that day is a constant
 * here or an environment variable, so the move is a config edit rather than a
 * search across thirty files.
 *
 * `SITE_URL` and `BASE_PATH` are read by astro.config.mjs. Set them and
 * nothing else has to move:
 *
 *   SITE_URL=https://alexandrosnt.github.io BASE_PATH=/Reach npm run build
 */

/** The company. One product today, more later; the site is built for both. */
export const ORG = {
	name: process.env.PUBLIC_ORG_NAME ?? 'Reach',
	/** Filled in once the domain is bought. */
	legalName: process.env.PUBLIC_ORG_LEGAL_NAME ?? '',
	github: 'https://github.com/alexandrosnt',
};

/** The product this site leads with. */
export const PRODUCT = {
	name: 'Reach',
	tagline: 'Manage your servers, everywhere.',
	/** Under 160 characters: longer and search results cut it mid-sentence. */
	description:
		'Reach is a free, open source SSH client and remote server manager for Windows, macOS, Linux and Android. Terminals, SFTP, tunnels, secrets and automation in one app.',
	repo: 'https://github.com/alexandrosnt/Reach',
	licence: 'MIT',
	categories: ['DeveloperApplication', 'UtilitiesApplication'],
	platforms: ['Windows', 'macOS', 'Linux', 'Android'],
	/** Search engines read this; it is also the honest answer. */
	price: '0',
	currency: 'USD',
};

/**
 * Words people actually type when they are looking for something like this.
 * Used for the meta keywords line and, more usefully, to keep the copy on the
 * landing page honest about what the product is called in the wild.
 */
export const KEYWORDS = [
	'SSH client',
	'open source SSH client',
	'free SSH client Windows',
	'SFTP client',
	'terminal emulator',
	'server manager',
	'remote server management',
	'PuTTY alternative',
	'MobaXterm alternative',
	'Termius alternative',
	'SSH key manager',
	'port forwarding tool',
	'Ansible GUI',
	'OpenTofu GUI',
];

/** The canonical origin, without a trailing slash. */
export const SITE_URL = (process.env.SITE_URL ?? 'https://reachssh.com').replace(
	/\/$/,
	'',
);

/** The path the site is served from, always with a leading and trailing slash. */
export const BASE_PATH = `/${(process.env.BASE_PATH ?? '/').replace(/^\/|\/$/g, '')}/`.replace(
	'//',
	'/',
);

/** An absolute URL for a path relative to the site root. */
export function absolute(path: string): string {
	const clean = path.replace(/^\//, '');
	return `${SITE_URL}${BASE_PATH}${clean}`;
}
