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
let followPath = $state<boolean>(false);

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
}

export function clearConnection(connId: string): void {
	delete currentPath[connId];
	delete entries[connId];
}
