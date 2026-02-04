import { invoke } from '@tauri-apps/api/core';

export async function setMasterPassword(password: string): Promise<void> {
  return invoke('credential_set_master_password', { password });
}

export async function verifyMasterPassword(password: string): Promise<boolean> {
  return invoke<boolean>('credential_verify_master_password', { password });
}

export async function isLocked(): Promise<boolean> {
  return invoke<boolean>('credential_is_locked');
}

export async function lock(): Promise<void> {
  return invoke('credential_lock');
}

export async function hasMasterPassword(): Promise<boolean> {
  return invoke<boolean>('credential_has_master_password');
}

export async function savePassword(sessionId: string, password: string): Promise<void> {
  return invoke('credential_save_password', { sessionId, password });
}

export async function getPassword(sessionId: string): Promise<string | null> {
  return invoke<string | null>('credential_get_password', { sessionId });
}

export async function hasPassword(sessionId: string): Promise<boolean> {
  return invoke<boolean>('credential_has_password', { sessionId });
}

export async function deletePassword(sessionId: string): Promise<void> {
  return invoke('credential_delete_password', { sessionId });
}
