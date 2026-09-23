/**
 * Dragging a remote file out without fetching it first.
 *
 * On Windows the drop target is handed a description of the file and a way to
 * ask for its contents. Nothing crosses the network unless the drop actually
 * happens, and the bytes are written wherever the user let go — the app is
 * never told where that was, and does not need to be.
 *
 * The call does not return until the drag ends, because that is how the
 * platform's drag loop works.
 */

import { invoke } from '@tauri-apps/api/core';

export interface DragFile {
	/** The name the copy should be given. */
	name: string;
	/** Where to read it from on the remote machine. */
	path: string;
	/** Known up front, so the drop target can show progress. */
	size: number;
}

/** Returns true if the files were dropped somewhere. */
export async function dragoutStart(connectionId: string, files: DragFile[]): Promise<boolean> {
	return invoke('dragout_start', { connectionId, files });
}
