<script lang="ts">
	/**
	 * CSV into a table. The file's first rows are shown so the columns can be
	 * matched by eye; matching names are paired up automatically. The import
	 * is one transaction: all rows or none.
	 */
	import { onMount } from 'svelte';
	import { open as openDialog } from '@tauri-apps/plugin-dialog';
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import * as db from '$lib/ipc/db';
	import type { JobProgress, JobSummary } from '$lib/ipc/db';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		connId: string;
		database: string;
		schema: string | null;
		table: string;
		onclose: () => void;
		ondone?: () => void;
	}

	let { connId, database, schema, table, onclose, ondone }: Props = $props();

	let path = $state('');
	let delimiter = $state(',');
	let header = $state(true);
	let emptyAsNull = $state(true);
	let preview = $state<string[][]>([]);
	let mapping = $state<(string | null)[]>([]);
	let tableColumns = $state<string[]>([]);
	let job = $state<string | null>(null);
	let progress = $state<JobProgress | null>(null);
	let summary = $state<JobSummary | null>(null);
	let error = $state<string | null>(null);

	onMount(async () => {
		try {
			const d = await db.tableDesign(connId, database, schema, table);
			tableColumns = d.columns.filter((c) => !c.generated).map((c) => c.name);
		} catch (e) {
			error = String(e);
		}
	});

	async function choose(): Promise<void> {
		const picked = await openDialog({ multiple: false, filters: [{ name: 'CSV', extensions: ['csv', 'tsv', 'txt'] }] });
		if (typeof picked !== 'string') return;
		path = picked;
		if (picked.toLowerCase().endsWith('.tsv')) delimiter = '\t';
		await load();
	}

	async function load(): Promise<void> {
		if (!path) return;
		try {
			preview = await db.csvPreview(path, delimiter);
			const heads = preview[0] ?? [];
			mapping = heads.map((h) => {
				const match = tableColumns.find((c) => c.toLowerCase() === h.trim().toLowerCase());
				return header ? (match ?? null) : null;
			});
			error = null;
		} catch (e) {
			error = String(e);
		}
	}

	async function start(): Promise<void> {
		error = null;
		try {
			job = await db.importCsv(connId, database, schema, table, { path, delimiter, header, mapping, emptyAsNull });
			summary = await db.followJob(job, (p) => (progress = p));
			ondone?.();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
			job = null;
		}
	}

	let running = $derived(!!job && !summary && !error);
	let body = $derived(header ? preview.slice(1, 6) : preview.slice(0, 5));
</script>

<Modal open {onclose} title={t('db.import_title', { table })} maxWidth="760px">
	{#snippet children()}
		<div class="body">
			<div class="file">
				<input readonly value={path} placeholder={t('db.choose_csv')} />
				<Button variant="secondary" size="sm" onclick={choose} disabled={running}>{t('db.browse')}</Button>
			</div>
			<div class="opts">
				<label>
					{t('db.delimiter')}
					<select bind:value={delimiter} onchange={load}>
						<option value=",">,</option>
						<option value=";">;</option>
						<option value={'\t'}>Tab</option>
						<option value="|">|</option>
					</select>
				</label>
				<label><input type="checkbox" bind:checked={header} onchange={load} /> {t('db.first_row_header')}</label>
				<label><input type="checkbox" bind:checked={emptyAsNull} /> {t('db.empty_as_null')}</label>
			</div>

			{#if preview.length}
				<div class="map">
					<table>
						<thead>
							<tr>
								{#each preview[0] as h, i (i)}
									<th>
										<div class="csvcol">{header ? h : t('db.column_n', { n: i + 1 })}</div>
										<select bind:value={mapping[i]}>
											<option value={null}>{t('db.skip')}</option>
											{#each tableColumns as c (c)}<option value={c}>{c}</option>{/each}
										</select>
									</th>
								{/each}
							</tr>
						</thead>
						<tbody>
							{#each body as row, r (r)}
								<tr>{#each row as v, i (i)}<td>{v}</td>{/each}</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}

			{#if progress && !summary}
				<div class="progress">{t('db.importing', { done: progress.done.toLocaleString(), total: progress.total.toLocaleString() })}</div>
			{/if}
			{#if summary}<div class="done">{t('db.imported', { n: summary.rows.toLocaleString() })}</div>{/if}
			{#if error}<div class="error">{error}</div>{/if}
		</div>
	{/snippet}
	{#snippet actions()}
		{#if running}
			<Button variant="secondary" onclick={() => job && db.cancelJob(job)}>{t('db.stop')}</Button>
		{:else if summary}
			<Button variant="primary" onclick={onclose}>{t('db.done')}</Button>
		{:else}
			<Button variant="secondary" onclick={onclose}>{t('db.cancel')}</Button>
			<Button variant="primary" disabled={!path || !mapping.some(Boolean)} onclick={start}>{t('db.start_import')}</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	.body {
		display: flex;
		flex-direction: column;
		gap: 12px;
		font-size: var(--text-sm);
	}

	.file {
		display: flex;
		gap: 8px;
	}

	.file input {
		flex: 1;
		padding: 6px 8px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.opts {
		display: flex;
		flex-wrap: wrap;
		gap: 16px;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.opts label {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	select {
		padding: 3px 6px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: var(--text-xs);
	}

	.map {
		overflow: auto;
		max-height: 300px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
	}

	table {
		border-collapse: collapse;
		font-size: var(--text-xs);
	}

	th {
		position: sticky;
		top: 0;
		padding: 6px;
		background: var(--color-bg-elevated);
		border-bottom: 1px solid var(--color-border);
		text-align: left;
		vertical-align: top;
	}

	.csvcol {
		margin-bottom: 4px;
		color: var(--color-text-secondary);
		font-weight: 600;
		white-space: nowrap;
	}

	td {
		padding: 4px 6px;
		border-bottom: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
		white-space: nowrap;
		max-width: 200px;
		overflow: hidden;
		text-overflow: ellipsis;
		font-family: var(--font-mono);
	}

	.progress {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.done {
		padding: 8px 10px;
		border-radius: var(--radius-sm);
		background: color-mix(in srgb, #30d158 12%, transparent);
		font-size: var(--text-xs);
	}

	.error {
		padding: 8px 10px;
		border-radius: var(--radius-sm);
		background: color-mix(in srgb, var(--color-danger) 12%, transparent);
		color: var(--color-danger);
		font-size: var(--text-xs);
		white-space: pre-wrap;
	}
</style>
