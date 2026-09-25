/**
 * What the UI needs to know about each engine: names, ports, and the types
 * the designer offers. The type lists are the common ones, not every type
 * the engine has; the Type field accepts anything typed into it.
 */

import type { Engine } from '$lib/ipc/db';

export interface EngineInfo {
	id: Engine;
	name: string;
	/** Two letters for the badge. */
	short: string;
	/** Badge colour. */
	color: string;
	port: number;
	/** CodeMirror SQL dialect name. */
	dialect: 'PostgreSQL' | 'MySQL' | 'MariaSQL' | 'SQLite' | 'MSSQL' | null;
	defaultUser: string;
	types: string[];
	/** Types whose Length column means something. */
	sized: string[];
	/** Types that also take a scale (Decimals). */
	scaled: string[];
}

export const ENGINES: EngineInfo[] = [
	{
		id: 'postgres',
		name: 'PostgreSQL',
		short: 'PG',
		color: '#336791',
		port: 5432,
		dialect: 'PostgreSQL',
		defaultUser: 'postgres',
		types: [
			'integer', 'bigint', 'smallint', 'numeric', 'real', 'double precision', 'boolean',
			'text', 'varchar', 'char', 'uuid', 'date', 'time', 'timestamp', 'timestamptz', 'interval',
			'json', 'jsonb', 'bytea', 'inet', 'cidr', 'macaddr', 'money', 'xml', 'tsvector',
			'integer[]', 'text[]'
		],
		sized: ['varchar', 'char', 'bit', 'varbit', 'numeric'],
		scaled: ['numeric']
	},
	{
		id: 'mysql',
		name: 'MySQL',
		short: 'My',
		color: '#00758f',
		port: 3306,
		dialect: 'MySQL',
		defaultUser: 'root',
		types: [
			'int', 'bigint', 'smallint', 'tinyint', 'mediumint', 'decimal', 'float', 'double', 'bit',
			'varchar', 'char', 'text', 'mediumtext', 'longtext', 'tinytext', 'json',
			'date', 'datetime', 'timestamp', 'time', 'year',
			'blob', 'mediumblob', 'longblob', 'binary', 'varbinary', "enum('a','b')", "set('a','b')"
		],
		sized: ['varchar', 'char', 'binary', 'varbinary', 'decimal', 'float', 'double', 'bit', 'datetime', 'timestamp', 'time'],
		scaled: ['decimal', 'float', 'double']
	},
	{
		id: 'mariadb',
		name: 'MariaDB',
		short: 'Ma',
		color: '#c0765a',
		port: 3306,
		dialect: 'MariaSQL',
		defaultUser: 'root',
		types: [
			'int', 'bigint', 'smallint', 'tinyint', 'mediumint', 'decimal', 'float', 'double', 'bit',
			'varchar', 'char', 'text', 'mediumtext', 'longtext', 'json', 'uuid', 'inet6',
			'date', 'datetime', 'timestamp', 'time', 'year',
			'blob', 'mediumblob', 'longblob', 'binary', 'varbinary', "enum('a','b')", "set('a','b')"
		],
		sized: ['varchar', 'char', 'binary', 'varbinary', 'decimal', 'float', 'double', 'bit', 'datetime', 'timestamp', 'time'],
		scaled: ['decimal', 'float', 'double']
	},
	{
		id: 'sqlite',
		name: 'SQLite',
		short: 'SL',
		color: '#4c8bbf',
		port: 0,
		dialect: 'SQLite',
		defaultUser: '',
		types: ['integer', 'real', 'text', 'blob', 'numeric', 'boolean', 'date', 'datetime', 'varchar'],
		sized: ['varchar', 'numeric'],
		scaled: ['numeric']
	},
	{
		id: 'mssql',
		name: 'SQL Server',
		short: 'MS',
		color: '#a91d22',
		port: 1433,
		dialect: 'MSSQL',
		defaultUser: 'sa',
		types: [
			'int', 'bigint', 'smallint', 'tinyint', 'bit', 'decimal', 'numeric', 'float', 'real', 'money',
			'nvarchar', 'varchar', 'nchar', 'char', 'nvarchar(max)', 'varchar(max)', 'text', 'ntext',
			'uniqueidentifier', 'date', 'datetime', 'datetime2', 'datetimeoffset', 'time', 'smalldatetime',
			'varbinary', 'varbinary(max)', 'binary', 'xml'
		],
		sized: ['varchar', 'nvarchar', 'char', 'nchar', 'varbinary', 'binary', 'decimal', 'numeric'],
		scaled: ['decimal', 'numeric']
	},
	{
		id: 'redis',
		name: 'Redis',
		short: 'Rd',
		color: '#d82c20',
		port: 6379,
		dialect: null,
		defaultUser: '',
		types: [],
		sized: [],
		scaled: []
	}
];

export function engineInfo(id: Engine | undefined): EngineInfo {
	return ENGINES.find((e) => e.id === id) ?? ENGINES[0];
}

export const FK_ACTIONS = ['NO ACTION', 'RESTRICT', 'CASCADE', 'SET NULL', 'SET DEFAULT'];

export function indexMethods(engine: Engine): string[] {
	switch (engine) {
		case 'postgres':
			return ['btree', 'hash', 'gin', 'gist', 'brin', 'spgist'];
		case 'mysql':
		case 'mariadb':
			return ['btree', 'hash', 'fulltext', 'spatial'];
		case 'mssql':
			return ['nonclustered', 'clustered'];
		default:
			return [];
	}
}

export function mysqlEngines(): string[] {
	return ['InnoDB', 'MyISAM', 'MEMORY', 'ARCHIVE', 'CSV', 'Aria'];
}
