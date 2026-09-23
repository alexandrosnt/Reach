/**
 * Putting the input method's candidate window back where the caret is.
 *
 * Issue #49: typing Chinese works normally until the window is dragged by its
 * title bar. After that the candidate list draws itself at the top-left corner
 * of the *screen*, outside the app, and clicking back into the terminal does
 * not help. Reported on Windows 11.
 *
 * What this is not, established by measuring the running app rather than by
 * reasoning about it:
 *
 *   - It is not a stale window origin. The webview tracks a title-bar drag
 *     exactly; `window.screenX` followed every move to the pixel. wry already
 *     calls `NotifyParentWindowPositionChanged` on `WM_MOVE`, so the webview
 *     is told, and it listens.
 *   - It is not the hidden textarea being left somewhere wrong. Its inline
 *     position, its client rect and its computed screen position were all
 *     still correct after the drag.
 *   - It is not focus. The textarea keeps focus throughout, and cycling focus
 *     changes nothing. An earlier version of this file blurred and refocused
 *     the textarea; that shipped in v0.6.0, was confirmed to run, and did not
 *     fix the bug.
 *
 * What is left is the one thing a drag does not do: change the layout. A
 * resize also enters the same Windows modal loop, and a resize is fine —
 * because it changes the window's size, so the page re-lays-out, the caret's
 * rectangle changes, and a fresh position is pushed down to the input method.
 * A drag changes no layout at all. The caret rectangle is identical before and
 * after, nothing is pushed, and the input method goes on using a position it
 * cached in *screen* coordinates before the window moved. That is exactly the
 * offset seen on screen, and it explains why blurring and refocusing did
 * nothing: neither changes the caret's rectangle either.
 *
 * That reasoning was wrong too, and the reporter found what actually fixes
 * it: click another window, then click back into Reach. That is not a DOM
 * focus change and not a caret change — it is the *native window* losing and
 * regaining focus. wry handles `WM_SETFOCUS` by calling `MoveFocus` on the
 * WebView2 controller, and the controller is what holds the stale position.
 * A title-bar drag never produces `WM_SETFOCUS`; wry calls `MoveFocus` when
 * a drag starts and not when it ends.
 *
 * So the terminal now asks the backend to do that deliberately once the drag
 * settles, which is `webview_refocus`. The caret nudge below is kept because
 * it costs a frame and may still help where the rectangle itself went stale,
 * but it is no longer the mechanism being relied on.
 *
 * The input method itself lives in the WebView2 browser process, which is a
 * different process from this app, reached through neither the page nor Tauri.
 * Nudging what it reads is the lever available from here.
 */

/**
 * How long to wait after the last movement before treating a drag as
 * finished.
 *
 * Moving a window emits a steady stream of events; acting on each one would
 * repeat the work dozens of times while the mouse is still down. Waiting for
 * the stream to stop means it happens once, after the user lets go.
 */
export const REARM_SETTLE_MS = 200;

/** How far to move the caret before putting it back. One pixel is enough. */
export const NUDGE_PX = 1;

export interface ImeRearmState {
	/** Whether this terminal's hidden textarea currently holds focus. */
	focused: boolean;
	/** Whether the user is part-way through composing a character. */
	composing: boolean;
}

/**
 * Whether a window move should be followed by re-establishing the caret
 * position on this terminal.
 *
 * Two things must not happen. Touching a terminal that does not hold focus
 * would move a caret the user is not using — every open tab hears the same
 * move event, so an unguarded version would act on all of them. Touching one
 * mid-composition would disturb the rectangle the input method is actively
 * drawing against, while the user still has an unfinished character in it.
 */
export function shouldRearmIme(state: ImeRearmState): boolean {
	return state.focused && !state.composing;
}

/** The subset of a textarea this needs, so tests need no DOM. */
export interface NudgeTarget {
	style: { left: string; top: string };
	/** Read to force layout to be flushed; the value is discarded. */
	readonly offsetHeight: number;
}

/**
 * Move the caret by a pixel and put it back, so its rectangle is recomputed
 * and a fresh position reaches the input method.
 *
 * Both halves force layout, because two writes inside one task would coalesce
 * into no change at all and the whole point is that a change is observed. The
 * restore is deferred by `schedule` so the moved position survives a frame.
 *
 * Returns whether it ran. It declines unless xterm has already written a
 * pixel position, since inventing one would put the caret somewhere the
 * terminal did not ask for. The unit has to be checked rather than just
 * parsed: xterm's stylesheet parks the textarea at `-9999em` until the first
 * sync, and `parseFloat` reads that as a perfectly finite -9999, which would
 * be written back as `-9998px` — a different place entirely.
 */
const PIXELS = /^-?\d+(\.\d+)?px$/;

export function nudgeCaret(
	target: NudgeTarget,
	schedule: (fn: () => void) => void = (fn) => requestAnimationFrame(fn)
): boolean {
	const left = target.style.left;
	const top = target.style.top;
	if (!PIXELS.test(left.trim())) return false;
	const px = Number.parseFloat(left);
	if (!Number.isFinite(px)) return false;

	target.style.left = `${px + NUDGE_PX}px`;
	void target.offsetHeight;

	schedule(() => {
		target.style.left = left;
		target.style.top = top;
		void target.offsetHeight;
	});

	return true;
}
