/**
 * O(1) virtual scrolling math utilities.
 * Given a scroll position and viewport, calculates exactly which items are visible.
 */

export interface VirtualScrollState {
	/** Index of first visible item */
	startIndex: number;
	/** Index of last visible item (exclusive) */
	endIndex: number;
	/** Offset in px for the visible window */
	offsetY: number;
	/** Total height of all items in px */
	totalHeight: number;
	/** Number of visible items */
	visibleCount: number;
}

/**
 * Calculate the visible range of items given scroll position.
 * All operations are O(1) — division and multiplication only.
 *
 * @param scrollTop - Current scroll position in px
 * @param viewportHeight - Height of the visible area in px
 * @param itemHeight - Height of each item in px (uniform)
 * @param totalItems - Total number of items in the list
 * @param overscan - Number of extra items to render above/below viewport (default: 3)
 */
export function calculateVisibleRange(
	scrollTop: number,
	viewportHeight: number,
	itemHeight: number,
	totalItems: number,
	overscan: number = 3
): VirtualScrollState {
	if (totalItems === 0 || itemHeight === 0) {
		return {
			startIndex: 0,
			endIndex: 0,
			offsetY: 0,
			totalHeight: 0,
			visibleCount: 0
		};
	}

	const totalHeight = totalItems * itemHeight;
	const visibleCount = Math.ceil(viewportHeight / itemHeight);

	// O(1): single division to find start
	const rawStart = Math.floor(scrollTop / itemHeight);
	const startIndex = Math.max(0, rawStart - overscan);
	const endIndex = Math.min(totalItems, rawStart + visibleCount + overscan);

	const offsetY = startIndex * itemHeight;

	return {
		startIndex,
		endIndex,
		offsetY,
		totalHeight,
		visibleCount: endIndex - startIndex
	};
}
