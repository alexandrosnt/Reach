/**
 * Put away the channels nobody is using yet.
 *
 * The server was scaffolded with twenty public channels for twelve people.
 * That ratio is the problem: an empty channel reads as an abandoned server,
 * and twelve people spread across twenty rooms never meet each other. The
 * advice is consistent — a server this size wants five to eight channels, and
 * a channel should be populated or put away.
 *
 * So this hides rather than deletes. Deleting throws away history and is not
 * undoable; hiding is a single permission overwrite that can be lifted the day
 * a channel has a reason to exist. Nothing is lost, and anyone with a staff
 * role still sees everything.
 *
 * Dry run by default. It also counts messages in every channel first, so the
 * decision is made against what is actually there rather than against a guess.
 *
 *   node scripts/discord/trim.mjs            # show the plan and the counts
 *   node scripts/discord/trim.mjs --apply    # hide them
 *   node scripts/discord/trim.mjs --park     # gather them under one category
 *   node scripts/discord/trim.mjs --restore  # make everything visible again
 *
 * --park exists because hiding a channel from @everyone does not hide it from
 * you. A server owner bypasses every permission overwrite, so the nine that
 * were just put away are still cluttering the owner's own sidebar, and
 * Discord's only remedy for that is client side: mute the channel and turn on
 * "Hide Muted Channels". A bot token cannot reach a person's notification
 * settings, so that part cannot be automated.
 *
 * What a bot can do is move them, so the muting is one action instead of nine.
 * With all of them under a single category, right-click it once, Mute
 * Category, and they are gone. Unmute to get them back.
 */

import { readFile } from 'node:fs/promises';

const API = 'https://discord.com/api/v10';
const APPLY = process.argv.includes('--apply');
const RESTORE = process.argv.includes('--restore');
const PARK = process.argv.includes('--park');

/** Discord's permission bit for seeing a channel at all. */
const VIEW_CHANNEL = 1n << 10n;

/** Where the put-away channels are gathered, so muting them is one action. */
const PARKED_CATEGORY = 'PARKED — not in use yet';

/* --- credentials ------------------------------------------------------- */

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

/* --- the plan ---------------------------------------------------------- */

/**
 * What stays visible, by the bare name without its emoji.
 *
 * Six rooms plus three that fill themselves. `announcements`, `releases` and
 * `github` are not conversation — they are the evidence that something is
 * happening, which is exactly what a quiet server is short of.
 */
const KEEP = new Set([
	'rules',
	'welcome',
	'announcements',
	'releases',
	'github',
	'help',
	'bug-reports',
	'general',
	'showcase',
]);

/**
 * Channels that belong to a role rather than to everyone. Left alone — they
 * are already private, and hiding them again would be a no-op that muddies
 * the output.
 */
const STAFF = new Set(['staff-chat', 'mod-log']);

/** Strip leading emoji and separators so 'ð-bug-reports' matches 'bug-reports'. */
const bare = (name) => name.replace(/^[^a-z0-9]+/i, '').toLowerCase();

/* --- read the server --------------------------------------------------- */

const channels = await api(`/guilds/${GUILD}/channels`).then((r) => r.json());
if (!Array.isArray(channels)) {
	console.error('Could not read channels:', JSON.stringify(channels).slice(0, 200));
	process.exit(1);
}

const TEXT = 0;
const text = channels.filter((c) => c.type === TEXT).sort((a, b) => a.position - b.position);

/** How much is actually in each one. The whole argument rests on this. */
const counted = await Promise.all(
	text.map(async (c) => {
		const res = await api(`/channels/${c.id}/messages?limit=100`);
		const messages = res.ok ? await res.json() : [];
		return { ...c, messages: Array.isArray(messages) ? messages.length : 0, readable: res.ok };
	}),
);

const keep = [];
const hide = [];
for (const c of counted) {
	const name = bare(c.name);
	if (STAFF.has(name)) continue;
	(KEEP.has(name) ? keep : hide).push(c);
}

/* --- report ------------------------------------------------------------ */

const row = (c, mark) =>
	`  ${mark} #${c.name.padEnd(22)} ${String(c.messages).padStart(3)} message${c.messages === 1 ? '' : 's'}` +
	(c.readable ? '' : '  (not readable by the bot)');

console.log(`\n${counted.length} public text channels, ${counted.reduce((n, c) => n + c.messages, 0)} messages between them.\n`);
console.log(`Staying visible (${keep.length}):`);
for (const c of keep) console.log(row(c, '·'));
console.log(`\nHiding from @everyone (${hide.length}):`);
for (const c of hide) console.log(row(c, '×'));

if (PARK) {
	// Staff channels are excluded above: they are private by design, not put
	// away, and their category is somewhere you still want to look.
	const CATEGORY = 4;
	const categories = channels.filter((c) => c.type === CATEGORY);
	let parked = categories.find((c) => c.name === PARKED_CATEGORY);

	if (!parked) {
		const res = await api(`/guilds/${GUILD}/channels`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({
				name: PARKED_CATEGORY,
				type: CATEGORY,
				position: 99, // bottom of the list, out of the way even unmuted
				permission_overwrites: [{ id: GUILD, type: 0, deny: VIEW_CHANNEL.toString() }],
			}),
		});
		if (!res.ok) {
			console.error(`Could not create the category: ${res.status} ${(await res.text()).slice(0, 160)}`);
			process.exit(1);
		}
		parked = await res.json();
		console.log(`\nCreated category "${PARKED_CATEGORY}".`);
	} else {
		console.log(`\nUsing the existing "${PARKED_CATEGORY}" category.`);
	}

	let moved = 0;
	for (const c of hide) {
		if (c.parent_id === parked.id) continue;
		const res = await api(`/channels/${c.id}`, {
			method: 'PATCH',
			headers: {
				'Content-Type': 'application/json',
				'X-Audit-Log-Reason': 'Parking unused channels',
			},
			// lock_permissions false: each channel keeps the overwrite that hides
			// it, rather than inheriting the category's and losing its own.
			body: JSON.stringify({ parent_id: parked.id, lock_permissions: false }),
		});
		if (res.ok) {
			moved += 1;
			console.log(`  moved: #${c.name}`);
		} else {
			console.error(`  failed: #${c.name} - ${res.status} ${(await res.text()).slice(0, 120)}`);
		}
	}

	console.log(`\n${moved} channel${moved === 1 ? '' : 's'} moved.`);
	console.log('\nTo get them out of your own sidebar:');
	console.log(`  1. Right-click "${PARKED_CATEGORY}" -> Mute Category -> Until I turn it back on`);
	console.log('  2. Right-click the server name -> Notification Settings');
	console.log('  3. Tick "Hide Muted Channels"');
	console.log('\nUnmute the category to get them all back. Nothing was deleted.');
	process.exit(0);
}


if (RESTORE) {
	console.log('\n--restore: making every listed channel visible again.');
} else if (!APPLY) {
	console.log('\nNothing changed. Add --apply to hide them, or --restore to undo later.');
	console.log('Hiding is a permission overwrite, not a delete. No history is lost.\n');
	process.exit(0);
}

/* --- apply ------------------------------------------------------------- */

// The @everyone role always shares the guild's own id.
const EVERYONE = GUILD;
const targets = RESTORE ? [...keep, ...hide] : hide;

let changed = 0;
for (const c of targets) {
	const existing = (c.permission_overwrites ?? []).find((o) => o.id === EVERYONE);
	const deny = BigInt(existing?.deny ?? '0');
	const allow = BigInt(existing?.allow ?? '0');

	const nextDeny = RESTORE ? deny & ~VIEW_CHANNEL : deny | VIEW_CHANNEL;
	if (nextDeny === deny) continue; // already in the state we want

	const res = await api(`/channels/${c.id}/permissions/${EVERYONE}`, {
		method: 'PUT',
		headers: {
			'Content-Type': 'application/json',
			'X-Audit-Log-Reason': RESTORE ? 'Restoring channel visibility' : 'Quieting unused channels',
		},
		body: JSON.stringify({ type: 0, allow: allow.toString(), deny: nextDeny.toString() }),
	});

	if (res.ok) {
		changed += 1;
		console.log(`  ${RESTORE ? 'shown' : 'hidden'}: #${c.name}`);
	} else {
		console.error(`  failed: #${c.name} — ${res.status} ${(await res.text()).slice(0, 120)}`);
	}
}

console.log(`\n${changed} channel${changed === 1 ? '' : 's'} changed.`);
