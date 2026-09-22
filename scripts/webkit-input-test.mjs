/**
 * Tests the WebKit character-loss workaround (issue #47).
 *
 * The double below is not a sketch of xterm — the three handlers are
 * transcribed from @xterm/xterm 5.5.0's own minified source, so the first test
 * genuinely reproduces the dropped characters and the rest prove the adapter
 * stops them. If an xterm upgrade changes those handlers, re-transcribe them
 * here first; a green run against a stale replica would mean nothing.
 *
 * Node 24 strips TypeScript types on import, so the module under test is the
 * exact one the webview loads.
 *
 * Run: node scripts/webkit-input-test.mjs
 */

import { installWebkitInputFix } from '../src/lib/terminal/webkit-input-fix.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);

/**
 * xterm 5.5.0's real input handling, transcribed from lib/xterm.js.
 *
 *   _inputEvent: if (e.data && "insertText" === e.inputType &&
 *                    (!e.composed || !this._keyDownSeen) && !screenReaderMode)
 *   _keyDown:    this._keyDownHandled = false; this._keyDownSeen = true; ...
 *                returns false once the composition helper takes the key,
 *                which is what keyCode 229 does.
 *   _keyUp:      this._keyDownSeen = false; this._keyPressHandled = false;
 */
function makeTerminal({ screenReaderMode = false } = {}) {
	const emitted = [];
	const core = {
		_keyDownSeen: false,
		_keyPressHandled: false,
		_compositionHelper: { _isComposing: false, _isSendingComposition: false },
		_inputEvent(e) {
			if (
				e.data &&
				e.inputType === 'insertText' &&
				(!e.composed || !this._keyDownSeen) &&
				!term.options.screenReaderMode
			) {
				if (this._keyPressHandled) return false;
				emitted.push(e.data);
				return true;
			}
			return false;
		},
		_keyDown(e) {
			this._keyDownSeen = true;
			// CompositionHelper.keydown returns false for keyCode 229, and
			// _keyDown passes that straight out.
			if (e.keyCode === 229) return false;
			emitted.push(e.key);
			return true;
		},
		_keyUp() {
			this._keyDownSeen = false;
			this._keyPressHandled = false;
		}
	};

	const textarea = new EventTarget();
	const term = { _core: core, textarea, options: { screenReaderMode }, emitted };
	return term;
}

const webkit = () => true;
const chromium = () => false;

/** One keystroke as WebKit delivers it: text first, then keydown(229). */
function webkitKeystroke(term, ch, { keyup = false, keyCode = 229, mods = {} } = {}) {
	term._core._inputEvent({ data: ch, inputType: 'insertText', composed: true, isComposing: false });
	term._core._keyDown({ key: ch, keyCode, isComposing: false, ...mods });
	if (keyup) {
		term._core._keyUp({ key: ch });
		term.textarea.dispatchEvent(new Event('keyup'));
	}
}

const type = (term, text, opts) => [...text].forEach((c) => webkitKeystroke(term, c, opts));

// ---------------------------------------------------------------------------

section('the bug itself, against a replica of xterm 5.5.0');
{
	const term = makeTerminal();
	type(term, 'hello');
	check(
		'typing fast with no fix loses every character after the first',
		term.emitted.join('') === 'h',
		`emitted ${JSON.stringify(term.emitted.join(''))}, expected "h"`
	);

	const slow = makeTerminal();
	type(slow, 'hello', { keyup: true });
	check(
		'typing slowly was never broken, so the fix has something to match',
		slow.emitted.join('') === 'hello',
		`emitted ${JSON.stringify(slow.emitted.join(''))}`
	);
}

section('with the adapter installed');
{
	const term = makeTerminal();
	installWebkitInputFix(term, { isWebKitEngine: webkit });
	type(term, 'hello');
	check(
		'every character of fast typing arrives',
		term.emitted.join('') === 'hello',
		`emitted ${JSON.stringify(term.emitted.join(''))}`
	);

	const slow = makeTerminal();
	installWebkitInputFix(slow, { isWebKitEngine: webkit });
	type(slow, 'hello', { keyup: true });
	check(
		'slow typing still arrives exactly once, not twice',
		slow.emitted.join('') === 'hello',
		`emitted ${JSON.stringify(slow.emitted.join(''))}`
	);

	const long = makeTerminal();
	installWebkitInputFix(long, { isWebKitEngine: webkit });
	const burst = 'the quick brown fox jumps over the lazy dog 0123456789';
	type(long, burst);
	check('a long burst is neither dropped nor duplicated', long.emitted.join('') === burst);
}

section('engines that do not have the bug are left alone');
{
	const term = makeTerminal();
	const before = { input: term._core._inputEvent, keyDown: term._core._keyDown };
	const dispose = installWebkitInputFix(term, { isWebKitEngine: chromium });
	check(
		'on Chromium nothing is patched at all',
		term._core._inputEvent === before.input && term._core._keyDown === before.keyDown
	);
	check('and the returned disposer is safe to call', (dispose(), true));
}

section('never clear the flag on a key that is not accounted for');
// Each of these would mean clearing _keyDownSeen for a keydown whose character
// had not already been emitted, which is the gate doing its actual job.
{
	const real = makeTerminal();
	installWebkitInputFix(real, { isWebKitEngine: webkit });
	real._core._keyDown({ key: 'a', keyCode: 65, isComposing: false });
	check('a genuine keyboard keydown still marks the key as down', real._core._keyDownSeen === true);

	const mismatch = makeTerminal();
	installWebkitInputFix(mismatch, { isWebKitEngine: webkit });
	mismatch._core._inputEvent({ data: 's', inputType: 'insertText', composed: true, isComposing: false });
	mismatch._core._keyDown({ key: 't', keyCode: 229, isComposing: false });
	check(
		'a 229 keydown for a different character is not retired',
		mismatch._core._keyDownSeen === true
	);

	for (const mod of ['ctrlKey', 'metaKey', 'altKey']) {
		const t = makeTerminal();
		installWebkitInputFix(t, { isWebKitEngine: webkit });
		webkitKeystroke(t, 's', { mods: { [mod]: true } });
		check(`a ${mod} chord is not retired`, t._core._keyDownSeen === true);
	}

	const paste = makeTerminal();
	installWebkitInputFix(paste, { isWebKitEngine: webkit });
	paste._core._inputEvent({ data: 'many chars', inputType: 'insertText', composed: true, isComposing: false });
	paste._core._keyDown({ key: 'many chars', keyCode: 229, isComposing: false });
	check('multi-character insertion is not treated as one keystroke', paste._core._keyDownSeen === true);

	const reader = makeTerminal({ screenReaderMode: true });
	installWebkitInputFix(reader, { isWebKitEngine: webkit });
	webkitKeystroke(reader, 's');
	check('screen reader mode is left entirely to xterm', reader._core._keyDownSeen === true);
}

section('never interrupt an input method mid-character');
// Clearing the gate while a CJK candidate is being composed is how this fix
// would turn into a worse bug than the one it fixes.
{
	const term = makeTerminal();
	installWebkitInputFix(term, { isWebKitEngine: webkit });
	term._core._inputEvent({ data: 's', inputType: 'insertText', composed: true, isComposing: false });
	term._core._compositionHelper._isComposing = true;
	term._core._keyDown({ key: 's', keyCode: 229, isComposing: false });
	check('a composition starting mid-keystroke cancels the retire', term._core._keyDownSeen === true);

	const during = makeTerminal();
	installWebkitInputFix(during, { isWebKitEngine: webkit });
	during._core._compositionHelper._isComposing = true;
	webkitKeystroke(during, 's');
	check('text accepted during composition is never recorded', during._core._keyDownSeen === true);

	const sending = makeTerminal();
	installWebkitInputFix(sending, { isWebKitEngine: webkit });
	sending._core._inputEvent({ data: 's', inputType: 'insertText', composed: true, isComposing: false });
	sending._core._compositionHelper._isSendingComposition = true;
	sending._core._keyDown({ key: 's', keyCode: 229, isComposing: false });
	check('a composition being sent also cancels it', sending._core._keyDownSeen === true);

	const started = makeTerminal();
	installWebkitInputFix(started, { isWebKitEngine: webkit });
	started._core._inputEvent({ data: 's', inputType: 'insertText', composed: true, isComposing: false });
	started.textarea.dispatchEvent(new Event('compositionstart'));
	started._core._keyDown({ key: 's', keyCode: 229, isComposing: false });
	check('compositionstart on the textarea discards the pending character', started._core._keyDownSeen === true);
}

section('an on/off switch that works while installed');
{
	let on = false;
	const term = makeTerminal();
	installWebkitInputFix(term, { isWebKitEngine: webkit, enabled: () => on });
	type(term, 'hello');
	check('disabled behaves exactly like no fix', term.emitted.join('') === 'h');

	on = true;
	term._core._keyUp({});
	term.emitted.length = 0;
	type(term, 'hello');
	check('and re-enabling takes effect immediately', term.emitted.join('') === 'hello');
}

section('surviving an xterm upgrade');
// A rename upstream must degrade to today's bug, not to a crash on keystroke.
{
	for (const [name, mutate] of [
		['a renamed _inputEvent', (t) => delete t._core._inputEvent],
		['a renamed _keyDown', (t) => delete t._core._keyDown],
		['a renamed _keyDownSeen', (t) => delete t._core._keyDownSeen],
		['a missing _core', (t) => delete t._core],
		['a terminal with no textarea', (t) => (t.textarea = null)]
	]) {
		const term = makeTerminal();
		mutate(term);
		let warned = '';
		let threw = false;
		try {
			const dispose = installWebkitInputFix(term, {
				isWebKitEngine: webkit,
				warn: (m) => (warned = m)
			});
			dispose();
		} catch {
			threw = true;
		}
		check(`${name} warns instead of throwing`, !threw && warned.includes('WebKit input fix'), warned);
	}
}

section('disposal');
{
	const term = makeTerminal();
	const before = { input: term._core._inputEvent, keyDown: term._core._keyDown };
	const dispose = installWebkitInputFix(term, { isWebKitEngine: webkit });
	check('installing replaces the handlers', term._core._inputEvent !== before.input);
	dispose();
	check(
		'disposing puts xterm back exactly as it was',
		term._core._inputEvent === before.input && term._core._keyDown === before.keyDown
	);

	const wrapped = makeTerminal();
	const disposeWrapped = installWebkitInputFix(wrapped, { isWebKitEngine: webkit });
	const outer = wrapped._core._inputEvent;
	wrapped._core._inputEvent = function (e) {
		return outer.call(this, e);
	};
	const mine = wrapped._core._inputEvent;
	disposeWrapped();
	check('but does not tear off a patch someone else layered on top', wrapped._core._inputEvent === mine);
}

console.log(failures === 0 ? '\nALL PASS' : `\n${failures} FAILED`);
process.exit(failures === 0 ? 0 : 1);
