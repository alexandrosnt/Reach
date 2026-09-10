/**
 * One-shot scaffolder for the Reach Discord server.
 *
 * Deliberately not a long-running bot. It brings the guild in line with
 * `plan.mjs` and exits. It holds no gateway connection, so once it has run you
 * can kick it; nothing in the server depends on it afterwards, and the GitHub
 * feed is delivered by Discord's own webhook receiver rather than by any
 * process of ours.
 *
 * What it will and will not touch:
 *
 *   roles        created if missing, then ordered. An existing role is left
 *                exactly as it is — colour, permissions, everything.
 *   channels     created if missing, and renamed when the plan's emoji or
 *                casing differs from what is there. Never deleted, and an
 *                existing channel's topic is left alone.
 *   permissions  authoritative. Every planned channel has its overwrites
 *                rewritten to match the plan on each --apply, existing
 *                channels included, because a half-applied permission is
 *                worse than none: it looks locked and is not.
 *
 * That last one is the only place this overwrites your manual edits. If you
 * tune a channel's permissions in the client, put the change in plan.mjs too
 * or the next run will undo it.
 *
 * Dry run (the default) prints what it would do and touches nothing:
 *   npm run discord:setup
 *
 * For real:
 *   npm run discord:setup -- --apply
 *
 * Reads DISCORD_BOT_TOKEN and DISCORD_GUILD_ID from the environment or from a
 * local .env. The token is never printed and never written anywhere.
 */

import { readFile, writeFile, mkdir } from 'fs/promises';
import { join, dirname } from 'path';
import { fileURLToPath, pathToFileURL } from 'url';
import { PLAN, ROLES, EVERYONE_PERMISSIONS } from './plan.mjs';
import { P, bits, TALK } from './permissions.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..', '..');
const SECRETS_DIR = join(ROOT, '.discord');

const API = 'https://discord.com/api/v10';
const APPLY = process.argv.includes('--apply');

/** Discord channel types we use. */
const TEXT = 0;
const CATEGORY = 4;
const ANNOUNCEMENT = 5;

/** Overwrite target types. */
const OW_ROLE = 0;

// ---------------------------------------------------------------------------
// naming
// ---------------------------------------------------------------------------

/**
 * The identity of a channel for matching purposes: its name with any leading
 * decoration stripped.
 *
 * Channels are matched on this rather than on the full name, so changing an
 * emoji in the plan renames the channel that is already there instead of
 * creating a second one beside it. Only leading characters are stripped, so
 * `off-topic` keeps its hyphen.
 */
export const slugOf = (name) => name.toLowerCase().replace(/^[^a-z0-9]+/, '');

/** What the channel should actually be called. */
const channelName = (ch) => (ch.emoji ? ch.emoji + '-' + ch.name : ch.name);

/** Categories read better with a space, and Discord keeps their casing. */
const categoryName = (g) => (g.emoji ? g.emoji + ' ' + g.category : g.category);

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
	50013: 'The bot is missing a permission, or is trying to act on a role above its own.',
	10004: 'Unknown guild - check DISCORD_GUILD_ID.',
	30013: 'This server is at its channel limit (500).',
	30005: 'This server is at its role limit (250).'
};

async function api(method, path, body) {
	for (let attempt = 0; attempt < 5; attempt++) {
		const res = await fetch(API + path, {
			method,
			headers: {
				Authorization: 'Bot ' + process.env.DISCORD_BOT_TOKEN,
				'Content-Type': 'application/json',
				'User-Agent': 'ReachSetup (https://github.com/alexandrosnt/Reach, 0.5.2)'
			},
			body: body === undefined ? undefined : JSON.stringify(body)
		});

		// Creating channels and roles in bulk hits the bucket limit routinely.
		// This is expected traffic, not an error.
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
// permission overwrites
// ---------------------------------------------------------------------------

/**
 * The overwrite list for one planned channel.
 *
 * Built from scratch each run rather than merged into what is already there.
 * Merging sounds safer and is not: a stale deny left behind by an earlier plan
 * keeps applying, and nobody ever notices a permission that is quietly still
 * in force.
 */
export function overwritesFor(channel, guildId, roleIds) {
	/** @everyone always shares the guild's id. */
	let everyoneDeny = 0n;
	let everyoneAllow = 0n;
	const out = [];

	const idOf = (name) => {
		const id = roleIds.get(name);
		if (!id) throw new Error(`Channel #${channel.name} references unknown role "${name}"`);
		return id;
	};

	if (channel.private) {
		// Hidden from everyone, revealed to the named roles.
		everyoneDeny |= P.VIEW_CHANNEL;
		for (const name of channel.private) {
			out.push({ id: idOf(name), type: OW_ROLE, allow: bits(TALK), deny: '0' });
		}
	}

	if (channel.readOnly) {
		everyoneDeny |= P.SEND_MESSAGES;
		// Reading and reacting stay open; a read-only channel people cannot
		// react to is a noticeboard, not a channel.
		everyoneAllow |= P.VIEW_CHANNEL | P.READ_MESSAGE_HISTORY | P.ADD_REACTIONS;
	}

	if (channel.noThreads) {
		everyoneDeny |= P.CREATE_PUBLIC_THREADS | P.CREATE_PRIVATE_THREADS;
	}

	for (const name of channel.writers ?? []) {
		let allow = P.VIEW_CHANNEL | P.SEND_MESSAGES;
		if (channel.noThreads) allow |= P.CREATE_PUBLIC_THREADS;
		out.push({ id: idOf(name), type: OW_ROLE, allow: allow.toString(), deny: '0' });
	}

	if (everyoneDeny !== 0n || everyoneAllow !== 0n) {
		out.unshift({
			id: guildId,
			type: OW_ROLE,
			allow: everyoneAllow.toString(),
			deny: everyoneDeny.toString()
		});
	}

	return out;
}

/** Compare two overwrite lists ignoring order, so a no-op run stays quiet. */
function sameOverwrites(a, b) {
	const norm = (list) =>
		[...(list ?? [])]
			.map((o) => `${o.id}:${o.type}:${o.allow ?? '0'}:${o.deny ?? '0'}`)
			.sort()
			.join('|');
	return norm(a) === norm(b);
}

// ---------------------------------------------------------------------------
// roles
// ---------------------------------------------------------------------------

async function ensureRoles(guildId, counters) {
	const existing = await api('GET', '/guilds/' + guildId + '/roles');
	const byName = new Map(existing.map((r) => [r.name, r]));
	const roleIds = new Map();

	console.log('ROLES');
	for (const role of ROLES) {
		const found = byName.get(role.name);
		if (found) {
			roleIds.set(role.name, found.id);
			console.log('  @' + role.name + '  (exists)');
			counters.skipped++;
			continue;
		}
		if (!APPLY) {
			console.log('  @' + role.name + '  would create');
			counters.created++;
			continue;
		}
		const made = await api('POST', '/guilds/' + guildId + '/roles', {
			name: role.name,
			permissions: bits(role.permissions),
			color: role.color,
			hoist: role.hoist,
			mentionable: role.mentionable
		});
		roleIds.set(role.name, made.id);
		console.log('  @' + role.name + '  created' + (role.hoist ? ' (hoisted)' : ''));
		counters.created++;
	}
	console.log('');
	return roleIds;
}

/**
 * Put the roles in plan order, directly under the bot's own role.
 *
 * Discord refuses to move a role above the actor's highest, so this can only
 * ever arrange them below the bot. Failing here is cosmetic — the roles work,
 * they are just in the wrong order in the list — so it warns rather than
 * aborting a run that has already created everything.
 */
async function orderRoles(guildId, botUserId, roleIds) {
	if (!APPLY || roleIds.size === 0) return;

	// Ordering is cosmetic: the roles already work, they are just listed in
	// the wrong order. Nothing in here is worth failing a run that has already
	// created every role and channel, so the whole thing degrades to a note.
	try {
		let all = await api('GET', '/guilds/' + guildId + '/roles');

		// `@me` only resolves on the OAuth2 route; a bot has to name its own id.
		const me = await api('GET', '/guilds/' + guildId + '/members/' + botUserId);
		let mine = all.filter((r) => me.roles.includes(r.id));
		let ceiling = mine.length > 0 ? Math.max(...mine.map((r) => r.position)) : all.length;

		const ordered = ROLES.map((r) => roleIds.get(r.name)).filter(Boolean);

		// Discord starts every new role at position 1, the bot's own included,
		// so on a fresh server there is nowhere to put these and nothing the
		// bot may assign — a role at or above your own is off limits.
		//
		// A bot may, however, raise its own role, which is the one move that
		// unblocks both. Worth trying before asking a human to drag anything.
		if (ceiling - ordered.length < 1) {
			const botRole = mine.find((r) => r.managed) ?? mine[0];
			if (botRole) {
				await api('PATCH', '/guilds/' + guildId + '/roles', [
					{ id: botRole.id, position: ordered.length + 1 }
				]);
				all = await api('GET', '/guilds/' + guildId + '/roles');
				mine = all.filter((r) => me.roles.includes(r.id));
				ceiling = Math.max(...mine.map((r) => r.position));
				console.log('Raised @' + botRole.name + ' to position ' + ceiling + ' to make room.');
			}
		}

		if (ceiling - ordered.length < 1) {
			console.log(
				'Note:  no room to order the roles, and the bot could not raise itself.\n' +
					"       Drag the bot's role to the top in Server Settings -> Roles and re-run.\n" +
					'       The roles work regardless; only their order in the list is wrong.\n'
			);
			return;
		}

		const positions = ordered.map((id, i) => ({ id, position: ceiling - 1 - i }));
		await api('PATCH', '/guilds/' + guildId + '/roles', positions);
		console.log('Roles ordered beneath @' + (mine[0]?.name ?? 'the bot') + '.\n');
	} catch (err) {
		console.log('Note:  could not order roles (' + err.message.split('\n')[0] + ').');
		console.log('       They exist and work; only their order in the list is wrong.\n');
	}
}

/** Bring the @everyone baseline in line with the plan. */
async function syncEveryone(guildId, counters) {
	const wanted = bits(EVERYONE_PERMISSIONS);
	const all = await api('GET', '/guilds/' + guildId + '/roles');
	const everyone = all.find((r) => r.id === guildId);

	if (everyone && everyone.permissions === wanted) {
		console.log('@everyone baseline  (already correct)\n');
		counters.skipped++;
		return;
	}
	if (!APPLY) {
		console.log('@everyone baseline  would update\n');
		counters.created++;
		return;
	}
	await api('PATCH', '/guilds/' + guildId + '/roles/' + guildId, { permissions: wanted });
	console.log('@everyone baseline  updated\n');
	counters.created++;
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
	console.log('Mode:  ' + (APPLY ? 'APPLY - this will change the server' : 'dry run - nothing will change'));
	if (!isCommunity) {
		console.log('Note:  not a Community server, so announcement channels fall back to plain text.');
	}
	console.log('');

	const counters = { created: 0, renamed: 0, skipped: 0, permsSynced: 0 };

	const roleIds = await ensureRoles(guildId, counters);
	await syncEveryone(guildId, counters);

	// A dry run has no ids for roles it has not created, and overwrites cannot
	// be described without them. Say so once rather than throwing later.
	const canDescribePerms = ROLES.every((r) => roleIds.has(r.name));
	if (!canDescribePerms) {
		console.log('Note:  channel permissions are shown only once the roles exist.\n');
	}

	const existing = await api('GET', '/guilds/' + guildId + '/channels');

	/**
	 * Existing channels keyed by "parentName/channelName", so two channels
	 * sharing a name under different categories do not collide.
	 */
	const byKey = new Map();
	for (const c of existing) {
		const parent = existing.find((p) => p.id === c.parent_id);
		byKey.set((parent ? slugOf(parent.name) : '') + '/' + slugOf(c.name), c);
	}

	const webhooks = {};

	for (const group of PLAN) {
		let category = byKey.get('/' + slugOf(group.category));
		const wantedCategory = categoryName(group);

		if (category && category.name !== wantedCategory) {
			if (APPLY) {
				category = await api('PATCH', '/channels/' + category.id, { name: wantedCategory });
				console.log(wantedCategory + '  renamed');
			} else {
				console.log(wantedCategory + '  would rename from ' + category.name);
			}
			counters.renamed++;
		} else if (category) {
			console.log(wantedCategory + '  (exists)');
			counters.skipped++;
		} else if (APPLY) {
			category = await api('POST', '/guilds/' + guildId + '/channels', {
				name: wantedCategory,
				type: CATEGORY
			});
			console.log(wantedCategory + '  created');
			counters.created++;
		} else {
			console.log(wantedCategory + '  would create');
			counters.created++;
		}

		for (const ch of group.channels) {
			const key = slugOf(group.category) + '/' + slugOf(ch.name);
			let channel = byKey.get(key);
			const overwrites = canDescribePerms ? overwritesFor(ch, guildId, roleIds) : null;
			const tags = [
				ch.private ? 'private' : null,
				ch.readOnly ? 'read-only' : null,
				ch.webhook ? 'feed' : null
			].filter(Boolean);
			const suffix = tags.length ? ' (' + tags.join(', ') + ')' : '';

			if (channel) {
				// The channel is there, but its name or permissions may not
				// match the plan. This is where an edited plan takes effect,
				// and both go in one PATCH so a rate limit cannot leave a
				// channel renamed but still wide open.
				const patch = {};
				const changes = [];
				const wanted = channelName(ch);

				if (channel.name !== wanted) {
					patch.name = wanted;
					changes.push('renamed');
					counters.renamed++;
				}
				if (overwrites && !sameOverwrites(channel.permission_overwrites, overwrites)) {
					patch.permission_overwrites = overwrites;
					changes.push('permissions');
					counters.permsSynced++;
				}

				if (changes.length === 0) {
					console.log('  #' + wanted + '  (exists)' + suffix);
					counters.skipped++;
				} else if (APPLY) {
					channel = await api('PATCH', '/channels/' + channel.id, patch);
					console.log('  #' + wanted + '  ' + changes.join(' + ') + ' updated' + suffix);
				} else {
					console.log('  #' + wanted + '  would update ' + changes.join(' + ') + suffix);
				}
			} else {
				const payload = {
					name: channelName(ch),
					type: ch.announcement && isCommunity ? ANNOUNCEMENT : TEXT,
					topic: ch.topic,
					parent_id: category ? category.id : undefined
				};
				if (overwrites) payload.permission_overwrites = overwrites;

				if (APPLY) {
					channel = await api('POST', '/guilds/' + guildId + '/channels', payload);
					console.log('  #' + payload.name + '  created' + suffix);
				} else {
					console.log('  #' + payload.name + '  would create' + suffix);
				}
				counters.created++;
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

	await orderRoles(guildId, me.id, roleIds);

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

	const verb = APPLY ? '' : ' would be';
	const tail =
		counters.created + verb + ' created, ' +
		counters.renamed + verb + ' renamed, ' +
		counters.permsSynced + verb + ' repermissioned, ' +
		counters.skipped + ' already correct.';
	console.log((APPLY ? 'Done. ' : 'Dry run. ') + tail);
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
