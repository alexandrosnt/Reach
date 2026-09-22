/**
 * Tests the rule and the mechanism that put the input method's candidate
 * window back at the caret after the window has been dragged (issue #49).
 *
 * Node 24 strips TypeScript types on import, so the exact module the webview
 * runs is what is under test here.
 *
 * Run: node scripts/ime-test.mjs
 */

import { shouldRearmIme, nudgeCaret, REARM_SETTLE_MS, NUDGE_PX } from '../src/lib/terminal/ime.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);

/** A stand-in for xterm's hidden textarea that records every layout flush. */
function makeTarget(left = '230px', top = '0px') {
	const flushes = [];
	const target = {
		style: { left, top },
		get offsetHeight() {
			flushes.push({ left: target.style.left, top: target.style.top });
			return 22;
		}
	};
	return { target, flushes };
}

section('the terminal the user is actually typing in');
check(
	'a focused, idle terminal is re-armed',
	shouldRearmIme({ focused: true, composing: false }) === true
);

section('never touch a terminal the user is not in');
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
	// Moving the caret here would disturb the rectangle the input method is
	// drawing against while the user still has an unfinished character in it.
	shouldRearmIme({ focused: true, composing: true }) === false
);

section('the settle delay');
check('waits long enough to outlast a drag stream', REARM_SETTLE_MS >= 100);
check('but is not a visible pause', REARM_SETTLE_MS <= 500);

section('the nudge moves the caret, then puts it back');
{
	const { target, flushes } = makeTarget('230px', '0px');
	let deferred = null;
	const ran = nudgeCaret(target, (fn) => (deferred = fn));

	check('it reports that it ran', ran === true);
	check(
		'the caret is moved first',
		target.style.left === `${230 + NUDGE_PX}px`,
		`left was ${target.style.left}`
	);
	check('and nothing is restored until the frame arrives', deferred !== null);

	deferred();
	check('the caret ends up exactly where it started', target.style.left === '230px');
	check('and the vertical position is untouched', target.style.top === '0px');

	check(
		'layout is flushed on both halves, or the two writes would cancel out',
		flushes.length === 2,
		`flushed ${flushes.length} time(s)`
	);
	check(
		'the flushed positions differ, which is the whole point',
		flushes[0].left !== flushes[1].left,
		JSON.stringify(flushes)
	);
	check('the second flush sees the original position', flushes[1].left === '230px');
}

section('a fractional position survives the round trip');
{
	const { target } = makeTarget('230.5px', '11.25px');
	let deferred = null;
	nudgeCaret(target, (fn) => (deferred = fn));
	check('moved from the fractional origin', target.style.left === `${230.5 + NUDGE_PX}px`);
	deferred();
	check('restored verbatim, not rounded', target.style.left === '230.5px');
	check('top restored verbatim too', target.style.top === '11.25px');
}

section('decline rather than invent a position');
// xterm writes the inline position itself; before it has, the stylesheet
// parks the textarea far off-screen. Nudging from there would drag the caret
// somewhere the terminal never asked for.
for (const [name, left] of [
	['an unpositioned textarea', ''],
	['a position given in em', '-9999em'],
	['a keyword position', 'auto'],
	['nonsense', 'not-a-length']
]) {
	const { target, flushes } = makeTarget(left);
	let deferred = null;
	const ran = nudgeCaret(target, (fn) => (deferred = fn));
	check(`${name} is declined`, ran === false);
	check(`${name} is left exactly as it was`, target.style.left === left);
	check(`${name} schedules no restore`, deferred === null);
	check(`${name} forces no layout`, flushes.length === 0);
}

section('a zero position is a real position, not a missing one');
{
	const { target } = makeTarget('0px', '0px');
	let deferred = null;
	check('it runs', nudgeCaret(target, (fn) => (deferred = fn)) === true);
	check('moved off zero', target.style.left === `${NUDGE_PX}px`);
	deferred();
	check('and back to zero', target.style.left === '0px');
}

console.log(failures === 0 ? '\nALL PASS' : `\n${failures} FAILED`);
process.exit(failures === 0 ? 0 : 1);
