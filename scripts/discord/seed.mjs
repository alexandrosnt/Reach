/**
 * Put something in the channels that survived the trim.
 *
 * Hiding the unused ones stopped the server looking abandoned; it did not make
 * it look alive. Four of the nine that remain have never had a message, and
 * one of those is the channel Discord itself points at as the rules.
 *
 * This posts only things that are true today — the rules, and the fact that
 * the site moved to its own domain. Nothing here is filler. A server seeded
 * with manufactured chatter reads worse than an empty one, because the
 * chatter is obvious and the emptiness at least is honest.
 *
 *   node scripts/discord/seed.mjs            # print what would be posted
 *   node scripts/discord/seed.mjs --apply    # post it
 *
 * Idempotent per channel: a channel that already has a message from this bot
 * is skipped rather than posted to twice.
 */

import { readFile } from 'node:fs/promises';

const API = 'https://discord.com/api/v10';
const APPLY = process.argv.includes('--apply');

try {
	const env = await readFile(new URL('../../.env', import.meta.url), 'utf8');
	for (const line of env.split(/\r?\n/)) {
		const at = line.indexOf('=');
		if (at < 1 || line.trimStart().startsWith('#')) continue;
		const key = line.slice(0, at).trim();
		if (!process.env[key]) process.env[key] = line.slice(at + 1).trim();
	}
} catch {
	/* The shell may already carry them. */
}

const TOKEN = process.env.DISCORD_BOT_TOKEN;
if (!TOKEN) {
	console.error('DISCORD_BOT_TOKEN must be set.');
	process.exit(1);
}

const api = (path, init = {}) =>
	fetch(API + path, {
		...init,
		headers: { Authorization: `Bot ${TOKEN}`, ...(init.headers ?? {}) },
	});

/* --- what goes where --------------------------------------------------- */

const GENERAL = '1547542243060224000';
const BUGS = '1547542234059116557';

/**
 * Five rules, not fifteen.
 *
 * A long list on a twelve-person server is a costume — it implies problems
 * nobody has had yet, and nobody reads past the third line anyway. These are
 * the ones that would actually be enforced.
 */
const RULES = [
	'**Rules**',
	'',
	'1. Be decent to each other. That covers most of it.',
	'2. No spam, no self-promo, no DMing people who did not ask.',
	'3. Keep it roughly on topic in the topic channels.',
	`4. Security problems do not go here. Open a private advisory on GitHub instead: <https://github.com/alexandrosnt/Reach/security/advisories/new>`,
	'5. Nobody here will ever ask for your SSH keys, your vault password, or a screenshot with credentials in it. If someone does, they are not staff.',
	'',
	'Discord rules apply on top of these, and you must be 13 or older to be here.',
].join('\n');

/**
 * Real news, posted the day it is true. An announcements channel whose first
 * message is "welcome to announcements" teaches people to mute it.
 */
const ANNOUNCEMENT = [
	'**Reach has its own site now: <https://reachssh.com>**',
	'',
	'The docs moved off the GitHub Pages URL onto a proper domain. Old links',
	'redirect, so nothing you have saved is broken.',
	'',
	'A few things came with the move:',
	'',
	'- The download button now hands you the actual installer for whatever you',
	'  are on, with the file size, instead of dropping you on the releases page',
	'- The front page runs a working mock of the app — tabs, a terminal you can',
	'  type into, the file explorer with its right-click menu',
	'- Compressing and extracting from the file explorer is documented at last,',
	'  along with dragging a file straight out to your desktop',
	'',
	'v0.6.5 is the current release. Windows, macOS, Linux and Android.',
].join('\n');

const POSTS = [
	{ channel: '1547655112128729118', name: '#rules', content: RULES, pin: true },
	{ channel: '1547542201238683698', name: '#📣-announcements', content: ANNOUNCEMENT, pin: false },
];

/* --- go ---------------------------------------------------------------- */

if (!APPLY) {
	console.log('--- dry run: nothing posted ---');
	for (const post of POSTS) {
		console.log(`\n=== ${post.name}${post.pin ? '  (pinned)' : ''} ===\n`);
		console.log(post.content);
	}
	console.log('\n--- run again with --apply to post ---');
	process.exit(0);
}

const me = await api('/users/@me').then((r) => r.json());

for (const post of POSTS) {
	const existing = await api(`/channels/${post.channel}/messages?limit=50`)
		.then((r) => (r.ok ? r.json() : []))
		.then((list) => (Array.isArray(list) ? list.find((m) => m.author?.id === me.id) : null));

	if (existing) {
		console.log(`${post.name}: already has a message from this bot, skipped.`);
		continue;
	}

	const res = await api(`/channels/${post.channel}/messages`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ content: post.content, allowed_mentions: { parse: [] } }),
	});

	if (!res.ok) {
		console.error(`${post.name}: failed ${res.status} — ${(await res.text()).slice(0, 160)}`);
		continue;
	}

	const message = await res.json();
	console.log(`${post.name}: posted (${message.id}).`);

	if (post.pin) {
		const pinned = await api(`/channels/${post.channel}/pins/${message.id}`, {
			method: 'PUT',
			headers: { 'X-Audit-Log-Reason': 'Seeding the server' },
		});
		console.log(`${post.name}: ${pinned.ok ? 'pinned' : `pin failed ${pinned.status}`}`);
	}
}

console.log(`\n#general (${GENERAL}) and #showcase are deliberately left empty.`);
console.log('A bot saying hello in the room meant for people is worse than silence.');
console.log(`Post in there yourself, and put a screenshot in #showcase — those two are yours.`);
console.log(`Bug reports land in ${BUGS}; it fills itself once people arrive.`);
