/**
 * Which webview Reach is running in, when a fix has to be aimed at one.
 *
 * Tauri gives every platform its own engine: WebView2 (Chromium) on Windows,
 * WKWebView on macOS, WebKitGTK on Linux. Most of the app does not care. The
 * terminal does, because the two WebKits present a WebGL canvas differently
 * from Chromium (issue #47).
 */

export function isMac(): boolean {
	return typeof navigator !== 'undefined' && /Mac|iPod|iPhone|iPad/.test(navigator.platform);
}

/**
 * True inside WKWebView or WebKitGTK; false inside WebView2.
 *
 * WebView2 advertises itself as Chrome and Edge in the user agent. WebKit
 * proper does not.
 */
export function isWebKit(): boolean {
	if (typeof navigator === 'undefined') return false;
	const ua = navigator.userAgent;
	return /AppleWebKit/.test(ua) && !/Chrome|Chromium|Edg\//.test(ua);
}

/**
 * True on Windows, where a file can be dragged out without being fetched
 * first: the drop target pulls the bytes through a stream after the drop.
 * Everywhere else the file has to exist locally before the gesture begins.
 */
export function isWindows(): boolean {
	if (typeof navigator === 'undefined') return false;
	const data = (navigator as { userAgentData?: { platform?: string } }).userAgentData;
	if (data?.platform) return data.platform === 'Windows';
	return /Win/.test(navigator.platform);
}
