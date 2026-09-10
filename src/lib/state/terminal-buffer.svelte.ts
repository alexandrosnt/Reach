export interface BufferReader {
	/** Live dimensions. A mirrored terminal must match these or wrapping breaks. */
	size?: () => { cols: number; rows: number };
	read: (startLine?: number, maxLines?: number) => string;
	lineCount: () => number;
}

const readers = new Map<string, BufferReader>();

export function registerBufferReader(id: string, reader: BufferReader): void {
	readers.set(id, reader);
}

/** Current size of a terminal, if it is mounted and reports one. */
export function readTerminalSize(id: string): { cols: number; rows: number } | null {
	return readers.get(id)?.size?.() ?? null;
}

export function unregisterBufferReader(id: string): void {
	readers.delete(id);
}

export function readTerminalBuffer(id: string, maxLines: number = 50): string {
	const reader = readers.get(id);
	if (!reader) return '';
	try {
		return reader.read(undefined, maxLines);
	} catch {
		return '';
	}
}

export function getBufferLineCount(id: string): number {
	const reader = readers.get(id);
	if (!reader) return 0;
	try {
		return reader.lineCount();
	} catch {
		return 0;
	}
}

export function readTerminalBufferFrom(id: string, startLine: number, maxLines: number = 100): string {
	const reader = readers.get(id);
	if (!reader) return '';
	try {
		return reader.read(startLine, maxLines);
	} catch {
		return '';
	}
}
