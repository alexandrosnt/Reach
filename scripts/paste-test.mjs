/**
 * Tests the terminal's paste-warning rule in Node, with no terminal.
 *
 * Node 24 strips TypeScript types on import, so the exact module the webview
 * runs is what is under test here.
 *
 * Run: node scripts/paste-test.mjs
 */

import {
	countPasteLines,
	effectiveThreshold,
	shouldWarnOnPaste,
	MIN_PASTE_THRESHOLD,
	DEFAULT_PASTE_THRESHOLD
} from '../src/lib/terminal/paste.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);

const on = { enabled: true, threshold: DEFAULT_PASTE_THRESHOLD };

section('counting lines');
check('empty text carries nothing', countPasteLines('') === 0);
check('one command is one line', countPasteLines('ls -la') === 1);
check(
	'a trailing newline counts, because it is what runs the command',
	countPasteLines('ls -la\n') === 2
);
check('two commands are two lines', countPasteLines('cd /tmp\nls') === 2);
check('CRLF counts the same as LF', countPasteLines('cd /tmp\r\nls') === 2);
check('a lone CR counts too', countPasteLines('cd /tmp\rls') === 2);
check('five lines', countPasteLines('a\nb\nc\nd\ne') === 5);

section('the default matches the behaviour the terminal always had');
check('a bare command is pasted straight in', shouldWarnOnPaste('ls -la', on) === false);
check('an empty clipboard warns about nothing', shouldWarnOnPaste('', on) === false);
check('a command with a trailing newline warns', shouldWarnOnPaste('rm -rf /tmp/x\n', on) === true);
check('two lines warn', shouldWarnOnPaste('cd /tmp\nls', on) === true);

section('turning it off');
const off = { enabled: false, threshold: DEFAULT_PASTE_THRESHOLD };
check('a hundred lines go straight in when disabled', shouldWarnOnPaste('x\n'.repeat(100), off) === false);
check('disabled beats any threshold', shouldWarnOnPaste('a\nb\nc', { enabled: false, threshold: 2 }) === false);

section('raising the threshold');
const atTen = { enabled: true, threshold: 10 };
check('nine lines pass', shouldWarnOnPaste('x\n'.repeat(8) + 'x', atTen) === false);
check('ten lines warn', shouldWarnOnPaste('x\n'.repeat(9) + 'x', atTen) === true);
check('the boundary is inclusive', countPasteLines('x\n'.repeat(9) + 'x') === 10);

section('a threshold that would be nonsense');
check('zero is read as the minimum', effectiveThreshold(0) === MIN_PASTE_THRESHOLD);
check('one is read as the minimum', effectiveThreshold(1) === MIN_PASTE_THRESHOLD);
check('a negative number is read as the minimum', effectiveThreshold(-5) === MIN_PASTE_THRESHOLD);
check('NaN is read as the minimum', effectiveThreshold(Number.NaN) === MIN_PASTE_THRESHOLD);
check('a fraction rounds down', effectiveThreshold(7.9) === 7);
check(
	'a one-word paste never stops, whatever is stored',
	shouldWarnOnPaste('ls', { enabled: true, threshold: 1 }) === false
);

console.log(failures === 0 ? '\nALL PASS' : `\n${failures} FAILED`);
process.exit(failures === 0 ? 0 : 1);
