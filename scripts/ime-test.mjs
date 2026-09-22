/**
 * Tests the rule that decides when the terminal re-arms the input method
 * after the window has been dragged (issue #49).
 *
 * Node 24 strips TypeScript types on import, so the exact module the webview
 * runs is what is under test here.
 *
 * Run: node scripts/ime-test.mjs
 */

import { shouldRearmIme, REARM_SETTLE_MS } from '../src/lib/terminal/ime.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);

section('the terminal the user is actually typing in');
check(
	'a focused, idle terminal is re-armed',
	shouldRearmIme({ focused: true, composing: false }) === true
);

section('never steal focus');
check(
	'a terminal that does not hold focus is left alone',
	shouldRearmIme({ focused: false, composing: false }) === false
);
check(
	'not even when it is composing',
	shouldRearmIme({ focused: false, composing: true }) === false
);

section('never interrupt a half-typed character');
check(
	'a composing terminal is left alone',
	// Blurring here would throw away the characters the IME is still holding,
	// which is worse than the misplaced candidate window being fixed.
	shouldRearmIme({ focused: true, composing: true }) === false
);

section('the settle delay');
check('waits long enough to outlast a drag stream', REARM_SETTLE_MS >= 100);
check('but is not a visible pause', REARM_SETTLE_MS <= 500);

console.log(failures === 0 ? '\nALL PASS' : `\n${failures} FAILED`);
process.exit(failures === 0 ? 0 : 1);
