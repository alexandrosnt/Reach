/**
 * Teaches Node the `$lib` alias that Vite gives the app.
 *
 * Without this, a module can only be covered by the plain-Node test scripts if
 * it imports nothing — which silently pushes shared helpers into being copied
 * rather than imported. This maps `$lib/x` to `src/lib/x` the way SvelteKit
 * does, trying the bare path, then `.ts`, then an `index.ts` directory entry.
 *
 * Loaded through scripts/resolve-lib.mjs, which is what tests pass to
 * `node --import`. Resolution hooks run on their own thread, so the register
 * call has to live in a separate file from the hook itself.
 */

import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const ROOT = new URL('../', import.meta.url);

export function resolve(specifier, context, nextResolve) {
	if (specifier === '$lib' || specifier.startsWith('$lib/')) {
		const rest = specifier === '$lib' ? '' : specifier.slice('$lib/'.length);
		const base = new URL(`src/lib/${rest}`, ROOT).href;
		for (const candidate of [base, `${base}.ts`, `${base}/index.ts`]) {
			if (existsSync(fileURLToPath(candidate))) {
				return nextResolve(candidate, context);
			}
		}
	}
	return nextResolve(specifier, context);
}
