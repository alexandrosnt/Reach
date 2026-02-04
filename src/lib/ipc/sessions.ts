import { invoke } from '@tauri-apps/api/core';

export interface AuthMethod {
  type: 'Password' | 'Key' | 'Agent';
  path?: string; // only for Key type
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
}): Promise<SessionConfig> {
  return invoke<SessionConfig>('session_create', {
    name: params.name,
    host: params.host,
    port: params.port,
    username: params.username,
    authMethod: params.authMethod,
    folderId: params.folderId,
    tags: params.tags,
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
