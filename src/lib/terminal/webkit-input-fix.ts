/**
 * Stopping WebKit and xterm.js between them from eating typed characters.
 *
 * Issue #47. Typing quickly into the terminal on macOS loses characters
 * outright — not late, gone. Text fields elsewhere in the app are fine, and
 * the same xterm build in a Chromium webview is fine. It reproduces in a bare
 * xterm page in Safari with no SSH, no addons and no Reach code, which puts it
 * squarely between the engine and the library.
 *
 * WebKit delivers a printable keystroke in an order Chromium does not:
 *
 *     beforeinput  data="s"  inputType="insertText"
 *     input        data="s"  composed=true
 *     keydown      key="s"   keyCode=229
 *
 * The keydown arrives *after* the text, and carries 229 — the value an engine
 * uses to say "an input method dealt with this key", so no character comes out
 * of the keyboard path. xterm's two handlers then work against each other:
 *
 *     _inputEvent  accepts text only while  (!ev.composed || !this._keyDownSeen)
 *     _keyDown     sets  this._keyDownSeen = true  on every key
 *     _keyUp       sets  this._keyDownSeen = false
 *
 * Type slowly and each keyup lands before the next keypress, so the flag is
 * clear in time and every character gets through. Type quickly and keys
 * overlap: the second character's `input` arrives while the first key is still
 * held, `composed` is true, `_keyDownSeen` is true, the guard fails, and
 * `_inputEvent` returns false having emitted nothing. No error, no warning.
 * One capture from a live session had 31 text insertions produce 17 `onData`
 * events.
 *
 * The keydown that sets the flag is the one that emitted nothing. So this
 * clears `_keyDownSeen` again, but only for a keydown that has been positively
 * identified as the trailing half of text already accepted: keyCode 229, the
 * same single character `_inputEvent` just took, no composition in progress,
 * no modifiers. Nothing is injected and nothing is sent twice — the character
 * went out through xterm's own unmodified handler.
 *
 * A bare modifier keydown is the same fault one step earlier, and it is worse
 * while it lasts. Holding Shift sets the flag before the `input` event
 * carrying the shifted character is delivered, so that character is rejected
 * and lost every single time, not just when typing quickly. On macOS this
 * made `:` impossible to type, and with it every shifted symbol, so vim could
 * not be put into command mode. The flag is therefore cleared after a
 * modifier-only keydown too. Both cases are one rule: a keydown that emitted
 * nothing must not gate the text that follows it.
 *
 * Credit for the diagnosis, the event traces and the shape of this adapter
 * goes to steve3d, who found it, tested it on the affected machine, and then
 * found the modifier case as well.
 *
 * Letting text through is only half the job. On a 229 keydown xterm also runs
 * a rescue, `_handleAnyTextareaChanges`, which snapshots the textarea and a
 * macrotask later emits whatever grew — the fallback that made this bug
 * intermittent rather than total. With the gate cleared, a character can be
 * taken by `_inputEvent` *and* still land inside that pending comparison, and
 * then it goes out twice. That is upstream xterm.js #6045, and it is not
 * theoretical: unguarded, `npm run webkit:live` turns "ab" into "abb" in real
 * WebKit. So the rescue is suppressed for exactly the keydowns whose text was
 * already emitted, and left alone everywhere else, because recovering text
 * nobody accepted is the job it exists to do.
 *
 * Upstream: xterm.js #5374 and #5887 (this gate dropping characters), #6045
 * (the rescue above) and PR #6054, which proposes not arming the flag for
 * modifier-only keydowns — the shape used here. All open when checked, and
 * `_keyDownSeen` is byte-identical in 6.0.0 and on master, so upgrading alone
 * is not a fix.
 *
 * This reaches into xterm's private API. It verifies every internal it needs
 * before touching anything and installs nothing, with a warning, if the shape
 * changes — so an xterm upgrade degrades to today's bug rather than a crash or
 * a doubled character. `npm run webkit:test` checks the logic anywhere;
 * `npm run webkit:live` checks it against real xterm in real WebKit on a Mac,
 * which is what CI runs and what should be believed over the other.
 */

import { isWebKit } from '$lib/platform';

/**
 * Keys that are only a modifier. Pressing one produces no character, ever.
 *
 * xterm sets `_keyDownSeen` on these exactly as it does on a real key, so
 * merely holding Shift blocks the `input` event that follows it — which on
 * WebKit is the only thing that carries the text. That is why `:` could not
 * be typed on macOS, and with it every other shifted symbol, so vim could not
 * be put into command mode. Same root cause as the 229 keydown below: a
 * keydown that emits nothing has no business gating text that follows.
 */
const BARE_MODIFIERS = new Set(['Shift', 'Control', 'Alt', 'Meta', 'CapsLock', 'AltGraph']);

/** The parts of xterm's internals this adapter needs. */
interface InputCore {
	_keyDownSeen: boolean;
	_inputEvent(event: InputEvent): boolean;
	_keyDown(event: KeyboardEvent): boolean | undefined;
	_compositionHelper?: {
		_isComposing: boolean;
		_isSendingComposition: boolean;
		_handleAnyTextareaChanges?: () => void;
	};
}

/** Just enough of xterm's Terminal to install against, so tests need no DOM. */
interface TerminalLike {
	textarea?: HTMLTextAreaElement | null;
	options: { screenReaderMode?: boolean };
}

/**
 * Escape hatch, for when a shim built on another library's private API goes
 * wrong on a machine nobody here can reproduce.
 *
 * Setting `reach.webkitInputFix` to `off` in local storage and reloading
 * leaves xterm completely untouched, which restores the stock behaviour
 * including its bugs. There is deliberately no setting in the interface:
 * nobody should be choosing this, it exists so that a bad interaction can be
 * confirmed or ruled out in one step instead of by downgrading.
 */
export const DISABLE_KEY = 'reach.webkitInputFix';

function enabledByDefault(): boolean {
	try {
		return globalThis.localStorage?.getItem(DISABLE_KEY) !== 'off';
	} catch {
		// Storage can throw outright when site data is blocked. The fix is
		// worth more than the opt-out, so failure means on.
		return true;
	}
}

export interface WebkitInputFixOptions {
	/** Lets the fix be switched off at runtime without uninstalling it. */
	enabled?: () => boolean;
	/** Which engine we are on. Injectable so tests can drive both sides. */
	isWebKitEngine?: () => boolean;
	/** Called when a keydown is retired; for tracing a live session. */
	log?: (stage: string, detail: unknown) => void;
	/** Reporting channel for an unusable xterm; injectable for tests. */
	warn?: (message: string) => void;
}

/**
 * Install the adapter on one terminal, after `term.open()` so the textarea
 * exists. Returns the function that removes it again.
 *
 * On Chromium this is a no-op: the event order that causes the bug does not
 * happen there, and the terminal is left exactly as it was.
 */
export function installWebkitInputFix(
	term: TerminalLike,
	{
		enabled = enabledByDefault,
		isWebKitEngine = isWebKit,
		log = () => {},
		warn = (m) => console.warn(m)
	}: WebkitInputFixOptions = {}
): () => void {
	const noop = () => {};

	if (!isWebKitEngine()) return noop;

	const core = (term as unknown as { _core?: InputCore })._core;
	const textarea = term.textarea;

	// An xterm upgrade that renames or reshapes any of this must not take the
	// terminal down with it. Leaving the original behaviour in place is a far
	// better failure than throwing on every keystroke.
	if (
		!core ||
		!textarea ||
		typeof core._inputEvent !== 'function' ||
		typeof core._keyDown !== 'function' ||
		typeof core._keyDownSeen !== 'boolean'
	) {
		warn('[Terminal] WebKit input fix not installed: unrecognised xterm internals');
		return noop;
	}

	// The deferred textarea diff has to be reachable, because letting a
	// character through `_inputEvent` without also suppressing this would send
	// it twice (upstream xterm.js #6045, reproduced in `npm run webkit:live`).
	// Without it the adapter would trade dropped characters for doubled ones,
	// so if it is gone, install nothing.
	const helper = core._compositionHelper;
	const originalHandleChanges = helper?._handleAnyTextareaChanges;
	if (!helper || typeof originalHandleChanges !== 'function') {
		warn('[Terminal] WebKit input fix not installed: cannot reach the deferred textarea diff');
		return noop;
	}
	// Narrowed once here; the closure below is called long after the guard.
	const runRescue: () => void = originalHandleChanges;

	const originalInput = core._inputEvent;
	const originalKeyDown = core._keyDown;

	/**
	 * Set for the duration of a keydown whose character `_inputEvent` already
	 * emitted, so xterm's own fallback does not emit it a second time.
	 *
	 * The fallback exists to rescue text the gate rejected. When the gate did
	 * not reject anything there is nothing to rescue, and running it anyway is
	 * what produces the duplicate. It is only ever set across the synchronous
	 * call into the original handler, so no keystroke can leave it stuck on.
	 */
	let alreadyEmitted = false;

	/**
	 * The single character `_inputEvent` accepted most recently, waiting for
	 * its trailing keydown to be identified. Null whenever there is no such
	 * character outstanding.
	 */
	let acceptedText: string | null = null;

	const composing = (): boolean =>
		Boolean(core._compositionHelper?._isComposing || core._compositionHelper?._isSendingComposition);

	const clear = (): void => {
		acceptedText = null;
	};

	function patchedInput(this: InputCore, event: InputEvent): boolean {
		// Any new input event ends whatever the previous one was waiting for.
		clear();

		const result = originalInput.call(this, event);

		// Only remember text xterm itself accepted and emitted. A rejected
		// event, a paste (more than one character), or anything an input
		// method is in the middle of, is not ours to account for.
		if (
			enabled() &&
			result === true &&
			event.inputType === 'insertText' &&
			!event.isComposing &&
			!composing() &&
			event.data?.length === 1
		) {
			acceptedText = event.data;
		}

		return result;
	}

	function patchedKeyDown(this: InputCore, event: KeyboardEvent): boolean | undefined {
		// Decide before calling through, because the original resets state.
		//
		// `composed` on an input event and `isComposing` are different things:
		// composed=true only means the event crosses shadow DOM boundaries, not
		// that an input method is active. The composition checks here are the
		// ones that actually protect a half-typed CJK character.
		const retire =
			enabled() &&
			acceptedText !== null &&
			event.key === acceptedText &&
			event.keyCode === 229 &&
			!event.isComposing &&
			!composing() &&
			!event.ctrlKey &&
			!event.metaKey &&
			!event.altKey &&
			!term.options.screenReaderMode;

		// Captured before the original runs, because the first thing it does is
		// arm the flag unconditionally.
		const isBareModifier = enabled() && BARE_MODIFIERS.has(event.key);
		const seenBeforeKeyDown = this._keyDownSeen;

		clear();

		// Suppress xterm's rescue only across this one synchronous call, and
		// only when there is nothing left to rescue.
		alreadyEmitted = retire;
		let result: boolean | undefined;
		try {
			result = originalKeyDown.call(this, event);
		} finally {
			alreadyEmitted = false;
		}

		// `false` is what xterm returns once its composition helper has taken
		// the key, which is the 229 path and the only one worth retiring. A
		// composition that started inside the original handler cancels it.
		if (retire && result === false && !composing()) {
			this._keyDownSeen = false;
			log('retired-keydown', { key: event.key, keyCode: event.keyCode });
			return result;
		}

		// A modifier held down must not block the text that follows it.
		//
		// This restores what the flag was rather than forcing it false, which
		// is what upstream's own proposed patch does (xterm.js #6054, "Don't
		// set _keyDownSeen for pure modifier keydowns", still unmerged). The
		// difference matters: if some other key really is down, the gate is
		// still doing its job and must keep doing it. All this removes is the
		// arming that a modifier had no business doing.
		//
		// No return value is checked here, unlike above: xterm returns *true*
		// for a bare modifier, having emitted nothing, because the key reaches
		// `evaluateKeyboardEvent` and yields no key to send. Nothing was
		// emitted, so letting the following `input` event through cannot emit
		// anything twice — and on WebKit that event is the only path a shifted
		// character has. Chords that do emit from the keydown, Ctrl+C and the
		// like, are untouched: they carry a real keyCode, which re-arms the
		// flag on its own keydown, and they fire no `input` event anyway.
		if (isBareModifier && !composing()) {
			this._keyDownSeen = seenBeforeKeyDown;
			log('unarmed-modifier-keydown', { key: event.key, restoredTo: seenBeforeKeyDown });
		}

		return result;
	}

	function patchedHandleChanges(this: unknown): void {
		if (alreadyEmitted) return;
		runRescue.call(this);
	}

	core._inputEvent = patchedInput;
	core._keyDown = patchedKeyDown;
	helper._handleAnyTextareaChanges = patchedHandleChanges;

	// Anything that ends the keystroke, or hands control to an input method,
	// invalidates the pending character.
	const resetEvents = ['keyup', 'compositionstart', 'compositionend', 'blur'] as const;
	for (const type of resetEvents) {
		textarea.addEventListener(type, clear, true);
	}

	return () => {
		// Only unwind our own patch. Something else may have wrapped it since.
		if (core._inputEvent === patchedInput) core._inputEvent = originalInput;
		if (core._keyDown === patchedKeyDown) core._keyDown = originalKeyDown;
		if (helper._handleAnyTextareaChanges === patchedHandleChanges) {
			helper._handleAnyTextareaChanges = originalHandleChanges;
		}
		for (const type of resetEvents) {
			textarea.removeEventListener(type, clear, true);
		}
		clear();
	};
}
