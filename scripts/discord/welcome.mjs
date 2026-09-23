/**
 * Put a welcome message in #welcome, and pin it.
 *
 * The server was scaffolded with roles and twenty channels and then left
 * empty, which is the state a visitor reads as abandoned. A pinned welcome is
 * the cheapest fix: it tells someone who just joined what this is, where to
 * go, and that a person is behind it.
 *
 * Dry run by default, like every other script here, because this one posts to
 * a server real people are in and an unpinnable typo is public.
 *
 *   node scripts/discord/welcome.mjs            # print what would be posted
 *   node scripts/discord/welcome.mjs --apply    # post it and pin it
 *
 * Idempotent: it looks for an existing message from this bot in the channel
 * and edits that one rather than posting a second. Running it twice does not
 * produce two welcomes.
 */

import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const API = 'https://discord.com/api/v10';
const APPLY = process.argv.includes('--apply');

/* --- credentials ------------------------------------------------------- */

// The other scripts read the environment; .env is loaded here too so this
// works the same way whether or not the shell already has it.
try {
	const env = await readFile(new URL('../../.env', import.meta.url), 'utf8');
	for (const line of env.split(/\r?\n/)) {
		const at = line.indexOf('=');
		if (at < 1 || line.trimStart().startsWith('#')) continue;
		const key = line.slice(0, at).trim();
		if (!process.env[key]) process.env[key] = line.slice(at + 1).trim();
	}
} catch {
	/* No .env is fine if the shell already carries the variables. */
}

const TOKEN = process.env.DISCORD_BOT_TOKEN;
const GUILD = process.env.DISCORD_GUILD_ID;

if (!TOKEN || !GUILD) {
	console.error('DISCORD_BOT_TOKEN and DISCORD_GUILD_ID must be set.');
	process.exit(1);
}

const api = (path, init = {}) =>
	fetch(API + path, {
		...init,
		headers: { Authorization: `Bot ${TOKEN}`, ...(init.headers ?? {}) },
	});

/* --- the message ------------------------------------------------------- */

const SITE = 'https://reachssh.com';
const REPO = 'https://github.com/alexandrosnt/Reach';

/**
 * Written to be read once, by someone who just arrived and does not yet care.
 *
 * Short on purpose. A wall of headed sections in a welcome channel gets
 * scrolled past, and the two things a newcomer wants are what this is and
 * where to put a bug. The rest can wait until they have a reason.
 */
const CONTENT = [
	'Welcome.',
	'',
	'Reach is a free, open source SSH client. Terminals, file transfer, tunnels,',
	'an encrypted vault for your keys and passwords, and Ansible and OpenTofu if',
	'you use them. Windows, Mac, Linux and Android. No paid version, nothing to',
	'upgrade to.',
	'',
	`Download: ${SITE}`,
	`Code: ${REPO}`,
	'',
	'Broken? <#1547542234059116557>',
	'Stuck? <#1547542233253806100>',
	'Anything else? <#1547542243060224000>',
	'',
	'Built by one person, in the open. I read everything posted here, and a good',
	'few of the fixes in the last couple of releases came from someone in this',
	'server noticing something was off.',
	'',
	'Grab it, break it, tell me what happened.',
].join('\n');

/* --- posting ----------------------------------------------------------- */

const WELCOME_CHANNEL = '1547542199921545246'; // #👋-welcome
const BANNER = fileURLToPath(new URL('../../assets/banner.png', import.meta.url));

if (!APPLY) {
	console.log('--- dry run: nothing posted ---\n');
	console.log(`channel : #👋-welcome (${WELCOME_CHANNEL})`);
	console.log(`image   : assets/banner.png`);
	console.log(`pin     : yes\n`);
	console.log(CONTENT);
	console.log('\n--- run again with --apply to post ---');
	process.exit(0);
}

// An existing message from this bot is edited rather than duplicated.
const me = await api('/users/@me').then((r) => r.json());
const existing = await api(`/channels/${WELCOME_CHANNEL}/messages?limit=50`)
	.then((r) => (r.ok ? r.json() : []))
	.then((list) => list.find((m) => m.author?.id === me.id));

if (existing) {
	// Editing cannot change an attachment, so the text is updated in place and
	// the image the original carried stays as it is.
	const res = await api(`/channels/${WELCOME_CHANNEL}/messages/${existing.id}`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ content: CONTENT }),
	});
	console.log(res.ok ? `Edited the existing welcome (${existing.id}).` : `Edit failed: ${res.status}`);
	if (!res.ok) console.error((await res.text()).slice(0, 300));
	process.exit(res.ok ? 0 : 1);
}

const form = new FormData();
form.append('payload_json', JSON.stringify({ content: CONTENT }));
form.append(
	'files[0]',
	new Blob([await readFile(BANNER)], { type: 'image/png' }),
	'reach.png',
);

const posted = await api(`/channels/${WELCOME_CHANNEL}/messages`, { method: 'POST', body: form });

if (!posted.ok) {
	console.error(`Post failed: ${posted.status}`);
	console.error((await posted.text()).slice(0, 300));
	process.exit(1);
}

const message = await posted.json();
console.log(`Posted (${message.id}).`);

// PUT /channels/{id}/pins/{message} — not /messages/{id}/pin, which 404s.
const pinned = await api(`/channels/${WELCOME_CHANNEL}/pins/${message.id}`, {
	method: 'PUT',
	headers: { 'X-Audit-Log-Reason': 'Welcome message' },
});
console.log(pinned.ok ? 'Pinned.' : `Pin failed: ${pinned.status} — needs Manage Messages.`);
