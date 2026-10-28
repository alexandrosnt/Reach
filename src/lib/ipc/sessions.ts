import { invoke } from '@tauri-apps/api/core';
import type { SecretRef } from './pwmanager';
import { t } from '#lib/state/i18n.svelte.js';

export interface AuthMethod {
  type: 'Password' | 'Key' | 'Agent';
  password?: string; // for Password type - stored encrypted in vault
  path?: string; // for Key type - a key file on this machine
  passphrase?: string; // for Key type - stored encrypted in vault
  /**
   * For Key type - a key imported into the vault, used instead of `path`.
   * Set when the user picked an imported key, so the session also works on a
   * machine that has no copy of the key file.
   */
  key_id?: string;
  /** For Password type - an entry in a password manager; wins over `password`. */
  password_ref?: SecretRef | null;
  /** For Key type - an entry in a password manager; wins over `passphrase`. */
  passphrase_ref?: SecretRef | null;
}

export interface JumpHostConfig {
  host: string;
  port: number;
  username: string;
  auth_method: AuthMethod;
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
  jump_chain?: JumpHostConfig[] | null; // ProxyJump chain
  proxy?: ProxySessionConfig | null; // Proxy config (SOCKS5/Tor, HTTP)
  shell?: string | null; // Optional per-session login shell (e.g. "fish -l")
  /** What the session connects with. Absent means SSH: older saves have no kind. */
  kind?: SessionKind;
  /** Windows logon domain, RDP only. */
  domain?: string | null;
  /** A local folder shown inside the remote desktop as a drive, RDP only. */
  share_path?: string | null;
  /** VNC only: a saved SSH session to reach the server through. */
  via_session_id?: string | null;
  /** Key sessions: also offer the SSH agent's keys if this key is refused. Off by default. */
  try_agent_keys?: boolean | null;
  /** ssh_config settings: imported files, lines set in Reach, approvals. */
  ssh_options?: SshOptions | null;
  /** WSL only: the distribution name, e.g. "Ubuntu" */
  wsl_distro?: string | null;
}

/** A config file as it was read at import. */
export interface SshConfigFile {
  path: string;
  text: string;
  role: 'user' | 'system' | 'included';
}

/** A session's ssh_config settings (see src-tauri/src/ssh/sshconf). */
export interface SshOptions {
  /** What the session was imported from; resolved again at every connect. */
  imported?: { alias: string; files: SshConfigFile[]; at: number } | null;
  /** Lines set in Reach ("MACs +hmac-sha1"); they win over the files. */
  lines?: string[];
  /** Local commands the user allowed for this session, exactly as written. */
  approved_commands?: string[];
  /** Weakening settings the user has seen and kept ("Keyword value"). */
  accepted_weakenings?: string[];
  /** This person's approvals in plain words: filled from the signed ones when
   *  a session is read, signed by the backend when it is saved. */
  my_approved_commands?: string[] | null;
  my_accepted_weakenings?: string[] | null;
}

export type SessionKind = 'ssh' | 'rdp' | 'vnc' | 'wsl';

export function sessionKind(session: Pick<SessionConfig, 'kind'>): SessionKind {
  return session.kind ?? 'ssh';
}

export interface ProxySessionConfig {
  proxy_type: string;
  host: string;
  port: number;
  username?: string | null;
  password?: string | null;
  /** An entry in a password manager; wins over `password`. */
  password_ref?: SecretRef | null;
}

export interface Folder {
  id: string;
  name: string;
  parent_id: string | null;
  vault_id: string | null;
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
  jumpChain?: JumpHostConfig[] | null;
  proxy?: ProxySessionConfig | null;
  shell?: string | null;
  kind?: SessionKind;
  domain?: string | null;
  /** For RDP: what the machine is, since the protocol cannot say. */
  detectedOs?: string | null;
  sharePath?: string | null;
  viaSessionId?: string | null;
  tryAgentKeys?: boolean | null;
  sshOptions?: SshOptions | null;
  wslDistro?: string | null;
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
    jumpChain: params.jumpChain ?? null,
    proxy: params.proxy ?? null,
    shell: params.shell?.trim() ? params.shell.trim() : null,
    kind: params.kind ?? 'ssh',
    domain: params.domain?.trim() ? params.domain.trim() : null,
    detectedOs: params.detectedOs ?? null,
    sharePath: params.sharePath?.trim() ? params.sharePath.trim() : null,
    viaSessionId: params.viaSessionId || null,
    tryAgentKeys: params.tryAgentKeys || null,
    sshOptions: params.sshOptions ?? null,
    wslDistro: params.wslDistro || null,
  });
}

export async function sessionUpdate(session: SessionConfig): Promise<SessionConfig> {
  return invoke<SessionConfig>('session_update', { session });
}

/** One password manager entry of a session waiting for the user's approval. */
export interface RefSlot {
  /** `password`, `passphrase`, `jump:<n>:password`, `jump:<n>:passphrase` or `proxy`. */
  slot: string;
  provider: string;
  entry_id: string;
}

/**
 * A session's password manager entries the current user has not approved,
 * with where they would go, read from the stored session in one go. The
 * confirmation dialog shows this and sends `context` and `refs` back.
 */
export interface UntrustedRefs {
  session_id: string;
  session_name: string;
  context: string;
  host: string;
  port: number;
  username: string;
  /** `user@host:port` for each jump host. */
  jumps: string[];
  /** `type user@host:port` of the proxy, if one is used. */
  proxy?: string | null;
  /** The session's own ssh_config lines that change where it connects (HostName, ProxyJump, ProxyCommand). */
  route_lines: string[];
  /** The SSH session a VNC session goes through, as stored now. */
  via?: ViaRoute | null;
  refs: RefSlot[];
}

/** Where the SSH session a VNC session goes through connects. */
export interface ViaRoute {
  session_id: string;
  session_name: string;
  host: string;
  port: number;
  username: string;
  jumps: string[];
  proxy?: string | null;
  route_lines: string[];
}

/** What a connect error starts with when the session has unapproved entries; JSON follows. */
export const UNTRUSTED_REFS_PREFIX = 'PWMANAGER_UNTRUSTED:';

/** The unapproved entries a connect was refused for, or null for any other error. */
export function untrustedRefsFromError(err: unknown): UntrustedRefs | null {
  const text = String(err);
  const at = text.indexOf(UNTRUSTED_REFS_PREFIX);
  if (at < 0) return null;
  try {
    return JSON.parse(text.slice(at + UNTRUSTED_REFS_PREFIX.length)) as UntrustedRefs;
  } catch {
    return null;
  }
}

/**
 * For a caller that cannot ask (a database, container host or cluster that
 * goes through a saved session): the unapproved-entries error as a sentence
 * pointing at the session list. Other errors pass through unchanged.
 */
export function explainUntrustedRefs<T>(call: Promise<T>): Promise<T> {
  return call.catch((err: unknown) => {
    throw untrustedRefsFromError(err) ? t('pwmanager.untrusted_reopen') : err;
  });
}

/** The session's unapproved password manager entries a connect would use, or null. */
export async function sessionUntrustedSecretRefs(sessionId: string): Promise<UntrustedRefs | null> {
  return invoke<UntrustedRefs | null>('session_untrusted_secret_refs', { sessionId });
}

/**
 * Approve entries, as the user just did in the confirmation dialog. Refused
 * if the session changed since `asked` was read.
 */
export async function sessionApproveSecretRefs(asked: UntrustedRefs): Promise<SessionConfig> {
  return invoke<SessionConfig>('session_approve_secret_refs', {
    sessionId: asked.session_id,
    context: asked.context,
    refs: asked.refs,
  });
}

export async function sessionDelete(sessionId: string): Promise<void> {
  return invoke('session_delete', { sessionId });
}

/** Move sessions into a folder (`null`: out of any folder) in one call.
 *  Only the folder changes; returns how many moved. */
export async function sessionMoveToFolder(sessionIds: string[], folderId: string | null): Promise<number> {
  return invoke<number>('session_move_to_folder', { sessionIds, folderId });
}

export async function sessionListFolders(): Promise<Folder[]> {
  return invoke<Folder[]>('session_list_folders');
}

export async function sessionCreateFolder(name: string, parentId: string | null, vaultId?: string | null): Promise<Folder> {
  return invoke<Folder>('session_create_folder', { name, parentId, vaultId: vaultId ?? null });
}

export async function sessionUpdateFolderParent(folderId: string, parentId: string | null): Promise<Folder> {
  return invoke<Folder>('session_update_folder_parent', { folderId, parentId });
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

/** Fetch installed WSL distributions on Windows */
export async function wslListDistros(): Promise<string[]> {
  return invoke<string[]>('wsl_list_distros');
}
