/**
 * When a paste into the terminal is worth stopping to look at.
 *
 * A multi-line paste is the one clipboard accident with real consequences: a
 * newline means the shell runs what precedes it the moment it arrives, so a
 * stray copy can execute on a production host before anyone has read it.
 * Every terminal worth trusting stops to ask — and every one of them also
 * lets you turn that off, because someone pasting three-line blocks all day
 * is being asked a question they have already answered.
 *
 * The rule lives here rather than inside the terminal component so it can be
 * tested without one, and so "how many lines is too many" has exactly one
 * definition shared by the check and the count shown in the dialog.
 */

/** The smallest threshold that still means anything. */
export const MIN_PASTE_THRESHOLD = 2;

/**
 * Warn from two lines up.
 *
 * This is deliberately the same trigger the terminal has always used — any
 * paste carrying a newline — so turning the setting on changes nothing for
 * anyone who never touches it. The threshold exists to make the warning
 * *rarer*, never to make it appear where it did not before.
 */
export const DEFAULT_PASTE_THRESHOLD = 2;

export interface PasteWarningPolicy {
	/** False turns the confirmation off entirely. */
	enabled: boolean;
	/** Warn once the paste reaches this many lines. */
	threshold: number;
}

/**
 * How many lines a paste carries.
 *
 * A trailing newline counts: `"ls\n"` is two, because the empty line after it
 * is precisely what makes the shell run the command on arrival rather than
 * leaving it on the prompt. That is the thing being warned about, so it has
 * to be counted.
 */
export function countPasteLines(text: string): number {
	if (text === '') return 0;
	return text.split(/\r\n|\r|\n/).length;
}

/**
 * The threshold actually in force.
 *
 * A stored 0 or 1 is read as the minimum rather than as "warn about
 * everything": at 1 a plain one-word paste would stop for confirmation, which
 * reads as a bug rather than as a setting. Turning the warning off is what
 * `enabled` is for.
 */
export function effectiveThreshold(threshold: number): number {
	if (!Number.isFinite(threshold)) return MIN_PASTE_THRESHOLD;
	return Math.max(MIN_PASTE_THRESHOLD, Math.floor(threshold));
}

/** Whether this paste should be confirmed before it reaches the shell. */
export function shouldWarnOnPaste(text: string, policy: PasteWarningPolicy): boolean {
	if (!policy.enabled) return false;
	return countPasteLines(text) >= effectiveThreshold(policy.threshold);
}
