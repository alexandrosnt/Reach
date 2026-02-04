import { invoke } from '@tauri-apps/api/core';

export interface SystemStats {
  cpu: number;
  ram: number;
  ramTotal: number;
  ramUsed: number;
  disk: number;
  users: string[];
}

export async function monitoringStart(connectionId: string): Promise<void> {
  return invoke('monitoring_start', { connectionId });
}

export async function monitoringStop(connectionId: string): Promise<void> {
  return invoke('monitoring_stop', { connectionId });
}

export async function monitoringGetStats(connectionId: string): Promise<SystemStats> {
  return invoke<SystemStats>('monitoring_get_stats', { connectionId });
}
