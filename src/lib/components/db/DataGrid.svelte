<script lang="ts" module>
	export interface GridRow {
		cells: (string | null)[];
		/** Values as loaded; absent for a row being inserted. */
		original?: (string | null)[];
		deleted?: boolean;
	}
</script>

<script lang="ts">
	/**
	 * A spreadsheet-like grid for results and table data. Only the rows in
	 * view are in the DOM, so a page of thousands scrolls smoothly. Editing is
	 * optional and never touches the database itself: the grid reports each
	 * change and the owner decides what it becomes.
	 */
	import type { ColumnInfo } from '$lib/ipc/db';
	import DbMenu, { type MenuItem } from './DbMenu.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		columns: ColumnInfo[];
		rows: GridRow[];
		editable?: boolean;
		/** Current sort, for the header arrows. */
		sort?: { column: string; desc: boolean }[];
		/** Row number of the first row (for paged tables). */
		offset?: number;
		onsort?: (column: string, add: boolean) => void;
		onedit?: (row: number, col: number, value: string | null) => void;
		ondeleterows?: (rows: number[]) => void;
		onduplicate?: (row: number) => void;
		/** Extra items for the cell menu (e.g. "Filter by this value"). */
		cellMenu?: (row: number, col: number) => MenuItem[];
	}

	let { columns, rows, editable = false, sort = [], offset = 0, onsort, onedit, ondeleterows, onduplicate, cellMenu }: Props = $props();

	const ROW_H = 26;
	const OVERSCAN = 12;

	let scroller = $state<HTMLDivElement>();
	let scrollTop = $state(0);
	let viewHeight = $state(400);
	let widths = $state<number[]>([]);

	// Selection: an anchor cell and the cell the range extends to.
	let anchor = $state<[number, number] | null>(null);
	let focus = $state<[number, number] | null>(null);
	let editing = $state<{ r: number; c: number; value: string; isNull: boolean } | null>(null);
	let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

	// Widths from the header and a sample of rows, recomputed when the
	// column set changes (a new query), kept when only rows change.
	let columnKey = $derived(columns.map((c) => c.name).join('\u0000'));
	$effect(() => {
		void columnKey;
		const sample = rows.slice(0, 60);
		widths = columns.map((c, i) => {
			const longest = Math.max(c.name.length + 2, ...sample.map((r) => Math.min(60, (r.cells[i] ?? 'NULL').length)));
			return Math.max(70, Math.min(420, longest * 7.4 + 20));
		});
	});

	let first = $derived(Math.max(0, Math.floor(scrollTop / ROW_H) - OVERSCAN));
	let last = $derived(Math.min(rows.length, Math.ceil((scrollTop + viewHeight) / ROW_H) + OVERSCAN));
	let visible = $derived(rows.slice(first, last));
	let template = $derived(`52px ${widths.map((w) => `${w}px`).join(' ')}`);

	function inRange(r: number, c: number): boolean {
		if (!anchor || !focus) return false;
		const [r1, r2] = [Math.min(anchor[0], focus[0]), Math.max(anchor[0], focus[0])];
		const [c1, c2] = [Math.min(anchor[1], focus[1]), Math.max(anchor[1], focus[1])];
		return r >= r1 && r <= r2 && c >= c1 && c <= c2;
	}

	function selectedRows(): number[] {
		if (!anchor || !focus) return [];
		const out = [];
		for (let r = Math.min(anchor[0], focus[0]); r <= Math.max(anchor[0], focus[0]); r++) out.push(r);
		return out;
	}

	function onCellDown(e: MouseEvent, r: number, c: number): void {
		if (e.button !== 0) return;
		if (editing && (editing.r !== r || editing.c !== c)) commit();
		if (e.shiftKey && anchor) focus = [r, c];
		else {
			anchor = [r, c];
			focus = [r, c];
		}
		scroller?.focus();
	}

	function onRowHeader(e: MouseEvent, r: number): void {
		if (e.shiftKey && anchor) focus = [r, columns.length - 1];
		else {
			anchor = [r, 0];
			focus = [r, columns.length - 1];
		}
		scroller?.focus();
	}

	function changed(row: GridRow, c: number): boolean {
		return !!row.original && row.original[c] !== row.cells[c];
	}

	function startEdit(r: number, c: number, initial?: string): void {
		if (!editable || rows[r]?.deleted) return;
		const v = rows[r].cells[c];
		editing = { r, c, value: initial ?? v ?? '', isNull: initial === undefined && v === null };
	}

	function commit(): void {
		if (!editing) return;
		const { r, c, value, isNull } = editing;
		editing = null;
		const next = isNull && value === '' ? null : value;
		if (rows[r] && rows[r].cells[c] !== next) onedit?.(r, c, next);
	}

	function cancelEdit(): void {
		editing = null;
		scroller?.focus();
	}

	function move(dr: number, dc: number, extend: boolean): void {
		const [r, c] = focus ?? [0, 0];
		const next: [number, number] = [Math.max(0, Math.min(rows.length - 1, r + dr)), Math.max(0, Math.min(columns.length - 1, c + dc))];
		focus = next;
		if (!extend) anchor = next;
		ensureVisible(next[0]);
	}

	function ensureVisible(r: number): void {
		if (!scroller) return;
		const top = r * ROW_H;
		const headerH = ROW_H;
		if (top < scroller.scrollTop) scroller.scrollTop = top;
		else if (top + ROW_H > scroller.scrollTop + scroller.clientHeight - headerH) scroller.scrollTop = top + ROW_H - scroller.clientHeight + headerH;
	}

	function copy(): void {
		if (!anchor || !focus) return;
		const lines: string[] = [];
		const [c1, c2] = [Math.min(anchor[1], focus[1]), Math.max(anchor[1], focus[1])];
		for (const r of selectedRows()) {
			const cells = [];
			for (let c = c1; c <= c2; c++) cells.push((rows[r].cells[c] ?? '').replace(/[\t\n]/g, ' '));
			lines.push(cells.join('\t'));
		}
		// A whole-row selection copies with its header, ready for a sheet.
		if (c1 === 0 && c2 === columns.length - 1 && lines.length > 1) lines.unshift(columns.map((c) => c.name).join('\t'));
		navigator.clipboard.writeText(lines.join('\n'));
	}

	function onKey(e: KeyboardEvent): void {
		if (editing) return;
		const mod = e.ctrlKey || e.metaKey;
		switch (e.key) {
			case 'ArrowDown':
				move(1, 0, e.shiftKey);
				break;
			case 'ArrowUp':
				move(-1, 0, e.shiftKey);
				break;
			case 'ArrowRight':
				move(0, 1, e.shiftKey);
				break;
			case 'ArrowLeft':
				move(0, -1, e.shiftKey);
				break;
			case 'Tab':
				move(0, e.shiftKey ? -1 : 1, false);
				break;
			case 'PageDown':
				move(Math.floor(viewHeight / ROW_H) - 1, 0, e.shiftKey);
				break;
			case 'PageUp':
				move(-Math.floor(viewHeight / ROW_H) + 1, 0, e.shiftKey);
				break;
			case 'Enter':
			case 'F2':
				if (focus) startEdit(focus[0], focus[1]);
				break;
			case 'Delete':
				if (editable && focus && !mod) {
					if (anchor && focus && anchor[1] === 0 && focus[1] === columns.length - 1) ondeleterows?.(selectedRows());
					else onedit?.(focus[0], focus[1], null);
				}
				break;
			default:
				if (mod && e.key.toLowerCase() === 'c') copy();
				else if (mod && e.key.toLowerCase() === 'a') {
					anchor = [0, 0];
					focus = [rows.length - 1, columns.length - 1];
				} else if (editable && focus && e.key.length === 1 && !mod && !e.altKey) {
					startEdit(focus[0], focus[1], e.key);
				} else return;
		}
		e.preventDefault();
	}

	function onEditKey(e: KeyboardEvent): void {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			commit();
			move(1, 0, false);
			scroller?.focus();
		} else if (e.key === 'Tab') {
			e.preventDefault();
			commit();
			move(0, e.shiftKey ? -1 : 1, false);
			scroller?.focus();
		} else if (e.key === 'Escape') {
			e.preventDefault();
			cancelEdit();
		}
	}

	function openMenu(e: MouseEvent, r: number, c: number): void {
		e.preventDefault();
		if (!inRange(r, c)) {
			anchor = [r, c];
			focus = [r, c];
		}
		const items: MenuItem[] = [
			{ label: t('db.copy'), hint: 'Ctrl+C', action: copy },
			{ label: t('db.copy_value'), action: () => navigator.clipboard.writeText(rows[r].cells[c] ?? '') }
		];
		if (editable && !rows[r].deleted) {
			items.push({ separator: true, label: '' });
			items.push({ label: t('db.edit_cell'), hint: 'Enter', action: () => startEdit(r, c) });
			items.push({ label: t('db.set_null'), action: () => onedit?.(r, c, null) });
			items.push({ label: t('db.set_empty'), action: () => onedit?.(r, c, '') });
			items.push({ separator: true, label: '' });
			items.push({ label: t('db.duplicate_row'), action: () => onduplicate?.(r) });
			const sel = selectedRows();
			items.push({ label: sel.length > 1 ? t('db.delete_rows', { n: sel.length }) : t('db.delete_row'), danger: true, action: () => ondeleterows?.(sel.length ? sel : [r]) });
		}
		if (cellMenu) {
			const extra = cellMenu(r, c);
			if (extra.length) items.push({ separator: true, label: '' }, ...extra);
		}
		menu = { x: e.clientX, y: e.clientY, items };
	}

	// Column resize by dragging the header's right edge.
	function startResize(e: PointerEvent, i: number): void {
		e.preventDefault();
		e.stopPropagation();
		const x0 = e.clientX;
		const w0 = widths[i];
		const onMove = (ev: PointerEvent) => (widths[i] = Math.max(40, w0 + ev.clientX - x0));
		const onUp = () => {
			window.removeEventListener('pointermove', onMove);
			window.removeEventListener('pointerup', onUp);
		};
		window.addEventListener('pointermove', onMove);
		window.addEventListener('pointerup', onUp);
	}

	function sortOf(name: string): { desc: boolean; n: number } | null {
		const i = sort.findIndex((s) => s.column === name);
		return i < 0 ? null : { desc: sort[i].desc, n: i + 1 };
	}

	function display(v: string | null): string {
		if (v === null) return 'NULL';
		return v.length > 300 ? v.slice(0, 300) + '…' : v.replace(/\r?\n/g, '⏎ ');
	}

	function autofocus(node: HTMLTextAreaElement): void {
		node.focus();
		node.setSelectionRange(node.value.length, node.value.length);
	}
</script>

<div
	class="grid"
	role="grid"
	tabindex="0"
	aria-rowcount={rows.length}
	aria-colcount={columns.length}
	bind:this={scroller}
	bind:clientHeight={viewHeight}
	onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
	onkeydown={onKey}
>
	<div class="inner" style:height="{(rows.length + 1) * ROW_H}px" style:width="calc(52px + {widths.reduce((a, b) => a + b, 0)}px)">
		<div class="header" role="row" style:grid-template-columns={template}>
			<div class="rownum corner"></div>
			{#each columns as col, i (i)}
				{@const s = sortOf(col.name)}
				<div
					class="th"
					role="columnheader"
					title="{col.name} · {col.typeName}"
					aria-sort={s ? (s.desc ? 'descending' : 'ascending') : 'none'}
				>
					<button class="th-btn" disabled={!onsort} onclick={(e) => onsort?.(col.name, e.shiftKey)}>
						<span class="name">{col.name}</span>
						{#if col.typeName}<span class="type">{col.typeName}</span>{/if}
						{#if s}<span class="arrow">{s.desc ? '▼' : '▲'}{sort.length > 1 ? s.n : ''}</span>{/if}
					</button>
					<span class="resize" role="separator" aria-orientation="vertical" onpointerdown={(e) => startResize(e, i)}></span>
				</div>
			{/each}
		</div>

		{#each visible as row, vi (first + vi)}
			{@const r = first + vi}
			<div
				class="tr"
				role="row"
				class:deleted={row.deleted}
				class:inserted={!row.original}
				style:top="{(r + 1) * ROW_H}px"
				style:grid-template-columns={template}
			>
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<div class="rownum" role="rowheader" tabindex="-1" onclick={(e) => onRowHeader(e, r)}>
					{#if !row.original}+{:else if row.deleted}−{:else}{offset + r + 1}{/if}
				</div>
				{#each row.cells as v, c (c)}
					<!-- svelte-ignore a11y_click_events_have_key_events -->
					<div
						class="td"
						role="gridcell"
						tabindex="-1"
						class:null={v === null}
						class:selected={inRange(r, c)}
						class:focus={focus?.[0] === r && focus?.[1] === c}
						class:changed={changed(row, c)}
						onmousedown={(e) => onCellDown(e, r, c)}
						onmouseenter={(e) => e.buttons === 1 && anchor && (focus = [r, c])}
						ondblclick={() => startEdit(r, c)}
						oncontextmenu={(e) => openMenu(e, r, c)}
					>
						{#if editing && editing.r === r && editing.c === c}
							<textarea
								class="editor"
								rows="1"
								bind:value={editing.value}
								oninput={() => editing && (editing.isNull = false)}
								onkeydown={onEditKey}
								onblur={commit}
								use:autofocus
							></textarea>
						{:else}
							{display(v)}
						{/if}
					</div>
				{/each}
			</div>
		{/each}
	</div>

	{#if !rows.length}
		<div class="empty">{t('db.no_rows')}</div>
	{/if}
</div>

{#if menu}
	<DbMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
	.grid {
		position: relative;
		height: 100%;
		overflow: auto;
		outline: none;
		font-size: var(--text-sm);
		background: var(--color-bg-primary);
		font-variant-numeric: tabular-nums;
	}

	.inner {
		position: relative;
		min-width: 100%;
	}

	.header {
		position: sticky;
		top: 0;
		z-index: 2;
		display: grid;
		height: 26px;
		background: var(--color-bg-elevated);
		border-bottom: 1px solid var(--color-border);
	}

	.th {
		position: relative;
		display: flex;
		align-items: center;
		border-right: 1px solid var(--color-border);
		min-width: 0;
	}

	.th-btn {
		display: flex;
		align-items: center;
		gap: 6px;
		width: 100%;
		height: 100%;
		padding: 0 8px;
		border: none;
		background: transparent;
		color: var(--color-text-primary);
		font-size: var(--text-xs);
		font-weight: 600;
		text-align: left;
		cursor: pointer;
		min-width: 0;
	}

	.th-btn:disabled {
		cursor: default;
	}

	.th-btn .name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.th-btn .type {
		color: var(--color-text-tertiary);
		font-weight: 400;
		font-size: 10px;
		white-space: nowrap;
	}

	.arrow {
		margin-left: auto;
		color: var(--color-accent);
		font-size: 9px;
	}

	.resize {
		position: absolute;
		top: 0;
		right: -3px;
		width: 6px;
		height: 100%;
		cursor: col-resize;
		z-index: 1;
	}

	.tr {
		position: absolute;
		left: 0;
		right: 0;
		display: grid;
		height: 26px;
	}

	.rownum {
		position: sticky;
		left: 0;
		z-index: 1;
		display: flex;
		align-items: center;
		justify-content: flex-end;
		padding: 0 8px;
		background: var(--color-bg-elevated);
		color: var(--color-text-tertiary);
		font-size: 11px;
		border-right: 1px solid var(--color-border);
		border-bottom: 1px solid var(--color-border);
		cursor: pointer;
		user-select: none;
	}

	.corner {
		z-index: 3;
		border-bottom: none;
	}

	.td {
		position: relative;
		padding: 0 8px;
		line-height: 25px;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		border-right: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
		border-bottom: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
		color: var(--color-text-primary);
		cursor: cell;
		user-select: none;
	}

	.td.null {
		color: var(--color-text-tertiary);
		font-style: italic;
	}

	.td.selected {
		background: color-mix(in srgb, var(--color-accent) 16%, transparent);
	}

	.td.focus {
		box-shadow: inset 0 0 0 2px var(--color-accent);
	}

	.td.changed {
		background: color-mix(in srgb, #ffd60a 18%, transparent);
	}

	.tr.inserted .td {
		background: color-mix(in srgb, #30d158 12%, transparent);
	}

	.tr.deleted .td {
		text-decoration: line-through;
		color: var(--color-text-tertiary);
		background: color-mix(in srgb, var(--color-danger) 10%, transparent);
	}

	.editor {
		position: absolute;
		inset: 0;
		width: 100%;
		min-height: 100%;
		padding: 3px 6px;
		border: 2px solid var(--color-accent);
		background: var(--color-bg-elevated);
		color: var(--color-text-primary);
		font: inherit;
		resize: none;
		outline: none;
		z-index: 3;
	}

	.empty {
		position: absolute;
		top: 60px;
		left: 0;
		right: 0;
		text-align: center;
		color: var(--color-text-tertiary);
		font-size: var(--text-sm);
	}
</style>
