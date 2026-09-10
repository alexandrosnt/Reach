/**
 * Exercises setup.mjs against a fake Discord. Nothing here touches the network
 * and nothing here needs a token.
 *
 * Worth having for a throwaway script because the thing it mutates is a real
 * server: the failure that matters is not "it crashed", it is "it quietly made
 * fifteen duplicate channels on the second run".
 *
 * Run: node scripts/discord/setup.test.mjs
 */
import { pathToFileURL } from 'url';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const SETUP = pathToFileURL(
	join(dirname(fileURLToPath(import.meta.url)), 'setup.mjs')
).href;
const GUILD = '111111111111111111';

let idSeq = 1000;
const nextId = () => String(++idSeq);

/** Fake guild state, shared across a run. */
function makeGuild(features = []) {
	return { channels: [], webhooks: new Map(), features, posts: [] };
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

		if (path === '/users/@me') return ok({ id: '9', username: 'ReachSetup' });
		if (path === `/guilds/${GUILD}`) return ok({ id: GUILD, name: 'Reach', features: g.features });
		if (path === `/guilds/${GUILD}/channels` && method === 'GET') return ok(g.channels);

		if (path === `/guilds/${GUILD}/channels` && method === 'POST') {
			const c = { id: nextId(), ...body };
			g.channels.push(c);
			g.posts.push(c);
			return ok(c, 201);
		}

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
async function run(g, { apply, community = false }) {
	g.features = community ? ['COMMUNITY'] : [];
	installFetch(g);
	process.env.DISCORD_BOT_TOKEN = 'fake';
	process.env.DISCORD_GUILD_ID = GUILD;
	process.argv = ['node', 'setup.mjs', ...(apply ? ['--apply'] : [])];

	const mod = await import(`${SETUP}?v=${Math.random()}`);
	const lines = [];
	const realLog = console.log;
	console.log = (...a) => lines.push(a.join(' '));
	try {
		await mod.main();
	} finally {
		console.log = realLog;
	}
	return lines.join('\n');
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

// -- 1. dry run must not create anything ------------------------------------
console.log('dry run');
{
	const g = makeGuild();
	const out = await run(g, { apply: false });
	check('creates no channels', g.channels.length === 0, `created ${g.channels.length}`);
	check('creates no webhooks', g.webhooks.size === 0);
	check('says "would create"', out.includes('would create'));
	check('reports 19 planned', /Dry run\. 19 would be created, 0 already there\./.test(out), out.split('\n').at(-2));
}

// -- 2. apply on an empty guild ---------------------------------------------
console.log('\napply, empty guild');
const fresh = makeGuild();
{
	const out = await run(fresh, { apply: true });
	const cats = fresh.channels.filter((c) => c.type === 4);
	const texts = fresh.channels.filter((c) => c.type === 0);
	check('4 categories', cats.length === 4, `got ${cats.length}`);
	check('15 text channels', texts.length === 15, `got ${texts.length}`);
	check('every channel has a parent', texts.every((c) => c.parent_id), 'some are orphaned');
	check('every channel has a topic', texts.every((c) => c.topic && c.topic.length > 0));

	const ro = fresh.channels.filter((c) => c.permission_overwrites);
	check('4 read-only channels', ro.length === 4, `got ${ro.length}: ${ro.map((c) => c.name)}`);
	check(
		'read-only denies SEND_MESSAGES to @everyone',
		ro.every((c) => c.permission_overwrites[0].id === GUILD && c.permission_overwrites[0].deny === '2048'),
		JSON.stringify(ro[0]?.permission_overwrites)
	);
	// The two feed channels and the two the maintainers post in. Everything
	// else must stay writable, or the server is a noticeboard.
	check('the right channels are read-only',
		JSON.stringify(ro.map((c) => c.name).sort()) ===
			JSON.stringify(['announcements', 'github', 'releases', 'welcome']),
		JSON.stringify(ro.map((c) => c.name).sort()));

	check('2 webhooks', [...fresh.webhooks.values()].flat().length === 2);
	check('webhook only on feed channels', (() => {
		const ids = [...fresh.webhooks.keys()];
		const names = ids.map((id) => fresh.channels.find((c) => c.id === id)?.name).sort();
		return JSON.stringify(names) === JSON.stringify(['github', 'releases']);
	})());
	check('webhook token never printed', !out.includes('sekrit'), 'LEAKED');
	check('masked line shown', out.includes('/********'));
	check('reports 19 created', /Done\. 19 created, 0 already there\./.test(out), out.split('\n').at(-1));
}

// -- 3. re-running must be a no-op ------------------------------------------
console.log('\nre-apply, same guild (idempotency)');
{
	const before = fresh.channels.length;
	const out = await run(fresh, { apply: true });
	check('no new channels', fresh.channels.length === before, `${before} -> ${fresh.channels.length}`);
	check('no duplicate webhooks', [...fresh.webhooks.values()].flat().length === 2);
	check('reuses the webhook', out.includes('reused'));
	check('reports 0 created, 19 there', /Done\. 0 created, 19 already there\./.test(out), out.split('\n').at(-1));
}

// -- 4. a partly-built server -----------------------------------------------
console.log('\npartial guild (a category and one channel already made by hand)');
{
	const g = makeGuild();
	const cat = { id: nextId(), name: 'SUPPORT', type: 4 };
	g.channels.push(cat, { id: nextId(), name: 'help', type: 0, parent_id: cat.id });
	const out = await run(g, { apply: true });
	check('does not duplicate SUPPORT', g.channels.filter((c) => c.name === 'SUPPORT').length === 1);
	check('does not duplicate #help', g.channels.filter((c) => c.name === 'help').length === 1);
	check('still creates the other 17', /Done\. 17 created, 2 already there\./.test(out), out.split('\n').at(-1));
}

// -- 5. same channel name under a different category is not confused --------
console.log('\nname collision across categories');
{
	const g = makeGuild();
	const cat = { id: nextId(), name: 'ARCHIVE', type: 4 };
	g.channels.push(cat, { id: nextId(), name: 'general', type: 0, parent_id: cat.id });
	await run(g, { apply: true });
	const generals = g.channels.filter((c) => c.name === 'general');
	check('creates COMMUNITY/general despite ARCHIVE/general', generals.length === 2, `got ${generals.length}`);
}

// -- 6. community server gets a real announcement channel ------------------
console.log('\ncommunity server');
{
	const g = makeGuild();
	await run(g, { apply: true, community: true });
	const ann = g.channels.find((c) => c.name === 'announcements');
	check('announcements is type 5', ann?.type === 5, `type ${ann?.type}`);
	const rel = g.channels.find((c) => c.name === 'releases');
	check('releases stays type 0', rel?.type === 0, `type ${rel?.type}`);
}

// -- 7. a foreign webhook we cannot use ------------------------------------
console.log('\nforeign webhook (no token visible to us)');
{
	const g = makeGuild();
	await run(g, { apply: true });
	const gh = g.channels.find((c) => c.name === 'github');
	g.webhooks.set(gh.id, [{ id: 'x', name: 'GitHub Activity' }]); // no token
	const out = await run(g, { apply: true });
	check('replaces the tokenless webhook', [...g.webhooks.get(gh.id)].some((h) => h.token));
	check('says created, not reused', out.includes('webhook "GitHub Activity" created'));
}

console.log(`\n${failures === 0 ? 'ALL PASS' : failures + ' FAILURE(S)'}`);
process.exitCode = failures === 0 ? 0 : 1;
