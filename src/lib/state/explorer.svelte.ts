export interface FileEntry {
	name: string;
	path: string;
	isDirectory: boolean;
	size: number;
	modified: number;
	permissions?: string;
}

let currentPath = $state<Record<string, string>>({});
let entries = $state<Record<string, FileEntry[]>>({});
// Follow the terminal's folder (#162). Off unless turned on: it runs a
// command on the server after each line typed. Remembered on this device.
const FOLLOW_KEY = 'reach_explorer_follow_terminal';
let followPath = $state<boolean>(readFollow());
/** Lines typed into each connection's terminal, counted, for following. */
let entered = $state<Record<string, number>>({});

function readFollow(): boolean {
	try {
		return typeof localStorage !== 'undefined' && localStorage.getItem(FOLLOW_KEY) === '1';
	} catch {
		return false;
	}
}

export function getCurrentPath(connId: string): string {
	return currentPath[connId] ?? '/';
}

export function getEntries(connId: string): FileEntry[] {
	return entries[connId] ?? [];
}

export function getFollowPath(): boolean {
	return followPath;
}

export function setCurrentPath(connId: string, path: string): void {
	currentPath[connId] = path;
}

export function setEntries(connId: string, fileEntries: FileEntry[]): void {
	entries[connId] = fileEntries;
}

export function toggleFollowPath(): void {
	followPath = !followPath;
	try {
		localStorage.setItem(FOLLOW_KEY, followPath ? '1' : '0');
	} catch {
		// Not remembered; it still works for now.
	}
}

/** The terminal on this connection was sent a line (Enter): its folder may have changed. */
export function noteLineEntered(connId: string): void {
	entered[connId] = (entered[connId] ?? 0) + 1;
}

export function linesEntered(connId: string): number {
	return entered[connId] ?? 0;
}

export function clearConnection(connId: string): void {
	delete currentPath[connId];
	delete entries[connId];
}
