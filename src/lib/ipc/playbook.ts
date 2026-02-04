import { invoke } from '@tauri-apps/api/core';

export interface PlaybookRun {
  id: string;
  playbook_name: string;
  status: 'Running' | 'Completed' | 'Failed' | 'Stopped';
  current_step: number;
  total_steps: number;
}

export interface PlaybookStepEvent {
  step_index: number;
  step_name: string;
  status: 'running' | 'completed' | 'failed';
  output: string;
}

export async function playbookRun(yamlContent: string, connectionId: string): Promise<PlaybookRun> {
  return invoke<PlaybookRun>('playbook_run', { yamlContent, connectionId });
}

export async function playbookStop(runId: string): Promise<void> {
  return invoke('playbook_stop', { runId });
}

export async function playbookList(): Promise<PlaybookRun[]> {
  return invoke<PlaybookRun[]>('playbook_list');
}

export async function playbookGetRun(runId: string): Promise<PlaybookRun> {
  return invoke<PlaybookRun>('playbook_get_run', { runId });
}

export interface SavedPlaybook {
  id: string;
  name: string;
  yaml_content: string;
  created_at: number;
  updated_at: number;
}

export async function playbookSave(yamlContent: string, id?: string): Promise<SavedPlaybook> {
  return invoke<SavedPlaybook>('playbook_save', { id: id ?? null, yamlContent });
}

export async function playbookListSaved(): Promise<SavedPlaybook[]> {
  return invoke<SavedPlaybook[]>('playbook_list_saved');
}

export async function playbookDelete(id: string): Promise<void> {
  return invoke('playbook_delete', { id });
}
