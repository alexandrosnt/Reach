/**
 * The local CapsLock, NumLock and ScrollLock, as best the webview can tell,
 * so every remote desktop can mirror them the way mstsc and RDM do: the
 * remote follows the local keyboard on connect, on focus and on every lock
 * key, in every session.
 *
 * A page cannot ask for the lock state; only an input event carries it. Any
 * key or mouse event anywhere in the window is read, except a lock key's
 * own events: there WebKitGTK (and others) may still report the state from
 * before the toggle. A lock key press instead flips what is known; the next
 * mouse move or key corrects it if that guess was wrong.
 *
 * WebKit (WebKitGTK on Linux, and macOS) never reports NumLock at all: it
 * reads as off whatever the keyboard says. There NumLock comes from the
 * system (the keyboard LED, on Linux), from what a keypad key types (a digit
 * means on), and from NumLock presses; until any of those, it counts as on,
 * the usual state.
 */

import { isMac, isWebKit } from '#lib/platform.js';
import { rdpLocalNumlock } from '#lib/ipc/rdp.js';

export interface Locks {
	caps: boolean;
	num: boolean;
	scroll: boolean;
}

const LOCK_FIELD: Record<string, keyof Locks> = {
	CapsLock: 'caps',
	NumLock: 'num',
	ScrollLock: 'scroll',
};

/** Whether this key (by `KeyboardEvent.code`) is a lock key. */
export function isLockKey(code: string): boolean {
	return code in LOCK_FIELD;
}

let known: Locks | null = null;
const listeners = new Set<(locks: Locks) => void>();

/** Whether `getModifierState('NumLock')` means anything here. */
function numReadable(): boolean {
	return !isWebKit();
}

function read(e: KeyboardEvent | MouseEvent): Locks {
	return {
		caps: e.getModifierState('CapsLock'),
		// A Mac keyboard has no NumLock and its keypad always types digits.
		num: isMac() || (numReadable() ? e.getModifierState('NumLock') : (known?.num ?? true)),
		scroll: e.getModifierState('ScrollLock'),
	};
}

/** The keypad keys that type a digit with NumLock on and move without it. */
const KEYPAD = /^Numpad([0-9]|Decimal)$/;

/** What a keypad key says about NumLock: a digit (or the decimal sign) means
 *  on, a movement key off. Shift inverts the keypad on Windows; skipped. */
function numFromKeypad(e: KeyboardEvent): boolean | null {
	if (e.type !== 'keydown' || e.shiftKey || !KEYPAD.test(e.code)) return null;
	return /^[0-9.,]$/.test(e.key);
}

/** Ask the system for NumLock where the webview cannot read it. */
export async function refreshFromSystem(): Promise<void> {
	if (isMac() || numReadable()) return;
	let num: boolean | null = null;
	try {
		num = await rdpLocalNumlock();
	} catch {
		return;
	}
	if (num === null) return;
	set({ caps: false, scroll: false, ...known, num });
}

function same(a: Locks, b: Locks): boolean {
	return a.caps === b.caps && a.num === b.num && a.scroll === b.scroll;
}

function set(locks: Locks): void {
	if (known && same(known, locks)) return;
	known = locks;
	for (const fn of listeners) fn(locks);
}

function onInput(e: Event): void {
	if (!(e instanceof KeyboardEvent || e instanceof MouseEvent)) return;
	if (typeof e.getModifierState !== 'function') return;
	if (e instanceof KeyboardEvent) {
		const field = LOCK_FIELD[e.code];
		if (field) {
			onLockKey(e, field);
			return;
		}
		const num = isMac() ? null : numFromKeypad(e);
		if (num !== null) {
			set({ ...read(e), num });
			return;
		}
	}
	set(read(e));
}

function onLockKey(e: KeyboardEvent, field: keyof Locks): void {
	// macOS sends CapsLock's keydown when it turns on and its keyup when it
	// turns off, each with the new state already.
	if (isMac()) {
		// The Mac keypad's Clear key arrives as NumLock; NumLock stays on.
		if (field === 'num') return;
		set({ ...(known ?? read(e)), [field]: e.getModifierState(e.code) });
		return;
	}
	if (e.type !== 'keydown' || e.repeat) return;
	if (!known) {
		// Nothing read yet: this event is all there is.
		set(read(e));
		return;
	}
	set({ ...known, [field]: !known[field] });
	// The LED has the last word, once the system has switched it.
	if (field === 'num') setTimeout(() => void refreshFromSystem(), 80);
}

let installed = false;

function install(): void {
	if (installed || typeof window === 'undefined') return;
	installed = true;
	const opts = { capture: true, passive: true };
	for (const type of ['keydown', 'keyup', 'mousedown', 'mousemove', 'pointerdown']) {
		window.addEventListener(type, onInput, opts);
	}
	window.addEventListener('focus', () => void refreshFromSystem());
	void refreshFromSystem();
}

/** The local lock state, or null before any input has shown it. */
export function localLocks(): Locks | null {
	install();
	return known;
}

/** Call `fn` whenever the local lock state is learnt or changes. */
export function onLocksChange(fn: (locks: Locks) => void): () => void {
	install();
	listeners.add(fn);
	return () => listeners.delete(fn);
}
