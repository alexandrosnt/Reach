<script lang="ts">
	/**
	 * A query tab: editor above, results below, history on the side.
	 *
	 * Each tab has its own server session, so a transaction or SET opened here
	 * stays here. Running goes through the backend's guard: anything it holds
	 * back comes to the review dialog as the exact statements, and nothing in
	 * the script runs until the answer is yes.
	 */
	import { onMount, untrack } from 'svelte';
	import { save as saveDialog } from '@tauri-apps/plugin-dialog';
	import FaIcon from '$lib/components/shared/FaIcon.svelte';
	import { faPlay, faForwardFast, faStop, faDiagramProject, faWandMagicSparkles, faClockRotateLeft, faFileExport, faLightbulb } from '@fortawesome/free-solid-svg-icons';
	import SqlEditor, { type EditorApi } from './SqlEditor.svelte';
	import DataGrid from './DataGrid.svelte';
	import SqlReviewDialog from './SqlReviewDialog.svelte';
	import DbMenu, { type MenuItem } from './DbMenu.svelte';
	import * as db from '$lib/ipc/db';
	import type { ResultSet, Statement } from '$lib/ipc/db';
	import * as st from '$lib/state/db.svelte';
	import { engineInfo } from '$lib/db/engines';
	import { openAIPanel, sendMessage } from '$lib/state/ai-chat.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		tab: Extract<st.DbTab, { kind: 'query' }>;
		active: boolean;
	}

	let { tab, active }: Props = $props();

	// The starting text only; after that the editor owns it.
	let sql = $state(untrack(() => tab.initialSql ?? ''));
	let editor = $state<EditorApi>();
	let completion = $state<Record<string, string[]>>({});
	let running = $state(false);
	let results = $state<ResultSet[]>([]);
	let messages = $state<{ ok: boolean; text: string; sql?: string }[]>([]);
	let resultTab = $state(0);
	let pending = $state<{ script: string; statements: Statement[] } | null>(null);
	let showHistory = $state(false);
	let history = $state<st.HistoryEntry[]>([]);
	let split = $state(45);
	let exportMenu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

	let conn = $derived(st.getConnection(tab.connId));
	let open = $derived(st.getOpen(tab.connId));
	let engine = $derived(open?.engine ?? conn?.engine ?? 'postgres');
	let info = $derived(engineInfo(engine));

	onMount(() => {
		history = st.readHistory(tab.connId);
	});

	// Completion follows the tab's database and schema.
	$effect(() => {
		const { connId, database, schema } = tab;
		db.completionColumns(connId, database, schema)
			.then((c) => (completion = c))
			.catch(() => (completion = {}));
	});

	$effect(() => {
		st.setBusy(tab.id, running);
	});

	$effect(() => {
		if (active) queueMicrotask(() => editor?.focus());
	});

	/** The selection, the statement under the cursor, or everything. */
	async function scriptFor(mode: 'current' | 'all'): Promise<string> {
		if (mode === 'all') return sql;
		const sel = editor?.selection();
		if (sel?.trim()) return sel;
		const cursor = editor?.cursor() ?? 0;
		const statements = await db.analyze(engine, sql);
		// The one the cursor is in, or else the last one before it.
		const hit =
			statements.find((s) => cursor >= s.from && cursor <= s.to) ??
			[...statements].reverse().find((s) => s.to <= cursor) ??
			statements[0];
		return hit?.sql ?? '';
	}

	async function run(mode: 'current' | 'all', confirmed = false, script?: string): Promise<void> {
		if (running) return;
		const text = script ?? (await scriptFor(mode));
		if (!text.trim()) return;
		running = true;
		const started = performance.now();
		try {
			const out = await db.execute(tab.connId, tab.database, tab.id, text, confirmed, 1000);
			if (out.status === 'confirm') {
				pending = { script: text, statements: out.statements };
				return;
			}
			pending = null;
			results = out.results.filter((r) => r.columns.length > 0);
			resultTab = results.length ? 0 : -1;
			messages = out.results.map((r) => ({
				ok: true,
				sql: r.statement,
				text:
					r.columns.length > 0
						? t('db.msg_rows', { n: r.rows.length, more: r.truncated ? '+' : '', ms: r.elapsedMs })
						: t('db.msg_affected', { n: r.rowsAffected ?? 0, ms: r.elapsedMs })
			}));
			if (out.error) {
				messages.push({ ok: false, sql: out.error.statement, text: out.error.message });
				if (!results.length) resultTab = -1;
			}
			// A changed structure should show in the tree without a manual refresh.
			if (out.results.some((r) => /^\s*(create|drop|alter|rename)\b/i.test(r.statement))) st.invalidate(tab.connId, tab.database);
			st.pushHistory(tab.connId, { sql: text, at: Date.now(), ms: Math.round(performance.now() - started), ok: !out.error, database: tab.database });
			history = st.readHistory(tab.connId);
		} catch (e) {
			messages = [{ ok: false, text: String(e) }];
			results = [];
			resultTab = -1;
		} finally {
			running = false;
		}
	}

	async function stop(): Promise<void> {
		try {
			await db.cancel(tab.connId, tab.database, tab.id);
		} catch (e) {
			addToast(String(e), 'error');
		}
	}

	async function explain(): Promise<void> {
		const text = await scriptFor('current');
		if (!text.trim()) return;
		const prefix = engine === 'sqlite' ? 'EXPLAIN QUERY PLAN ' : 'EXPLAIN ';
		run('current', false, prefix + text.trim().replace(/;$/, ''));
	}

	async function askAI(kind: 'explain' | 'optimize'): Promise<void> {
		const text = (await scriptFor('current')).trim() || sql.trim();
		if (!text) return;
		const ask =
			kind === 'explain'
				? `Explain what this ${info.name} SQL does, step by step, and point out anything risky:\n\n${text}`
				: `Suggest how to make this ${info.name} query faster. Mention indexes that would help and show the rewritten SQL:\n\n${text}`;
		openAIPanel();
		await sendMessage(ask);
	}

	function useHistory(h: st.HistoryEntry): void {
		editor?.insert(h.sql);
	}

	function setDatabase(v: string): void {
		st.retargetQuery(tab.id, v, null);
	}

	function openExport(e: MouseEvent): void {
		const statement = results[resultTab]?.statement;
		if (!statement) return;
		exportMenu = {
			x: e.clientX,
			y: e.clientY,
			items: (['csv', 'json', 'sql'] as const).map((f) => ({ label: t(`db.export_${f}`), action: () => exportResult(statement, f) }))
		};
	}

	async function exportResult(statement: string, format: 'csv' | 'json' | 'sql'): Promise<void> {
		const path = await saveDialog({ defaultPath: `result.${format}`, filters: [{ name: format.toUpperCase(), extensions: [format] }] });
		if (!path) return;
		try {
			const n = await db.exportData(tab.connId, tab.database, tab.schema, null, statement, format, path);
			addToast(t('db.exported', { n }), 'success');
		} catch (e) {
			addToast(String(e), 'error', 6000);
		}
	}

	function startSplit(e: PointerEvent): void {
		const box = (e.currentTarget as HTMLElement).parentElement!.getBoundingClientRect();
		const onMove = (ev: PointerEvent) => (split = Math.min(85, Math.max(15, ((ev.clientY - box.top) / box.height) * 100)));
		const onUp = () => {
			window.removeEventListener('pointermove', onMove);
			window.removeEventListener('pointerup', onUp);
		};
		window.addEventListener('pointermove', onMove);
		window.addEventListener('pointerup', onUp);
	}

	function ago(at: number): string {
		const s = Math.round((Date.now() - at) / 1000);
		if (s < 60) return t('db.just_now');
		if (s < 3600) return t('db.minutes_ago', { n: Math.round(s / 60) });
		if (s < 86400) return t('db.hours_ago', { n: Math.round(s / 3600) });
		return new Date(at).toLocaleDateString();
	}
</script>

<div class="query">
	<div class="runbar">
		<select class="dbpick" value={tab.database} onchange={(e) => setDatabase(e.currentTarget.value)} title={t('db.database')}>
			{#each open?.databases ?? [tab.database] as d (d)}
				<option value={d}>{d}</option>
			{/each}
		</select>
		<button class="btn primary" onclick={() => run('current')} disabled={running} title="Ctrl+Enter">
			<FaIcon icon={faPlay} /> <span>{t('db.run')}</span>
		</button>
		<button class="btn" onclick={() => run('all')} disabled={running} title="Ctrl+Shift+Enter">
			<FaIcon icon={faForwardFast} /> <span>{t('db.run_all')}</span>
		</button>
		{#if running}
			<button class="btn danger" onclick={stop} disabled={engine === 'sqlite'}>
				<FaIcon icon={faStop} /> <span>{t('db.stop')}</span>
			</button>
		{/if}
		<button class="btn" onclick={explain} disabled={running || engine === 'mssql'} title={t('db.explain_hint')}>
			<FaIcon icon={faDiagramProject} /> <span>{t('db.explain')}</span>
		</button>
		<span class="spacer"></span>
		<button class="btn ghost" onclick={() => askAI('explain')} title={t('db.ai_explain')}>
			<FaIcon icon={faLightbulb} />
		</button>
		<button class="btn ghost" onclick={() => askAI('optimize')} title={t('db.ai_optimize')}>
			<FaIcon icon={faWandMagicSparkles} />
		</button>
		<button class="btn ghost" class:on={showHistory} onclick={() => (showHistory = !showHistory)} title={t('db.history')}>
			<FaIcon icon={faClockRotateLeft} />
		</button>
		{#if conn?.production}<span class="pill prod">{t('db.production')}</span>{/if}
		{#if conn?.readOnly}<span class="pill">{t('db.read_only')}</span>{/if}
	</div>

	<div class="body">
		<div class="main">
			<div class="editor-pane" style:height="{split}%">
				<SqlEditor
					value={sql}
					dialect={info.dialect}
					schema={completion}
					defaultSchema={tab.schema}
					placeholder={t('db.editor_placeholder')}
					onchange={(v) => (sql = v)}
					onrun={(mode) => run(mode)}
					bind:api={editor}
				/>
			</div>
			<div class="splitter" role="separator" aria-orientation="horizontal" onpointerdown={startSplit}></div>
			<div class="results">
				<div class="result-tabs">
					{#each results as r, i (i)}
						<button class="rtab" class:on={resultTab === i} onclick={() => (resultTab = i)}>
							{t('db.result_n', { n: i + 1 })} <span class="muted">{r.rows.length}{r.truncated ? '+' : ''}</span>
						</button>
					{/each}
					<button class="rtab" class:on={resultTab === -1} onclick={() => (resultTab = -1)}>
						{t('db.messages')}
						{#if messages.some((m) => !m.ok)}<span class="err-dot"></span>{/if}
					</button>
					<span class="spacer"></span>
					{#if resultTab >= 0 && results[resultTab]}
						{#if results[resultTab].truncated}<span class="muted small">{t('db.first_n_rows', { n: results[resultTab].rows.length })}</span>{/if}
						<button class="btn ghost small" onclick={openExport}><FaIcon icon={faFileExport} /> {t('db.export')}</button>
					{/if}
				</div>
				<div class="result-body">
					{#if resultTab >= 0 && results[resultTab]}
						<DataGrid columns={results[resultTab].columns} rows={results[resultTab].rows.map((cells) => ({ cells, original: cells }))} />
					{:else}
						<div class="messages">
							{#if !messages.length}
								<p class="hint">{t('db.run_hint')}</p>
							{/if}
							{#each messages as m, i (i)}
								<div class="msg" class:bad={!m.ok}>
									{#if m.sql}<code>{m.sql.length > 160 ? m.sql.slice(0, 160) + '…' : m.sql}</code>{/if}
									<span>{m.text}</span>
								</div>
							{/each}
						</div>
					{/if}
				</div>
			</div>
		</div>

		{#if showHistory}
			<aside class="history">
				<div class="history-head">
					<strong>{t('db.history')}</strong>
					<button class="link" onclick={() => { st.clearHistory(tab.connId); history = []; }}>{t('db.clear')}</button>
				</div>
				<div class="history-list">
					{#each history as h, i (i)}
						<button class="hitem" class:bad={!h.ok} onclick={() => useHistory(h)} title={h.sql}>
							<code>{h.sql.length > 140 ? h.sql.slice(0, 140) + '…' : h.sql}</code>
							<span class="muted small">{h.database} · {ago(h.at)} · {h.ms} ms</span>
						</button>
					{:else}
						<p class="hint">{t('db.history_empty')}</p>
					{/each}
				</div>
			</aside>
		{/if}
	</div>
</div>

<SqlReviewDialog
	open={!!pending}
	title={t('db.confirm_run')}
	intro={conn?.production ? t('db.confirm_run_prod') : t('db.confirm_run_intro')}
	statements={pending?.statements ?? []}
	confirmLabel={t('db.run_anyway')}
	danger
	typeToConfirm={conn?.production && pending?.statements.some((s) => s.risk.kind === 'danger') ? conn.name : null}
	onconfirm={() => pending && run('all', true, pending.script)}
	onclose={() => (pending = null)}
/>

{#if exportMenu}
	<DbMenu x={exportMenu.x} y={exportMenu.y} items={exportMenu.items} onclose={() => (exportMenu = null)} />
{/if}

<style>
	.query {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
	}

	.runbar {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 6px 8px;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		flex-wrap: wrap;
	}

	.dbpick {
		max-width: 180px;
		padding: 4px 6px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: var(--text-xs);
	}

	.btn {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 4px 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: transparent;
		color: var(--color-text-primary);
		font-size: var(--text-xs);
		cursor: pointer;
	}

	.btn:hover:not(:disabled) {
		background: var(--color-surface-hover);
	}

	.btn:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.btn.primary {
		background: var(--color-accent);
		border-color: var(--color-accent);
		color: white;
	}

	.btn.danger {
		color: var(--color-danger);
		border-color: color-mix(in srgb, var(--color-danger) 50%, transparent);
	}

	.btn.ghost {
		border-color: transparent;
		color: var(--color-text-secondary);
	}

	.btn.ghost.on {
		color: var(--color-accent);
	}

	.btn.small {
		padding: 2px 8px;
	}

	.pill {
		padding: 1px 6px;
		border-radius: 3px;
		font-size: 10px;
		font-weight: 600;
		background: var(--color-surface-hover);
		color: var(--color-text-secondary);
	}

	.pill.prod {
		background: color-mix(in srgb, var(--color-danger) 20%, transparent);
		color: var(--color-danger);
	}

	.spacer {
		flex: 1;
	}

	.body {
		flex: 1;
		display: flex;
		min-height: 0;
	}

	.main {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
	}

	.editor-pane {
		min-height: 60px;
	}

	.splitter {
		height: 5px;
		cursor: row-resize;
		background: var(--color-border);
		flex-shrink: 0;
	}

	.results {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-height: 0;
	}

	.result-tabs {
		display: flex;
		align-items: center;
		gap: 2px;
		padding: 0 6px;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		overflow-x: auto;
	}

	.rtab {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 6px 10px;
		border: none;
		border-bottom: 2px solid transparent;
		background: transparent;
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		cursor: pointer;
		white-space: nowrap;
	}

	.rtab.on {
		color: var(--color-text-primary);
		border-bottom-color: var(--color-accent);
	}

	.muted {
		color: var(--color-text-tertiary);
	}

	.small {
		font-size: 10px;
	}

	.err-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--color-danger);
	}

	.result-body {
		flex: 1;
		min-height: 0;
	}

	.messages {
		height: 100%;
		overflow: auto;
		padding: 10px;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.msg {
		display: flex;
		flex-direction: column;
		gap: 3px;
		padding: 6px 10px;
		border-left: 3px solid #30d158;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.msg.bad {
		border-left-color: var(--color-danger);
		color: var(--color-danger);
		white-space: pre-wrap;
	}

	.msg code,
	.hitem code {
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		white-space: pre-wrap;
		word-break: break-word;
	}

	.hint {
		color: var(--color-text-tertiary);
		font-size: var(--text-sm);
		margin: 8px 2px;
	}

	.history {
		width: 280px;
		display: flex;
		flex-direction: column;
		border-left: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		min-height: 0;
	}

	.history-head {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 8px 10px;
		font-size: var(--text-xs);
		border-bottom: 1px solid var(--color-border);
	}

	.history-list {
		flex: 1;
		overflow: auto;
	}

	.hitem {
		display: flex;
		flex-direction: column;
		gap: 4px;
		width: 100%;
		padding: 8px 10px;
		border: none;
		border-bottom: 1px solid var(--color-border);
		background: transparent;
		text-align: left;
		font-size: var(--text-xs);
		cursor: pointer;
	}

	.hitem:hover {
		background: var(--color-surface-hover);
	}

	.hitem.bad code {
		color: var(--color-danger);
	}

	.link {
		border: none;
		background: none;
		color: var(--color-accent);
		cursor: pointer;
		font-size: var(--text-xs);
	}

	@media (max-width: 720px) {
		.btn span {
			display: none;
		}

		.history {
			position: absolute;
			right: 0;
			top: 0;
			bottom: 0;
			z-index: 5;
			box-shadow: var(--shadow-elevated);
		}
	}
</style>
