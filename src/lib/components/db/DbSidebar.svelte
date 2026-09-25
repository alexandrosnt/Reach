<script lang="ts">
	/**
	 * Connections, and for each open one its databases, schemas and objects.
	 * Below them, the machines Reach already has a terminal on, each a click
	 * away from "which databases run here?".
	 */
	import { onMount } from 'svelte';
	import FaIcon from '$lib/components/shared/FaIcon.svelte';
	import {
		faPlus,
		faChevronRight,
		faTable,
		faEye,
		faCode,
		faListOl,
		faMagnifyingGlass,
		faServer,
		faDesktop,
		faRotate,
		faEllipsis,
		faPlug
	} from '@fortawesome/free-solid-svg-icons';
	import EngineBadge from './EngineBadge.svelte';
	import DbMenu, { type MenuItem } from './DbMenu.svelte';
	import type { DbConnectionView, DbObject, Detected, ObjectKind } from '$lib/ipc/db';
	import { detect } from '$lib/ipc/db';
	import * as st from '$lib/state/db.svelte';
	import { getTabs as getTerminalTabs } from '$lib/state/tabs.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		onNewConnection: () => void;
		onEdit: (c: DbConnectionView) => void;
		onBackup: (connId: string, database: string, schema: string | null) => void;
		onRestore: (connId: string, database: string) => void;
		onImport: (connId: string, database: string, schema: string | null, table: string) => void;
		onExport: (connId: string, database: string, schema: string | null, table: string) => void;
		onRunSql: (connId: string, database: string, sql: string) => void;
		onDetected: (d: Detected, source: { connectionId: string | null; title: string }) => void;
		onDelete: (c: DbConnectionView) => void;
	}

	let { onNewConnection, onEdit, onBackup, onRestore, onImport, onExport, onRunSql, onDetected, onDelete }: Props = $props();

	let filter = $state('');
	let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
	let detecting = $state<Record<string, boolean>>({});
	let found = $state<Record<string, Detected[]>>({});

	onMount(() => {
		if (!st.isLoaded()) st.loadConnections();
	});

	const SYSTEM_DBS = new Set(['information_schema', 'performance_schema', 'mysql', 'sys', 'master', 'model', 'msdb', 'tempdb', 'postgres']);

	let saved = $derived(st.getConnections());
	let unsaved = $derived(st.getEphemeral());
	let terminals = $derived(getTerminalTabs().filter((tb) => tb.type === 'ssh' && tb.connectionId));

	const GROUPS: { kind: ObjectKind; label: () => string }[] = [
		{ kind: 'table', label: () => t('db.tables') },
		{ kind: 'view', label: () => t('db.views') },
		{ kind: 'materializedView', label: () => t('db.materialized_views') },
		{ kind: 'function', label: () => t('db.functions') },
		{ kind: 'procedure', label: () => t('db.procedures') },
		{ kind: 'sequence', label: () => t('db.sequences') }
	];

	function matches(o: DbObject): boolean {
		const f = filter.trim().toLowerCase();
		return !f || o.name.toLowerCase().includes(f);
	}

	async function toggleConnection(c: DbConnectionView): Promise<void> {
		if (!st.getOpen(c.id)) {
			const info = await st.openConnection(c.id);
			if (info?.engine === 'redis') st.openRedis(c.id);
			return;
		}
		if (st.engineOf(c.id) === 'redis') {
			st.openRedis(c.id);
			return;
		}
		st.toggleExpanded(c.id);
	}

	function toggleDatabase(connId: string, database: string): void {
		st.toggleExpanded(connId, database);
		if (!st.isExpanded(connId, database)) return;
		if (st.hasSchemas(st.engineOf(connId))) st.loadSchemas(connId, database);
		else st.loadObjects(connId, database, null);
	}

	function toggleSchema(connId: string, database: string, schema: string): void {
		st.toggleExpanded(connId, database, schema);
		if (st.isExpanded(connId, database, schema)) st.loadObjects(connId, database, schema);
	}

	// Load children for anything expanded on first render (an open
	// connection opens onto its default database).
	$effect(() => {
		for (const c of [...saved, ...unsaved]) {
			const open = st.getOpen(c.id);
			if (!open || open.engine === 'redis') continue;
			for (const database of open.databases) {
				if (!st.isExpanded(c.id, database)) continue;
				if (st.hasSchemas(open.engine)) {
					if (!st.getSchemas(c.id, database)) st.loadSchemas(c.id, database);
					for (const schema of st.getSchemas(c.id, database) ?? []) {
						if (st.isExpanded(c.id, database, schema) && !st.getObjects(c.id, database, schema)) st.loadObjects(c.id, database, schema);
					}
				} else if (!st.getObjects(c.id, database, null)) {
					st.loadObjects(c.id, database, null);
				}
			}
		}
	});

	function showMenu(e: MouseEvent, items: MenuItem[]): void {
		e.preventDefault();
		e.stopPropagation();
		menu = { x: e.clientX, y: e.clientY, items };
	}

	function connectionMenu(e: MouseEvent, c: DbConnectionView): void {
		const open = !!st.getOpen(c.id);
		const redis = c.engine === 'redis';
		const isSaved = saved.some((s) => s.id === c.id);
		const items: MenuItem[] = [];
		if (!open) items.push({ label: t('db.connect'), action: () => toggleConnection(c) });
		if (open && !redis) {
			const db = st.getOpen(c.id)!.defaultDatabase;
			items.push({ label: t('db.new_query'), action: () => st.openQuery(c.id, db, null) });
			items.push({ label: t('db.server_monitor'), action: () => st.openMonitor(c.id) });
		}
		if (open && redis) items.push({ label: t('db.open_keys'), action: () => st.openRedis(c.id) });
		if (open) {
			items.push({ label: t('db.refresh'), action: () => refreshConnection(c.id) });
			items.push({ label: t('db.disconnect'), action: () => st.closeConnection(c.id) });
		}
		if (isSaved) {
			items.push({ separator: true, label: '' });
			items.push({ label: t('db.edit_connection'), action: () => onEdit(c) });
			items.push({ label: t('db.delete_connection'), danger: true, action: () => onDelete(c) });
		} else {
			items.push({ separator: true, label: '' });
			items.push({ label: t('db.save_connection'), action: () => onEdit(c) });
		}
		showMenu(e, items);
	}

	async function refreshConnection(connId: string): Promise<void> {
		const info = await st.openConnection(connId, st.getEphemeral().find((x) => x.id === connId));
		if (info) for (const d of info.databases) st.invalidate(connId, d);
	}

	function databaseMenu(e: MouseEvent, connId: string, database: string): void {
		const hasSchemas = st.hasSchemas(st.engineOf(connId));
		showMenu(e, [
			{ label: t('db.new_query'), action: () => st.openQuery(connId, database, null) },
			{ label: t('db.new_table'), disabled: hasSchemas, action: () => st.openDesigner(connId, database, null, null) },
			{ separator: true, label: '' },
			{ label: t('db.backup'), action: () => onBackup(connId, database, null) },
			{ label: t('db.restore'), action: () => onRestore(connId, database) },
			{ separator: true, label: '' },
			{
				label: t('db.refresh'),
				action: () => {
					st.invalidate(connId, database);
					if (hasSchemas) st.loadSchemas(connId, database, true);
					else st.loadObjects(connId, database, null, true);
				}
			}
		]);
	}

	function schemaMenu(e: MouseEvent, connId: string, database: string, schema: string): void {
		showMenu(e, [
			{ label: t('db.new_query'), action: () => st.openQuery(connId, database, schema) },
			{ label: t('db.new_table'), action: () => st.openDesigner(connId, database, schema, null) },
			{ separator: true, label: '' },
			{ label: t('db.backup'), action: () => onBackup(connId, database, schema) },
			{ separator: true, label: '' },
			{ label: t('db.refresh'), action: () => st.loadObjects(connId, database, schema, true) }
		]);
	}

	function qualifiedName(connId: string, schema: string | null, name: string): string {
		const e = st.engineOf(connId);
		const q = (n: string) =>
			e === 'mysql' || e === 'mariadb' ? '`' + n.replace(/`/g, '``') + '`' : e === 'mssql' ? '[' + n.replace(/]/g, ']]') + ']' : '"' + n.replace(/"/g, '""') + '"';
		return schema ? `${q(schema)}.${q(name)}` : q(name);
	}

	function objectMenu(e: MouseEvent, connId: string, database: string, schema: string | null, o: DbObject): void {
		const name = qualifiedName(connId, schema, o.name);
		const items: MenuItem[] = [];
		if (o.kind === 'table' || o.kind === 'view' || o.kind === 'materializedView') {
			items.push({ label: t('db.open_data'), action: () => st.openTable(connId, database, schema, o.name) });
		}
		if (o.kind === 'table') {
			items.push({ label: t('db.design_table'), action: () => st.openDesigner(connId, database, schema, o.name) });
		}
		if (o.kind !== 'table' && o.kind !== 'sequence') {
			items.push({ label: t('db.show_definition'), action: () => showDefinition(connId, database, schema, o) });
		}
		items.push({
			label: t('db.query_this'),
			action: () =>
				st.openQuery(connId, database, schema, o.kind === 'table' || o.kind === 'view' || o.kind === 'materializedView' ? `SELECT * FROM ${name};\n` : '')
		});
		if (o.kind === 'table') {
			items.push({ separator: true, label: '' });
			items.push({ label: t('db.import_csv'), action: () => onImport(connId, database, schema, o.name) });
			items.push({ label: t('db.export'), action: () => onExport(connId, database, schema, o.name) });
			items.push({ label: t('db.backup_table'), action: () => onBackup(connId, database, schema) });
		}
		items.push({ separator: true, label: '' });
		items.push({ label: t('db.copy_name'), action: () => navigator.clipboard.writeText(o.name) });
		if (o.kind === 'table') {
			items.push({ label: t('db.truncate'), danger: true, action: () => onRunSql(connId, database, `TRUNCATE TABLE ${name}`) });
		}
		const dropKind =
			o.kind === 'materializedView' ? 'MATERIALIZED VIEW' : o.kind === 'function' ? 'FUNCTION' : o.kind === 'procedure' ? 'PROCEDURE' : o.kind.toUpperCase();
		items.push({
			label: t('db.drop', { kind: t(`db.kind_${o.kind}`) }),
			danger: true,
			// PostgreSQL routines are listed with their arguments, which is
			// exactly what DROP FUNCTION needs to pick the right one.
			action: () => onRunSql(connId, database, `DROP ${dropKind} ${o.kind === 'function' || o.kind === 'procedure' ? (schema ? qualifiedName(connId, null, schema) + '.' : '') + o.name : name}`)
		});
		showMenu(e, items);
	}

	async function showDefinition(connId: string, database: string, schema: string | null, o: DbObject): Promise<void> {
		try {
			const { definition } = await import('$lib/ipc/db');
			const def = await definition(connId, database, schema, o.name, o.kind);
			st.openQuery(connId, database, schema, def);
		} catch (err) {
			addToast(String(err), 'error');
		}
	}

	function openObject(connId: string, database: string, schema: string | null, o: DbObject): void {
		if (o.kind === 'table' || o.kind === 'view' || o.kind === 'materializedView') st.openTable(connId, database, schema, o.name);
		else if (o.kind !== 'sequence') showDefinition(connId, database, schema, o);
	}

	async function findDatabases(key: string, connectionId: string | null): Promise<void> {
		detecting[key] = true;
		try {
			found[key] = await detect(connectionId);
			if (!found[key].length) addToast(t('db.none_found'), 'info');
		} catch (e) {
			addToast(String(e), 'error');
		} finally {
			detecting[key] = false;
		}
	}

	function iconFor(kind: ObjectKind) {
		return kind === 'table' ? faTable : kind === 'view' || kind === 'materializedView' ? faEye : kind === 'sequence' ? faListOl : faCode;
	}

	function formatRows(n: number | null): string {
		if (n === null || n === undefined) return '';
		if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
		if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
		return String(n);
	}
</script>

{#snippet objectGroups(connId: string, database: string, schema: string | null)}
	{@const objects = st.getObjects(connId, database, schema)}
	{#if !objects}
		<div class="loading" style:--depth={schema ? 3 : 2}>{t('db.loading')}</div>
	{:else}
		{#each GROUPS as g (g.kind)}
			{@const list = objects.filter((o) => o.kind === g.kind && matches(o))}
			{#if list.length || (g.kind === 'table' && !filter)}
				{@const open = st.isExpanded(connId, database, schema, g.kind) || (g.kind === 'table' && !st.isExpanded(connId, database, schema, 'closed:table')) || !!filter}
				<button
					class="node group"
					style:--depth={schema ? 3 : 2}
					onclick={() =>
						g.kind === 'table'
							? st.toggleExpanded(connId, database, schema, 'closed:table')
							: st.toggleExpanded(connId, database, schema, g.kind)}
					oncontextmenu={(e) => (g.kind === 'table' ? (schema || !st.hasSchemas(st.engineOf(connId)) ? showMenu(e, [{ label: t('db.new_table'), action: () => st.openDesigner(connId, database, schema, null) }]) : null) : null)}
				>
					<span class="chev" class:open><FaIcon icon={faChevronRight} /></span>
					<span class="label">{g.label()}</span>
					<span class="count">{list.length}</span>
				</button>
				{#if open}
					{#each list as o (o.kind + o.name)}
						<button
							class="node object"
							style:--depth={schema ? 4 : 3}
							title={o.comment ?? o.name}
							ondblclick={() => openObject(connId, database, schema, o)}
							onclick={(e) => {
								if (e.detail === 1 && (o.kind === 'table' || o.kind === 'view')) openObject(connId, database, schema, o);
							}}
							oncontextmenu={(e) => objectMenu(e, connId, database, schema, o)}
						>
							<span class="icon"><FaIcon icon={iconFor(o.kind)} /></span>
							<span class="label">{o.name}</span>
							{#if o.rows !== null}<span class="count">{formatRows(o.rows)}</span>{/if}
						</button>
					{/each}
				{/if}
			{/if}
		{/each}
	{/if}
{/snippet}

{#snippet connectionNode(c: DbConnectionView)}
	{@const open = st.getOpen(c.id)}
	{@const expanded = open && st.isExpanded(c.id)}
	<div class="conn" style:--tag={c.color ?? 'transparent'}>
		<button
			class="node connection"
			class:open={!!open}
			onclick={() => toggleConnection(c)}
			oncontextmenu={(e) => connectionMenu(e, c)}
			title={c.engine === 'sqlite' ? c.filePath : `${c.username}@${c.host}:${c.port}`}
		>
			{#if c.engine !== 'redis'}
				<span class="chev" class:open={expanded}><FaIcon icon={faChevronRight} /></span>
			{:else}
				<span class="chev"></span>
			{/if}
			<EngineBadge engine={c.engine} />
			<span class="label">{c.name}</span>
			{#if c.production}<span class="pill prod">{t('db.prod')}</span>{/if}
			{#if c.readOnly}<span class="pill">{t('db.ro')}</span>{/if}
			{#if st.isConnecting(c.id)}
				<span class="spinner" aria-label={t('db.connecting')}></span>
			{:else if open}
				<span class="dot" aria-label={t('db.connected')}></span>
			{/if}
			<span
				class="more"
				role="button"
				tabindex="-1"
				aria-label={t('db.more')}
				onclick={(e) => connectionMenu(e, c)}
				onkeydown={() => {}}
			>
				<FaIcon icon={faEllipsis} />
			</span>
		</button>

		{#if expanded && open && open.engine !== 'redis'}
			{#each open.databases as database (database)}
				{@const dbOpen = st.isExpanded(c.id, database)}
				<button
					class="node database"
					class:system={SYSTEM_DBS.has(database) && database !== open.defaultDatabase}
					style:--depth={1}
					onclick={() => toggleDatabase(c.id, database)}
					oncontextmenu={(e) => databaseMenu(e, c.id, database)}
				>
					<span class="chev" class:open={dbOpen}><FaIcon icon={faChevronRight} /></span>
					<span class="icon"><FaIcon icon={faServer} /></span>
					<span class="label">{database}</span>
				</button>
				{#if dbOpen}
					{#if st.hasSchemas(open.engine)}
						{@const schemas = st.getSchemas(c.id, database)}
						{#if !schemas}
							<div class="loading" style:--depth={2}>{t('db.loading')}</div>
						{:else}
							{#each schemas as schema (schema)}
								{@const sOpen = st.isExpanded(c.id, database, schema)}
								<button
									class="node schema"
									style:--depth={2}
									onclick={() => toggleSchema(c.id, database, schema)}
									oncontextmenu={(e) => schemaMenu(e, c.id, database, schema)}
								>
									<span class="chev" class:open={sOpen}><FaIcon icon={faChevronRight} /></span>
									<span class="label">{schema}</span>
								</button>
								{#if sOpen}
									{@render objectGroups(c.id, database, schema)}
								{/if}
							{/each}
						{/if}
					{:else}
						{@render objectGroups(c.id, database, null)}
					{/if}
				{/if}
			{/each}
		{/if}
	</div>
{/snippet}

<aside class="sidebar">
	<div class="head">
		<div class="search">
			<FaIcon icon={faMagnifyingGlass} />
			<input bind:value={filter} placeholder={t('db.filter_objects')} spellcheck="false" />
		</div>
		<button class="icon-btn" onclick={onNewConnection} title={t('db.new_connection')} aria-label={t('db.new_connection')}>
			<FaIcon icon={faPlus} />
		</button>
	</div>

	<div class="scroll">
		{#if st.isLoaded() && !saved.length && !unsaved.length}
			<div class="empty">
				<p>{t('db.no_connections')}</p>
				<button class="link" onclick={onNewConnection}>{t('db.add_first')}</button>
			</div>
		{/if}

		{#each saved as c (c.id)}
			{@render connectionNode(c)}
		{/each}
		{#each unsaved as c (c.id)}
			{@render connectionNode(c)}
		{/each}

		<div class="section-title">{t('db.on_your_machines')}</div>
		{#each [{ key: 'local', connectionId: null, title: t('db.this_device'), icon: faDesktop }, ...terminals.map((tb) => ({ key: tb.connectionId!, connectionId: tb.connectionId!, title: tb.sessionName || tb.title, icon: faServer }))] as m (m.key)}
			<div class="machine">
				<span class="icon"><FaIcon icon={m.icon} /></span>
				<span class="label">{m.title}</span>
				<button class="find" onclick={() => findDatabases(m.key, m.connectionId)} disabled={detecting[m.key]}>
					{#if detecting[m.key]}
						<span class="spinner"></span>
					{:else}
						<FaIcon icon={found[m.key] ? faRotate : faMagnifyingGlass} />
					{/if}
					{found[m.key] ? t('db.search_again') : t('db.find_databases')}
				</button>
			</div>
			{#each found[m.key] ?? [] as d (d.host + d.port)}
				<button class="node detected" style:--depth={1} onclick={() => onDetected(d, { connectionId: m.connectionId, title: m.title })}>
					<EngineBadge engine={d.engine} size={16} />
					<span class="label">{d.label ?? d.engine} <span class="muted">:{d.port}</span></span>
					<span class="icon"><FaIcon icon={faPlug} /></span>
				</button>
			{/each}
		{/each}
	</div>
</aside>

{#if menu}
	<DbMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
	.sidebar {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
		background: var(--color-bg-primary);
		border-right: 1px solid var(--color-border);
	}

	.head {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 8px;
		border-bottom: 1px solid var(--color-border);
	}

	.search {
		flex: 1;
		height: 30px;
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 0 8px;
		border-radius: var(--radius-sm);
		background: var(--color-surface-sunken, var(--color-bg-elevated));
		color: var(--color-text-tertiary);
		font-size: var(--text-xs);
	}

	.search input {
		flex: 1;
		min-width: 0;
		padding: 6px 0;
		border: none;
		background: transparent;
		color: var(--color-text-primary);
		font-size: var(--text-sm);
		outline: none;
	}

	.icon-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		width: 30px;
		height: 30px;
		padding: 0;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.icon-btn:hover {
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
	}

	.scroll {
		flex: 1;
		overflow: auto;
		padding: 4px 0 12px;
	}

	.conn {
		border-left: 3px solid var(--tag);
	}

	.node {
		--depth: 0;
		display: flex;
		align-items: center;
		gap: 6px;
		width: 100%;
		padding: 4px 8px 4px calc(6px + var(--depth) * 14px);
		border: none;
		background: transparent;
		color: var(--color-text-primary);
		font-size: var(--text-sm);
		text-align: left;
		cursor: pointer;
		white-space: nowrap;
	}

	.node:hover {
		background: var(--color-surface-hover);
	}

	.node.connection {
		padding-top: 6px;
		padding-bottom: 6px;
		font-weight: 500;
	}

	.node.database.system .label {
		color: var(--color-text-tertiary);
	}

	.node.group .label {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.label {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.muted,
	.count {
		color: var(--color-text-tertiary);
		font-size: var(--text-xs);
	}

	.chev {
		display: inline-flex;
		width: 10px;
		font-size: 8px;
		color: var(--color-text-tertiary);
		transition: transform 0.12s;
	}

	.chev.open {
		transform: rotate(90deg);
	}

	.icon {
		display: inline-flex;
		width: 14px;
		justify-content: center;
		font-size: 11px;
		color: var(--color-text-tertiary);
	}

	.pill {
		padding: 0 5px;
		border-radius: 3px;
		font-size: 9px;
		font-weight: 600;
		letter-spacing: 0.04em;
		background: var(--color-surface-hover);
		color: var(--color-text-secondary);
	}

	.pill.prod {
		background: color-mix(in srgb, var(--color-danger) 20%, transparent);
		color: var(--color-danger);
	}

	.dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: #30d158;
	}

	.more {
		display: none;
		padding: 0 4px;
		color: var(--color-text-tertiary);
	}

	.node.connection:hover .more,
	.node.connection:focus-visible .more {
		display: inline-flex;
	}

	.spinner {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		border: 2px solid var(--color-border);
		border-top-color: var(--color-accent);
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.loading {
		padding: 4px 8px 4px calc(20px + var(--depth) * 14px);
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
	}

	.empty {
		padding: 20px 14px;
		text-align: center;
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.link {
		border: none;
		background: none;
		color: var(--color-accent);
		cursor: pointer;
		font-size: var(--text-sm);
	}

	.section-title {
		margin: 14px 10px 4px;
		font-size: var(--text-2xs, 10px);
		font-weight: 600;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: var(--color-text-tertiary);
	}

	.machine {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 4px 8px 4px 10px;
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
	}

	.find {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		padding: 2px 8px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: transparent;
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		cursor: pointer;
		white-space: nowrap;
	}

	.find:hover:not(:disabled) {
		color: var(--color-text-primary);
		border-color: var(--color-accent);
	}
</style>
