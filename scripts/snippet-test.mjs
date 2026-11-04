/**
 * Tests what a snippet types into the terminal (issue #154), in Node, with
 * no terminal. Node strips TypeScript types on import, so the exact module
 * the webview runs is what is under test here.
 *
 * Run: node scripts/snippet-test.mjs
 */

import { snippetInput } from '../src/lib/terminal/snippet.ts';

let failures = 0;
const show = (s) => JSON.stringify(s);
const eq = (name, got, want) => {
	if (got === want) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}\n        got  ${show(got)}\n        want ${show(want)}`);
	}
};
const section = (s) => console.log(`\n${s}`);

const S = '\x1b[200~';
const E = '\x1b[201~';
const issue = 'apt update\napt list --upgradable\napt upgrade\nneedrestart -r l';
const sent = () => 'apt update\rapt list --upgradable\rapt upgrade\rneedrestart -r l';

section('a shell that takes a bracketed paste (#154)');
eq(
	'paste: the whole snippet in one paste, nothing runs until Enter',
	snippetInput(issue, { bracketed: true, run: false }),
	S + sent() + E
);
eq(
	'run: the same paste, then one Enter after it',
	snippetInput(issue, { bracketed: true, run: true }),
	S + sent() + E + '\r'
);
eq(
	'one line is a paste too',
	snippetInput('ls -la', { bracketed: true, run: true }),
	S + 'ls -la' + E + '\r'
);

section('a shell that does not');
eq(
	'paste: the lines as typed, the last waits for Enter',
	snippetInput(issue, { bracketed: false, run: false }),
	sent()
);
eq(
	'run: the lines, and Enter after the last',
	snippetInput(issue, { bracketed: false, run: true }),
	sent() + '\r'
);
eq('no markers typed in as text', snippetInput('ls', { bracketed: false, run: false }).includes('\x1b'), false);

section('line ends');
eq('LF is the Enter key, CR', snippetInput('a\nb', { bracketed: false, run: false }), 'a\rb');
eq('CRLF is one Enter, not two', snippetInput('a\r\nb', { bracketed: false, run: false }), 'a\rb');
eq('a lone CR stays one Enter', snippetInput('a\rb', { bracketed: false, run: false }), 'a\rb');
eq(
	'trailing line ends are dropped, so Run presses Enter once',
	snippetInput('ls\n\n', { bracketed: false, run: true }),
	'ls\r'
);
eq(
	'trailing CRLF too',
	snippetInput('ls\r\n', { bracketed: true, run: true }),
	S + 'ls' + E + '\r'
);
eq('a blank line inside is kept', snippetInput('a\n\nb', { bracketed: false, run: false }), 'a\r\rb');

section('nothing to type');
eq('empty', snippetInput('', { bracketed: true, run: true }), '');
eq('only line ends', snippetInput('\n\r\n', { bracketed: true, run: true }), '');

section('paste markers inside a snippet');
eq(
	'an end marker cannot end the paste early and run the rest',
	snippetInput('echo hi' + E + 'rm -rf ~\n', { bracketed: true, run: false }),
	S + 'echo hirm -rf ~' + E
);
eq(
	'a start marker is dropped as well',
	snippetInput(S + 'ls', { bracketed: true, run: false }),
	S + 'ls' + E
);
eq(
	'markers are not typed into a shell without bracketed paste either',
	snippetInput('a' + E + 'b', { bracketed: false, run: false }),
	'ab'
);

console.log(failures ? `\n${failures} failed` : '\nall passed');
process.exit(failures ? 1 : 0);
