/**
 * Two tabs side by side (issue #106): the rules, kept apart from the state so
 * they can be tested as they run.
 *
 * A split shows two tabs, `first` (left or top) and `second` (right or
 * bottom). The active tab is the one with focus, always one of the two while
 * a split is open. Choosing another tab in the bar puts it in the focused
 * pane, so the other pane stays where it was; closing either of the two ends
 * the split and leaves the other.
 */

export type Direction = 'right' | 'down';

export interface Split {
	first: string;
	second: string;
	direction: Direction;
	/** The first pane's share of the room, 0..1. */
	ratio: number;
}

/** The divider stays where both panes are still usable. */
export const MIN_RATIO = 0.2;
export const MAX_RATIO = 0.8;

export function clampRatio(r: number): number {
	if (!Number.isFinite(r)) return 0.5;
	return Math.min(MAX_RATIO, Math.max(MIN_RATIO, r));
}

/** Put `tabId` beside the focused tab. Nothing to split with itself. */
export function open(focusedId: string | undefined, tabId: string, direction: Direction, ratio = 0.5): Split | null {
	if (!focusedId || focusedId === tabId) return null;
	return { first: focusedId, second: tabId, direction, ratio: clampRatio(ratio) };
}

/** One of the two panes gets focus; any other tab replaces the focused pane. */
export function activate(split: Split | null, focusedId: string | undefined, tabId: string): Split | null {
	if (!split || tabId === split.first || tabId === split.second) return split;
	if (focusedId === split.first) return { ...split, first: tabId };
	if (focusedId === split.second) return { ...split, second: tabId };
	return split;
}

/** A tab closing: the split ends if it was one of the two, and the other keeps focus. */
export function close(split: Split | null, tabId: string): { split: Split | null; focus?: string } {
	if (split && tabId === split.first) return { split: null, focus: split.second };
	if (split && tabId === split.second) return { split: null, focus: split.first };
	return { split };
}

export function paneOf(split: Split | null, tabId: string): 'first' | 'second' | null {
	if (!split) return null;
	if (tabId === split.first) return 'first';
	if (tabId === split.second) return 'second';
	return null;
}

/** Half the divider's thickness, in pixels, kept clear on each side of it. */
const HALF_GAP = 3;

/** Where a pane sits inside the terminal area, as inline CSS. */
export function paneStyle(split: Split, pane: 'first' | 'second'): string {
	const at = `${(split.ratio * 100).toFixed(3)}%`;
	if (split.direction === 'right') {
		return pane === 'first'
			? `left:0;top:0;bottom:0;right:auto;width:calc(${at} - ${HALF_GAP}px)`
			: `top:0;bottom:0;right:0;left:calc(${at} + ${HALF_GAP}px)`;
	}
	return pane === 'first'
		? `left:0;right:0;top:0;bottom:auto;height:calc(${at} - ${HALF_GAP}px)`
		: `left:0;right:0;bottom:0;top:calc(${at} + ${HALF_GAP}px)`;
}

/** The divider's place, as inline CSS. */
export function dividerStyle(split: Split): string {
	const at = `${(split.ratio * 100).toFixed(3)}%`;
	return split.direction === 'right'
		? `top:0;bottom:0;left:calc(${at} - ${HALF_GAP}px);width:${HALF_GAP * 2}px`
		: `left:0;right:0;top:calc(${at} - ${HALF_GAP}px);height:${HALF_GAP * 2}px`;
}

/**
 * Where a tab dragged over the terminal area would go: the right third opens
 * it on the right, the bottom third below, anywhere else nowhere.
 */
export function dropZone(x: number, y: number, width: number, height: number): Direction | null {
	if (width <= 0 || height <= 0) return null;
	const fx = x / width;
	const fy = y / height;
	const right = fx >= 2 / 3;
	const down = fy >= 2 / 3;
	if (right && down) return fx - 2 / 3 >= fy - 2 / 3 ? 'right' : 'down';
	if (right) return 'right';
	if (down) return 'down';
	return null;
}

/** The tab a keyboard split brings in: the next one, or the one before the last. */
export function neighbour(ids: string[], focusedId: string | undefined): string | null {
	const i = focusedId ? ids.indexOf(focusedId) : -1;
	if (i < 0 || ids.length < 2) return null;
	return ids[i + 1] ?? ids[i - 1] ?? null;
}
