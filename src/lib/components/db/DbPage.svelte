<script lang="ts">
	/**
	 * The Databases workspace: the connection tree on the left, tabs on the
	 * right. On a narrow window (a phone) the tree becomes a drawer.
	 *
	 * Dialogs live here so the tree, the grids and the designer can all ask
	 * for them without knowing about each other.
	 */
	import { onMount } from 'svelte';
	import { save as saveDialog } from '@tauri-apps/plugin-dialog';
	import FaIcon from '$lib/components/shared/FaIcon.svelte';
	import { faXmark, faBars, faDatabase, faPlus, faMagnifyingGlass } from '@fortawesome/free-solid-svg-icons';
	import DbSidebar from './DbSidebar.svelte';
	import ConnectionEditor from './ConnectionEditor.svelte';
	import QueryTab from './QueryTab.svelte';
	import TableDataTab from './TableDataTab.svelte';
	import TableDesigner from './TableDesigner.svelte';
	import MonitorTab from './MonitorTab.svelte';
	import RedisTab from './RedisTab.svelte';
	import BackupDialog from './BackupDialog.svelte';
	import ImportDialog from './ImportDialog.svelte';
	import SqlReviewDialog from './SqlReviewDialog.svelte';
	import DbMenu, { type MenuItem } from './DbMenu.svelte';
	import EngineBadge from './EngineBadge.svelte';
	import * as db from '$lib/ipc/db';
	import type { DbConnection, DbConnectionView, Detected, Statement } from '$lib/ipc/db';
	import * as st from '$lib/state/db.svelte';
	import { engineInfo } from '$lib/db/engines';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	let editor = $state<{ initial: Partial<DbConnectionView> | null; connectOnly: boolean } | null>(null);
	let job = $state<{ mode: 'backup' | 'restore'; connId: string; database: string; schema: string | null } | null>(null);
	let importing = $state<{ connId: string; database: string; schema: string | null; table: string } | null>(null);
	let pendingSql = $state<{ connId: string; database: string; statements: Statement[]; script: string } | null>(null);
	let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
	let drawer = $state(false);

	let tabs = $derived(st.getTabs());
	let activeId = $derived(st.getActiveTabId());
	let recent = $derived(st.getConnections().slice(0, 6));

	onMount(() => {
		if (!st.isLoaded()) st.loadConnections();
	});

	// Opening something from the drawer closes it.
	$effect(() => {
		void activeId;
		drawer = false;
	});

	function newConnection(): void {
		editor = { initial: null, connectOnly: false };
	}

	function edit(c: DbConnectionView): void {
		const isSaved = st.getConnections().some((x) => x.id === c.id);
		editor = { initial: isSaved ? c : { ...c, id: '', route: c.route.kind === 'live' ? { kind: 'direct' } : c.route }, connectOnly: false };
	}

	async function onDelete(c: DbConnectionView): Promise<void> {
		if (!confirm(t('db.delete_connection_confirm', { name: c.name }))) return;
		try {
			await st.remove(c.id);
		} catch (e) {
			addToast(String(e), 'error');
		}
	}

	/** A database found on a machine: prefill the editor with what is known. */
	function onDetected(d: Detected, source: { connectionId: string | null; title: string }): void {
		const info = engineInfo(d.engine);
		editor = {
			connectOnly: true,
			initial: {
				name: `${info.name} · ${source.title}${d.label && !d.label.includes(d.engine) ? ` (${d.label})` : ''}`,
				engine: d.engine,
				host: d.host,
				port: d.port,
				username: info.defaultUser,
				route: source.connectionId ? { kind: 'live', connectionId: source.connectionId } : { kind: 'direct' },
				tls: source.connectionId ? 'disable' : 'prefer'
			}
		};
	}

	async function connectUnsaved(c: DbConnection): Promise<void> {
		const id = `found-${crypto.randomUUID()}`;
		const info = await st.openConnection(id, { ...c, id });
		if (info?.engine === 'redis') st.openRedis(id);
	}

	async function onSaved(c: DbConnectionView): Promise<void> {
		// A newly added connection is one people want to use now.
		if (!st.getOpen(c.id)) {
			const info = await st.openConnection(c.id);
			if (info?.engine === 'redis') st.openRedis(c.id);
		}
	}

	async function exportTable(connId: string, database: string, schema: string | null, table: string, e?: MouseEvent): Promise<void> {
		const pick = async (format: 'csv' | 'json' | 'sql') => {
			const path = await saveDialog({ defaultPath: `${table}.${format}`, filters: [{ name: format.toUpperCase(), extensions: [format] }] });
			if (!path) return;
			try {
				const n = await db.exportData(connId, database, schema, table, null, format, path);
				addToast(t('db.exported', { n: n.toLocaleString() }), 'success');
			} catch (err) {
				addToast(String(err), 'error', 6000);
			}
		};
		menu = {
			x: e?.clientX ?? window.innerWidth / 2,
			y: e?.clientY ?? window.innerHeight / 3,
			items: (['csv', 'json', 'sql'] as const).map((f) => ({ label: t(`db.export_${f}`), action: () => pick(f) }))
		};
	}

	/** DROP and TRUNCATE from the tree: through the same guard as the editor. */
	async function runSql(connId: string, database: string, script: string, confirmed = false): Promise<void> {
		try {
			const out = await db.execute(connId, database, '__tree__', script, confirmed);
			if (out.status === 'confirm') {
				pendingSql = { connId, database, statements: out.statements, script };
				return;
			}
			pendingSql = null;
			if (out.error) addToast(out.error.message, 'error', 8000);
			else addToast(t('db.done'), 'success');
			st.invalidate(connId, database);
		} catch (e) {
			addToast(String(e), 'error', 6000);
		}
	}

	function closeTab(id: string): void {
		if (st.isDirty(id) && !confirm(t('db.close_dirty_confirm'))) return;
		st.closeTab(id);
	}

	function tabColor(connId: string): string {
		return st.getConnection(connId)?.color ?? 'transparent';
	}
</script>

<div class="dbpage">
	<div class="side" class:drawer>
		<DbSidebar
			onNewConnection={newConnection}
			onEdit={edit}
			{onDelete}
			onBackup={(connId, database, schema) => (job = { mode: 'backup', connId, database, schema })}
			onRestore={(connId, database) => (job = { mode: 'restore', connId, database, schema: null })}
			onImport={(connId, database, schema, table) => (importing = { connId, database, schema, table })}
			onExport={(connId, database, schema, table) => exportTable(connId, database, schema, table)}
			onRunSql={(connId, database, sql) => runSql(connId, database, sql)}
			{onDetected}
		/>
	</div>
	{#if drawer}
		<button class="scrim" aria-label={t('db.close')} onclick={() => (drawer = false)}></button>
	{/if}

	<div class="main">
		<div class="tabstrip" role="tablist">
			<button class="drawer-btn" onclick={() => (drawer = !drawer)} aria-label={t('db.connections')}>
				<FaIcon icon={faBars} />
			</button>
			{#each tabs as tb (tb.id)}
				<div class="tab" class:on={tb.id === activeId} style:--tag={tabColor(tb.connId)}>
					<button class="tab-title" role="tab" aria-selected={tb.id === activeId} onclick={() => st.activate(tb.id)} title={tb.title}>
						{#if st.engineOf(tb.connId)}<EngineBadge engine={st.engineOf(tb.connId)!} size={14} />{/if}
						<span>{tb.title}</span>
						{#if st.isTabBusy(tb.id)}<span class="spinner"></span>{:else if st.isDirty(tb.id)}<span class="dirty" aria-label={t('db.unsaved')}></span>{/if}
					</button>
					<button class="tab-close" onclick={() => closeTab(tb.id)} aria-label={t('db.close')}><FaIcon icon={faXmark} /></button>
				</div>
			{/each}
		</div>

		<div class="views">
			{#each tabs as tb (tb.id)}
				<div class="view" class:on={tb.id === activeId}>
					{#if tb.kind === 'query'}
						<QueryTab tab={tb} active={tb.id === activeId} />
					{:else if tb.kind === 'table'}
						<TableDataTab
							tab={tb}
							onExport={() => exportTable(tb.connId, tb.database, tb.schema, tb.table)}
							onImport={() => (importing = { connId: tb.connId, database: tb.database, schema: tb.schema, table: tb.table })}
						/>
					{:else if tb.kind === 'design'}
						<TableDesigner tab={tb} />
					{:else if tb.kind === 'monitor'}
						<MonitorTab tab={tb} active={tb.id === activeId} />
					{:else if tb.kind === 'redis'}
						<RedisTab tab={tb} />
					{/if}
				</div>
			{/each}

			{#if !tabs.length}
				<div class="start">
					<FaIcon icon={faDatabase} />
					<h2>{t('db.start_title')}</h2>
					<p>{t('db.start_hint')}</p>
					<div class="start-actions">
						<button class="big" onclick={newConnection}><FaIcon icon={faPlus} /> {t('db.new_connection')}</button>
						<button class="big ghost" onclick={() => (drawer = true)}><FaIcon icon={faMagnifyingGlass} /> {t('db.find_on_servers')}</button>
					</div>
					{#if recent.length}
						<div class="recent">
							{#each recent as c (c.id)}
								<button
									class="recent-item"
									onclick={async () => {
										const info = st.getOpen(c.id) ?? (await st.openConnection(c.id));
										if (!info) return;
										if (info.engine === 'redis') st.openRedis(c.id);
										else st.openQuery(c.id, info.defaultDatabase, null);
									}}
								>
									<EngineBadge engine={c.engine} size={20} />
									<span class="rname">{c.name}</span>
									<span class="rhost">{c.engine === 'sqlite' ? c.filePath : c.host}</span>
								</button>
							{/each}
						</div>
					{/if}
				</div>
			{/if}
		</div>
	</div>
</div>

{#if editor}
	<ConnectionEditor
		open
		initial={editor.initial}
		allowConnectOnly={editor.connectOnly}
		onclose={() => (editor = null)}
		onsaved={onSaved}
		onconnect={connectUnsaved}
	/>
{/if}

{#if job}
	<BackupDialog mode={job.mode} connId={job.connId} database={job.database} schema={job.schema} onclose={() => (job = null)} />
{/if}

{#if importing}
	<ImportDialog
		connId={importing.connId}
		database={importing.database}
		schema={importing.schema}
		table={importing.table}
		onclose={() => (importing = null)}
	/>
{/if}

<SqlReviewDialog
	open={!!pendingSql}
	title={t('db.confirm_run')}
	intro={t('db.confirm_run_intro')}
	statements={pendingSql?.statements ?? []}
	confirmLabel={t('db.run_anyway')}
	danger
	typeToConfirm={pendingSql && st.getConnection(pendingSql.connId)?.production ? st.getConnection(pendingSql.connId)?.name : null}
	onconfirm={() => pendingSql && runSql(pendingSql.connId, pendingSql.database, pendingSql.script, true)}
	onclose={() => (pendingSql = null)}
/>

{#if menu}
	<DbMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
	.dbpage {
		position: relative;
		display: flex;
		height: 100%;
		min-height: 0;
		background: var(--color-bg-primary);
	}

	.side {
		width: 290px;
		flex-shrink: 0;
		min-height: 0;
	}

	.main {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
	}

	.tabstrip {
		display: flex;
		align-items: stretch;
		height: 34px;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		overflow-x: auto;
		scrollbar-width: none;
	}

	.drawer-btn {
		display: none;
		padding: 0 12px;
		border: none;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.tab {
		display: flex;
		align-items: center;
		max-width: 240px;
		border-right: 1px solid var(--color-border);
		border-top: 2px solid var(--tag);
	}

	.tab.on {
		background: var(--color-bg-primary);
	}

	.tab-title {
		display: flex;
		align-items: center;
		gap: 6px;
		min-width: 0;
		height: 100%;
		padding: 0 4px 0 10px;
		border: none;
		background: transparent;
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		cursor: pointer;
	}

	.tab.on .tab-title {
		color: var(--color-text-primary);
	}

	.tab-title span:not(.spinner):not(.dirty) {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.tab-close {
		padding: 0 8px;
		height: 100%;
		border: none;
		background: transparent;
		color: var(--color-text-tertiary);
		font-size: 11px;
		cursor: pointer;
	}

	.tab-close:hover {
		color: var(--color-text-primary);
	}

	.dirty {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: #ffd60a;
		flex-shrink: 0;
	}

	.spinner {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		border: 2px solid var(--color-border);
		border-top-color: var(--color-accent);
		animation: spin 0.8s linear infinite;
		flex-shrink: 0;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.views {
		position: relative;
		flex: 1;
		min-height: 0;
	}

	.view {
		position: absolute;
		inset: 0;
		display: none;
	}

	.view.on {
		display: block;
	}

	.start {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 10px;
		height: 100%;
		padding: 24px;
		text-align: center;
		color: var(--color-text-secondary);
	}

	.start > :global(svg) {
		font-size: 36px;
		color: var(--color-text-tertiary);
	}

	.start h2 {
		margin: 4px 0 0;
		color: var(--color-text-primary);
		font-size: 18px;
		font-weight: 600;
	}

	.start p {
		margin: 0;
		max-width: 440px;
		font-size: var(--text-sm);
		line-height: 1.5;
	}

	.start-actions {
		display: flex;
		gap: 10px;
		margin-top: 8px;
		flex-wrap: wrap;
		justify-content: center;
	}

	.big {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		padding: 8px 16px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-accent);
		background: var(--color-accent);
		color: white;
		font-size: var(--text-sm);
		cursor: pointer;
	}

	.big.ghost {
		background: transparent;
		color: var(--color-text-primary);
		border-color: var(--color-border);
	}

	.recent {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
		gap: 8px;
		width: min(720px, 100%);
		margin-top: 18px;
	}

	.recent-item {
		display: grid;
		grid-template-columns: auto 1fr;
		grid-template-rows: auto auto;
		column-gap: 10px;
		align-items: center;
		padding: 10px 12px;
		border-radius: var(--radius-card, 8px);
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		text-align: left;
		cursor: pointer;
	}

	.recent-item:hover {
		border-color: var(--color-accent);
	}

	.recent-item :global(.badge) {
		grid-row: span 2;
	}

	.rname {
		color: var(--color-text-primary);
		font-size: var(--text-sm);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.rhost {
		color: var(--color-text-tertiary);
		font-size: var(--text-xs);
		font-family: var(--font-mono);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.scrim {
		display: none;
	}

	@media (max-width: 760px) {
		.side {
			position: absolute;
			z-index: 20;
			top: 0;
			bottom: 0;
			left: 0;
			width: min(86vw, 320px);
			transform: translateX(-100%);
			transition: transform 0.18s ease;
			box-shadow: var(--shadow-elevated);
		}

		.side.drawer {
			transform: none;
		}

		.scrim {
			display: block;
			position: absolute;
			inset: 0;
			z-index: 19;
			border: none;
			background: rgba(0, 0, 0, 0.35);
		}

		.drawer-btn {
			display: block;
		}
	}
</style>
