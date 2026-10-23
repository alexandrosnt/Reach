import { Channel, invoke } from '@tauri-apps/api/core';
import type { MouseAction, RdpStatus } from '$lib/ipc/rdp';
import type { SecretRef } from '$lib/ipc/pwmanager';

/**
 * Remote desktop over VNC. The backend sends frames in the RDP wire format
 * (see src-tauri/src/vnc), so the same panel paints both; only the commands
 * differ. Keys go as X11 keysyms; see `$lib/vnc/keysym`.
 */

export interface VncConnectParams {
	id: string;
	host: string;
	port: number;
	/** The VNC password; empty for a server that asks for none. */
	password: string;
	/** A password-manager entry, resolved by the backend at connect; wins over `password`. */
	passwordRef?: SecretRef | null;
	/** The saved session this desktop is for; required with `passwordRef` (the backend takes the entry from it). */
	sessionId?: string | null;
	/** A saved SSH session to reach the server through. */
	viaSessionId?: string | null;
}

export type VncStatus = RdpStatus;

export async function vncConnect(params: VncConnectParams, onFrame: (frame: ArrayBuffer) => void): Promise<void> {
	const onFrameChannel = new Channel<ArrayBuffer | Uint8Array | number[]>();
	onFrameChannel.onmessage = (data) => {
		if (data instanceof ArrayBuffer) {
			onFrame(data);
			return;
		}
		const bytes = ArrayBuffer.isView(data) ? new Uint8Array(data.buffer, data.byteOffset, data.byteLength) : Uint8Array.from(data);
		const copy = new ArrayBuffer(bytes.byteLength);
		new Uint8Array(copy).set(bytes);
		onFrame(copy);
	};
	const { sessionId, ...rest } = params;
	await invoke('vnc_connect', { params: rest, sessionId: sessionId ?? null, onFrame: onFrameChannel });
}

export async function vncDisconnect(id: string): Promise<void> {
	await invoke('vnc_disconnect', { id });
}

export async function vncDisconnectAll(): Promise<void> {
	await invoke('vnc_disconnect_all');
}

export async function vncAck(id: string): Promise<void> {
	await invoke('vnc_ack', { id });
}

/** As `rdpMouse`: `button` 0 left, 1 middle, 2 right; `delta` positive for away. */
export async function vncMouse(id: string, x: number, y: number, action: MouseAction, button = 0, delta = 0): Promise<void> {
	await invoke('vnc_mouse', { id, x, y, action, button, delta });
}

export async function vncKey(id: string, keysym: number, down: boolean): Promise<void> {
	await invoke('vnc_key', { id, keysym, down });
}

export async function vncResize(id: string, width: number, height: number): Promise<void> {
	await invoke('vnc_resize', { id, width, height });
}

/** The desktop gained focus: offer the local clipboard's text to the server. */
export async function vncClipboardSync(id: string): Promise<void> {
	await invoke('vnc_clipboard_sync', { id });
}
