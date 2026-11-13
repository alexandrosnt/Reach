import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { sftpTransferCancel } from '#lib/ipc/sftp.js';
import {
	addTransfer,
	completeTransfer,
	failTransfer,
	removeTransfer,
	updateTransferProgress
} from '#lib/state/transfers.svelte.js';

interface Progress {
	bytesTransferred: number;
	totalBytes: number;
	percent: number;
}

export interface TransferRun {
	kind: 'uploading' | 'downloading';
	/** The connection it runs on: its row shows only there. */
	connectionId: string;
	filename: string;
	totalBytes: number;
	/** Starts the work under the id given. */
	start: (transferId: string) => Promise<unknown>;
	/** Whether it gets a row in the transfer list. */
	visible?: boolean;
	/** Also told when the user cancels, after the backend is asked to stop. */
	onCancel?: () => void;
}

/**
 * Run one upload or download and settle when the backend says it ended.
 *
 * The id is made here and its events are listened to before the work
 * starts: a failure that comes at once, such as a folder the user may not
 * write (#161), used to be reported before anyone listened, and the
 * transfer then showed as running forever.
 *
 * Resolves `true` when done and `false` when the user cancelled it; rejects
 * with the backend's own message when it failed.
 */
export async function runTransfer(run: TransferRun): Promise<boolean> {
	const id = crypto.randomUUID();
	const visible = run.visible ?? true;
	let cancelled = false;

	let settle!: (error?: string) => void;
	const ended = new Promise<string | undefined>((resolve) => (settle = resolve));

	const unlisten: UnlistenFn[] = await Promise.all([
		listen<Progress>(`transfer-progress-${id}`, (event) => {
			if (!visible) return;
			const p = event.payload;
			updateTransferProgress(id, p.bytesTransferred, p.totalBytes, p.percent);
		}),
		listen(`transfer-complete-${id}`, () => settle()),
		listen<string>(`transfer-error-${id}`, (event) => {
			settle(typeof event.payload === 'string' ? event.payload : 'Transfer failed');
		})
	]);

	try {
		if (visible) {
			addTransfer(id, run.filename, run.totalBytes, run.kind, () => {
				if (cancelled) return;
				cancelled = true;
				removeTransfer(id);
				void sftpTransferCancel(id);
				run.onCancel?.();
			}, run.connectionId);
		}
		try {
			await run.start(id);
		} catch (err) {
			settle(String(err));
		}
		const error = await ended;
		if (cancelled) return false;
		if (error !== undefined) {
			if (visible) failTransfer(id, error);
			throw new Error(error);
		}
		if (visible) completeTransfer(id);
		return true;
	} finally {
		for (const u of unlisten) u();
	}
}
