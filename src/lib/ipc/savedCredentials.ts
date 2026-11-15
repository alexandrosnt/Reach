import { invoke } from '@tauri-apps/api/core';
import type { AuthMethod } from './sessions';

/** A login kept once and used by many sessions in the same vault. */
export interface SavedCredential {
  id: string;
  name: string;
  username: string;
  /** Windows logon domain, used by RDP sessions. */
  domain?: string | null;
  auth_method: AuthMethod;
  /** The vault it is in (absent: this device's own sessions). */
  vault_id?: string | null;
}

export async function savedCredentialList(): Promise<SavedCredential[]> {
  return invoke<SavedCredential[]>('saved_credential_list');
}

export async function savedCredentialCreate(params: {
  name: string;
  username: string;
  domain?: string | null;
  authMethod: AuthMethod;
  vaultId?: string | null;
}): Promise<SavedCredential> {
  return invoke<SavedCredential>('saved_credential_create', {
    name: params.name,
    username: params.username,
    domain: params.domain?.trim() ? params.domain.trim() : null,
    authMethod: params.authMethod,
    vaultId: params.vaultId ?? null,
  });
}

export async function savedCredentialUpdate(credential: SavedCredential): Promise<SavedCredential> {
  return invoke<SavedCredential>('saved_credential_update', { credential });
}

/** Delete a credential; the sessions and folders using it go back to their
 *  own login. Returns how many were changed. */
export async function savedCredentialDelete(credentialId: string): Promise<number> {
  return invoke<number>('saved_credential_delete', { credentialId });
}

/** The credentials a session or folder in `vaultId` can use: same vault only. */
export function credentialsFor(all: SavedCredential[], vaultId: string | null | undefined): SavedCredential[] {
  return all.filter((c) => (c.vault_id ?? null) === (vaultId ?? null));
}
