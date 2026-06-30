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

export interface ProxyConfig {
  proxy_type: string;
  host: string;
  port: number;
  username?: string;
  password?: string;
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
  proxy?: ProxyConfig;
  /** Optional per-session login shell (e.g. "fish" or "fish -l"). Empty = remote default. */
  shell?: string;
  /** Inject the auto shell-color/prompt init after login (default true). */
  injectColors?: boolean;
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
    proxy: params.proxy ?? null,
    shell: params.shell?.trim() ? params.shell.trim() : null,
    injectColors: params.injectColors ?? null,
  });
}

export async function sshSend(connectionId: string, data: number[]): Promise<void> {
  return invoke('ssh_send', { connectionId, data });
}

/** Signal the backend that this terminal's data listener is attached, so it
 *  flushes buffered output (motd/banner) emitted before mount. */
export async function sshReady(connectionId: string): Promise<void> {
  return invoke('ssh_ready', { connectionId });
}

/** Host-key verification request emitted by the backend (`ssh-hostkey-prompt`). */
export interface HostKeyPrompt {
  promptId: string;
  host: string;
  port: number;
  fingerprint: string;
  keyType: string;
  /** true = the stored key changed (possible MITM); false = unknown host (TOFU). */
  changed: boolean;
  oldFingerprint?: string | null;
}

/** Report the user's accept/reject decision for a host-key prompt. */
export async function sshHostkeyResponse(promptId: string, accept: boolean): Promise<void> {
  return invoke('ssh_hostkey_response', { promptId, accept });
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

export type KeyFileKind = 'private_key' | 'public_key' | 'not_a_key' | 'not_found';

export interface KeyCandidate {
  path: string;
  name: string;
  algo?: string | null;
}

export interface KeyFileInfo {
  path: string;
  kind: KeyFileKind;
  algo?: string | null;
  comment?: string | null;
  encrypted: boolean;
  suggestedPrivateKey?: KeyCandidate | null;
  siblingPrivateKeys: KeyCandidate[];
}

/** Inspect a key-file path to detect wrong-file selections (e.g. a public key)
 *  and surface private-key suggestions from the same folder. */
export async function inspectKeyFile(path: string): Promise<KeyFileInfo> {
  return invoke<KeyFileInfo>('inspect_key_file', { path });
}
