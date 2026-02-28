import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { emitTo } from '@tauri-apps/api/event';

export interface EditorFile {
	connectionId: string;
	path: string;
	filename: string;
	content: string;
}

let editorLabel: string | null = null;

export async function openEditor(
	connectionId: string,
	path: string,
	filename: string,
	content: string
): Promise<void> {
	const payload = { connectionId, path, filename, content };

	// Try to reuse existing editor window
	if (editorLabel) {
		try {
			// setFocus + emitTo — if either throws, the window is dead
			await emitTo(editorLabel, 'editor-open-file', payload);
			const win = await WebviewWindow.getByLabel(editorLabel);
			if (win) await win.setFocus();
			return;
		} catch {
			// Window is gone — clean up and create a new one
			editorLabel = null;
		}
	}

	// Create a new editor window with a unique label to avoid registry collisions
	const label = `editor-${Date.now()}`;
	editorLabel = label;

	const win = new WebviewWindow(label, {
		url: '/?editor=true',
		title: 'Editor',
		width: 900,
		height: 700,
		decorations: false,
		center: true,
	});

	win.once('tauri://created', async () => {
		setTimeout(async () => {
			try {
				await emitTo(label, 'editor-open-file', payload);
			} catch {
				// ignore — window might have been closed immediately
			}
		}, 500);
	});

	win.once('tauri://error', () => {
		if (editorLabel === label) editorLabel = null;
	});
}
