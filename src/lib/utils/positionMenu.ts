/**
 * Svelte action: place a popup/context menu at viewport coordinates, flipping
 * and clamping so it never spills off-screen.
 *
 * The menu opens downward/rightward from the click by default, but flips
 * upward if it would overflow the bottom edge and leftward if it would overflow
 * the right edge, then clamps to the viewport with a small margin. The target
 * element must be `position: fixed` (coordinates are viewport-relative, i.e.
 * `clientX`/`clientY`).
 *
 * Usage:
 *   <div class="context-menu" use:positionMenu={{ x: menu.x, y: menu.y }}>…</div>
 */
export interface MenuPos {
	x: number;
	y: number;
}

const MARGIN = 8;

export function positionMenu(node: HTMLElement, pos: MenuPos) {
	function place({ x, y }: MenuPos) {
		const w = node.offsetWidth;
		const h = node.offsetHeight;
		const vw = window.innerWidth;
		const vh = window.innerHeight;

		// Horizontal: flip to the left of the cursor if opening right overflows,
		// then clamp inside the viewport.
		let left = x + w + MARGIN > vw ? x - w : x;
		left = Math.min(Math.max(MARGIN, left), Math.max(MARGIN, vw - w - MARGIN));

		// Vertical: flip above the cursor if opening down overflows, then clamp.
		let top = y + h + MARGIN > vh ? y - h : y;
		top = Math.min(Math.max(MARGIN, top), Math.max(MARGIN, vh - h - MARGIN));

		node.style.left = `${left}px`;
		node.style.top = `${top}px`;
	}

	let current = pos;
	place(current);

	// A menu can change height after it opens — a group that expands, an item
	// that appears once something has loaded. Placement is computed from the
	// height, so without re-running it the extra rows are laid out below the
	// bottom of the screen: the menu looks like it did nothing at all.
	const observer =
		typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(() => place(current));
	observer?.observe(node);

	return {
		update(next: MenuPos) {
			current = next;
			place(current);
		},
		destroy() {
			observer?.disconnect();
		}
	};
}
