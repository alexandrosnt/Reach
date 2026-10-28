/**
 * The whole interface's size on a desktop: Ctrl+= and Ctrl+- (⌘ on a Mac),
 * Ctrl+0 to reset, and Settings > Appearance. Ctrl+wheel stays the
 * terminal's own text size.
 *
 * It is the webview's zoom, as a browser's, not CSS `zoom`: the page lays
 * out at the new size with a matching device pixel ratio, so xterm measures
 * its cells and maps the mouse correctly, which it does not under CSS zoom.
 * A phone sizes text by the system's own setting instead (MainActivity), so
 * this does nothing there.
 */
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { getSettings, updateSetting } from '#lib/state/settings.svelte.js';
import { isMobile } from '#lib/platform.js';

/** A browser's steps, so each press is a size people know. */
export const UI_ZOOM_STEPS = [0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2];

export function applyUiZoom(): void {
	if (isMobile()) return;
	void getCurrentWebview()
		.setZoom(getSettings().uiZoom)
		.catch(() => {
			// No webview (a test, or a window closing): nothing to size.
		});
}

/** One step larger (1) or smaller (-1), or back to 100% (0). */
export function stepUiZoom(direction: 1 | -1 | 0): void {
	if (isMobile()) return;
	const current = getSettings().uiZoom;
	let next = 1;
	if (direction === 1) next = UI_ZOOM_STEPS.find((z) => z > current + 0.001) ?? current;
	if (direction === -1) next = [...UI_ZOOM_STEPS].reverse().find((z) => z < current - 0.001) ?? current;
	if (next === current) return;
	updateSetting('uiZoom', next);
	applyUiZoom();
}
