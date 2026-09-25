/**
 * The Databases workspace: saved connections, which are open, the object tree
 * and the tabs in the main area.
 *
 * Tab contents (a query's text and results, a grid's pending edits, a
 * designer's draft) live in the tab components themselves, which stay mounted
 * while hidden, the way terminal tabs do. This file keeps only what other
 * parts need to see: titles, which tabs are busy, which have unsaved work.
 */

import * as db from '$lib/ipc/db';
import type { Connected, DbConnection, DbConnectionView, DbObject, Engine } from '$lib/ipc/db';
import { addToast } from '$lib/state/toasts.svelte';

export type DbTab =
	| { kind: 'query'; id: string; connId: string; database: string; schema: string | null; title: string; initialSql?: string }
	| { kind: 'table'; id: string; connId: string; database: string; schema: string | null; table: string; title: string }
	| { kind: 'design'; id: string; connId: string; database: string; schema: string | null; table: string | null; title: string }
	| { kind: 'redis'; id: string; connId: string; title: string }
	| { kind: 'monitor'; id: string; connId: string; title: string };

let connections = $state<DbConnectionView[]>([]);
/** Connections found on a terminal tab's server, open but not saved. */
let ephemeral = $state<Record<string, DbConnectionView>>({});
let open = $state<Record<string, Connected>>({});
let connecting = $state<Record<string, boolean>>({});
let loaded = $state(false);

let tabs = $state<DbTab[]>([]);
let activeTabId = $state<string | null>(null);
let busyTabs = $state<Record<string, boolean>>({});
let dirtyTabs = $state<Record<string, boolean>>({});

// Tree caches, keyed by connection / database / schema.
let schemaCache = $state<Record<string, string[]>>({});
let objectCache = $state<Record<string, DbObject[]>>({});
let expanded = $state<Record<string, boolean>>({});

const nodeKey = (...parts: (string | null)[]) => parts.map((p) => p ?? '').join('\u0000');

// --- Connections -----------------------------------------------------------

export function getConnections(): DbConnectionView[] {
	return connections;
}

export function getConnection(id: string): DbConnectionView | undefined {
	return connections.find((c) => c.id === id) ?? ephemeral[id];
}

export function getEphemeral(): DbConnectionView[] {
	return Object.values(ephemeral);
}

export function isLoaded(): boolean {
	return loaded;
}

export async function loadConnections(): Promise<void> {
	try {
		connections = await db.listConnections();
	} catch (e) {
		addToast(String(e), 'error');
	} finally {
		loaded = true;
	}
}

export async function save(connection: DbConnection, keepPassword: boolean): Promise<DbConnectionView> {
	const saved = await db.saveConnection(connection, keepPassword);
	const i = connections.findIndex((c) => c.id === saved.id);
	if (i >= 0) connections[i] = saved;
	else connections = [saved, ...connections];
	return saved;
}

export async function remove(id: string): Promise<void> {
	await db.deleteConnection(id);
	connections = connections.filter((c) => c.id !== id);
	forget(id);
}

export function getOpen(id: string): Connected | undefined {
	return open[id];
}

export function isConnecting(id: string): boolean {
	return !!connecting[id];
}

/** Open a saved connection, or an unsaved one found on a server. */
export async function openConnection(id: string, unsaved?: DbConnection): Promise<Connected | undefined> {
	connecting[id] = true;
	try {
		const info = await db.connect(id, unsaved);
		if (unsaved) ephemeral[id] = { ...unsaved, id, hasPassword: !!unsaved.password };
		open[id] = info;
		expanded[nodeKey(id)] = true;
		// Open straight onto the default database so the tables are one look away.
		if (info.engine !== 'redis') expanded[nodeKey(id, info.defaultDatabase)] = true;
		return info;
	} catch (e) {
		addToast(String(e), 'error', 6000);
		return undefined;
	} finally {
		connecting[id] = false;
	}
}

export async function closeConnection(id: string): Promise<void> {
	try {
		await db.disconnect(id);
	} catch {
		// Already gone
	}
	forget(id);
}

function forget(id: string): void {
	delete open[id];
	delete ephemeral[id];
	for (const k of Object.keys(schemaCache)) if (k.startsWith(id + '\u0000') || k === id) delete schemaCache[k];
	for (const k of Object.keys(objectCache)) if (k.startsWith(id + '\u0000')) delete objectCache[k];
	const gone = tabs.filter((t) => t.connId === id).map((t) => t.id);
	tabs = tabs.filter((t) => t.connId !== id);
	if (activeTabId && gone.includes(activeTabId)) activeTabId = tabs.at(-1)?.id ?? null;
}

export function engineOf(connId: string): Engine | undefined {
	return open[connId]?.engine ?? getConnection(connId)?.engine;
}

// --- Tree ------------------------------------------------------------------

export function isExpanded(...parts: (string | null)[]): boolean {
	return !!expanded[nodeKey(...parts)];
}

export function toggleExpanded(...parts: (string | null)[]): void {
	const k = nodeKey(...parts);
	expanded[k] = !expanded[k];
}

export function setExpanded(value: boolean, ...parts: (string | null)[]): void {
	expanded[nodeKey(...parts)] = value;
}

/** Engines with schemas between the database and its tables. */
export function hasSchemas(engine: Engine | undefined): boolean {
	return engine === 'postgres' || engine === 'mssql';
}

export function getSchemas(connId: string, database: string): string[] | undefined {
	return schemaCache[nodeKey(connId, database)];
}

export async function loadSchemas(connId: string, database: string, force = false): Promise<void> {
	const k = nodeKey(connId, database);
	if (schemaCache[k] && !force) return;
	try {
		schemaCache[k] = await db.listSchemas(connId, database);
		// The usual schema opens by itself.
		const first = schemaCache[k][0];
		if (first && (first === 'public' || first === 'dbo')) expanded[nodeKey(connId, database, first)] = true;
	} catch (e) {
		addToast(String(e), 'error');
	}
}

export function getObjects(connId: string, database: string, schema: string | null): DbObject[] | undefined {
	return objectCache[nodeKey(connId, database, schema)];
}

export async function loadObjects(connId: string, database: string, schema: string | null, force = false): Promise<void> {
	const k = nodeKey(connId, database, schema);
	if (objectCache[k] && !force) return;
	try {
		objectCache[k] = await db.listObjects(connId, database, schema);
	} catch (e) {
		addToast(String(e), 'error');
	}
}

/** After DDL: forget what the tree knows about a database so it reloads. */
export function invalidate(connId: string, database: string): void {
	for (const k of Object.keys(objectCache)) if (k.startsWith(nodeKey(connId, database))) delete objectCache[k];
	delete schemaCache[nodeKey(connId, database)];
}

// --- Tabs ------------------------------------------------------------------

export function getTabs(): DbTab[] {
	return tabs;
}

export function getActiveTabId(): string | null {
	return activeTabId;
}

export function activate(id: string): void {
	activeTabId = id;
}

function newId(): string {
	return crypto.randomUUID();
}

function add(tab: DbTab): string {
	tabs = [...tabs, tab];
	activeTabId = tab.id;
	return tab.id;
}

export function openQuery(connId: string, database: string, schema: string | null, initialSql?: string): string {
	const n = tabs.filter((t) => t.kind === 'query' && t.connId === connId).length + 1;
	const name = getConnection(connId)?.name ?? '';
	return add({ kind: 'query', id: newId(), connId, database, schema, title: `${name} · Query ${n}`, initialSql });
}

export function openTable(connId: string, database: string, schema: string | null, table: string): string {
	const existing = tabs.find((t) => t.kind === 'table' && t.connId === connId && t.database === database && t.schema === schema && t.table === table);
	if (existing) {
		activeTabId = existing.id;
		return existing.id;
	}
	return add({ kind: 'table', id: newId(), connId, database, schema, table, title: table });
}

export function openDesigner(connId: string, database: string, schema: string | null, table: string | null): string {
	const existing = table
		? tabs.find((t) => t.kind === 'design' && t.connId === connId && t.database === database && t.schema === schema && t.table === table)
		: undefined;
	if (existing) {
		activeTabId = existing.id;
		return existing.id;
	}
	return add({ kind: 'design', id: newId(), connId, database, schema, table, title: table ? `${table} · Design` : 'New table' });
}

export function openRedis(connId: string): string {
	const existing = tabs.find((t) => t.kind === 'redis' && t.connId === connId);
	if (existing) {
		activeTabId = existing.id;
		return existing.id;
	}
	return add({ kind: 'redis', id: newId(), connId, title: getConnection(connId)?.name ?? 'Redis' });
}

export function openMonitor(connId: string): string {
	const existing = tabs.find((t) => t.kind === 'monitor' && t.connId === connId);
	if (existing) {
		activeTabId = existing.id;
		return existing.id;
	}
	return add({ kind: 'monitor', id: newId(), connId, title: `${getConnection(connId)?.name ?? ''} · Server` });
}

export function renameTab(id: string, title: string): void {
	const t = tabs.find((x) => x.id === id);
	if (t) t.title = title;
}

/** A query tab moved to another database. Its old server session goes. */
export function retargetQuery(id: string, database: string, schema: string | null): void {
	const t = tabs.find((x) => x.id === id);
	if (t && t.kind === 'query') {
		db.closeSession(t.connId, t.database, t.id).catch(() => {});
		t.database = database;
		t.schema = schema;
	}
}

/** Point a new-table designer at the table it just created. */
export function retarget(id: string, table: string, title: string): void {
	const t = tabs.find((x) => x.id === id);
	if (t && t.kind === 'design') {
		t.table = table;
		t.title = title;
	}
}

export function closeTab(id: string): void {
	const i = tabs.findIndex((t) => t.id === id);
	if (i < 0) return;
	const tab = tabs[i];
	if (tab.kind === 'query') {
		// Give the tab's server session back.
		db.closeSession(tab.connId, tab.database, tab.id).catch(() => {});
	}
	tabs = tabs.filter((t) => t.id !== id);
	delete busyTabs[id];
	delete dirtyTabs[id];
	if (activeTabId === id) activeTabId = tabs[Math.min(i, tabs.length - 1)]?.id ?? null;
}

export function setBusy(id: string, busy: boolean): void {
	busyTabs[id] = busy;
}

export function isTabBusy(id: string): boolean {
	return !!busyTabs[id];
}

/** Something is running; the DevOps switch waits for it. */
export function isAnyBusy(): boolean {
	return Object.values(busyTabs).some(Boolean);
}

export function setDirty(id: string, dirty: boolean): void {
	dirtyTabs[id] = dirty;
}

export function isDirty(id: string): boolean {
	return !!dirtyTabs[id];
}

// --- Query history ---------------------------------------------------------

export interface HistoryEntry {
	sql: string;
	at: number;
	ms: number;
	ok: boolean;
	database: string;
}

const HISTORY_MAX = 200;
const historyKey = (connId: string) => `reach-db-history-${connId}`;

export function readHistory(connId: string): HistoryEntry[] {
	try {
		return JSON.parse(localStorage.getItem(historyKey(connId)) ?? '[]');
	} catch {
		return [];
	}
}

export function pushHistory(connId: string, entry: HistoryEntry): void {
	try {
		const list = readHistory(connId).filter((h) => h.sql !== entry.sql);
		list.unshift(entry);
		localStorage.setItem(historyKey(connId), JSON.stringify(list.slice(0, HISTORY_MAX)));
	} catch {
		// Storage full or unavailable; history is a convenience.
	}
}

export function clearHistory(connId: string): void {
	try {
		localStorage.removeItem(historyKey(connId));
	} catch {
		// Nothing to clear
	}
}
