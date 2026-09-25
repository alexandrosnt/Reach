import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type Engine = 'postgres' | 'mysql' | 'mariadb' | 'sqlite' | 'mssql' | 'redis';
export type TlsMode = 'prefer' | 'require' | 'disable';

export type Route =
	| { kind: 'direct' }
	| { kind: 'session'; sessionId: string }
	| { kind: 'live'; connectionId: string };

export interface DbConnection {
	id: string;
	name: string;
	engine: Engine;
	host: string;
	port: number;
	username: string;
	password?: string | null;
	database?: string | null;
	filePath?: string | null;
	route: Route;
	tls: TlsMode;
	color?: string | null;
	readOnly: boolean;
	production: boolean;
	lastUsedAt: number;
}

export interface DbConnectionView extends DbConnection {
	hasPassword: boolean;
}

export interface ServerInfo {
	version: string;
	currentDatabase: string | null;
	currentUser: string | null;
}

export interface Connected {
	engine: Engine;
	databases: string[];
	defaultDatabase: string;
	server: ServerInfo;
}

export interface ColumnInfo {
	name: string;
	typeName: string;
}

export interface ResultSet {
	statement: string;
	columns: ColumnInfo[];
	rows: (string | null)[][];
	truncated: boolean;
	rowsAffected: number | null;
	elapsedMs: number;
}

export type ObjectKind = 'table' | 'view' | 'materializedView' | 'function' | 'procedure' | 'sequence' | 'trigger';

export interface DbObject {
	name: string;
	kind: ObjectKind;
	rows: number | null;
	comment: string | null;
}

export type Risk = { kind: 'read' } | { kind: 'session' } | { kind: 'write' } | { kind: 'danger'; reason: string };

export interface Statement {
	sql: string;
	risk: Risk;
	from: number;
	to: number;
}

export type ExecOutcome =
	| { status: 'confirm'; statements: Statement[] }
	| {
			status: 'done';
			results: ResultSet[];
			error: { index: number; statement: string; message: string } | null;
	  };

export interface Cell {
	column: string;
	value: string | null;
}

export type RowEdit =
	| { kind: 'update'; key: Cell[]; values: Cell[] }
	| { kind: 'insert'; values: Cell[] }
	| { kind: 'delete'; key: Cell[] };

export interface ColumnDef {
	original: string | null;
	name: string;
	dataType: string;
	length: number | null;
	scale: number | null;
	notNull: boolean;
	default: string | null;
	autoIncrement: boolean;
	comment: string | null;
	collation: string | null;
	unsigned: boolean;
	onUpdate: string | null;
	generated: string | null;
	generatedStored: boolean;
	defaultConstraint: string | null;
}

export interface IndexColumn {
	name: string;
	descending: boolean;
	length: number | null;
}

export interface IndexDef {
	original: string | null;
	name: string;
	columns: IndexColumn[];
	unique: boolean;
	constraint: boolean;
	method: string | null;
	whereClause: string | null;
	comment: string | null;
}

export interface ForeignKeyDef {
	original: string | null;
	name: string;
	columns: string[];
	refSchema: string | null;
	refTable: string;
	refColumns: string[];
	onDelete: string | null;
	onUpdate: string | null;
}

export interface CheckDef {
	original: string | null;
	name: string;
	expression: string;
}

export interface TriggerDef {
	original: string | null;
	name: string;
	definition: string;
}

export interface TableOptions {
	engine: string | null;
	charset: string | null;
	collation: string | null;
	autoIncrement: number | null;
}

export interface TableDesign {
	schema: string | null;
	originalName: string | null;
	name: string;
	columns: ColumnDef[];
	primaryKey: string[];
	primaryKeyName: string | null;
	indexes: IndexDef[];
	foreignKeys: ForeignKeyDef[];
	checks: CheckDef[];
	triggers: TriggerDef[];
	comment: string | null;
	options: TableOptions;
}

export interface DdlPlan {
	statements: string[];
	transactional: boolean;
}

export interface ServerSession {
	id: string;
	user: string | null;
	database: string | null;
	client: string | null;
	state: string | null;
	seconds: number | null;
	query: string | null;
	isSelf: boolean;
}

export interface Detected {
	engine: Engine;
	host: string;
	port: number;
	label: string | null;
}

export interface DumpOptions {
	tables: string[] | null;
	data: boolean;
	structure: boolean;
	views: boolean;
	routines: boolean;
}

export interface JobProgress {
	step: string;
	done: number;
	total: number;
	rows: number;
}

export interface JobSummary {
	tables: number;
	rows: number;
	statements: number;
	failed: string[];
}

export type JobEvent =
	| ({ kind: 'progress' } & JobProgress)
	| ({ kind: 'done' } & JobSummary)
	| { kind: 'failed'; message: string };

export interface KeyInfo {
	key: string;
	kind: string;
	ttlMs: number | null;
}

export interface ScanPage {
	cursor: string;
	keys: KeyInfo[];
}

export type RedisValue =
	| { kind: 'string'; value: string; binary: boolean }
	| { kind: 'hash'; entries: [string, string][] }
	| { kind: 'list'; items: string[] }
	| { kind: 'set'; members: string[] }
	| { kind: 'zset'; members: [string, number][] }
	| { kind: 'stream'; entries: [string, [string, string][]][] }
	| { kind: 'none' }
	| { kind: 'other'; typeName: string };

export interface KeyValue {
	key: string;
	ttlMs: number | null;
	length: number;
	truncated: boolean;
	value: RedisValue;
}

export interface Reply {
	text: string;
	isError: boolean;
}

export type RedisOutcome = { status: 'confirm'; reason: string } | { status: 'done'; reply: Reply };

// --- Connections

export const listConnections = () => invoke<DbConnectionView[]>('db_list_connections');
export const saveConnection = (connection: DbConnection, keepPassword: boolean) =>
	invoke<DbConnectionView>('db_save_connection', { connection, keepPassword });
export const deleteConnection = (id: string) => invoke<void>('db_delete_connection', { id });
export const testConnection = (connection: DbConnection, keepPassword: boolean) =>
	invoke<ServerInfo>('db_test_connection', { connection, keepPassword });
export const connect = (id: string, connection?: DbConnection) =>
	invoke<Connected>('db_connect', { id, connection: connection ?? null });
export const disconnect = (id: string) => invoke<void>('db_disconnect', { id });

// --- Tree

export const listDatabases = (id: string) => invoke<string[]>('db_databases', { id });
export const listSchemas = (id: string, database: string) => invoke<string[]>('db_schemas', { id, database });
export const listObjects = (id: string, database: string, schema: string | null) =>
	invoke<DbObject[]>('db_objects', { id, database, schema });
export const completionColumns = (id: string, database: string, schema: string | null) =>
	invoke<Record<string, string[]>>('db_completion', { id, database, schema });
export const definition = (id: string, database: string, schema: string | null, name: string, kind: string) =>
	invoke<string>('db_definition', { id, database, schema, name, kind });

// --- SQL

export const analyze = (engine: Engine, script: string) => invoke<Statement[]>('db_analyze', { engine, script });
export const execute = (
	id: string,
	database: string,
	owner: string,
	script: string,
	confirmed: boolean,
	maxRows?: number
) => invoke<ExecOutcome>('db_execute', { id, database, owner, script, confirmed, maxRows: maxRows ?? null });
export const cancel = (id: string, database: string, owner: string) =>
	invoke<void>('db_cancel', { id, database, owner });
export const closeSession = (id: string, database: string, owner: string) =>
	invoke<void>('db_close_session', { id, database, owner });

// --- Table data

export interface SortKey {
	column: string;
	desc: boolean;
}

export const tableRows = (
	id: string,
	database: string,
	schema: string | null,
	table: string,
	offset: number,
	limit: number,
	order: SortKey[],
	filter: string | null
) =>
	invoke<{ result: ResultSet; total: number | null }>('db_table_rows', {
		id,
		database,
		schema,
		table,
		offset,
		limit,
		order,
		filter
	});
export const tableDesign = (id: string, database: string, schema: string | null, table: string) =>
	invoke<TableDesign>('db_table_design', { id, database, schema, table });
export const previewEdits = (id: string, database: string, schema: string | null, table: string, edits: RowEdit[]) =>
	invoke<string[]>('db_preview_edits', { id, database, schema, table, edits });
export const applyEdits = (id: string, database: string, schema: string | null, table: string, edits: RowEdit[]) =>
	invoke<ResultSet[]>('db_apply_edits', { id, database, schema, table, edits });

// --- Designer

export const previewDesign = (engine: Engine, old: TableDesign | null, next: TableDesign) =>
	invoke<DdlPlan>('db_preview_design', { engine, old, new: next });
export const applyDesign = (id: string, database: string, old: TableDesign | null, next: TableDesign) =>
	invoke<TableDesign>('db_apply_design', { id, database, old, new: next });

// --- Server

export const serverInfo = (id: string, database: string) => invoke<ServerInfo>('db_server_info', { id, database });
export const sessions = (id: string) => invoke<ServerSession[]>('db_sessions', { id });
export const kill = (id: string, serverId: number, onlyQuery: boolean) =>
	invoke<void>('db_kill', { id, serverId, onlyQuery });

// --- Jobs

export const backup = (id: string, database: string, schema: string | null, options: DumpOptions, path: string) =>
	invoke<string>('db_backup', { id, database, schema, options, path });
export const restore = (id: string, database: string, path: string, keepGoing: boolean) =>
	invoke<string>('db_restore', { id, database, path, keepGoing });
export const csvPreview = (path: string, delimiter: string) =>
	invoke<string[][]>('db_csv_preview', { path, delimiter });
export const importCsv = (
	id: string,
	database: string,
	schema: string | null,
	table: string,
	spec: { path: string; delimiter: string; header: boolean; mapping: (string | null)[]; emptyAsNull: boolean }
) => invoke<string>('db_import_csv', { id, database, schema, table, spec });
export const exportData = (
	id: string,
	database: string,
	schema: string | null,
	table: string | null,
	query: string | null,
	format: 'csv' | 'json' | 'sql',
	path: string
) => invoke<number>('db_export', { id, database, schema, table, query, format, path });
export const cancelJob = (job: string) => invoke<void>('db_cancel_job', { job });

/** Follow a job to its end. Resolves with the summary, rejects with the failure. */
export async function followJob(job: string, onProgress: (p: JobProgress) => void): Promise<JobSummary> {
	let unlisten: UnlistenFn | undefined;
	try {
		return await new Promise<JobSummary>((resolve, reject) => {
			listen<JobEvent>(`db-job-${job}`, (e) => {
				const ev = e.payload;
				if (ev.kind === 'progress') onProgress(ev);
				else if (ev.kind === 'done') resolve(ev);
				else reject(new Error(ev.message));
			}).then((u) => (unlisten = u));
		});
	} finally {
		unlisten?.();
	}
}

// --- Detection

export const detect = (connectionId: string | null) => invoke<Detected[]>('db_detect', { connectionId });

// --- Redis

export const redisDatabases = (id: string) => invoke<[number, number][]>('db_redis_databases', { id });
export const redisScan = (id: string, db: number, pattern: string, cursor: string, count: number) =>
	invoke<ScanPage>('db_redis_scan', { id, db, pattern, cursor, count });
export const redisGet = (id: string, db: number, key: string) => invoke<KeyValue>('db_redis_get', { id, db, key });
export const redisCommand = (
	id: string,
	db: number,
	input: { line: string } | { args: string[] },
	confirmed: boolean
) =>
	invoke<RedisOutcome>('db_redis_command', {
		id,
		db,
		line: 'line' in input ? input.line : null,
		args: 'args' in input ? input.args : null,
		confirmed
	});
export const redisInfo = (id: string) => invoke<[string, [string, string][]][]>('db_redis_info', { id });
export const redisClients = (id: string) => invoke<Record<string, string>[]>('db_redis_clients', { id });
