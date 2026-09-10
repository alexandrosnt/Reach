/**
 * Points the GitHub repository at the two Discord channels.
 *
 * Discord has a receiver for GitHub's payload format: take a normal Discord
 * webhook URL, append `/github`, and it renders pushes, pull requests, issues
 * and releases as embeds. GitHub POSTs straight there. Nothing of ours runs,
 * nothing polls, and there is no bot in the path at all.
 *
 *   npm run discord:github              # dry run
 *   npm run discord:github -- --apply   # for real
 *
 * Needs GITHUB_TOKEN in .env, with the `admin:repo_hook` scope (classic) or
 * Repository permissions -> Webhooks: Read and write (fine-grained). Neither
 * that token nor the Discord URLs are printed; the Discord URLs are read from
 * .discord/webhooks.json, which discord:setup wrote.
 *
 * No webhook secret is set, and that is deliberate. A GitHub webhook secret
 * only means something when the receiver has been given the same secret and
 * programmed to check the signature. Discord is never told it — there is no
 * field for it anywhere in Discord — so it cannot verify anything, and GitHub
 * would just be computing an HMAC for a header nobody reads.
 *
 * The webhook URL is the credential here. Anyone holding it can post to that
 * channel. That is why it lives in .discord/, which is gitignored, and why
 * nothing here ever prints it.
 */

import { readFile } from 'fs/promises';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const WEBHOOKS = join(ROOT, '.discord', 'webhooks.json');
const APPLY = process.argv.includes('--apply');

/**
 * Which GitHub events go to which channel.
 *
 * Only events Discord's `/github` receiver actually renders are listed. It
 * ignores everything else silently — a 204 and no message — so an event added
 * here that Discord does not understand looks like a working webhook that
 * never posts. `workflow_run` and `check_run` are the notable absentees: CI
 * results need a GitHub Action posting an ordinary Discord message instead.
 */
const FEEDS = {
	// Most people only want to know that a version shipped.
	releases: ['release'],

	// Everything a contributor would want, and nothing a lurker would mute the
	// server over. `create` and `delete` (branches and tags) are deliberately
	// left out: on an active repo they drown everything else.
	github: ['push', 'pull_request', 'pull_request_review', 'issues', 'issue_comment']
};

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

/** owner/name, from the git remote unless GITHUB_REPO overrides it. */
async function repoSlug() {
	if (process.env.GITHUB_REPO) return process.env.GITHUB_REPO;
	const config = await readFile(join(ROOT, '.git', 'config'), 'utf-8');
	const m = /github\.com[:/]([^/\s]+\/[^/\s]+?)(?:\.git)?\s/.exec(config);
	if (!m) throw new Error('Could not read the GitHub repo from .git/config. Set GITHUB_REPO.');
	return m[1];
}

await loadDotEnv();

if (!process.env.GITHUB_TOKEN) {
	console.error(
		'Set GITHUB_TOKEN in .env.\n\n' +
			'  Classic token:      github.com/settings/tokens  ->  scope: admin:repo_hook\n' +
			'  Fine-grained token: github.com/settings/personal-access-tokens\n' +
			'                      this repository -> Webhooks: Read and write\n\n' +
			'It is only used here, and never printed.'
	);
	process.exit(1);
}

let store;
try {
	store = JSON.parse(await readFile(WEBHOOKS, 'utf-8'));
} catch {
	console.error(
		'.discord/webhooks.json is missing. Run:  npm run discord:setup -- --apply\n' +
			'That is what creates the Discord webhooks this points GitHub at.'
	);
	process.exit(1);
}

const repo = await repoSlug();
const GH = 'https://api.github.com';
const H = {
	Authorization: 'Bearer ' + process.env.GITHUB_TOKEN,
	Accept: 'application/vnd.github+json',
	'X-GitHub-Api-Version': '2022-11-28',
	'Content-Type': 'application/json',
	'User-Agent': 'ReachSetup'
};

async function gh(method, path, body) {
	const res = await fetch(GH + path, {
		method,
		headers: H,
		body: body ? JSON.stringify(body) : undefined
	});
	if (res.status === 401) throw new Error('GitHub rejected GITHUB_TOKEN (401).');

	if (res.status === 403 || res.status === 404) {
		// GitHub names the exact permission it wanted in a response header, and
		// puts the reason in the body. Guessing at "probably a scope problem"
		// when the server already said which one is a waste of everyone's time.
		const needs = res.headers.get('x-accepted-github-permissions');
		const scopes = res.headers.get('x-oauth-scopes');
		let message = '';
		try {
			message = JSON.parse(await res.text()).message ?? '';
		} catch {
			/* body was not JSON */
		}

		throw new Error(
			`${method} ${path} -> ${res.status}` +
				(message ? `: ${message}` : '') +
				(needs ? `\n\n  GitHub wants:  ${needs}` : '') +
				(scopes !== null ? `\n  Token has:     ${scopes || '(no scopes)'}` : '') +
				'\n\n  Fine-grained token: github.com/settings/personal-access-tokens -> your token' +
				'\n                      -> Repository permissions -> Webhooks: Read and write' +
				'\n  Classic token:      github.com/settings/tokens -> scope: admin:repo_hook' +
				`\n\n  The token can already reach ${repo}; only the webhook permission is missing.`
		);
	}

	if (!res.ok) throw new Error(`${method} ${path} -> ${res.status}: ${await res.text()}`);
	return res.status === 204 ? null : res.json();
}

console.log('Repo:  ' + repo);
console.log('Mode:  ' + (APPLY ? 'APPLY' : 'dry run - nothing will change') + '\n');

const existing = await gh('GET', `/repos/${repo}/hooks`);
const same = (a, b) => a.length === b.length && [...a].sort().join() === [...b].sort().join();

for (const [channel, events] of Object.entries(FEEDS)) {
	const entry = store[channel];
	if (!entry) {
		console.log(`#${channel}: no Discord webhook recorded, skipping`);
		continue;
	}

	// content_type must be json: Discord's receiver parses nothing else, and
	// GitHub's own default is form-encoded, so leaving it alone produces
	// deliveries that succeed and post nothing.
	const config = {
		url: entry.githubUrl,
		content_type: 'json',
		insecure_ssl: '0'
	};

	// GitHub returns the URL back, so an existing hook is identified by it.
	const found = existing.find((h) => h.config?.url === entry.githubUrl);

	if (!found) {
		if (APPLY) {
			await gh('POST', `/repos/${repo}/hooks`, { name: 'web', active: true, events, config });
			console.log(`#${channel}: created  ->  ${events.join(', ')}`);
		} else {
			console.log(`#${channel}: would create  ->  ${events.join(', ')}`);
		}
		continue;
	}

	if (same(found.events, events) && found.active) {
		console.log(`#${channel}: already correct  ->  ${events.join(', ')}`);
		continue;
	}

	if (APPLY) {
		await gh('PATCH', `/repos/${repo}/hooks/${found.id}`, { active: true, events, config });
		console.log(`#${channel}: updated  ->  ${events.join(', ')}`);
	} else {
		console.log(
			`#${channel}: would update  ${found.events.join(', ')}  ->  ${events.join(', ')}`
		);
	}
}

if (APPLY) {
	console.log('\nChecking what GitHub actually delivered...\n');
	const hooks = await gh('GET', `/repos/${repo}/hooks`);
	for (const [channel] of Object.entries(FEEDS)) {
		const entry = store[channel];
		const hook = hooks.find((h) => h.config?.url === entry?.githubUrl);
		if (!hook) continue;
		// Every new hook gets a `ping`. Discord answers it with a 204, so this
		// is a real end-to-end check rather than a claim that it should work.
		const deliveries = await gh('GET', `/repos/${repo}/hooks/${hook.id}/deliveries?per_page=3`);
		const last = deliveries[0];
		console.log(
			`  #${channel}: ` +
				(last
					? `last delivery "${last.event}" -> ${last.status_code} ${last.status}`
					: 'no deliveries yet')
		);
	}
	console.log('\nPush something, or open an issue, and watch the channel.');
} else {
	console.log('\nRe-run with --apply to make it real.');
}
