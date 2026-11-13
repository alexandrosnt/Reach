export interface FileEntry {
	name: string;
	path: string;
	isDirectory: boolean;
	size: number;
	modified: number;
	permissions?: string;
	/** The owning user and group, as the server names them. */
	owner?: string;
	group?: string;
}

/** The columns of the Details view, in order. Name is always shown. */
export const DETAIL_COLUMNS = ['size', 'modified', 'owner', 'group', 'access'] as const;
export type DetailColumn = (typeof DETAIL_COLUMNS)[number];
export type SortKey = 'name' | DetailColumn;

interface ViewPrefs {
	details: boolean;
	hidden: DetailColumn[];
	sort: SortKey;
	desc: boolean;
}

// How the file list looks, remembered on this device.
const VIEW_KEY = 'reach_explorer_view';
let view = $state<ViewPrefs>(readView());

function readView(): ViewPrefs {
	const fallback: ViewPrefs = { details: false, hidden: [], sort: 'name', desc: false };
	try {
		const raw = typeof localStorage !== 'undefined' ? localStorage.getItem(VIEW_KEY) : null;
		if (!raw) return fallback;
		const v = JSON.parse(raw) as Partial<ViewPrefs>;
		const sorts: readonly string[] = ['name', ...DETAIL_COLUMNS];
		return {
			details: v.details === true,
			hidden: Array.isArray(v.hidden) ? v.hidden.filter((c): c is DetailColumn => (DETAIL_COLUMNS as readonly string[]).includes(c)) : [],
			sort: typeof v.sort === 'string' && sorts.includes(v.sort) ? (v.sort as SortKey) : 'name',
			desc: v.desc === true
		};
	} catch {
		return fallback;
	}
}

function saveView(): void {
	try {
		localStorage.setItem(VIEW_KEY, JSON.stringify(view));
	} catch {
		// Not remembered; it still applies for now.
	}
}

export function getView(): Readonly<ViewPrefs> {
	return view;
}

export function toggleDetailsView(): void {
	view.details = !view.details;
	saveView();
}

export function toggleColumn(col: DetailColumn): void {
	view.hidden = view.hidden.includes(col) ? view.hidden.filter((c) => c !== col) : [...view.hidden, col];
	if (view.sort === col && view.hidden.includes(col)) view.sort = 'name';
	saveView();
}

/** Sort by this column; choosing the current one again reverses it. */
export function sortBy(key: SortKey): void {
	if (view.sort === key) view.desc = !view.desc;
	else {
		view.sort = key;
		view.desc = false;
	}
	saveView();
}

/** Folders first, then by the chosen column, ties by name. */
export function compareEntries(a: FileEntry, b: FileEntry, key: SortKey, desc: boolean): number {
	if (a.isDirectory !== b.isDirectory) return a.isDirectory ? -1 : 1;
	const byName = a.name.localeCompare(b.name);
	let r: number;
	switch (key) {
		case 'size':
			r = a.size - b.size;
			break;
		case 'modified':
			r = a.modified - b.modified;
			break;
		case 'owner':
			r = (a.owner ?? '').localeCompare(b.owner ?? '');
			break;
		case 'group':
			r = (a.group ?? '').localeCompare(b.group ?? '');
			break;
		case 'access':
			r = (a.permissions ?? '').localeCompare(b.permissions ?? '');
			break;
		default:
			r = byName;
	}
	if (r === 0) r = byName;
	return desc ? -r : r;
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
