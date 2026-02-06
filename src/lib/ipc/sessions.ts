import { invoke } from '@tauri-apps/api/core';

export interface AuthMethod {
  type: 'Password' | 'Key' | 'Agent';
  password?: string; // for Password type - stored encrypted in vault
  path?: string; // for Key type
  passphrase?: string; // for Key type - stored encrypted in vault
}

export interface SessionConfig {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  auth_method: AuthMethod;
  folder_id: string | null;
  tags: string[];
  detected_os?: string | null;
  vault_id?: string | null; // Which vault this session belongs to
}

export interface Folder {
  id: string;
  name: string;
  parent_id: string | null;
}

export async function sessionList(): Promise<SessionConfig[]> {
  return invoke<SessionConfig[]>('session_list');
}

export async function sessionGet(sessionId: string): Promise<SessionConfig> {
  return invoke<SessionConfig>('session_get', { sessionId });
}

export async function sessionCreate(params: {
  name: string;
  host: string;
  port: number;
  username: string;
  authMethod: AuthMethod;
  folderId: string | null;
  tags: string[];
  vaultId?: string | null;
}): Promise<SessionConfig> {
  return invoke<SessionConfig>('session_create', {
    name: params.name,
    host: params.host,
    port: params.port,
    username: params.username,
    authMethod: params.authMethod,
    folderId: params.folderId,
    tags: params.tags,
    vaultId: params.vaultId ?? null,
  });
}

export async function sessionUpdate(session: SessionConfig): Promise<SessionConfig> {
  return invoke<SessionConfig>('session_update', { session });
}

export async function sessionDelete(sessionId: string): Promise<void> {
  return invoke('session_delete', { sessionId });
}

export async function sessionListFolders(): Promise<Folder[]> {
  return invoke<Folder[]>('session_list_folders');
}

export async function sessionCreateFolder(name: string, parentId: string | null): Promise<Folder> {
  return invoke<Folder>('session_create_folder', { name, parentId });
}

export async function sessionDeleteFolder(folderId: string): Promise<void> {
  return invoke('session_delete_folder', { folderId });
}

export interface ShareResult {
  shareId: string;
  shareUrl: string;
}

/** Share a session with another user via X25519 key re-wrap. */
export async function sessionShare(
  sessionId: string,
  recipientUuid: string,
  recipientPublicKey: string,
  expiresInHours?: number
): Promise<ShareResult> {
  return invoke<ShareResult>('session_share', {
    session_id: sessionId,
    recipient_uuid: recipientUuid,
    recipient_public_key: recipientPublicKey,
    expires_in_hours: expiresInHours
  });
}
