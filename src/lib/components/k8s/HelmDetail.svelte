<script lang="ts">
	/**
	 * A Helm release: its notes, the values it was installed with (and the
	 * chart's defaults, folded away), its history and its manifest. All read
	 * from the cluster directly.
	 *
	 * Rollback and uninstall are Helm's own work (hooks, ordering), so they
	 * run the `helm` program on the machine the cluster is reached through,
	 * and the page says so before anyone presses them.
	 */
	import { untrack } from 'svelte';
	import StatusPill from '#lib/components/devops/StatusPill.svelte';
	import ConfirmAction from '#lib/components/devops/ConfirmAction.svelte';
	import ErrorHelp from '#lib/components/devops/ErrorHelp.svelte';
	import DetailHeader from './DetailHeader.svelte';
	import ActionButton from './ActionButton.svelte';
	import Tabs from './Tabs.svelte';
	import { k8sHelmDetail, k8sHelmHistory, k8sHelmRun, type ReleaseDetail, type Revision } from '#lib/ipc/k8s.js';
	import { ago } from '#lib/devops/status.js';
	import { busy } from '#lib/state/k8s.svelte.js';
	import { addToast } from '#lib/state/toasts.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { helmStatus, type Ctx } from './model';

	interface Props {
		ctx: Ctx;
		row: Revision;
		/** Where helm runs: the SSH session's name, or empty for this computer. */
		via: string;
		narrow: boolean;
		tick: number;
		onclose: () => void;
		onchanged: () => void;
	}

	let { ctx, row, via, narrow, tick, onclose, onchanged }: Props = $props();

	let tab = $state('overview');
	let detail = $state<ReleaseDetail | null>(null);
	let detailError = $state('');
	let history = $state<Revision[]>([]);
	let historyError = $state('');
	let pending = $state<{ action: 'rollback'; revision: number } | { action: 'uninstall' } | null>(null);
	let running = $state(false);
	let output = $state<{ ok: boolean; text: string } | null>(null);

	// The release's current revision changes after a rollback: reload then too.
	let current = $derived(`${ctx.key}\n${row.namespace}\n${row.name}\n${row.revision}`);
	let previousName = '';

	$effect(() => {
		void current;
		untrack(() => {
			const name = `${row.namespace}/${row.name}`;
			if (name !== previousName) {
				tab = 'overview';
				output = null;
				history = [];
				previousName = name;
			}
			loadDetail();
		});
	});

	async function loadDetail(): Promise<void> {
		const { key } = ctx;
		const { namespace, name, revision } = row;
		try {
			const d = await k8sHelmDetail(key, namespace, name, revision);
			if (row.name === name && row.revision === revision) {
				detail = d;
				detailError = '';
			}
		} catch (e) {
			detailError = String(e);
		}
	}

	async function loadHistory(): Promise<void> {
		try {
			history = (await k8sHelmHistory(ctx.key, row.namespace, row.name)).sort((a, b) => b.revision - a.revision);
			historyError = '';
		} catch (e) {
			historyError = String(e);
		}
	}

	$effect(() => {
		void tick;
		void current;
		if (tab === 'history') untrack(loadHistory);
	});

	async function run(): Promise<void> {
		const p = pending;
		if (!p) return;
		running = true;
		output = null;
		try {
			const text = await busy(() =>
				k8sHelmRun(ctx.clusterId, p.action, row.namespace, row.name, p.action === 'rollback' ? p.revision : null)
			);
			output = { ok: true, text: text.trim() };
			addToast(
				p.action === 'rollback' ? t('k8s.helm_rolled_back', { name: row.name, revision: p.revision }) : t('k8s.helm_uninstalled_toast', { name: row.name }),
				'success'
			);
			if (p.action === 'uninstall') onclose();
			onchanged();
			if (p.action === 'rollback') loadHistory();
		} catch (e) {
			output = { ok: false, text: String(e) };
		} finally {
			running = false;
		}
	}

	function updated(r: Revision): string {
		return ago(r.updated) || r.updated;
	}
</script>

<div class="detail">
	<DetailHeader kind="Helm" name={row.name} namespace={row.namespace} status={helmStatus(row.status)} {narrow} readOnly={ctx.readOnly} {onclose}>
		{#snippet actions()}
			<ActionButton variant="danger" locked={ctx.readOnly} disabled={running} onclick={() => (pending = { action: 'uninstall' })}>
				{t('k8s.helm_uninstall')}
			</ActionButton>
			{#if running}<span class="running">{t('k8s.helm_running')}</span>{/if}
		{/snippet}
	</DetailHeader>

	<p class="where">{via ? t('k8s.helm_where_session', { session: via }) : t('k8s.helm_where_local')}</p>

	{#if output}
		<div class="output" class:bad={!output.ok}>
			<div class="output-head">
				<strong>{output.ok ? t('k8s.helm_output') : t('k8s.helm_failed_run')}</strong>
				<button type="button" class="dismiss" onclick={() => (output = null)} aria-label={t('k8s.dismiss')}>×</button>
			</div>
			{#if output.ok}
				<pre>{output.text || t('k8s.helm_no_output')}</pre>
			{:else}
				<ErrorHelp error={output.text} />
			{/if}
		</div>
	{/if}

	<Tabs
		tabs={[
			{ id: 'overview', label: t('k8s.tab_overview') },
			{ id: 'values', label: t('k8s.tab_values') },
			{ id: 'history', label: t('k8s.tab_history') },
			{ id: 'manifest', label: t('k8s.tab_manifest') }
		]}
		active={tab}
		onchange={(id) => (tab = id)}
	/>

	<div class="body">
		{#if detailError && tab !== 'history'}
			<div class="scroll"><ErrorHelp error={detailError} onretry={loadDetail} /></div>
		{:else if tab === 'overview'}
			<div class="scroll">
				<dl class="facts">
					<dt>{t('k8s.helm_chart')}</dt>
					<dd class="mono">{row.chart}-{row.chartVersion}</dd>
					<dt>{t('k8s.helm_app_version')}</dt>
					<dd class="mono">{row.appVersion || '—'}</dd>
					<dt>{t('k8s.helm_revision')}</dt>
					<dd>{row.revision}</dd>
					<dt>{t('k8s.helm_updated')}</dt>
					<dd title={row.updated}>{updated(row)}</dd>
					{#if row.description}
						<dt>{t('k8s.helm_description')}</dt>
						<dd>{row.description}</dd>
					{/if}
				</dl>
				<h3>{t('k8s.helm_notes')}</h3>
				{#if detail?.notes}
					<pre>{detail.notes}</pre>
				{:else}
					<p class="muted">{detail ? t('k8s.helm_no_notes') : t('common.loading')}</p>
				{/if}
			</div>
		{:else if tab === 'values'}
			<div class="scroll">
				<h3>{t('k8s.helm_user_values')}</h3>
				{#if detail?.values.trim() && detail.values.trim() !== '{}'}
					<pre>{detail.values}</pre>
				{:else}
					<p class="muted">{detail ? t('k8s.helm_no_values') : t('common.loading')}</p>
				{/if}
				{#if detail?.chartValues}
					<details>
						<summary>{t('k8s.helm_chart_defaults')}</summary>
						<pre>{detail.chartValues}</pre>
					</details>
				{/if}
			</div>
		{:else if tab === 'history'}
			<div class="scroll">
				{#if historyError}
					<ErrorHelp error={historyError} onretry={loadHistory} />
				{:else if history.length === 0}
					<p class="muted">{t('common.loading')}</p>
				{:else}
					<ul class="history">
						{#each history as h (h.revision)}
							<li class:current={h.revision === row.revision}>
								<div class="h-head">
									<strong>#{h.revision}</strong>
									<StatusPill status={helmStatus(h.status)} compact />
									<span class="mono">{h.chart}-{h.chartVersion}</span>
									{#if h.appVersion}<span class="muted">{t('k8s.helm_app_short', { version: h.appVersion })}</span>{/if}
									<span class="spacer"></span>
									<span class="muted" title={h.updated}>{updated(h)}</span>
								</div>
								{#if h.description}<p class="desc">{h.description}</p>{/if}
								{#if h.revision === row.revision}
									<span class="tag">{t('k8s.helm_current')}</span>
								{:else}
									<div>
										<ActionButton
											locked={ctx.readOnly}
											disabled={running}
											onclick={() => (pending = { action: 'rollback', revision: h.revision })}>{t('k8s.helm_rollback_here')}</ActionButton
										>
									</div>
								{/if}
							</li>
						{/each}
					</ul>
				{/if}
			</div>
		{:else}
			<div class="scroll">
				{#if detail}
					<pre>{detail.manifest}</pre>
				{:else}
					<p class="muted">{t('common.loading')}</p>
				{/if}
			</div>
		{/if}
	</div>
</div>

<ConfirmAction
	open={pending !== null}
	title={pending?.action === 'rollback' ? t('k8s.helm_rollback_title') : t('k8s.helm_uninstall_title')}
	message={pending?.action === 'rollback'
		? t('k8s.helm_rollback_message', { from: row.revision, to: pending.revision })
		: t('k8s.helm_uninstall_message')}
	items={[`${row.namespace}/${row.name}`]}
	where={ctx.clusterName}
	environment={ctx.environment}
	note={(pending?.action === 'rollback' ? t('k8s.helm_rollback_note') : t('k8s.helm_uninstall_note')) +
		' ' +
		(via ? t('k8s.helm_where_session', { session: via }) : t('k8s.helm_where_local'))}
	confirmLabel={pending?.action === 'rollback' ? t('k8s.helm_rollback') : t('k8s.helm_uninstall')}
	typeToConfirm={pending?.action === 'uninstall' || ctx.environment === 'production' ? row.name : undefined}
	onconfirm={run}
	onclose={() => (pending = null)}
/>

<style>
	.detail {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
	}

	.where {
		margin: 0;
		padding: 6px 12px;
		font-size: var(--text-xs);
		line-height: 1.45;
		color: var(--color-text-tertiary);
		border-bottom: 1px solid var(--color-border);
	}

	.running {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.output {
		display: flex;
		flex-direction: column;
		gap: 6px;
		max-height: 40%;
		overflow: auto;
		padding: 8px 12px;
		border-bottom: 1px solid var(--color-border);
		background: color-mix(in srgb, var(--color-success) 6%, var(--color-bg-elevated));
		font-size: var(--text-sm);
		flex-shrink: 0;
	}

	.output.bad {
		background: var(--color-bg-elevated);
	}

	.output-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.dismiss {
		width: 32px;
		height: 32px;
		border: none;
		border-radius: var(--radius-btn);
		background: none;
		color: var(--color-text-secondary);
		font-size: 18px;
		cursor: pointer;
	}

	.dismiss:focus-visible {
		outline: 2px solid var(--color-accent);
	}

	.body {
		flex: 1;
		min-height: 0;
		display: flex;
		flex-direction: column;
	}

	.scroll {
		flex: 1;
		overflow: auto;
		padding: 12px;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.facts {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 6px 16px;
		margin: 0;
		font-size: var(--text-sm);
	}

	dt {
		color: var(--color-text-secondary);
	}

	dd {
		margin: 0;
		color: var(--color-text-primary);
		overflow-wrap: anywhere;
	}

	h3 {
		margin: 4px 0 0;
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--color-text-primary);
	}

	pre {
		margin: 0;
		padding: 10px 12px;
		overflow: auto;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: var(--font-mono);
		font-size: 12px;
		line-height: 1.55;
		white-space: pre;
		user-select: text;
	}

	details summary {
		cursor: pointer;
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
		padding: 4px 0;
	}

	details pre {
		margin-top: 6px;
	}

	.mono {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.muted {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
	}

	.history {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.history li {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 8px 10px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		font-size: var(--text-sm);
	}

	.history li.current {
		border-color: color-mix(in srgb, var(--color-accent) 50%, var(--color-border));
	}

	.h-head {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}

	.spacer {
		flex: 1;
	}

	.desc {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.tag {
		align-self: flex-start;
		padding: 1px 8px;
		border-radius: 999px;
		font-size: var(--text-xs);
		color: var(--color-accent);
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
	}
</style>
