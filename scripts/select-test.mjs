/**
 * Tests how the theme-drawn select list moves between rows, in Node.
 *
 * Node 24 strips TypeScript types on import, so the exact module the webview
 * runs is what is under test here.
 *
 * Run: node scripts/select-test.mjs
 */

import { stepRow, findRow } from '../src/lib/utils/selectMenu.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);

const row = (label, index, disabled = false, group = false) => ({ label, index, disabled, group, inGroup: false });
const rows = [
	row('Off', 0),
	row('Printable text', 1),
	row('Group', -1, true, true),
	row('Disabled', 2, true),
	row('Everything', 3),
	row('Errors only', 4)
];

section('arrow keys');
check('down moves to the next row', stepRow(rows, 0, 1) === 1);
check('down skips a heading and a disabled row', stepRow(rows, 1, 1) === 4);
check('up skips them too', stepRow(rows, 4, -1) === 1);
check('down at the last row stays put', stepRow(rows, 5, 1) === 5);
check('up at the first row stays put', stepRow(rows, 0, -1) === 0);
check('Home lands on the first pickable row', stepRow(rows, -1, 1) === 0);
check('End lands on the last pickable row', stepRow(rows, rows.length, -1) === 5);
check('nothing pickable leaves it where it was', stepRow([row('G', -1, true, true)], -1, 1) === -1);

section('type to find');
check('a letter finds the row starting with it', findRow(rows, 'p', 0) === 1);
check('case does not matter', findRow(rows, 'P', 0) === 1);
check('the same letter again moves to the next match', findRow(rows, 'e', 4) === 5);
check('and wraps around', findRow(rows, 'e', 5) === 4);
check('a repeated letter cycles like one letter', findRow(rows, 'ee', 4) === 5);
check('a word matches from the current row', findRow(rows, 'err', 4) === 5);
check('disabled rows and headings are never found', findRow(rows, 'd', 0) === -1 && findRow(rows, 'g', 0) === -1);
check('no match gives -1', findRow(rows, 'z', 0) === -1);
check('non-Latin labels match', findRow([row('Ελληνικά', 0), row('中文', 1)], '中', 0) === 1);

console.log(failures ? `\n${failures} failed` : '\nall passed');
process.exit(failures ? 1 : 0);
