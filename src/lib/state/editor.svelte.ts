import { invoke } from '@tauri-apps/api/core';

export interface EditorFile {
	connectionId: string;
	path: string;
	filename: string;
	content: string;
}

export async function openEditor(
	connectionId: string,
	path: string,
	filename: string,
	content: string
): Promise<void> {
	await invoke('editor_open_file', {
		file: { connectionId, path, filename, content }
	});
}

/** Called by EditorWindow on mount to get the initial file */
export async function fetchPendingFile(): Promise<EditorFile | null> {
	return await invoke<EditorFile | null>('editor_get_pending_file');
}
