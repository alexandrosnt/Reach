import { invoke } from '@tauri-apps/api/core';

/**
 * A private key imported into the vault. Never carries key material — the
 * only thing that reads a stored private key is the connect command, inside
 * Rust.
 */
export interface StoredKeyInfo {
	id: string;
	name: string;
	/** `ssh-ed25519`, `ssh-rsa`, … as the key declares itself. */
	algo: string | null;
	/** The SHA-256 fingerprint OpenSSH would print. */
	fingerprint: string | null;
	/** Whether the key material is passphrase-protected. */
	encrypted: boolean;
	/** Whether a passphrase is saved with it. */
	hasPassphrase: boolean;
	publicKey: string | null;
	createdAt: number;
}

/**
 * Import a key from pasted text or from a file. A file is read in Rust, so
 * key material never passes through the frontend.
 */
export async function sshKeyImport(params: {
	name: string;
	privateKey?: string;
	path?: string;
	passphrase?: string;
	publicKey?: string;
}): Promise<StoredKeyInfo> {
	return invoke<StoredKeyInfo>('ssh_key_import', {
		name: params.name,
		privateKey: params.privateKey || null,
		path: params.path || null,
		passphrase: params.passphrase || null,
		publicKey: params.publicKey || null
	});
}

export async function sshKeyList(): Promise<StoredKeyInfo[]> {
	return invoke<StoredKeyInfo[]>('ssh_key_list');
}

export async function sshKeyUpdate(params: {
	id: string;
	name?: string;
	passphrase?: string;
	clearPassphrase?: boolean;
}): Promise<StoredKeyInfo> {
	return invoke<StoredKeyInfo>('ssh_key_update', {
		id: params.id,
		name: params.name ?? null,
		passphrase: params.passphrase || null,
		clearPassphrase: params.clearPassphrase ?? false
	});
}

export async function sshKeyDelete(id: string): Promise<void> {
	return invoke('ssh_key_delete', { id });
}

/** The public half, for pasting into a server's `authorized_keys`. */
export async function sshKeyPublic(id: string): Promise<string | null> {
	return invoke<string | null>('ssh_key_public', { id });
}
