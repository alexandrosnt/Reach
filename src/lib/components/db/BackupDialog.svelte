<script lang="ts">
	/**
	 * Backup to a .sql file and restore from one. Both run in the background
	 * and report progress; both work on every platform because Reach writes
	 * and reads the SQL itself rather than calling pg_dump or mysqldump.
	 */
	import { save as saveDialog, open as openDialog } from '@tauri-apps/plugin-dialog';
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import Toggle from '$lib/components/shared/Toggle.svelte';
	import * as db from '$lib/ipc/db';
	import type { JobProgress, JobSummary } from '$lib/ipc/db';
	import * as st from '$lib/state/db.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		mode: 'backup' | 'restore';
		connId: string;
		database: string;
		schema: string | null;
		onclose: () => void;
	}

	let { mode, connId, database, schema, onclose }: Props = $props();

	let structure = $state(true);
	let data = $state(true);
	let views = $state(true);
	let routines = $state(true);
	let pickTables = $state(false);
	let chosen = $state<Record<string, boolean>>({});
	let keepGoing = $state(false);
	let path = $state('');

	let job = $state<string | null>(null);
	let progress = $state<JobProgress | null>(null);
	let summary = $state<JobSummary | null>(null);
	let error = $state<string | null>(null);

	let conn = $derived(st.getConnection(connId));
	let tables = $derived((st.getObjects(connId, database, schema) ?? []).filter((o) => o.kind === 'table').map((o) => o.name));
	let running = $derived(!!job && !summary && !error);

	$effect(() => {
		if (mode === 'backup' && !st.getObjects(connId, database, schema)) st.loadObjects(connId, database, schema);
	});

	async function choosePath(): Promise<void> {
		if (mode === 'backup') {
			const stamp = new Date().toISOString().slice(0, 16).replace(/[T:]/g, '-');
			const picked = await saveDialog({ defaultPath: `${schema ?? database}-${stamp}.sql`, filters: [{ name: 'SQL', extensions: ['sql'] }] });
			if (picked) path = picked;
		} else {
			const picked = await openDialog({ multiple: false, filters: [{ name: 'SQL', extensions: ['sql'] }, { name: '*', extensions: ['*'] }] });
			if (typeof picked === 'string') path = picked;
		}
	}

	async function start(): Promise<void> {
		error = null;
		summary = null;
		progress = null;
		try {
			job =
				mode === 'backup'
					? await db.backup(connId, database, schema, { tables: pickTables ? tables.filter((x) => chosen[x]) : null, data, structure, views, routines }, path)
					: await db.restore(connId, database, path, keepGoing);
			summary = await db.followJob(job, (p) => (progress = p));
			if (mode === 'restore') st.invalidate(connId, database);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function stop(): Promise<void> {
		if (job) await db.cancelJob(job);
	}

	let pct = $derived(progress && progress.total ? Math.round((progress.done / progress.total) * 100) : 0);
</script>

<Modal open {onclose} title={mode === 'backup' ? t('db.backup_title', { name: schema ?? database }) : t('db.restore_title', { name: database })} maxWidth="520px">
	{#snippet children()}
		<div class="body">
			{#if mode === 'backup'}
				<p class="intro">{t('db.backup_intro')}</p>
				<div class="opts">
					<label><input type="checkbox" bind:checked={structure} /> {t('db.backup_structure')}</label>
					<label><input type="checkbox" bind:checked={data} /> {t('db.backup_data')}</label>
					<label><input type="checkbox" bind:checked={views} /> {t('db.views')}</label>
					<label><input type="checkbox" bind:checked={routines} /> {t('db.backup_routines')}</label>
				</div>
				<div class="row">
					<span>{t('db.only_some_tables')}</span>
					<Toggle hideLabel label={t('db.only_some_tables')} checked={pickTables} onchange={(v) => (pickTables = v)} />
				</div>
				{#if pickTables}
					<div class="tables">
						{#each tables as name (name)}
							<label><input type="checkbox" bind:checked={chosen[name]} /> {name}</label>
						{/each}
					</div>
				{/if}
			{:else}
				<p class="intro">{t('db.restore_intro', { db: database })}</p>
				{#if conn?.production}<p class="warn">{t('db.restore_prod_warn')}</p>{/if}
				<div class="row">
					<span>{t('db.keep_going')}</span>
					<Toggle hideLabel label={t('db.keep_going')} checked={keepGoing} onchange={(v) => (keepGoing = v)} />
				</div>
			{/if}

			<div class="file">
				<input readonly value={path} placeholder={mode === 'backup' ? t('db.choose_where') : t('db.choose_file')} />
				<Button variant="secondary" size="sm" onclick={choosePath} disabled={running}>{t('db.browse')}</Button>
			</div>

			{#if progress && !summary && !error}
				<div class="progress">
					<div class="bar"><div style:width="{pct}%"></div></div>
					<span>{progress.step} · {progress.done}/{progress.total}{progress.rows ? ` · ${progress.rows.toLocaleString()} ${t('db.rows')}` : ''}</span>
				</div>
			{/if}
			{#if summary}
				<div class="done">
					{mode === 'backup'
						? t('db.backup_done', { tables: summary.tables, rows: summary.rows.toLocaleString() })
						: t('db.restore_done', { statements: summary.statements, rows: summary.rows.toLocaleString() })}
					{#if summary.failed.length}
						<details>
							<summary>{t('db.n_failed', { n: summary.failed.length })}</summary>
							<pre>{summary.failed.join('\n')}</pre>
						</details>
					{/if}
				</div>
			{/if}
			{#if error}<div class="error">{error}</div>{/if}
		</div>
	{/snippet}
	{#snippet actions()}
		{#if running}
			<Button variant="secondary" onclick={stop}>{t('db.stop')}</Button>
		{:else if summary}
			<Button variant="primary" onclick={onclose}>{t('db.done')}</Button>
		{:else}
			<Button variant="secondary" onclick={onclose}>{t('db.cancel')}</Button>
			<Button variant={mode === 'restore' && conn?.production ? 'danger' : 'primary'} disabled={!path || (mode === 'backup' && !structure && !data)} onclick={start}>
				{mode === 'backup' ? t('db.start_backup') : t('db.start_restore')}
			</Button>
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

	.intro {
		margin: 0;
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		line-height: 1.5;
	}

	.warn {
		margin: 0;
		color: var(--color-danger);
		font-size: var(--text-xs);
	}

	.opts {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 6px;
	}

	label {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--color-text-primary);
	}

	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.tables {
		max-height: 180px;
		overflow: auto;
		padding: 6px 8px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
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

	.progress {
		display: flex;
		flex-direction: column;
		gap: 5px;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.bar {
		height: 6px;
		border-radius: 3px;
		background: var(--color-surface-hover);
		overflow: hidden;
	}

	.bar div {
		height: 100%;
		background: var(--color-accent);
		transition: width 0.2s;
	}

	.done {
		padding: 8px 10px;
		border-radius: var(--radius-sm);
		background: color-mix(in srgb, #30d158 12%, transparent);
		color: var(--color-text-primary);
		font-size: var(--text-xs);
	}

	.done pre {
		max-height: 160px;
		overflow: auto;
		white-space: pre-wrap;
		font-size: 11px;
		color: var(--color-danger);
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
