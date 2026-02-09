import { invoke } from '@tauri-apps/api/core';

export interface JumpHostEntry {
  host: string;
  port: number;
  user: string;
  identity_files: string[];
}

export interface SshHostEntry {
  name: string;
  hostname: string;
  port: number;
  user: string;
  identity_files: string[];
  proxy_jump: JumpHostEntry[];
}

/** List all named hosts from ~/.ssh/config. */
export async function sshconfigListHosts(): Promise<SshHostEntry[]> {
  return invoke<SshHostEntry[]>('sshconfig_list_hosts');
}

/** Resolve a single host from ~/.ssh/config with full details. */
export async function sshconfigResolveHost(hostname: string): Promise<SshHostEntry> {
  return invoke<SshHostEntry>('sshconfig_resolve_host', { hostname });
}

/** Check if an SSH config file exists. */
export async function sshconfigExists(): Promise<boolean> {
  return invoke<boolean>('sshconfig_exists');
}
