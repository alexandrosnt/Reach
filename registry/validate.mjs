/**
 * Validates the registry. This is what CI runs on every pull request.
 *
 * It checks the things a reviewer would otherwise have to check by eye, so
 * review can be spent on the part that actually matters: reading the script
 * and deciding whether it should run as root on someone's server. No amount of
 * validation substitutes for that.
 *
 *   node validate.mjs
 *
 * Exits non-zero on the first problem it can describe usefully, after
 * reporting all of them.
 */

import { readdir, readFile } from 'fs/promises';
import { createHash } from 'crypto';
import { join, dirname, basename } from 'path';
import { fileURLToPath } from 'url';

const HERE = dirname(fileURLToPath(import.meta.url));
const RECIPES = join(HERE, 'recipes');
const INDEX = join(HERE, 'recipes.json');

const DANGERS = ['benign', 'mutating', 'sensitive', 'destructive'];

const problems = [];
const note = (file, msg) => problems.push(`${file}: ${msg}`);

/** Mirrors src-tauri/src/recipe/schema.rs. Kept deliberately literal. */
function parseHeader(source, file) {
	const meta = { tags: [], targets: [], params: [] };
	let inHeader = false;
	let ended = false;
	let body = [];

	const lines = source.split(/\r?\n/);
	for (let i = 0; i < lines.length; i++) {
		const line = lines[i].trim();

		if (ended) {
			body.push(lines[i]);
			continue;
		}
		if (!line.startsWith('#')) {
			if (inHeader) return note(file, 'header is never closed with `# @end`');
			continue;
		}

		const content = line.replace(/^#+\s*/, '');
		if (content === '@reach-recipe') {
			inHeader = true;
			continue;
		}
		if (!inHeader) continue;
		if (content === '@end') {
			ended = true;
			continue;
		}

		const at = content.indexOf(':');
		if (at === -1) continue;
		const key = content.slice(0, at).trim().toLowerCase();
		const value = content.slice(at + 1).trim();

		if (key === 'tags' || key === 'targets') {
			meta[key] = value.split(',').map((s) => s.trim()).filter(Boolean);
		} else if (key === 'param') {
			const [name, label, def] = value.split('|').map((s) => (s ?? '').trim());
			meta.params.push({ name, label, default: def ?? '' });
		} else {
			meta[key] = value;
		}
	}

	if (!inHeader) return note(file, 'no `# @reach-recipe` marker');
	if (!ended) return note(file, 'header is never closed with `# @end`');
	meta.body = body.join('\n');
	return meta;
}

function checkRecipe(meta, file, source) {
	if (!meta) return;

	// --- identity ---------------------------------------------------------
	if (!meta.id) note(file, 'missing `id`');
	else if (!/^[a-z0-9]([a-z0-9-]*[a-z0-9])?$/.test(meta.id)) {
		note(file, `id "${meta.id}" must be lowercase letters, digits and hyphens`);
	} else if (meta.id !== basename(file, '.sh')) {
		// The URL in the index is built from the filename, so a mismatch points
		// clients at a file that does not exist.
		note(file, `id "${meta.id}" does not match the filename`);
	}

	if (!meta.name) note(file, 'missing `name`');
	if (!meta.description) note(file, 'missing `description` — it is what people read in the list');
	if (!meta.version) note(file, 'missing `version`');
	else if (!/^\d+\.\d+\.\d+$/.test(meta.version)) {
		note(file, `version "${meta.version}" should look like 1.2.3`);
	}
	if (meta.tags.length === 0) note(file, 'no `tags` — nobody will find it');

	// --- risk -------------------------------------------------------------
	if (!meta.danger) {
		note(file, 'missing `danger` — every recipe must state its own risk level');
	} else if (!DANGERS.includes(meta.danger)) {
		note(file, `danger "${meta.danger}" must be one of ${DANGERS.join(', ')}`);
	}

	// --- parameters -------------------------------------------------------
	for (const p of meta.params) {
		if (!/^[A-Z_][A-Z0-9_]*$/.test(p.name ?? '')) {
			// This string is interpolated into a shell assignment.
			note(file, `parameter "${p.name}" is not a valid shell identifier`);
		}
		if (!p.label) note(file, `parameter ${p.name} has no label`);
	}
	const names = meta.params.map((p) => p.name);
	const dupes = names.filter((n, i) => names.indexOf(n) !== i);
	if (dupes.length) note(file, `duplicate parameters: ${[...new Set(dupes)].join(', ')}`);

	// --- house rules ------------------------------------------------------
	if (!/^\s*set -euo pipefail\s*$/m.test(meta.body)) {
		note(file, 'does not `set -euo pipefail` — a failed step would carry on');
	}
	if (!source.startsWith('#!')) {
		note(file, 'no shebang — a recipe should still run as a plain script');
	}

	// Every parameter the header declares should actually be used.
	for (const p of meta.params) {
		const used = new RegExp(`\\$\\{?${p.name}\\b`).test(meta.body);
		if (!used) note(file, `parameter ${p.name} is declared but never used`);
	}

	// And every uppercase variable the script reads should come from somewhere,
	// because `set -u` turns a missing one into a failure partway through a run
	// on someone's server.
	//
	// "Somewhere" includes the script's own assignments, which is the whole
	// subtlety here: an earlier version of this check flagged every local a
	// recipe declared and produced fifteen false alarms on a correct file.
	// A validator that cries wolf is how people learn to skip the output.
	const assigned = new Set();
	for (const m of meta.body.matchAll(
		/^\s*(?:local\s+|export\s+|declare\s+(?:-\w+\s+)?|readonly\s+)?([A-Z_][A-Z0-9_]*)=/gm
	)) {
		assigned.add(m[1]);
	}
	for (const m of meta.body.matchAll(/\bfor\s+([A-Z_][A-Z0-9_]*)\s+in\b/g)) assigned.add(m[1]);
	for (const m of meta.body.matchAll(/\bread\s+(?:-\w+\s+)*([A-Z_][A-Z0-9_]*)/g)) assigned.add(m[1]);

	/** Set by the environment, not by us. */
	const AMBIENT = new Set([
		'DEBIAN_FRONTEND', 'PATH', 'HOME', 'USER', 'SHELL', 'PWD', 'LANG', 'LC_ALL',
		'TERM', 'HOSTNAME', 'UID', 'EUID', 'PPID', 'RANDOM', 'SECONDS', 'IFS',
		'BASH_VERSION', 'OSTYPE', 'PS1', 'TMPDIR', 'EDITOR'
	]);

	const undeclared = new Set();
	for (const m of meta.body.matchAll(/\$\{?([A-Z][A-Z0-9_]{2,})\}?/g)) {
		const name = m[1];
		if (AMBIENT.has(name) || assigned.has(name) || names.includes(name)) continue;
		undeclared.add(name);
	}
	for (const name of undeclared) {
		note(file, `reads $${name}, which is neither a parameter nor set by the script`);
	}
}

// ---------------------------------------------------------------------------

const files = (await readdir(RECIPES)).filter((f) => f.endsWith('.sh')).sort();
if (files.length === 0) problems.push('recipes/: no recipes found');

const seen = new Map();
const expected = [];

for (const file of files) {
	const source = await readFile(join(RECIPES, file), 'utf-8');
	const meta = parseHeader(source, file);
	checkRecipe(meta, file, source);
	if (!meta?.id) continue;

	if (seen.has(meta.id)) note(file, `duplicate id, also in ${seen.get(meta.id)}`);
	seen.set(meta.id, file);

	expected.push({
		id: meta.id,
		sha256: createHash('sha256').update(source, 'utf-8').digest('hex'),
		file
	});
}

// --- the index must match the files ---------------------------------------
let index;
try {
	index = JSON.parse(await readFile(INDEX, 'utf-8'));
} catch (e) {
	problems.push(`recipes.json: ${e.message}`);
}

if (Array.isArray(index)) {
	const byId = new Map(index.map((e) => [e.id, e]));

	for (const { id, sha256, file } of expected) {
		const entry = byId.get(id);
		if (!entry) {
			problems.push(`recipes.json: ${id} is missing. Run: node build-index.mjs --write`);
			continue;
		}
		if (entry.sha256 !== sha256) {
			// The hash is what Reach verifies before installing. A stale one
			// means every install of this recipe fails.
			problems.push(
				`recipes.json: ${id} sha256 is stale. Run: node build-index.mjs --write`
			);
		}
		if (!entry.url?.endsWith(`/${file}`)) {
			problems.push(`recipes.json: ${id} url does not point at recipes/${file}`);
		}
		if (!entry.url?.startsWith('https://')) {
			problems.push(`recipes.json: ${id} url must be https`);
		}
	}

	for (const entry of index) {
		if (!seen.has(entry.id)) {
			problems.push(`recipes.json: ${entry.id} has no file in recipes/`);
		}
	}
}

// ---------------------------------------------------------------------------
if (problems.length > 0) {
	console.error(`${problems.length} problem(s):\n`);
	for (const p of problems) console.error(`  ${p}`);
	console.error('\nSee CONTRIBUTING.md.');
	process.exit(1);
}

console.log(`OK: ${files.length} recipe(s), index current.`);
for (const { id } of expected) console.log(`  ${id}`);
