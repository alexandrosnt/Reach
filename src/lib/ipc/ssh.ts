import { invoke } from '@tauri-apps/api/core';

export interface JumpHostConnectParams {
  host: string;
  port: number;
  username: string;
  authMethod: string;
  password?: string;
  keyPath?: string;
  keyPassphrase?: string;
}

export interface SshConnectParams {
  id: string;
  host: string;
  port: number;
  username: string;
  authMethod: string;
  password?: string;
  keyPath?: string;
  keyPassphrase?: string;
  cols: number;
  rows: number;
  jumpChain?: JumpHostConnectParams[];
}

export interface ConnectionInfo {
  id: string;
  host: string;
  port: number;
  username: string;
}

export async function sshConnect(params: SshConnectParams): Promise<string> {
  return invoke<string>('ssh_connect', {
    id: params.id,
    host: params.host,
    port: params.port,
    username: params.username,
    authMethod: params.authMethod,
    password: params.password,
    keyPath: params.keyPath,
    keyPassphrase: params.keyPassphrase,
    cols: params.cols,
    rows: params.rows,
    jumpChain: params.jumpChain ?? null,
  });
}

export async function sshSend(connectionId: string, data: number[]): Promise<void> {
  return invoke('ssh_send', { connectionId, data });
}

export async function sshResize(connectionId: string, cols: number, rows: number): Promise<void> {
  return invoke('ssh_resize', { connectionId, cols, rows });
}

export async function sshDisconnect(connectionId: string): Promise<void> {
  return invoke('ssh_disconnect', { connectionId });
}

export async function sshListConnections(): Promise<ConnectionInfo[]> {
  return invoke<ConnectionInfo[]>('ssh_list_connections');
}

export async function sshDetectOs(connectionId: string): Promise<string> {
  return invoke<string>('ssh_detect_os', { connectionId });
}
