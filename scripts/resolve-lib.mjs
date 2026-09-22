/**
 * Registers the `$lib` resolution hook. Use as:
 *
 *   node --import ./scripts/resolve-lib.mjs scripts/<name>-test.mjs
 */

import { register } from 'node:module';

register('./resolve-lib-hooks.mjs', import.meta.url);
