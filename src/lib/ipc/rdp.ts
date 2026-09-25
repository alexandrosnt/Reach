import { Channel, invoke } from '@tauri-apps/api/core';

/**
 * Remote desktop over RDP.
 *
 * Frames do not come back from `invoke`; they arrive on a Channel for the life
 * of the session, as raw bytes rather than JSON. See src-tauri/src/rdp for the
 * wire format and the reasoning. Everything else here is one command each.
 */

export interface RdpConnectParams {
	id: string;
	host: string;
	port: number;
	username: string;
	password: string;
	domain?: string;
	/** A local folder to show inside the desktop as a drive. */
	sharePath?: string;
	/** Advertise the graphics pipeline; see the setting. */
	graphicsPipeline?: boolean;
	/** The size the panel can show. The server renders at this size. */
	width: number;
	height: number;
}

export type RdpStatus =
	| { state: 'connected' }
	| { state: 'loggedIn' }
	| { state: 'closed'; reason: string }
	| { state: 'error'; message: string };

/** Frame kinds, matching `kind` in src-tauri/src/rdp/mod.rs. */
export const FRAME_REGION = 1;
export const FRAME_FULL = 2;
export const FRAME_POINTER = 3;
export const FRAME_POINTER_SHAPE = 4;
export const FRAME_POINTER_HIDDEN = 5;
export const FRAME_POINTER_DEFAULT = 6;

/** Header length for REGION and FULL frames: kind + six u16 fields. */
export const FRAME_HEADER = 13;

/** Length of a POINTER frame: kind + two u16 fields. */
export const FRAME_POINTER_LEN = 5;

/** Header length of a POINTER_SHAPE frame: kind + w, h, hotspot x, hotspot y. */
export const FRAME_POINTER_SHAPE_HEADER = 9;

/**
 * Open a desktop. `onFrame` is called for every message until the session
 * ends. A message holds one or more frames back to back; after painting all
 * of them the caller must `rdpAck` it, or the next one never comes.
 *
 * The channel delivers whatever the runtime hands it; on every Tauri build
 * this has been an ArrayBuffer, but a typed array is accepted too so a
 * runtime change here fails loudly in one place rather than silently in the
 * frame parser.
 */
export async function rdpConnect(
	params: RdpConnectParams,
	onFrame: (frame: ArrayBuffer) => void
): Promise<void> {
	const onFrameChannel = new Channel<ArrayBuffer | Uint8Array | number[]>();
	onFrameChannel.onmessage = (data) => {
		if (data instanceof ArrayBuffer) {
			onFrame(data);
			return;
		}
		// A view's `.buffer` may be a SharedArrayBuffer as far as the types
		// know, and may be larger than the view; copying into a fresh buffer
		// settles both and costs one memcpy per frame.
		const bytes = ArrayBuffer.isView(data) ? new Uint8Array(data.buffer, data.byteOffset, data.byteLength) : Uint8Array.from(data);
		const copy = new ArrayBuffer(bytes.byteLength);
		new Uint8Array(copy).set(bytes);
		onFrame(copy);
	};
	await invoke('rdp_connect', { params, onFrame: onFrameChannel });
}

export async function rdpDisconnect(id: string): Promise<void> {
	await invoke('rdp_disconnect', { id });
}

/**
 * The desktop gained focus. The backend looks at the local clipboard and, if
 * it changed, offers it to the remote so a paste there finds it.
 */
export async function rdpClipboardSync(id: string): Promise<void> {
	await invoke('rdp_clipboard_sync', { id });
}

/** The app window goes full screen, or comes back, for a desktop. */
export async function rdpWindowFullscreen(on: boolean): Promise<void> {
	await invoke('rdp_window_fullscreen', { on });
}

/** Close every open desktop; for the way out of the app. */
export async function rdpDisconnectAll(): Promise<void> {
	await invoke('rdp_disconnect_all');
}

/**
 * One message has been painted. The backend keeps at most two unacknowledged
 * messages in flight and folds everything else into the next one, so this is
 * what turns a busy screen into fewer, larger updates instead of a backlog.
 */
export async function rdpAck(id: string): Promise<void> {
	await invoke('rdp_ack', { id });
}

export type MouseAction = 'move' | 'down' | 'up' | 'wheel';

/** `button`: 0 left, 1 middle, 2 right. `delta`: wheel step, negative for away. */
export async function rdpMouse(
	id: string,
	x: number,
	y: number,
	action: MouseAction,
	button = 0,
	delta = 0
): Promise<void> {
	await invoke('rdp_mouse', { id, x, y, action, button, delta });
}

/** A PC/AT set-1 scancode, `extended` for the E0-prefixed keys. */
export async function rdpKey(id: string, scancode: number, extended: boolean, release: boolean): Promise<void> {
	await invoke('rdp_key', { id, scancode, extended, release });
}

/** A character rather than a key, for text with no scancode here. */
export async function rdpUnicode(id: string, code: number, release: boolean): Promise<void> {
	await invoke('rdp_unicode', { id, code, release });
}

export async function rdpResize(id: string, width: number, height: number): Promise<void> {
	await invoke('rdp_resize', { id, width, height });
}
