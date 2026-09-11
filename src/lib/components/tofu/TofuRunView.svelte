<!--
	A run you can read instead of scroll.

	While `plan`, `apply` and `destroy` speak JSON, this shows what they said:
	the change summary as the header, the planned changes as a list, apply
	progress as rows that move from running to done, diagnostics as cards
	with a file and line, outputs as a table. Commands that speak prose —
	init, validate, fmt — get the raw log directly, and the raw log is always
	one click away for the rest. Structured first, raw always available.
-->
<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import {
		getRunView,
		getRunExitCode,
		getRunCommand,
		isCommandRunning,
		getCommandOutput
	} from '$lib/state/tofu.svelte';
	import TofuCommandOutput from './TofuCommandOutput.svelte';
	import type { TofuChangeAction } from '$lib/tofu/ui-json';

	let view = $derived(getRunView());
	let exitCode = $derived(getRunExitCode());
	let command = $derived(getRunCommand());
	let running = $derived(isCommandRunning());
	let hasOutput = $derived(getCommandOutput().length > 0);

	/** The person's choice, kept across runs. */
	let showRaw = $state(false);

	let structured = $derived(view.structured && !showRaw);

	function glyph(a: TofuChangeAction): string {
		switch (a) {
			case 'create':
				return '+';
			case 'update':
				return '~';
			case 'delete':
				return '−';
			case 'replace':
				return '±';
			case 'read':
				return '‹';
			case 'move':
				return '→';
			case 'import':
				return '↓';
			default:
				return '·';
		}
	}

	function actionLabel(a: TofuChangeAction): string {
		switch (a) {
			case 'create':
				return t('tofu.plan_create');
			case 'update':
				return t('tofu.plan_update');
			case 'delete':
				return t('tofu.plan_delete');
			case 'replace':
				return t('tofu.plan_replace');
			default:
				return a;
		}
	}

	/**
	 * What the exit code means for *this* command. `plan -detailed-exitcode`
	 * returns 2 for "changes pending", which is success with news, not failure.
	 */
	let verdict = $derived.by(() => {
		if (running || exitCode === null) return null;
		if (command === 'plan') {
			if (exitCode === 0) return { tone: 'ok', text: t('tofu.plan_no_changes') };
			if (exitCode === 2) return { tone: 'changes', text: t('tofu.run_changes_pending') };
			return { tone: 'error', text: t('tofu.run_failed', { code: exitCode }) };
		}
		if (exitCode === 0) return { tone: 'ok', text: t('tofu.run_succeeded') };
		return { tone: 'error', text: t('tofu.run_failed', { code: exitCode }) };
	});

	let summaryText = $derived.by(() => {
		const s = view.summary;
		if (!s) return null;
		return t('tofu.run_summary', { add: s.add, change: s.change, remove: s.remove });
	});

	let outputEntries = $derived(Object.entries(view.outputs));

	function fmtValue(v: unknown): string {
		if (typeof v === 'string') return v;
		try {
			return JSON.stringify(v);
		} catch {
			return String(v);
		}
	}
</script>

<div class="run-view">
	<div class="head">
		<div class="head-left">
			<span class="title">{t('tofu.output')}</span>
			{#if view.tofuVersion}
				<span class="chip mono">OpenTofu {view.tofuVersion}</span>
			{/if}
			{#if running}
				<span class="chip live"><span class="spinner"></span>{t('tofu.command_running')}</span>
			{:else if verdict}
				<span class="chip verdict-{verdict.tone}">{verdict.text}</span>
			{/if}
		</div>
		{#if view.structured && hasOutput}
			<button class="toggle" onclick={() => (showRaw = !showRaw)}>
				{showRaw ? t('tofu.run_show_structured') : t('tofu.run_show_raw')}
			</button>
		{/if}
	</div>

	{#if !structured}
		<TofuCommandOutput headless />
	{:else}
		<div class="sections">
			{#if view.summary}
				<div class="summary">
					<span class="n add">+{view.summary.add}</span>
					<span class="n upd">~{view.summary.change}</span>
					<span class="n del">−{view.summary.remove}</span>
					{#if view.summary.import > 0}<span class="n imp">↓{view.summary.import}</span>{/if}
					<span class="summary-text">{summaryText}</span>
				</div>
			{/if}

			{#if view.diagnostics.length > 0}
				<div class="section">
					<span class="section-title">{t('tofu.run_diagnostics')}</span>
					{#each view.diagnostics as d, i (i)}
						<div class="diag {d.severity === 'error' ? 'error' : 'warning'}">
							<span class="diag-summary">{d.summary}</span>
							{#if d.detail}<span class="diag-detail">{d.detail}</span>{/if}
							{#if d.location}<span class="diag-loc mono">{d.location}</span>{/if}
						</div>
					{/each}
				</div>
			{/if}

			{#if view.apply.length > 0}
				<div class="section">
					<span class="section-title">{t('tofu.run_progress')}</span>
					{#each view.apply as row (row.resource.addr)}
						<div class="row state-{row.state}">
							<span class="st"></span>
							<span class="glyph act-{row.action}">{glyph(row.action)}</span>
							<span class="addr mono">{row.resource.addr}</span>
							<span class="meta">
								{#if row.state === 'done' && row.id}<span class="mono dim">{row.id}</span>{/if}
								{#if row.state === 'errored'}<span class="err">{t('tofu.run_errored')}</span>{/if}
								{#if row.elapsed > 0}<span class="dim">{t('tofu.run_elapsed', { s: row.elapsed })}</span>{/if}
							</span>
						</div>
					{/each}
				</div>
			{/if}

			{#if view.planned.length > 0}
				<div class="section">
					<span class="section-title">{t('tofu.run_planned')}</span>
					{#each view.planned as p (p.resource.addr)}
						<div class="row">
							<span class="glyph act-{p.action}">{glyph(p.action)}</span>
							<span class="addr mono">{p.resource.addr}</span>
							<span class="meta"><span class="dim">{actionLabel(p.action)}</span>{#if p.reason}<span class="dim mono">{p.reason}</span>{/if}</span>
						</div>
					{/each}
				</div>
			{/if}

			{#if view.drift.length > 0}
				<div class="section">
					<span class="section-title">{t('tofu.run_drift')}</span>
					{#each view.drift as d (d.resource.addr)}
						<div class="row">
							<span class="glyph act-{d.action}">{glyph(d.action)}</span>
							<span class="addr mono">{d.resource.addr}</span>
							<span class="meta"><span class="dim">{actionLabel(d.action)}</span></span>
						</div>
					{/each}
				</div>
			{/if}

			{#if outputEntries.length > 0}
				<div class="section">
					<span class="section-title">{t('tofu.outputs_title')}</span>
					{#each outputEntries as [name, o] (name)}
						<div class="row">
							<span class="addr mono">{name}</span>
							<span class="meta"><span class="mono">{o.sensitive ? t('tofu.plan_sensitive_value') : fmtValue(o.value)}</span></span>
						</div>
					{/each}
				</div>
			{/if}

			{#if view.refreshed > 0 && view.planned.length === 0 && view.apply.length === 0 && !view.summary}
				<p class="dim">{t('tofu.run_refreshed', { count: view.refreshed })}</p>
			{/if}
		</div>
	{/if}
</div>

<style>
	.run-view {
		display: flex;
		flex-direction: column;
		gap: 8px;
		height: 100%;
		min-height: 0;
	}

	.head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 10px;
		flex-shrink: 0;
	}

	.head-left {
		display: flex;
		align-items: center;
		gap: 8px;
		min-width: 0;
		flex-wrap: wrap;
	}

	.title {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.chip {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 2px 8px;
		border-radius: 10px;
		font-size: 0.6875rem;
		border: 1px solid var(--color-border);
		color: var(--color-text-secondary);
	}

	.chip.live {
		color: var(--color-accent);
		border-color: color-mix(in srgb, var(--color-accent) 45%, var(--color-border));
	}

	.chip.verdict-ok {
		color: var(--color-success, #10b981);
		border-color: color-mix(in srgb, var(--color-success, #10b981) 45%, var(--color-border));
	}

	.chip.verdict-changes {
		color: var(--color-warning, #f59e0b);
		border-color: color-mix(in srgb, var(--color-warning, #f59e0b) 45%, var(--color-border));
	}

	.chip.verdict-error {
		color: var(--color-danger);
		border-color: color-mix(in srgb, var(--color-danger) 45%, var(--color-border));
	}

	.spinner {
		width: 10px;
		height: 10px;
		border: 2px solid color-mix(in srgb, var(--color-accent) 30%, transparent);
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.toggle {
		flex-shrink: 0;
		padding: 4px 10px;
		font-family: inherit;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		cursor: pointer;
	}

	.toggle:hover {
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
	}

	.sections {
		flex: 1;
		min-height: 120px;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 14px;
		padding: 10px 12px;
		border-radius: var(--radius-btn);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		scrollbar-width: thin;
	}

	.summary {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: 0.8125rem;
	}

	.n {
		font-family: var(--font-mono, monospace);
		font-weight: 600;
		font-variant-numeric: tabular-nums;
	}

	.add,
	.act-create,
	.act-import {
		color: var(--color-success, #10b981);
	}

	.upd,
	.act-update,
	.act-move {
		color: var(--color-warning, #f59e0b);
	}

	.del,
	.act-delete,
	.act-replace {
		color: var(--color-danger);
	}

	.imp {
		color: var(--color-accent);
	}

	.summary-text {
		color: var(--color-text-secondary);
	}

	.section {
		display: flex;
		flex-direction: column;
		gap: 3px;
	}

	.section-title {
		font-size: 0.625rem;
		font-weight: 600;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: var(--color-text-tertiary);
		margin-bottom: 3px;
	}

	.row {
		display: grid;
		grid-template-columns: auto auto minmax(0, 1fr) auto;
		align-items: center;
		gap: 8px;
		padding: 5px 8px;
		border-radius: 6px;
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		font-size: 0.75rem;
	}

	/* Rows without a state dot (plan, drift, outputs) collapse the first column. */
	.row > .st:first-child {
		display: block;
	}

	.row:not(.state-running):not(.state-done):not(.state-errored) {
		grid-template-columns: auto minmax(0, 1fr) auto;
	}

	.st {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--color-text-tertiary);
	}

	.state-running .st {
		background: var(--color-accent);
		box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-accent) 25%, transparent);
	}

	.state-done .st {
		background: var(--color-success, #10b981);
	}

	.state-errored .st {
		background: var(--color-danger);
	}

	.glyph {
		width: 12px;
		text-align: center;
		font-family: var(--font-mono, monospace);
		font-weight: 700;
	}

	.addr {
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.meta {
		display: flex;
		align-items: center;
		gap: 8px;
		justify-self: end;
		font-size: 0.6875rem;
	}

	.mono {
		font-family: var(--font-mono, monospace);
	}

	.dim {
		color: var(--color-text-tertiary);
	}

	.err {
		color: var(--color-danger);
	}

	.diag {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 8px 10px;
		border-left: 3px solid var(--color-danger);
		border-radius: 0 6px 6px 0;
		background: color-mix(in srgb, var(--color-danger) 8%, transparent);
		font-size: 0.75rem;
	}

	.diag.warning {
		border-left-color: var(--color-warning, #f59e0b);
		background: color-mix(in srgb, var(--color-warning, #f59e0b) 8%, transparent);
	}

	.diag-summary {
		color: var(--color-text-primary);
		font-weight: 500;
	}

	.diag-detail {
		color: var(--color-text-secondary);
		white-space: pre-wrap;
	}

	.diag-loc {
		font-size: 0.6875rem;
		color: var(--color-text-tertiary);
	}

	p.dim {
		margin: 0;
		font-size: 0.75rem;
	}
</style>
