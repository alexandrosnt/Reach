/**
 * Putting the input method's candidate window back where the caret is.
 *
 * Issue #49: a Simplified Chinese user found that typing worked normally until
 * they dragged the window by its title bar. After that the candidate list drew
 * itself at the top-left corner of the *screen*, outside the app, and clicking
 * back into the terminal did not help.
 *
 * The cause is not in this codebase. Dragging a window on Windows puts the
 * thread into the `WM_ENTERSIZEMOVE` modal loop, and for as long as that loop
 * runs WebView2's text-input manager cannot update the position it has given
 * the IME. When the loop ends the stale position is simply kept, so the IME
 * goes on drawing the candidate list where the caret used to be — or at the
 * screen origin when that position no longer makes sense. Only dragging
 * triggers it; resizing and startup both update the position correctly.
 *
 * Clicking back into the terminal does not fix it because the hidden textarea
 * never actually lost focus: the title bar is part of the same webview, so
 * there is no blur, and clicking an element that is already focused fires
 * nothing. The remedy is to force the focus cycle that did not happen —
 * blur, then focus again — which makes WebView2 recompute the caret position
 * from scratch. Setting the position directly through `ImmSetCandidateWindow`
 * is not an option: the webview overrides it.
 *
 * This module holds the decision of *whether* to do that, so the two ways of
 * getting it wrong are written down and tested rather than left to a reader.
 */

/**
 * How long to wait after the last movement before treating a drag as
 * finished.
 *
 * Moving a window emits a steady stream of events; re-arming on each one
 * would blur and refocus the terminal dozens of times while the mouse is
 * still down. Waiting for the stream to stop means it happens once, after the
 * user lets go.
 */
export const REARM_SETTLE_MS = 200;

export interface ImeRearmState {
	/** Whether this terminal's hidden textarea currently holds focus. */
	focused: boolean;
	/** Whether the user is part-way through composing a character. */
	composing: boolean;
}

/**
 * Whether a window move should be followed by a focus cycle on this
 * terminal's textarea.
 *
 * Two things must not happen. Re-arming a terminal that does not hold focus
 * would *take* focus — every open tab listens for the same move, so an
 * unguarded version would drag the caret to whichever terminal answered last.
 * Re-arming mid-composition would blur the textarea while the IME is holding
 * an unfinished character, throwing away what the user had typed — a worse
 * bug than the one being fixed.
 */
export function shouldRearmIme(state: ImeRearmState): boolean {
	return state.focused && !state.composing;
}
