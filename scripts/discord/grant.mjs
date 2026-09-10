/**
 * Grants roles to a member.
 *
 *   npm run discord:grant -- Maintainer Contributor          # the server owner
 *   npm run discord:grant -- 123456789012345678 Translator   # a specific user
 *
 * A bot may only assign a role positioned below its own highest, so this
 * checks that first and says so plainly rather than letting Discord answer
 * with a bare "Missing Permissions" that names neither the role nor the
 * reason. `setup.mjs` raises the bot's role high enough as part of ordering,
 * so in practice run that first.
 *
 * Additive: it never removes a role. Take one away in the client.
 */

import { readFile } from 'fs/promises';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { ROLES } from './plan.mjs';

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
	'Content-Type': 'application/json',
	'User-Agent': 'ReachGrant (https://github.com/alexandrosnt/Reach, 0.5.2)'
};
const A = 'https://discord.com/api/v10';

async function call(method, path, body) {
	const res = await fetch(A + path, {
		method,
		headers: H,
		body: body ? JSON.stringify(body) : undefined
	});
	const text = await res.text();
	let parsed = null;
	try {
		parsed = text ? JSON.parse(text) : null;
	} catch {
		parsed = text;
	}
	return { ok: res.ok, status: res.status, body: parsed };
}

// A snowflake is all digits; anything else is a role name.
const args = process.argv.slice(2);
const userArg = args.find((a) => /^\d{15,}$/.test(a));
const wanted = args.filter((a) => a !== userArg);

if (wanted.length === 0) {
	console.error('Name at least one role. Available:\n  ' + ROLES.map((r) => r.name).join('\n  '));
	process.exit(1);
}

const guild = (await call('GET', `/guilds/${GUILD}`)).body;
const userId = userArg ?? guild.owner_id;
const roles = (await call('GET', `/guilds/${GUILD}/roles`)).body;

const botUser = (await call('GET', '/users/@me')).body;
const botMember = (await call('GET', `/guilds/${GUILD}/members/${botUser.id}`)).body;
const botTop = Math.max(...roles.filter((r) => botMember.roles.includes(r.id)).map((r) => r.position));

const member = await call('GET', `/guilds/${GUILD}/members/${userId}`);
if (!member.ok) {
	console.error(`No member ${userId} in this guild (${member.status}).`);
	process.exit(1);
}
const held = new Set(member.body.roles);

console.log(
	`${member.body.user?.username ?? userId}` +
		(userId === guild.owner_id ? '  (server owner)' : '') +
		`\nBot can assign roles below position ${botTop}.\n`
);

let failed = 0;
for (const name of wanted) {
	const role = roles.find((r) => r.name.toLowerCase() === name.toLowerCase());
	if (!role) {
		console.log(`  ${name}: no such role`);
		failed++;
		continue;
	}
	if (held.has(role.id)) {
		console.log(`  ${role.name}: already held`);
		continue;
	}
	if (role.position >= botTop) {
		console.log(
			`  ${role.name}: blocked - the role sits at position ${role.position} and the bot's` +
				` own top is ${botTop}. Run discord:setup --apply first; it raises the bot.`
		);
		failed++;
		continue;
	}
	const res = await call('PUT', `/guilds/${GUILD}/members/${userId}/roles/${role.id}`);
	if (res.ok) {
		console.log(`  ${role.name}: granted`);
	} else {
		console.log(`  ${role.name}: failed (${res.status}) ${JSON.stringify(res.body)}`);
		failed++;
	}
}

process.exitCode = failed ? 1 : 0;
