<script lang="ts">
	/**
	 * A table's rows, a page at a time, editable when the table can name its
	 * rows. Edits collect locally — changed cells tinted, new rows green,
	 * deleted ones struck through — and become SQL only on Save, which shows
	 * the statements first and runs them as one transaction.
	 */
	import { onMount } from 'svelte';
	import FaIcon from '$lib/components/shared/FaIcon.svelte';
	import { faRotate, faPlus, faFilter, faFloppyDisk, faRotateLeft, faPenRuler, faChevronLeft, faChevronRight } from '@fortawesome/free-solid-svg-icons';
	import DataGrid, { type GridRow } from './DataGrid.svelte';
	import SqlReviewDialog from './SqlReviewDialog.svelte';
	import * as db from '$lib/ipc/db';
	import type { ColumnInfo, RowEdit, SortKey, TableDesign } from '$lib/ipc/db';
	import * as st from '$lib/state/db.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		tab: Extract<st.DbTab, { kind: 'table' }>;
		onExport: () => void;
		onImport: () => void;
	}

	let { tab, onExport, onImport }: Props = $props();

	let design = $state<TableDesign | null>(null);
	let columns = $state<ColumnInfo[]>([]);
	let rows = $state<GridRow[]>([]);
	let total = $state<number | null>(null);
	let offset = $state(0);
	let limit = $state(200);
	let sort = $state<SortKey[]>([]);
	let filter = $state('');
	let appliedFilter = $state('');
	let loading = $state(false);
	let error = $state<string | null>(null);
	let review = $state<{ statements: string[]; edits: RowEdit[] } | null>(null);
	let applying = $state(false);
	let applyError = $state<string | null>(null);

	let conn = $derived(st.getConnection(tab.connId));
	let key = $derived(design?.primaryKey ?? []);
	let editable = $derived(!!design && key.length > 0 && !conn?.readOnly);
	let edits = $derived(buildEdits());
	let dirty = $derived(edits.length > 0);

	$effect(() => {
		st.setDirty(tab.id, dirty);
	});

	onMount(async () => {
		try {
			design = await db.tableDesign(tab.connId, tab.database, tab.schema, tab.table);
		} catch {
			// A view, or a table the user cannot describe: rows still load.
			design = null;
		}
		await load();
	});

	async function load(): Promise<void> {
		if (dirty && !confirm(t('db.discard_confirm'))) return;
		loading = true;
		error = null;
		try {
			const page = await db.tableRows(tab.connId, tab.database, tab.schema, tab.table, offset, limit, sort, appliedFilter || null);
			columns = page.result.columns;
			rows = page.result.rows.map((cells) => ({ cells: [...cells], original: cells }));
			if (page.total !== null) total = page.total;
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	}

	function applyFilter(): void {
		appliedFilter = filter.trim();
		offset = 0;
		total = null;
		load();
	}

	function onsort(column: string, add: boolean): void {
		const i = sort.findIndex((s) => s.column === column);
		if (add) {
			if (i < 0) sort = [...sort, { column, desc: false }];
			else if (!sort[i].desc) sort[i].desc = true;
			else sort = sort.filter((s) => s.column !== column);
		} else {
			sort = i >= 0 && sort.length === 1 ? (sort[0].desc ? [] : [{ column, desc: true }]) : [{ column, desc: false }];
		}
		offset = 0;
		load();
	}

	function page(dir: number): void {
		offset = Math.max(0, offset + dir * limit);
		load();
	}

	function onedit(r: number, c: number, value: string | null): void {
		rows[r].cells[c] = value;
	}

	function ondeleterows(indices: number[]): void {
		const drop = new Set(indices.filter((i) => !rows[i].original));
		for (const i of indices) if (rows[i].original) rows[i].deleted = !rows[i].deleted;
		if (drop.size) rows = rows.filter((_, i) => !drop.has(i));
	}

	function autoColumns(): Set<number> {
		const auto = new Set<number>();
		columns.forEach((c, i) => {
			const def = design?.columns.find((d) => d.name === c.name);
			if (def?.autoIncrement || def?.generated) auto.add(i);
		});
		return auto;
	}

	function addRow(): void {
		rows = [...rows, { cells: columns.map(() => null) }];
	}

	function onduplicate(r: number): void {
		const auto = autoColumns();
		rows = [...rows, { cells: rows[r].cells.map((v, i) => (auto.has(i) ? null : v)) }];
	}

	function buildEdits(): RowEdit[] {
		const out: RowEdit[] = [];
		const keyIdx = key.map((k) => columns.findIndex((c) => c.name === k));
		const keyOf = (orig: (string | null)[]) => keyIdx.map((i, n) => ({ column: key[n], value: orig[i] ?? null }));
		const auto = autoColumns();
		for (const row of rows) {
			if (!row.original) {
				// Leave empty cells out so the column's default applies.
				const values = row.cells.flatMap((v, i) => (v === null || (auto.has(i) && v === '') ? [] : [{ column: columns[i].name, value: v }]));
				out.push({ kind: 'insert', values });
			} else if (row.deleted) {
				out.push({ kind: 'delete', key: keyOf(row.original) });
			} else {
				const values = row.cells.flatMap((v, i) => (v !== row.original![i] ? [{ column: columns[i].name, value: v }] : []));
				if (values.length) out.push({ kind: 'update', key: keyOf(row.original), values });
			}
		}
		return out;
	}

	async function openReview(): Promise<void> {
		try {
			const statements = await db.previewEdits(tab.connId, tab.database, tab.schema, tab.table, edits);
			applyError = null;
			review = { statements, edits };
		} catch (e) {
			addToast(String(e), 'error', 6000);
		}
	}

	async function applyChanges(): Promise<void> {
		if (!review) return;
		applying = true;
		applyError = null;
		try {
			await db.applyEdits(tab.connId, tab.database, tab.schema, tab.table, review.edits);
			review = null;
			addToast(t('db.saved_changes'), 'success');
			// Nothing is pending now; clear before reloading so the reload does
			// not ask whether to discard what was just saved.
			rows = [];
			await load();
		} catch (e) {
			applyError = String(e);
		} finally {
			applying = false;
		}
	}

	function discard(): void {
		rows = rows.filter((r) => r.original).map((r) => ({ cells: [...r.original!], original: r.original }));
	}

	function cellMenu(r: number, c: number) {
		const v = rows[r].cells[c];
		const col = columns[c].name;
		const q = (n: string) => {
			const e = st.engineOf(tab.connId);
			return e === 'mysql' || e === 'mariadb' ? '`' + n.replace(/`/g, '``') + '`' : e === 'mssql' ? '[' + n.replace(/]/g, ']]') + ']' : '"' + n.replace(/"/g, '""') + '"';
		};
		const cond = v === null ? `${q(col)} IS NULL` : `${q(col)} = '${v.replace(/'/g, "''")}'`;
		return [
			{
				label: t('db.filter_by_value'),
				action: () => {
					filter = cond;
					applyFilter();
				}
			}
		];
	}

	let from = $derived(rows.length ? offset + 1 : 0);
	let to = $derived(offset + rows.filter((r) => r.original).length);
</script>

<div class="table-tab">
	<div class="toolbar">
		<div class="filter">
			<FaIcon icon={faFilter} />
			<input
				bind:value={filter}
				placeholder={t('db.filter_placeholder')}
				spellcheck="false"
				onkeydown={(e) => e.key === 'Enter' && applyFilter()}
			/>
			{#if appliedFilter}
				<button class="clear" onclick={() => { filter = ''; applyFilter(); }} aria-label={t('db.clear')}>×</button>
			{/if}
		</div>
		<button class="btn" onclick={() => load()} disabled={loading} title={t('db.refresh')}><FaIcon icon={faRotate} /></button>
		{#if editable}
			<button class="btn" onclick={addRow} title={t('db.add_row')}><FaIcon icon={faPlus} /> <span>{t('db.add_row')}</span></button>
		{/if}
		{#if dirty}
			<button class="btn primary" onclick={openReview}><FaIcon icon={faFloppyDisk} /> <span>{t('db.save_n', { n: edits.length })}</span></button>
			<button class="btn" onclick={discard} title={t('db.discard')}><FaIcon icon={faRotateLeft} /></button>
		{/if}
		<span class="spacer"></span>
		{#if design}
			<button class="btn ghost" onclick={() => st.openDesigner(tab.connId, tab.database, tab.schema, tab.table)} title={t('db.design_table')}>
				<FaIcon icon={faPenRuler} />
			</button>
		{/if}
		<button class="btn ghost" onclick={onImport} disabled={!editable}>{t('db.import')}</button>
		<button class="btn ghost" onclick={onExport}>{t('db.export')}</button>
	</div>

	{#if design && !key.length && !conn?.readOnly}
		<div class="banner">{t('db.no_key_read_only')}</div>
	{/if}
	{#if error}
		<div class="banner bad">{error}</div>
	{/if}

	<div class="grid-wrap" class:loading>
		<DataGrid {columns} {rows} {editable} {sort} {offset} {onsort} {onedit} {ondeleterows} {onduplicate} {cellMenu} />
	</div>

	<div class="pager">
		<span class="muted">
			{#if total !== null}
				{t('db.rows_range', { from, to, total })}
			{:else}
				{t('db.rows_range_open', { from, to })}
			{/if}
		</span>
		<span class="spacer"></span>
		<select bind:value={limit} onchange={() => { offset = 0; load(); }}>
			{#each [100, 200, 500, 1000] as n (n)}<option value={n}>{t('db.per_page', { n })}</option>{/each}
		</select>
		<button class="btn" onclick={() => page(-1)} disabled={offset === 0 || loading} aria-label={t('db.previous')}><FaIcon icon={faChevronLeft} /></button>
		<button class="btn" onclick={() => page(1)} disabled={loading || rows.filter((r) => r.original).length < limit} aria-label={t('db.next')}><FaIcon icon={faChevronRight} /></button>
	</div>
</div>

<SqlReviewDialog
	open={!!review}
	title={t('db.review_changes')}
	intro={t('db.review_intro')}
	statements={review?.statements ?? []}
	confirmLabel={t('db.apply')}
	danger={conn?.production}
	typeToConfirm={conn?.production && review?.edits.some((e) => e.kind === 'delete') ? conn.name : null}
	busy={applying}
	error={applyError}
	onconfirm={applyChanges}
	onclose={() => (review = null)}
/>

<style>
	.table-tab {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
	}

	.toolbar,
	.pager {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 6px 8px;
		background: var(--color-bg-elevated);
		border-bottom: 1px solid var(--color-border);
		font-size: var(--text-xs);
	}

	.pager {
		border-bottom: none;
		border-top: 1px solid var(--color-border);
	}

	.filter {
		flex: 1;
		max-width: 520px;
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 0 8px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-tertiary);
	}

	.filter input {
		flex: 1;
		min-width: 0;
		padding: 5px 0;
		border: none;
		background: transparent;
		color: var(--color-text-primary);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		outline: none;
	}

	.clear {
		border: none;
		background: none;
		color: var(--color-text-tertiary);
		cursor: pointer;
		font-size: 14px;
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
		opacity: 0.45;
		cursor: default;
	}

	.btn.primary {
		background: var(--color-accent);
		border-color: var(--color-accent);
		color: white;
	}

	.btn.ghost {
		border-color: transparent;
		color: var(--color-text-secondary);
	}

	select {
		padding: 3px 6px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: var(--text-xs);
	}

	.spacer {
		flex: 1;
	}

	.muted {
		color: var(--color-text-tertiary);
	}

	.banner {
		padding: 6px 10px;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		background: color-mix(in srgb, var(--color-warning, #ffd60a) 10%, transparent);
		border-bottom: 1px solid var(--color-border);
	}

	.banner.bad {
		color: var(--color-danger);
		background: color-mix(in srgb, var(--color-danger) 10%, transparent);
		white-space: pre-wrap;
	}

	.grid-wrap {
		flex: 1;
		min-height: 0;
	}

	.grid-wrap.loading {
		opacity: 0.6;
	}

	@media (max-width: 720px) {
		.btn span {
			display: none;
		}
	}
</style>
