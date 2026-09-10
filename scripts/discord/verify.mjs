/**
 * Reads the live server back and checks it against the plan.
 *
 * Separate from setup.mjs on purpose. setup.mjs reports what it believes it
 * did; this asks Discord what is actually true. Those came apart once already
 * — a run that printed "permissions updated" for every channel while the role
 * ordering silently failed — and the only way to tell the difference is to go
 * and look.
 *
 * Read-only: it never writes anything, so it is safe to run any time.
 *
 * Run: npm run discord:verify
 */

import { readFile } from 'fs/promises';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { PLAN, ROLES, EVERYONE_PERMISSIONS } from './plan.mjs';
import { P, bits } from './permissions.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

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
		if (!(m[1] in process.env)) process.env[m[1]] = m[2].trim().replace(/^["']|["']$/g, '');
	}
}

await loadDotEnv();
const GUILD = process.env.DISCORD_GUILD_ID;
if (!process.env.DISCORD_BOT_TOKEN || !GUILD) {
	console.error('Set DISCORD_BOT_TOKEN and DISCORD_GUILD_ID. See scripts/discord/README.md.');
	process.exit(1);
}

const H = {
	Authorization: 'Bot ' + process.env.DISCORD_BOT_TOKEN,
	'User-Agent': 'ReachVerify (https://github.com/alexandrosnt/Reach, 0.5.2)'
};
const get = async (p) => {
	const res = await fetch('https://discord.com/api/v10' + p, { headers: H });
	if (!res.ok) throw new Error(`GET ${p} -> ${res.status}: ${await res.text()}`);
	return res.json();
};

const roles = await get(`/guilds/${GUILD}/roles`);
const channels = await get(`/guilds/${GUILD}/channels`);

const slugOf = (n) => n.toLowerCase().replace(/^[^a-z0-9]+/u, '');
const roleId = (n) => roles.find((r) => r.name === n)?.id;
const chan = (slug) => channels.find((c) => c.type !== 4 && slugOf(c.name) === slug);
const ow = (c, id) => (c.permission_overwrites ?? []).find((o) => o.id === id);
const has = (mask, bit) => (BigInt(mask ?? '0') & bit) === bit;

let bad = 0;
const t = (name, cond, extra = '') => {
	if (cond) {
		console.log('  ok    ' + name);
	} else {
		bad++;
		console.log('  BAD   ' + name + (extra ? '  [' + extra + ']' : ''));
	}
};
const section = (s) => console.log('\n' + s);

const PLANNED_CHANNELS = PLAN.reduce((n, g) => n + g.channels.length, 0);

// ---------------------------------------------------------------------------
section('roles');
for (const r of ROLES) {
	const live = roles.find((x) => x.name === r.name);
	t(`@${r.name} exists`, !!live);
	if (!live) continue;
	t(`@${r.name} has the planned permissions`, live.permissions === bits(r.permissions), live.permissions);
	t(`@${r.name} hoist is ${r.hoist}`, live.hoist === r.hoist);
}

const everyone = roles.find((r) => r.id === GUILD);
t('@everyone matches the baseline', everyone.permissions === bits(EVERYONE_PERMISSIONS), everyone.permissions);
t('@everyone cannot ping the server', !has(everyone.permissions, P.MENTION_EVERYONE));

// A bot cannot raise its own role, so this stays a warning rather than a
// failure — the roles work, they are merely listed in the wrong order.
const distinct = new Set(ROLES.map((r) => roles.find((x) => x.name === r.name)?.position)).size;
if (distinct === 1) {
	console.log('  note  all roles share one position; drag the bot to the top and re-run setup to order them');
}

// ---------------------------------------------------------------------------
section('structure');
t(`${PLAN.length} categories`, channels.filter((c) => c.type === 4).length === PLAN.length);
t(`${PLANNED_CHANNELS} channels`, channels.filter((c) => c.type !== 4).length === PLANNED_CHANNELS);
t('no duplicate names', new Set(channels.map((c) => c.name)).size === channels.length);

// ---------------------------------------------------------------------------
for (const group of PLAN) {
	section(group.emoji + ' ' + group.category);
	const cat = channels.find((c) => c.type === 4 && slugOf(c.name) === slugOf(group.category));
	t('category exists', !!cat);

	for (const ch of group.channels) {
		const live = chan(ch.name);
		if (!live) {
			t(`#${ch.name} exists`, false);
			continue;
		}
		const label = '#' + live.name;
		t(`${label} is in the right category`, live.parent_id === cat?.id);
		t(`${label} carries its emoji`, live.name === ch.emoji + '-' + ch.name, live.name);

		if (ch.private) {
			t(`${label} is hidden from @everyone`, has(ow(live, GUILD)?.deny, P.VIEW_CHANNEL));
			for (const r of ch.private) {
				t(`${label} is visible to @${r}`, has(ow(live, roleId(r))?.allow, P.VIEW_CHANNEL));
			}
		} else {
			t(`${label} is not hidden`, !has(ow(live, GUILD)?.deny, P.VIEW_CHANNEL));
		}

		if (ch.readOnly) {
			t(`${label} denies SEND_MESSAGES`, has(ow(live, GUILD)?.deny, P.SEND_MESSAGES));
			t(`${label} still allows reactions`, has(ow(live, GUILD)?.allow, P.ADD_REACTIONS));
		} else if (!ch.private) {
			t(`${label} is open`, (live.permission_overwrites ?? []).length === 0);
		}

		for (const r of ch.writers ?? []) {
			t(`${label} lets @${r} post`, has(ow(live, roleId(r))?.allow, P.SEND_MESSAGES));
		}

		if (ch.noThreads) {
			t(`${label} blocks threads`, has(ow(live, GUILD)?.deny, P.CREATE_PUBLIC_THREADS));
		}

		if (ch.webhook) {
			const hooks = await get(`/channels/${live.id}/webhooks`);
			t(`${label} has exactly one webhook`, hooks.length === 1, String(hooks.length));
		}
	}
}

console.log('\n' + (bad === 0 ? 'The live server matches the plan.' : bad + ' mismatch(es).'));
process.exitCode = bad ? 1 : 0;
