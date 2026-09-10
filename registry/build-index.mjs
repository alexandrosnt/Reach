/**
 * Rebuilds recipes.json from the .sh files in recipes/.
 *
 * The index carries a SHA-256 for every recipe and Reach refuses to install
 * one that does not match. That hash is the only thing standing between "the
 * maintainer reviewed these bytes" and "these bytes ran as root", so it is
 * generated from the file rather than typed by anyone.
 *
 *   node build-index.mjs            # check the index is current
 *   node build-index.mjs --write    # regenerate it
 *
 * Run with --write in the PR that adds a recipe, and commit the result.
 */

import { readdir, readFile, writeFile } from 'fs/promises';
import { createHash } from 'crypto';
import { join, dirname, basename } from 'path';
import { fileURLToPath } from 'url';

const HERE = dirname(fileURLToPath(import.meta.url));
const RECIPES = join(HERE, 'recipes');
const INDEX = join(HERE, 'recipes.json');
const WRITE = process.argv.includes('--write');

/** Where clients fetch the raw files from. */
const RAW_BASE =
	process.env.RECIPES_RAW_BASE ??
	'https://raw.githubusercontent.com/alexandrosnt/reach-recipes-registry/main/recipes';

const REPO = process.env.RECIPES_REPO ?? 'alexandrosnt/reach-recipes-registry';

/** Mirrors the Rust parser in src-tauri/src/recipe/schema.rs. */
function parseHeader(source, file) {
	if (!source.includes('@reach-recipe')) {
		throw new Error(`${file}: no @reach-recipe marker`);
	}
	const meta = { tags: [], targets: [], params: 0 };
	let inHeader = false;
	let ended = false;

	for (const raw of source.split(/\r?\n/)) {
		const line = raw.trim();
		if (!line.startsWith('#')) {
			if (inHeader && !ended) throw new Error(`${file}: header never closed with @end`);
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
			break;
		}
		const at = content.indexOf(':');
		if (at === -1) continue;
		const key = content.slice(0, at).trim().toLowerCase();
		const value = content.slice(at + 1).trim();

		if (key === 'tags' || key === 'targets') {
			meta[key] = value.split(',').map((s) => s.trim()).filter(Boolean);
		} else if (key === 'param') {
			meta.params += 1;
		} else {
			meta[key] = value;
		}
	}

	if (!ended) throw new Error(`${file}: header never closed with @end`);
	if (!meta.id) throw new Error(`${file}: missing id`);
	if (!meta.name) throw new Error(`${file}: missing name`);
	if (meta.id !== basename(file, '.sh')) {
		// The filename is what the URL is built from, so a mismatch would point
		// the index at a file that does not exist.
		throw new Error(`${file}: id "${meta.id}" does not match the filename`);
	}
	return meta;
}

const files = (await readdir(RECIPES)).filter((f) => f.endsWith('.sh')).sort();
const entries = [];
const problems = [];

for (const file of files) {
	const source = await readFile(join(RECIPES, file), 'utf-8');
	try {
		const meta = parseHeader(source, file);
		entries.push({
			id: meta.id,
			name: meta.name,
			version: meta.version ?? '',
			description: meta.description ?? '',
			author: meta.author ?? '',
			repo: REPO,
			tags: meta.tags,
			targets: meta.targets,
			danger: meta.danger ?? null,
			url: `${RAW_BASE}/${file}`,
			sha256: createHash('sha256').update(source, 'utf-8').digest('hex')
		});
	} catch (e) {
		problems.push(e.message);
	}
}

if (problems.length > 0) {
	console.error('Problems:\n  ' + problems.join('\n  '));
	process.exit(1);
}

const rendered = JSON.stringify(entries, null, '\t') + '\n';

if (WRITE) {
	await writeFile(INDEX, rendered, 'utf-8');
	console.log(`Wrote recipes.json with ${entries.length} recipe(s):`);
	for (const e of entries) console.log(`  ${e.id.padEnd(24)} ${e.danger ?? '-'}`);
} else {
	let current = '';
	try {
		current = await readFile(INDEX, 'utf-8');
	} catch {
		/* first run */
	}
	if (current === rendered) {
		console.log(`recipes.json is current (${entries.length} recipes).`);
	} else {
		console.error('recipes.json is out of date. Run: node build-index.mjs --write');
		process.exit(1);
	}
}
