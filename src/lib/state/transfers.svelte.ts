export interface Transfer {
	id: string;
	filename: string;
	bytesTransferred: number;
	totalBytes: number;
	percent: number;
	status: 'uploading' | 'downloading' | 'completed' | 'error';
	error?: string;
}

let transfers = $state<Record<string, Transfer>>({});

export function getTransfers(): Transfer[] {
	return Object.values(transfers).sort((a, b) => {
		const aActive = a.status === 'uploading' || a.status === 'downloading';
		const bActive = b.status === 'uploading' || b.status === 'downloading';
		if (aActive && !bActive) return -1;
		if (!aActive && bActive) return 1;
		return 0;
	});
}

export function addTransfer(
	id: string,
	filename: string,
	totalBytes: number,
	status: 'uploading' | 'downloading' = 'uploading'
): void {
	transfers[id] = {
		id,
		filename,
		bytesTransferred: 0,
		totalBytes,
		percent: 0,
		status
	};
}

export function updateTransferProgress(
	id: string,
	bytesTransferred: number,
	totalBytes: number,
	percent: number
): void {
	if (transfers[id]) {
		transfers[id] = { ...transfers[id], bytesTransferred, totalBytes, percent };
	}
}

export function completeTransfer(id: string): void {
	if (transfers[id]) {
		transfers[id] = { ...transfers[id], status: 'completed', percent: 100 };
	}
}

export function failTransfer(id: string, error: string): void {
	if (transfers[id]) {
		transfers[id] = { ...transfers[id], status: 'error', error };
	}
}

export function removeTransfer(id: string): void {
	delete transfers[id];
}

export function clearCompleted(): void {
	for (const id of Object.keys(transfers)) {
		if (transfers[id].status !== 'uploading' && transfers[id].status !== 'downloading') {
			delete transfers[id];
		}
	}
}
