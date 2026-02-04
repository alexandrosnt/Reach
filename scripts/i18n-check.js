/**
 * Validates that all locale files have every key from en.json (the base locale).
 * Run: node scripts/i18n-check.js
 */

import { readdir, readFile } from 'fs/promises';
import { join, basename } from 'path';
import { fileURLToPath } from 'url';
import { dirname } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const LOCALES_DIR = join(__dirname, '..', 'src', 'lib', 'i18n', 'locales');

async function main() {
	const baseFile = join(LOCALES_DIR, 'en.json');
	const baseContent = JSON.parse(await readFile(baseFile, 'utf-8'));
	const baseKeys = Object.keys(baseContent).sort();

	console.log(`Base locale (en.json): ${baseKeys.length} keys\n`);

	const files = await readdir(LOCALES_DIR);
	const localeFiles = files.filter((f) => f.endsWith('.json') && f !== 'en.json');

	let hasErrors = false;

	for (const file of localeFiles) {
		const locale = basename(file, '.json');
		const content = JSON.parse(await readFile(join(LOCALES_DIR, file), 'utf-8'));
		const localeKeys = new Set(Object.keys(content));

		const missing = baseKeys.filter((k) => !localeKeys.has(k));
		const extra = Object.keys(content).filter((k) => !baseKeys.includes(k));

		if (missing.length === 0 && extra.length === 0) {
			console.log(`  ${locale}: OK (${localeKeys.size} keys)`);
		} else {
			hasErrors = true;
			console.log(`  ${locale}: ISSUES`);
			if (missing.length > 0) {
				console.log(`    Missing (${missing.length}):`);
				missing.forEach((k) => console.log(`      - ${k}`));
			}
			if (extra.length > 0) {
				console.log(`    Extra (${extra.length}):`);
				extra.forEach((k) => console.log(`      + ${k}`));
			}
		}
	}

	if (hasErrors) {
		console.log('\nSome locale files have issues.');
		process.exit(1);
	} else {
		console.log('\nAll locale files are valid.');
	}
}

main().catch((err) => {
	console.error('Error:', err.message);
	process.exit(1);
});
