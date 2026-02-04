import { invoke } from '@tauri-apps/api/core';

export async function ptySpawn(
	id: string,
	shell?: string,
	cols?: number,
	rows?: number
): Promise<string> {
	return invoke<string>('pty_spawn', {
		id,
		shell: shell ?? null,
		cols: cols ?? 80,
		rows: rows ?? 24
	});
}

export async function ptyWrite(id: string, data: number[]): Promise<void> {
	return invoke('pty_write', { id, data });
}

export async function ptyResize(id: string, cols: number, rows: number): Promise<void> {
	return invoke('pty_resize', { id, cols, rows });
}

export async function ptyClose(id: string): Promise<void> {
	return invoke('pty_close', { id });
}
