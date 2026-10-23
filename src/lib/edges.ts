/**
 * Where Android's status bar, navigation bar and camera cutout are, as the
 * CSS variables --edge-top/right/bottom/left that --safe-* (app.css) use.
 *
 * The WebView's own env(safe-area-inset-*) covers the cutout only, so Reach
 * was drawn under the clock and the gesture bar, worst on a phone on its
 * side. Reach's Android activity knows them all: the page asks once at start
 * (window.ReachEdges) and is told again whenever they move (__reachEdges).
 * Elsewhere none of this exists and the variables stay unset.
 */

const EDGES = /^(\d+(\.\d+)?px )(\d+(\.\d+)?px ){2}\d+(\.\d+)?px$/;

type EdgesWindow = Window & {
	ReachEdges?: { get(): string };
	__reachEdges?: (css: string) => void;
};

function apply(css: string): void {
	if (!EDGES.test(css)) return;
	const [top, right, bottom, left] = css.split(' ');
	const style = document.documentElement.style;
	style.setProperty('--edge-top', top);
	style.setProperty('--edge-right', right);
	style.setProperty('--edge-bottom', bottom);
	style.setProperty('--edge-left', left);
}

let probe: HTMLDivElement | undefined;

/** The --safe-* edges in pixels, for menus placed by script: a menu kept
 *  inside the window could still open under the clock or the camera. */
export function safeInsets(): { top: number; right: number; bottom: number; left: number } {
	if (!probe) {
		probe = document.createElement('div');
		probe.style.cssText =
			'position:fixed;visibility:hidden;pointer-events:none;padding:var(--safe-top) var(--safe-right) var(--safe-bottom) var(--safe-left)';
		document.body.appendChild(probe);
	}
	const cs = getComputedStyle(probe);
	return {
		top: parseFloat(cs.paddingTop) || 0,
		right: parseFloat(cs.paddingRight) || 0,
		bottom: parseFloat(cs.paddingBottom) || 0,
		left: parseFloat(cs.paddingLeft) || 0
	};
}

export function initScreenEdges(): void {
	const w = window as EdgesWindow;
	w.__reachEdges = apply;
	try {
		const now = w.ReachEdges?.get();
		if (typeof now === 'string') apply(now);
	} catch {
		// Not Reach's Android activity.
	}
}
