/**
 * Tests the counting that drives the archive progress bar.
 *
 * The sample lines below are the real shapes these tools print, not invented
 * ones, because the whole mechanism rests on matching them.
 *
 * Run: node --import ./scripts/resolve-lib.mjs scripts/archive-progress-test.mjs
 */

import {
	createProgressCounter,
	isMeasurable,
	percentOf
} from '$lib/explorer/archive-progress';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);

section('each tool is counted by what it actually prints');
{
	const cases = [
		['tar', 'data/\ndata/one.txt\ndata/two.txt\n', 3],
		['bsdtar', 'a\nb\n', 2],
		['zip', '  adding: a.txt (deflated 12%)\n  adding: b.txt (stored 0%)\n', 2],
		['unzip', '  inflating: a.txt  \n   creating: dir/\n extracting: c.bin\n', 3],
		['7z', '+ a.txt\n+ dir/b.txt\n', 2]
	];
	for (const [tool, output, expected] of cases) {
		const c = createProgressCounter(tool);
		check(`${tool} counts ${expected}`, c.push(output) === expected, `got ${c.count()}`);
	}
}

section('noise is not mistaken for a file');
{
	const zip = createProgressCounter('zip');
	zip.push('\tzip warning: name not matched\n  adding: real.txt (stored 0%)\n\n');
	check('zip ignores its warnings and blank lines', zip.count() === 1, `got ${zip.count()}`);

	const unzip = createProgressCounter('unzip');
	unzip.push('Archive:  thing.zip\n  inflating: real.txt\n');
	check('unzip ignores its header', unzip.count() === 1, `got ${unzip.count()}`);
}

section('output split mid-line must not be lost or double-counted');
{
	const c = createProgressCounter('tar');
	// One filename arriving in three pieces, with no newline until the end.
	c.push('data/lo');
	check('a partial line counts for nothing yet', c.count() === 0);
	c.push('ng-na');
	check('still nothing', c.count() === 0);
	check('and counts exactly once when it completes', c.push('me.txt\n') === 1);

	const d = createProgressCounter('tar');
	d.push('a\nb\nc');
	check('a trailing fragment is held back', d.count() === 2);
	d.push('\n');
	check('and released by its newline', d.count() === 3);
}

section('carriage returns from a remote shell do not break matching');
{
	const c = createProgressCounter('unzip');
	c.push('  inflating: a.txt\r\n  inflating: b.txt\r\n');
	check('CRLF is handled', c.count() === 2, `got ${c.count()}`);
}

section('a tool that says nothing is honest about it');
{
	check('python3 is not measurable', isMeasurable('python3') === false);
	check('an unknown tool is not measurable', isMeasurable('something-new') === false);
	check('tar is', isMeasurable('tar') === true);
	const c = createProgressCounter('python3');
	check('and counting it yields nothing rather than noise', c.push('lots\nof\nlines\n') === 0);
}

section('the percentage never lies');
{
	check('no total means no percentage', percentOf(5, 0) === null);
	check('a negative total means no percentage', percentOf(5, -1) === null);
	check('half is half', percentOf(50, 100) === 50);
	check('zero done is zero', percentOf(0, 100) === 0);
	// A tool naming the last file has not finished writing it.
	check('it never reaches 100 while running', percentOf(100, 100) === 99);
	check('and cannot exceed that even if the count overruns', percentOf(500, 100) === 99);
}

console.log(failures === 0 ? '\nALL PASS' : `\n${failures} FAILED`);
process.exit(failures === 0 ? 0 : 1);
