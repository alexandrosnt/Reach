/**
 * One-shot scaffolder for the Reach Discord server.
 *
 * This is deliberately not a long-running bot. It creates the channel
 * structure in `plan.mjs`, creates the webhooks the GitHub feed needs, and
 * exits. Once it has run you can kick the bot from the server: nothing here
 * needs it to stay online, and the GitHub feed is delivered by Discord's own
 * webhook receiver rather than by any process of ours.
 *
 * Dry run (the default) prints what it would do and touches nothing:
 *   node scripts/discord/setup.mjs
 *
 * For real:
 *   node scripts/discord/setup.mjs --apply
 *
 * Reads DISCORD_BOT_TOKEN and DISCORD_GUILD_ID from the environment or from a
 * local .env. The token is never printed and never written anywhere.
 */

import { readFile, writeFile, mkdir } from 'fs/promises';
import { join, dirname } from 'path';
import { fileURLToPath, pathToFileURL } from 'url';
import { PLAN } from './plan.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..', '..');
const SECRETS_DIR = join(ROOT, '.discord');

const API = 'https://discord.com/api/v10';
const APPLY = process.argv.includes('--apply');

/** Discord channel types we use. */
const TEXT = 0;
const CATEGORY = 4;
const ANNOUNCEMENT = 5;

/** Permission bits are 64-bit, so they travel as strings. */
const SEND_MESSAGES = (1n << 11n).toString();

// ---------------------------------------------------------------------------
// env
// ---------------------------------------------------------------------------

/** Minimal .env reader, so this script needs no dependencies at all. */
async function loadDotEnv() {
	let raw;
	try {
		raw = await readFile(join(ROOT, '.env'), 'utf-8');
	} catch {
		return;
	}
	for (const line of raw.split(/\r?\n/)) {
		const m = /^\s*([A-Z0-9_]+)\s*=\s*(.*)$/.exec(line);
		if (!m) continue;
		const value = m[2].trim().replace(/^["']|["']$/g, '');
		if (!(m[1] in process.env)) process.env[m[1]] = value;
	}
}

// ---------------------------------------------------------------------------
// api
// ---------------------------------------------------------------------------

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Discord error codes worth translating into an instruction. */
const HINTS = {
	50001: 'The bot cannot see that guild. Is it actually invited to this server?',
	50013: 'The bot is missing a permission. It needs Manage Channels and Manage Webhooks.',
	10004: 'Unknown guild - check DISCORD_GUILD_ID.',
	30013: 'This server is at its channel limit (500).'
};

async function api(method, path, body) {
	for (let attempt = 0; attempt < 5; attempt++) {
		const res = await fetch(API + path, {
			method,
			headers: {
				Authorization: 'Bot ' + process.env.DISCORD_BOT_TOKEN,
				'Content-Type': 'application/json',
				'User-Agent': 'ReachSetup (https://github.com/alexandrosnt/Reach, 0.5.1)'
			},
			body: body === undefined ? undefined : JSON.stringify(body)
		});

		// Creating channels in bulk hits the bucket limit routinely. This is
		// expected traffic, not an error.
		if (res.status === 429) {
			const wait = Number(res.headers.get('retry-after') ?? '1');
			console.log('      rate limited, waiting ' + wait + 's');
			await sleep(wait * 1000 + 250);
			continue;
		}

		if (res.status === 401) {
			throw new Error('Discord rejected the token (401). Regenerate it in the Developer Portal.');
		}

		if (!res.ok) {
			const text = await res.text();
			let detail = text;
			try {
				const parsed = JSON.parse(text);
				detail = parsed.message ?? text;
				if (HINTS[parsed.code]) detail += '\n      ' + HINTS[parsed.code];
			} catch {
				/* not JSON, keep the raw body */
			}
			throw new Error(method + ' ' + path + ' -> ' + res.status + ': ' + detail);
		}

		return res.status === 204 ? null : res.json();
	}
	throw new Error('Gave up after 5 rate-limited attempts: ' + method + ' ' + path);
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

export async function main() {
	await loadDotEnv();

	const guildId = process.env.DISCORD_GUILD_ID;
	if (!process.env.DISCORD_BOT_TOKEN || !guildId) {
		console.error(
			'Set DISCORD_BOT_TOKEN and DISCORD_GUILD_ID, in the environment or in .env.\n' +
				'See scripts/discord/README.md for where both come from.'
		);
		process.exit(1);
	}

	const me = await api('GET', '/users/@me');
	const guild = await api('GET', '/guilds/' + guildId);
	const isCommunity = (guild.features ?? []).includes('COMMUNITY');

	console.log('Bot:   ' + me.username + ' (' + me.id + ')');
	console.log('Guild: ' + guild.name + ' (' + guild.id + ')');
	console.log('Mode:  ' + (APPLY ? 'APPLY - this will create channels' : 'dry run - nothing will change'));
	if (!isCommunity) {
		console.log('Note:  not a Community server, so announcement channels fall back to plain text.');
	}
	console.log('');

	const existing = await api('GET', '/guilds/' + guildId + '/channels');

	/**
	 * Existing channels keyed by "parentName/channelName", so two channels
	 * sharing a name under different categories do not collide.
	 */
	const byKey = new Map();
	for (const c of existing) {
		const parent = existing.find((p) => p.id === c.parent_id);
		byKey.set(((parent ? parent.name : '') + '/' + c.name).toLowerCase(), c);
	}

	const webhooks = {};
	let created = 0;
	let skipped = 0;

	for (const group of PLAN) {
		let category = byKey.get('/' + group.category.toLowerCase());

		if (category) {
			console.log(group.category + '  (exists)');
			skipped++;
		} else if (APPLY) {
			category = await api('POST', '/guilds/' + guildId + '/channels', {
				name: group.category,
				type: CATEGORY
			});
			console.log(group.category + '  created');
			created++;
		} else {
			console.log(group.category + '  would create');
			created++;
		}

		for (const ch of group.channels) {
			const key = (group.category + '/' + ch.name).toLowerCase();
			let channel = byKey.get(key);

			if (channel) {
				console.log('  #' + ch.name + '  (exists)');
				skipped++;
			} else {
				const payload = {
					name: ch.name,
					type: ch.announcement && isCommunity ? ANNOUNCEMENT : TEXT,
					topic: ch.topic,
					parent_id: category ? category.id : undefined
				};
				// The @everyone role always shares the guild's id.
				if (ch.readOnly) {
					payload.permission_overwrites = [{ id: guildId, type: 0, deny: SEND_MESSAGES }];
				}

				if (APPLY) {
					channel = await api('POST', '/guilds/' + guildId + '/channels', payload);
					console.log('  #' + ch.name + '  created' + (ch.readOnly ? ' (read-only)' : ''));
				} else {
					console.log('  #' + ch.name + '  would create' + (ch.readOnly ? ' (read-only)' : ''));
				}
				created++;
			}

			if (!ch.webhook || !APPLY || !channel) continue;

			// Reuse a webhook made on an earlier run rather than stacking
			// duplicates every time this is re-run.
			const hooks = await api('GET', '/channels/' + channel.id + '/webhooks');
			let hook = hooks.find((h) => h.name === ch.webhook);
			if (hook && !hook.token) {
				// A webhook someone else created: visible to us, but its token
				// never is, so it is useless here. Make our own.
				hook = undefined;
			}
			if (hook) {
				console.log('      webhook "' + ch.webhook + '" reused');
			} else {
				hook = await api('POST', '/channels/' + channel.id + '/webhooks', { name: ch.webhook });
				console.log('      webhook "' + ch.webhook + '" created');
			}

			webhooks[ch.name] = {
				channel: '#' + ch.name,
				name: ch.webhook,
				url: hook.url,
				githubUrl: hook.url + '/github'
			};
		}
		console.log('');
	}

	if (Object.keys(webhooks).length > 0) {
		await mkdir(SECRETS_DIR, { recursive: true });
		await writeFile(
			join(SECRETS_DIR, 'webhooks.json'),
			JSON.stringify(webhooks, null, 2) + '\n',
			'utf-8'
		);
		console.log('Webhook URLs written to .discord/webhooks.json (gitignored).');
		console.log('A webhook URL is a posting credential - treat it like a token.');
		for (const w of Object.values(webhooks)) {
			// The id half is not secret; the token half is, so it stays out of
			// the terminal and out of any transcript.
			console.log('  ' + w.channel + '  ' + w.url.replace(/\/[^/]+$/, '/********'));
		}
		console.log('');
	}

	console.log(
		APPLY
			? 'Done. ' + created + ' created, ' + skipped + ' already there.'
			: 'Dry run. ' + created + ' would be created, ' + skipped + ' already there.'
	);
	if (!APPLY) console.log('Re-run with --apply to make it real.');
}

// Only run when invoked as a script. Importing this module — which the test
// harness does, so it can await main() against a mocked Discord — must not
// fire a run at import time.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	main().catch((err) => {
		console.error('\n' + err.message);
		process.exit(1);
	});
}
