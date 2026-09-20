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
