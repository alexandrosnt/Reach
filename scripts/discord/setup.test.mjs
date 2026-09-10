/**
 * Exercises setup.mjs against a fake Discord. Nothing here touches the network
 * and nothing here needs a token.
 *
 * Worth having for a throwaway script because the thing it mutates is a real
 * server. The failures that matter are not crashes — they are the quiet ones:
 * a second run duplicating eighteen channels, a private channel that is not
 * actually private, a role reference that resolves to nothing.
 *
 * Run: node scripts/discord/setup.test.mjs
 */

import { pathToFileURL, fileURLToPath } from 'url';
import { join, dirname } from 'path';
import { mkdtemp, mkdir, copyFile, readFile } from 'fs/promises';
import { tmpdir } from 'os';
import { createHash } from 'crypto';
import { PLAN, ROLES, EVERYONE_PERMISSIONS } from './plan.mjs';
import { P, bits, TALK } from './permissions.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));

/**
 * setup.mjs writes .discord/webhooks.json relative to its OWN location, so
 * importing it from here pointed it at the real repo — and a test run
 * overwrote the live webhook URLs with mock ones (id 1199, token "sekrit").
 * The Discord webhooks survived; the file that named them did not, and the
 * GitHub feed pointed at nothing until it was regenerated.
 *
 * So the module under test is copied into a temp tree first. Its ROOT then
 * resolves there, and the worst a run can do is litter a temp directory.
 */
let staged = null;
async function stagedSetup() {
	if (staged) return staged;
	const dir = join(await mkdtemp(join(tmpdir(), 'reach-discord-')), 'scripts', 'discord');
	await mkdir(dir, { recursive: true });
	for (const f of ['setup.mjs', 'plan.mjs', 'permissions.mjs']) {
		await copyFile(join(HERE, f), join(dir, f));
	}
	staged = pathToFileURL(join(dir, 'setup.mjs')).href;
	return staged;
}

/** Hash of the real webhooks file, so the run can prove it never touched it. */
const REAL_WEBHOOKS = join(HERE, '..', '..', '.discord', 'webhooks.json');
async function realWebhooksHash() {
	try {
		return createHash('sha256').update(await readFile(REAL_WEBHOOKS)).digest('hex');
	} catch {
		return 'absent';
	}
}
const webhooksBefore = await realWebhooksHash();
const GUILD = '111111111111111111';
const BOT_ROLE = '222222222222222222';

const PLANNED_CATEGORIES = PLAN.length;
const PLANNED_CHANNELS = PLAN.reduce((n, g) => n + g.channels.length, 0);

let idSeq = 1000;
const nextId = () => String(++idSeq);

/**
 * A fake guild. `botPosition` defaults high so ordering has room; pass 1 to
 * reproduce a real fresh server, where Discord puts every role — the bot's
 * included — at position 1 and nothing can be ordered until the bot moves.
 */
function makeGuild({ community = false, botPosition = 30 } = {}) {
	return {
		channels: [],
		webhooks: new Map(),
		roles: [
			{ id: GUILD, name: '@everyone', position: 0, permissions: '0' },
			{ id: BOT_ROLE, name: 'doctor', position: botPosition, permissions: '8', managed: true }
		],
		features: community ? ['COMMUNITY'] : [],
		positionPatches: 0
	};
}

function installFetch(g) {
	globalThis.fetch = async (url, init = {}) => {
		const path = url.replace('https://discord.com/api/v10', '');
		const method = init.method ?? 'GET';
		const body = init.body ? JSON.parse(init.body) : undefined;
		const ok = (data, status = 200) => ({
			ok: true,
			status,
			headers: new Map(),
			json: async () => data,
			text: async () => JSON.stringify(data)
		});

		if (path === '/users/@me') return ok({ id: '9', username: 'doctor' });
		if (path === `/guilds/${GUILD}`) return ok({ id: GUILD, name: 'Reach', features: g.features });
		// A bot must name its own id here; `@me` is the OAuth2-only spelling and
		// Discord answers it with a 400.
		if (path === `/guilds/${GUILD}/members/@me`) {
			return { ok: false, status: 400, headers: new Map(), text: async () => '{"message":"Invalid Form Body"}' };
		}
		if (path === `/guilds/${GUILD}/members/9`) return ok({ roles: [BOT_ROLE] });

		// roles
		if (path === `/guilds/${GUILD}/roles` && method === 'GET') return ok(g.roles);
		if (path === `/guilds/${GUILD}/roles` && method === 'POST') {
			const r = { id: nextId(), position: 1, ...body };
			g.roles.push(r);
			return ok(r, 201);
		}
		if (path === `/guilds/${GUILD}/roles` && method === 'PATCH') {
			g.positionPatches++;
			for (const { id, position } of body) {
				const r = g.roles.find((x) => x.id === id);
				if (r) r.position = position;
			}
			return ok(g.roles);
		}
		const oneRole = new RegExp(`^/guilds/${GUILD}/roles/(\\d+)$`).exec(path);
		if (oneRole && method === 'PATCH') {
			const r = g.roles.find((x) => x.id === oneRole[1]);
			Object.assign(r, body);
			return ok(r);
		}

		// channels
		if (path === `/guilds/${GUILD}/channels` && method === 'GET') return ok(g.channels);
		if (path === `/guilds/${GUILD}/channels` && method === 'POST') {
			const c = { id: nextId(), permission_overwrites: [], ...body };
			g.channels.push(c);
			return ok(c, 201);
		}
		const oneChannel = /^\/channels\/(\d+)$/.exec(path);
		if (oneChannel && method === 'PATCH') {
			const c = g.channels.find((x) => x.id === oneChannel[1]);
			Object.assign(c, body);
			return ok(c);
		}

		// webhooks
		const wh = /^\/channels\/(\d+)\/webhooks$/.exec(path);
		if (wh && method === 'GET') return ok(g.webhooks.get(wh[1]) ?? []);
		if (wh && method === 'POST') {
			const list = g.webhooks.get(wh[1]) ?? [];
			const hook = {
				id: nextId(),
				name: body.name,
				token: 'sekrit',
				url: `https://discord.com/api/webhooks/${nextId()}/sekrit`
			};
			list.push(hook);
			g.webhooks.set(wh[1], list);
			return ok(hook, 201);
		}

		throw new Error(`unmocked: ${method} ${path}`);
	};
}

/** A fresh module instance per run, so no state leaks between cases. */
async function run(g, { apply }) {
	installFetch(g);
	process.env.DISCORD_BOT_TOKEN = 'fake';
	process.env.DISCORD_GUILD_ID = GUILD;
	process.argv = ['node', 'setup.mjs', ...(apply ? ['--apply'] : [])];

	const mod = await import(`${await stagedSetup()}?v=${Math.random()}`);
	const lines = [];
	const realLog = console.log;
	console.log = (...a) => lines.push(a.join(' '));
	try {
		await mod.main();
	} finally {
		console.log = realLog;
	}
	return { out: lines.join('\n'), mod };
}

let failures = 0;
function check(name, cond, extra = '') {
	if (cond) {
		console.log(`  PASS  ${name}`);
	} else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
}
const section = (s) => console.log(`\n${s}`);

const chanNamed = (g, slug) => g.channels.find((c) => c.name.endsWith(slug) && c.type !== 4);
const owFor = (c, id) => (c.permission_overwrites ?? []).find((o) => o.id === id);
const has = (mask, bit) => (BigInt(mask ?? '0') & bit) === bit;

// ===========================================================================
section('plan integrity (no network at all)');
{
	const roleNames = new Set(ROLES.map((r) => r.name));
	const referenced = new Set();
	for (const g of PLAN) {
		for (const c of g.channels) {
			for (const n of [...(c.writers ?? []), ...(c.private ?? [])]) referenced.add(n);
		}
	}
	const dangling = [...referenced].filter((n) => !roleNames.has(n));
	check('every role a channel references exists', dangling.length === 0, `dangling: ${dangling}`);

	const slugs = PLAN.flatMap((g) => g.channels.map((c) => `${g.category}/${c.name}`));
	check('no duplicate channel slugs', new Set(slugs).size === slugs.length);
	check('every channel has an emoji', PLAN.every((g) => g.channels.every((c) => c.emoji)));
	check('every category has an emoji', PLAN.every((g) => g.emoji));
	check('every channel has a topic', PLAN.every((g) => g.channels.every((c) => c.topic)));
	check('role names are unique', new Set(ROLES.map((r) => r.name)).size === ROLES.length);

	let permsOk = true;
	try {
		for (const r of ROLES) bits(r.permissions);
		bits(EVERYONE_PERMISSIONS);
	} catch (e) {
		permsOk = false;
	}
	check('every permission name is real', permsOk);

	// The whole point of the moderator tier: it can police, not rebuild.
	const mod = ROLES.find((r) => r.name === 'Moderator');
	check(
		'Moderator cannot reshape the server',
		!mod.permissions.some((p) => ['MANAGE_CHANNELS', 'MANAGE_ROLES', 'MANAGE_GUILD', 'ADMINISTRATOR'].includes(p))
	);
	check('@everyone cannot ping the server', !EVERYONE_PERMISSIONS.includes('MENTION_EVERYONE'));
	check(
		'@everyone holds no MANAGE_* bit',
		!EVERYONE_PERMISSIONS.some((p) => p.startsWith('MANAGE_'))
	);
	check(
		'recognition roles carry no powers',
		ROLES.filter((r) => !['Maintainer', 'Moderator', 'Core Contributor'].includes(r.name)).every(
			(r) => r.permissions.length === 0
		)
	);
}

// ===========================================================================
section('slugOf');
{
	const { mod } = await run(makeGuild(), { apply: false });
	const s = mod.slugOf;
	check('strips a leading emoji', s('\u{1F44B}-welcome') === 'welcome', s('\u{1F44B}-welcome'));
	check('keeps interior hyphens', s('\u{1F3B2}-off-topic') === 'off-topic', s('\u{1F3B2}-off-topic'));
	check('lowercases a category', s('\u{1F4CC} INFORMATION') === 'information', s('\u{1F4CC} INFORMATION'));
	check('leaves a bare name alone', s('general') === 'general');
}

// ===========================================================================
section('dry run');
{
	const g = makeGuild();
	const { out } = await run(g, { apply: false });
	check('creates no channels', g.channels.length === 0);
	check('creates no roles', g.roles.length === 2);
	check('creates no webhooks', g.webhooks.size === 0);
	check('does not reposition roles', g.positionPatches === 0);
	check('says "would create"', out.includes('would create'));
	check('warns perms are unknown until roles exist', out.includes('shown only once the roles exist'));
}

// ===========================================================================
section('apply to an empty guild');
const fresh = makeGuild();
{
	const { out } = await run(fresh, { apply: true });

	check('9 roles created', fresh.roles.length === 2 + ROLES.length, `${fresh.roles.length - 2}`);
	check('roles ordered once', fresh.positionPatches === 1);
	check(
		'roles are in plan order, highest first',
		ROLES.every((r, i) => {
			const made = fresh.roles.find((x) => x.name === r.name);
			const next = fresh.roles.find((x) => x.name === ROLES[i + 1]?.name);
			return !next || made.position > next.position;
		})
	);
	check(
		'all roles sit below the bot',
		ROLES.every((r) => fresh.roles.find((x) => x.name === r.name).position < 30)
	);
	check(
		'@everyone baseline applied',
		fresh.roles.find((r) => r.id === GUILD).permissions === bits(EVERYONE_PERMISSIONS)
	);

	const cats = fresh.channels.filter((c) => c.type === 4);
	const texts = fresh.channels.filter((c) => c.type !== 4);
	check(`${PLANNED_CATEGORIES} categories`, cats.length === PLANNED_CATEGORIES, `got ${cats.length}`);
	check(`${PLANNED_CHANNELS} channels`, texts.length === PLANNED_CHANNELS, `got ${texts.length}`);
	check('every channel has a parent', texts.every((c) => c.parent_id));
	check('every channel name carries its emoji', texts.every((c) => /^[^a-z0-9]/.test(c.name)));

	// -- the permission claims that actually matter -------------------------
	const staffIds = ['Maintainer', 'Moderator'].map((n) => fresh.roles.find((r) => r.name === n).id);

	const staffChat = chanNamed(fresh, 'staff-chat');
	check('private channel hides itself from @everyone', has(owFor(staffChat, GUILD)?.deny, P.VIEW_CHANNEL));
	check(
		'private channel is visible to both staff roles',
		staffIds.every((id) => has(owFor(staffChat, id)?.allow, P.VIEW_CHANNEL))
	);

	const announcements = chanNamed(fresh, 'announcements');
	check('read-only denies SEND_MESSAGES to @everyone', has(owFor(announcements, GUILD)?.deny, P.SEND_MESSAGES));
	check('read-only still allows reactions', has(owFor(announcements, GUILD)?.allow, P.ADD_REACTIONS));
	check(
		'writers get SEND_MESSAGES back',
		staffIds.every((id) => has(owFor(announcements, id)?.allow, P.SEND_MESSAGES))
	);

	const welcome = chanNamed(fresh, 'welcome');
	check('noThreads denies thread creation', has(owFor(welcome, GUILD)?.deny, P.CREATE_PUBLIC_THREADS));
	check('writers keep thread creation', has(owFor(welcome, staffIds[0])?.allow, P.CREATE_PUBLIC_THREADS));

	const general = chanNamed(fresh, 'general');
	check('an ordinary channel has no overwrites', (general.permission_overwrites ?? []).length === 0);

	const github = chanNamed(fresh, 'github');
	check('a feed channel is not accidentally private', !has(owFor(github, GUILD)?.deny, P.VIEW_CHANNEL));

	check('2 webhooks', [...fresh.webhooks.values()].flat().length === 2);
	check('webhook token never printed', !out.includes('sekrit'), 'LEAKED');
	check('masked line shown', out.includes('/********'));
}

// ===========================================================================
section('re-apply (idempotency)');
{
	const before = fresh.channels.length;
	const beforeRoles = fresh.roles.length;
	const { out } = await run(fresh, { apply: true });
	check('no new channels', fresh.channels.length === before, `${before} -> ${fresh.channels.length}`);
	check('no new roles', fresh.roles.length === beforeRoles);
	check('no duplicate webhooks', [...fresh.webhooks.values()].flat().length === 2);
	check('nothing repermissioned', /0 repermissioned/.test(out), out.split('\n').at(-1));
	check('nothing renamed', /0 renamed/.test(out), out.split('\n').at(-1));
	check('nothing created', /^Done\. 0 created/.test(out.split('\n').at(-1)), out.split('\n').at(-1));
}

// ===========================================================================
section('a channel someone renamed or unlocked by hand');
{
	const announcements = chanNamed(fresh, 'announcements');
	announcements.name = 'announcements';           // emoji stripped in the client
	announcements.permission_overwrites = [];        // and the lock removed
	const { out } = await run(fresh, { apply: true });

	check('re-adds the emoji', announcements.name.startsWith('\u{1F4E3}'), announcements.name);
	check('re-locks the channel', has(owFor(announcements, GUILD)?.deny, P.SEND_MESSAGES));
	check('reports both', /1 renamed/.test(out) && /1 repermissioned/.test(out), out.split('\n').at(-1));
	check('does not create a duplicate', fresh.channels.filter((c) => slugEnds(c, 'announcements')).length === 1);
}
function slugEnds(c, slug) {
	return c.type !== 4 && c.name.replace(/^[^a-z0-9]+/, '') === slug;
}

// ===========================================================================
section('a half-built server');
{
	const g = makeGuild();
	const cat = { id: nextId(), name: 'SUPPORT', type: 4 };
	g.channels.push(cat, { id: nextId(), name: 'help', type: 0, parent_id: cat.id, permission_overwrites: [] });
	const { out } = await run(g, { apply: true });
	check('does not duplicate the category', g.channels.filter((c) => c.type === 4 && slugOfName(c.name) === 'support').length === 1);
	check('does not duplicate #help', g.channels.filter((c) => slugEnds(c, 'help')).length === 1);
	check('renames the category to carry its emoji', cat.name === '\u{1F198} SUPPORT', cat.name);
	check('creates everything else', g.channels.length === PLANNED_CATEGORIES + PLANNED_CHANNELS, `${g.channels.length}`);
}
function slugOfName(n) {
	return n.toLowerCase().replace(/^[^a-z0-9]+/, '');
}

// ===========================================================================
section('a name reused under another category');
{
	const g = makeGuild();
	const cat = { id: nextId(), name: 'ARCHIVE', type: 4 };
	g.channels.push(cat, { id: nextId(), name: 'general', type: 0, parent_id: cat.id, permission_overwrites: [] });
	await run(g, { apply: true });
	check('creates COMMUNITY/general anyway', g.channels.filter((c) => slugEnds(c, 'general')).length === 2);
	check('leaves the archived one alone', g.channels.find((c) => c.parent_id === cat.id).name === 'general');
}

// ===========================================================================
section('community server');
{
	const g = makeGuild({ community: true });
	await run(g, { apply: true });
	check('announcements becomes an announcement channel', chanNamed(g, 'announcements').type === 5);
	check('releases stays a text channel', chanNamed(g, 'releases').type === 0);
}

// ===========================================================================
section('a role a channel references has vanished');
{
	const g = makeGuild();
	await run(g, { apply: true });
	// Someone deletes Moderator in the client; the plan still points at it.
	g.roles = g.roles.filter((r) => r.name !== 'Moderator');
	let threw = null;
	try {
		await run(g, { apply: true });
	} catch (e) {
		threw = e;
	}
	// It is recreated rather than left dangling, which is the safe outcome.
	check('recreates the missing role instead of failing', threw === null && g.roles.some((r) => r.name === 'Moderator'), String(threw));
}

// ===========================================================================
section('overwritesFor rejects an unknown role');
{
	const { mod } = await run(makeGuild(), { apply: false });
	let msg = '';
	try {
		mod.overwritesFor({ name: 'x', writers: ['Nope'] }, GUILD, new Map());
	} catch (e) {
		msg = e.message;
	}
	check('throws, naming the role', msg.includes('Nope'), msg || 'did not throw');
}

// ===========================================================================
section('a fresh server, where every role starts at position 1');
{
	const g = makeGuild({ botPosition: 1 });
	const { out } = await run(g, { apply: true });

	const bot = g.roles.find((r) => r.id === BOT_ROLE);
	check('the bot raises its own role', bot.position > 1, `still at ${bot.position}`);
	check('and says so', out.includes('Raised @doctor'), out.split('\n').find((l) => l.includes('Raised')) ?? '');
	check('then orders the rest', out.includes('Roles ordered beneath'));
	check(
		'every role ends up below the bot',
		ROLES.every((r) => g.roles.find((x) => x.name === r.name).position < bot.position)
	);
	check(
		'in plan order',
		ROLES.every((r, i) => {
			const a = g.roles.find((x) => x.name === r.name);
			const b = g.roles.find((x) => x.name === ROLES[i + 1]?.name);
			return !b || a.position > b.position;
		})
	);
	check('which is what makes them assignable at all', bot.position > Math.max(...ROLES.map((r) => g.roles.find((x) => x.name === r.name).position)));
}

// ===========================================================================
section('the test suite itself');
{
	// This is the regression guard for the bug above: a green run that has
	// quietly rewritten the real webhook file is not a green run.
	check(
		'the real .discord/webhooks.json was not touched',
		(await realWebhooksHash()) === webhooksBefore,
		'the suite wrote to real repo state'
	);
	check('the module under test ran from a temp tree', staged?.includes('reach-discord-'), staged ?? 'not staged');
}

console.log(`\n${failures === 0 ? 'ALL PASS' : failures + ' FAILURE(S)'}`);
process.exitCode = failures === 0 ? 0 : 1;
