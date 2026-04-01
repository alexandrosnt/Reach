import { invoke } from '@tauri-apps/api/core';

export interface Snippet {
  id: string;
  name: string;
  command: string;
  description?: string;
  tags: string[];
}

export async function snippetList(): Promise<Snippet[]> {
  return invoke<Snippet[]>('snippet_list');
}

export async function snippetCreate(params: {
  name: string;
  command: string;
  description?: string | null;
  tags: string[];
}): Promise<Snippet> {
  return invoke<Snippet>('snippet_create', {
    name: params.name,
    command: params.command,
    description: params.description ?? null,
    tags: params.tags,
  });
}

export async function snippetUpdate(snippet: Snippet): Promise<Snippet> {
  return invoke<Snippet>('snippet_update', { snippet });
}

export async function snippetDelete(snippetId: string): Promise<void> {
  return invoke('snippet_delete', { snippetId });
}
