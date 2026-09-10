/**
 * Sharing configuration IPC.
 *
 * Configuration only. Nothing here opens a connection — that happens in the
 * webview, in `$lib/share/peer`, and only after the user clicks Share.
 */

import { invoke } from '@tauri-apps/api/core';

/** One ICE server in the shape `RTCPeerConnection` takes. */
export interface IceServer {
	/** `stun:` / `stuns:` / `turn:` / `turns:` URLs. */
	urls: string[];
	/** TURN only. STUN takes no credentials, and the backend rejects them. */
	username?: string;
	credential?: string;
}

export interface ShareConfig {
	/** Master switch. Off is the shipped default. */
	enabled: boolean;
	iceServers: IceServer[];
}

export async function shareGetConfig(): Promise<ShareConfig> {
	return invoke<ShareConfig>('share_get_config');
}

export async function shareSetEnabled(enabled: boolean): Promise<ShareConfig> {
	return invoke<ShareConfig>('share_set_enabled', { enabled });
}

/** Replaces the whole list. The backend validates every entry first. */
export async function shareSetIceServers(servers: IceServer[]): Promise<ShareConfig> {
	return invoke<ShareConfig>('share_set_ice_servers', { servers });
}

export async function shareResetIceServers(): Promise<ShareConfig> {
	return invoke<ShareConfig>('share_reset_ice_servers');
}

/** Apply what the vault holds. Call once the vault is unlocked. */
export async function shareLoad(): Promise<ShareConfig> {
	return invoke<ShareConfig>('share_load');
}
