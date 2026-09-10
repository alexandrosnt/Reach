/**
 * Exercises github-webhook.mjs against a fake GitHub. No token, no network.
 *
 * The failure this is really guarding against is the quiet one: a webhook that
 * GitHub accepts and Discord silently drops, because the event is one Discord's
 * receiver does not render. That looks identical to a working webhook right up
 * until you notice the channel has been empty for a week.
 *
 * Run: node scripts/discord/github-webhook.test.mjs
 */

import { mkdtemp, mkdir, writeFile, rm } from 'fs/promises';
import { tmpdir } from 'os';
import { join } from 'path';
import { pathToFileURL } from 'url';
import { dirname } from 'path';
import { fileURLToPath } from 'url';

const SCRIPT = pathToFileURL(
	join(dirname(fileURLToPath(import.meta.url)), 'github-webhook.mjs')
).href;

/**
 * Every event Discord's `/github` receiver renders. Anything outside this set
 * posts nothing at all, so the plan must not contain one.
 * https://discord.com/developers/docs — GitHub-compatible webhook.
 */
const DISCORD_RENDERS = new Set([
	'commit_comment',
	'create',
	'delete',
	'fork',
	'gollum',
	'issue_comment',
	'issues',
	'member',
	'public',
	'pull_request',
	'pull_request_review',
	'pull_request_review_comment',
	'push',
	'release',
	'watch',
	'ping'
]);

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);

/** A throwaway repo root: .git/config, .env and .discord/webhooks.json. */
async function makeRoot({ withSecret = false } = {}) {
	const root = await mkdtemp(join(tmpdir(), 'reach-gh-'));
	await mkdir(join(root, '.git'));
	await mkdir(join(root, '.discord'));
	await writeFile(
		join(root, '.git', 'config'),
		'[remote "origin"]\n\turl = https://github.com/alexandrosnt/Reach.git\n\tfetch = +refs/heads/*\n'
	);
	const store = {
		releases: { channel: '#releases', url: 'https://discord.com/api/webhooks/1/aaa', githubUrl: 'https://discord.com/api/webhooks/1/aaa/github' },
		github: { channel: '#github', url: 'https://discord.com/api/webhooks/2/bbb', githubUrl: 'https://discord.com/api/webhooks/2/bbb/github' }
	};
	if (withSecret) store._secret = 'existing-secret';
	await writeFile(join(root, '.discord', 'webhooks.json'), JSON.stringify(store, null, 2));
	return root;
}

/** github-webhook.mjs resolves paths from its own location, so it has to be
 *  told where the repo is; it reads ROOT as ../.. — hence a copy alongside. */
async function stageScript(root) {
	const { readFile } = await import('fs/promises');
	const src = await readFile(new URL(SCRIPT), 'utf-8');
	await mkdir(join(root, 'scripts', 'discord'), { recursive: true });
	const dest = join(root, 'scripts', 'discord', 'github-webhook.mjs');
	await writeFile(dest, src);
	return pathToFileURL(dest).href;
}

function installFetch(state) {
	globalThis.fetch = async (url, init = {}) => {
		const path = String(url).replace('https://api.github.com', '');
		const method = init.method ?? 'GET';
		const body = init.body ? JSON.parse(init.body) : undefined;
		state.calls.push({ method, path, body });

		const ok = (data, status = 200) => ({
			ok: true,
			status,
			json: async () => data,
			text: async () => JSON.stringify(data)
		});

		if (/^\/repos\/[^/]+\/[^/]+\/hooks$/.test(path) && method === 'GET') return ok(state.hooks);
		if (/^\/repos\/[^/]+\/[^/]+\/hooks$/.test(path) && method === 'POST') {
			const h = { id: state.hooks.length + 100, ...body };
			state.hooks.push(h);
			return ok(h, 201);
		}
		const one = /^\/repos\/[^/]+\/[^/]+\/hooks\/(\d+)$/.exec(path);
		if (one && method === 'PATCH') {
			const h = state.hooks.find((x) => String(x.id) === one[1]);
			Object.assign(h, body);
			return ok(h);
		}
		if (/\/deliveries/.test(path)) {
			return ok([{ event: 'ping', status_code: 204, status: 'OK' }]);
		}
		throw new Error(`unmocked: ${method} ${path}`);
	};
}

async function run(root, { apply, hooks = [], token = 'ghp_fake' }) {
	const state = { hooks, calls: [] };
	installFetch(state);
	process.env.GITHUB_TOKEN = token;
	process.argv = ['node', 'github-webhook.mjs', ...(apply ? ['--apply'] : [])];

	const staged = await stageScript(root);
	const lines = [];
	const realLog = console.log;
	console.log = (...a) => lines.push(a.join(' '));
	try {
		await import(`${staged}?v=${Math.random()}`);
	} finally {
		console.log = realLog;
	}
	return { out: lines.join('\n'), state };
}

// ===========================================================================
section('the event plan');
{
	const src = await (await import('fs/promises')).readFile(new URL(SCRIPT), 'utf-8');
	const block = /const FEEDS = \{([\s\S]*?)\n\};/.exec(src)[1];
	const events = [...block.matchAll(/'([a-z_]+)'/g)].map((m) => m[1]);

	check('the plan lists some events', events.length > 0);
	const unrenderable = events.filter((e) => !DISCORD_RENDERS.has(e));
	check(
		'every event is one Discord actually renders',
		unrenderable.length === 0,
		`Discord would silently drop: ${unrenderable.join(', ')}`
	);
	check('push is covered', events.includes('push'));
	check('releases are covered', events.includes('release'));
	check('pull requests are covered', events.includes('pull_request'));
	check('issues are covered', events.includes('issues'));
}

// ===========================================================================
section('dry run');
{
	const root = await makeRoot();
	const { out, state } = await run(root, { apply: false });
	check('creates nothing', state.hooks.length === 0);
	check('makes no write calls', !state.calls.some((c) => c.method !== 'GET'));
	check('says what it would do', out.includes('would create'));
	check('names the repo from the git remote', out.includes('alexandrosnt/Reach'));
	await rm(root, { recursive: true, force: true });
}

// ===========================================================================
section('apply');
{
	const root = await makeRoot();
	const { out, state } = await run(root, { apply: true });

	check('creates two hooks', state.hooks.length === 2, `${state.hooks.length}`);
	check('both point at the /github endpoint', state.hooks.every((h) => h.config.url.endsWith('/github')));
	check('both send JSON', state.hooks.every((h) => h.config.content_type === 'json'));
	check('both are signed', state.hooks.every((h) => h.config.secret?.length >= 32));
	check('both share one secret', new Set(state.hooks.map((h) => h.config.secret)).size === 1);
	check('both are active', state.hooks.every((h) => h.active));
	check('TLS verification stays on', state.hooks.every((h) => h.config.insecure_ssl === '0'));

	const rel = state.hooks.find((h) => h.config.url.includes('/1/'));
	const dev = state.hooks.find((h) => h.config.url.includes('/2/'));
	check('the releases hook carries only releases', JSON.stringify(rel.events) === JSON.stringify(['release']));
	check('the activity hook does not carry releases', !dev.events.includes('release'));
	check('the activity hook carries pushes', dev.events.includes('push'));

	check('the Discord URL is never printed', !out.includes('aaa') && !out.includes('bbb'), 'LEAKED');
	check('the secret is never printed', !/[0-9a-f]{64}/.test(out), 'LEAKED');
	check('it reports the delivery result', out.includes('204'));
	await rm(root, { recursive: true, force: true });
}

// ===========================================================================
section('re-apply');
{
	const root = await makeRoot({ withSecret: true });
	const first = await run(root, { apply: true });
	const { out, state } = await run(root, { apply: true, hooks: first.state.hooks });
	check('creates no duplicates', state.hooks.length === 2, `${state.hooks.length}`);
	check('reports both as correct', (out.match(/already correct/g) ?? []).length === 2);
	check('reuses the stored secret', state.hooks.every((h) => h.config.secret === 'existing-secret'));
	await rm(root, { recursive: true, force: true });
}

// ===========================================================================
section('someone narrowed the events in the GitHub UI');
{
	const root = await makeRoot({ withSecret: true });
	const stale = [
		{
			id: 1,
			active: true,
			events: ['push'],
			config: { url: 'https://discord.com/api/webhooks/2/bbb/github', content_type: 'json' }
		}
	];
	const { out, state } = await run(root, { apply: true, hooks: stale });
	check('puts the events back', state.hooks.find((h) => h.id === 1).events.includes('pull_request'));
	check('does not create a second hook for it', state.hooks.filter((h) => h.config.url.includes('/2/')).length === 1);
	check('says it updated', out.includes('updated'));
	await rm(root, { recursive: true, force: true });
}

// ===========================================================================
section('a disabled hook');
{
	const root = await makeRoot({ withSecret: true });
	const off = [
		{
			id: 1,
			active: false,
			events: ['push', 'pull_request', 'pull_request_review', 'issues', 'issue_comment'],
			config: { url: 'https://discord.com/api/webhooks/2/bbb/github', content_type: 'json' }
		}
	];
	const { state } = await run(root, { apply: true, hooks: off });
	check('is switched back on', state.hooks.find((h) => h.id === 1).active === true);
	await rm(root, { recursive: true, force: true });
}

console.log(`\n${failures === 0 ? 'ALL PASS' : failures + ' FAILURE(S)'}`);
process.exitCode = failures === 0 ? 0 : 1;
