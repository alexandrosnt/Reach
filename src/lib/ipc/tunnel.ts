import { invoke } from '@tauri-apps/api/core';

export interface TunnelConfig {
  id: string;
  tunnel_type: 'Local' | 'Remote' | 'Dynamic';
  local_port: number;
  remote_host: string;
  remote_port: number;
  connection_id: string;
  active: boolean;
}

export async function tunnelCreate(
  tunnelType: 'Local' | 'Remote' | 'Dynamic',
  localPort: number,
  remoteHost: string,
  remotePort: number,
  connectionId: string
): Promise<TunnelConfig> {
  return invoke<TunnelConfig>('tunnel_create', { tunnelType, localPort, remoteHost, remotePort, connectionId });
}

export async function tunnelStart(tunnelId: string): Promise<void> {
  return invoke('tunnel_start', { tunnelId });
}

export async function tunnelStop(tunnelId: string): Promise<void> {
  return invoke('tunnel_stop', { tunnelId });
}

export async function tunnelList(): Promise<TunnelConfig[]> {
  return invoke<TunnelConfig[]>('tunnel_list');
}
