/**
 * Tests the rules behind dragging a remote file out to the desktop.
 *
 * Run: node --import ./scripts/resolve-lib.mjs scripts/drag-out-test.mjs
 */

import {
	isDraggable,
	cacheKey,
	safeLocalName,
	dragAction,
	MAX_DRAG_BYTES
} from '$lib/explorer/drag-out';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);

const file = (over = {}) => ({ name: 'notes.txt', isDirectory: false, size: 1024, ...over });

section('what can be dragged');
check('an ordinary file can', isDraggable(file()) === true);
check('an empty file can', isDraggable(file({ size: 0 })) === true);
check('a file exactly at the limit can', isDraggable(file({ size: MAX_DRAG_BYTES })) === true);
check('a file past the limit cannot', isDraggable(file({ size: MAX_DRAG_BYTES + 1 })) === false);
// There is no single file to hand the OS, and fetching a whole tree behind a
// mouse gesture is not something to do quietly.
check('a directory cannot', isDraggable(file({ isDirectory: true })) === false);
check('a nonsense size cannot', isDraggable(file({ size: -1 })) === false);

section('the limit is a real one, not a placeholder');
check('it is large enough to be useful', MAX_DRAG_BYTES >= 8 * 1024 * 1024);
check('and small enough to fetch during a gesture', MAX_DRAG_BYTES <= 256 * 1024 * 1024);

section('each remote file gets its own place to land');
{
	const a = cacheKey('conn-1', '/srv/data/report.pdf');
	const b = cacheKey('conn-1', '/home/me/report.pdf');
	const c = cacheKey('conn-2', '/srv/data/report.pdf');
	check('same file, same key, so a second drag reuses the download', a === cacheKey('conn-1', '/srv/data/report.pdf'));
	check('same name in a different directory is a different key', a !== b);
	check('same path on a different connection is a different key', a !== c);
	check('keys are filesystem-safe', /^[0-9a-f]{8}$/.test(a), a);
}

section('the copy keeps the name the file actually had');
// Whatever lands on the desktop is named by this, so it has to survive.
check('an ordinary name is untouched', safeLocalName('report.pdf') === 'report.pdf');
check('spaces and unicode survive', safeLocalName('my réport (1).pdf') === 'my réport (1).pdf');
{
	// The exact spelling does not matter; not escaping the folder does.
	const escaped = safeLocalName('../../etc/passwd');
	check(
		'path separators cannot escape the folder',
		!escaped.includes('/') && !escaped.includes('\\') && !escaped.startsWith('.'),
		escaped
	);
}
check('a windows-illegal character is replaced', safeLocalName('a:b*c?.txt') === 'a_b_c_.txt');
check('a leading dot cannot make it hidden', safeLocalName('.bashrc').startsWith('_'));
check('an empty name still yields something', safeLocalName('') === 'file');
check('a very long name is trimmed', safeLocalName('x'.repeat(500)).length <= 200);

section('what a drag attempt should do');
check('a ready file starts the drag', dragAction('ready', file()) === 'start');
check('one still downloading waits', dragAction('fetching', file()) === 'wait');
check('one not started yet waits', dragAction('idle', file()) === 'wait');
// Being cached does not make an oversized file draggable, and being a
// directory is refused before anything else is considered.
check('an oversized file is refused even if ready', dragAction('ready', file({ size: MAX_DRAG_BYTES + 1 })) === 'too-large');
check('a directory is refused first', dragAction('ready', file({ isDirectory: true, size: 1 })) === 'not-a-file');
check('a failed download waits rather than starting', dragAction('failed', file()) === 'wait');

console.log(failures === 0 ? '\nALL PASS' : `\n${failures} FAILED`);
process.exit(failures === 0 ? 0 : 1);
