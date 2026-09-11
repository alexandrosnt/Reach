<!--
	A playbook run you can read: plays, tasks, one chip per host.

	Fed by the default callback's lines (see ansible/recap). Each task is a
	row coloured by the worst thing that happened on it, with a chip per host
	— ok, changed, failed, unreachable, skipped, ignored — and a failed host's
	message under the row. The recap is the footer. Commands that produce no
	plays (galaxy, vault, inventory) show the raw log, and the raw log is one
	click away for the rest.
-->
<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import {
		getRunView,
		getRunExitCode,
		isCommandRunning,
		getCommandOutput
	} from '$lib/state/ansible.svelte';
	import { taskStatus, type HostResult } from '$lib/ansible/recap';
	import AnsibleCommandOutput from './AnsibleCommandOutput.svelte';

	let view = $derived(getRunView());
	let exitCode = $derived(getRunExitCode());
	let running = $derived(isCommandRunning());
	let hasOutput = $derived(getCommandOutput().length > 0);

	let showRaw = $state(false);
	let structured = $derived(view.structured && !showRaw);

	let verdict = $derived.by(() => {
		if (running || exitCode === null) return null;
		if (exitCode === 0) return { tone: 'ok', text: t('ansible.run_succeeded') };
		return { tone: 'error', text: t('ansible.run_failed', { code: exitCode }) };
	});

	/** Failed and unreachable results, for the messages under a task. */
	function problems(hosts: HostResult[]): HostResult[] {
		return hosts.filter((h) => (h.status === 'failed' || h.status === 'unreachable') && h.msg);
	}

	let totals = $derived.by(() => {
		const sum = { ok: 0, changed: 0, failed: 0, unreachable: 0, skipped: 0, ignored: 0 };
		for (const r of view.recap) {
			sum.ok += r.ok;
			sum.changed += r.changed;
			sum.failed += r.failed;
			sum.unreachable += r.unreachable;
			sum.skipped += r.skipped;
			sum.ignored += r.ignored;
		}
		return sum;
	});
</script>

<div class="run-view">
	<div class="head">
		<div class="head-left">
			<span class="title">{t('ansible.output')}</span>
			{#if running}
				<span class="chip live"><span class="spinner"></span>{t('ansible.command_running')}</span>
			{:else if verdict}
				<span class="chip verdict-{verdict.tone}">{verdict.text}</span>
			{/if}
		</div>
		{#if view.structured && hasOutput}
			<button class="toggle" onclick={() => (showRaw = !showRaw)}>
				{showRaw ? t('ansible.run_show_structured') : t('ansible.run_show_raw')}
			</button>
		{/if}
	</div>

	{#if !structured}
		<AnsibleCommandOutput headless />
	{:else}
		<div class="sections">
			{#if view.errors.length > 0}
				{#each view.errors as e, i (i)}
					<div class="diag error">{e}</div>
				{/each}
			{/if}
			{#if view.warnings.length > 0}
				{#each view.warnings as w, i (i)}
					<div class="diag warning">{w}</div>
				{/each}
			{/if}

			{#each view.plays as play, pi (pi)}
				<div class="play">
					{#if play.name}<span class="section-title">{t('ansible.run_play')} · {play.name}</span>{/if}
					{#each play.tasks as task, ti (ti)}
						{@const status = taskStatus(task)}
						<div class="task status-{status}">
							<span class="st"></span>
							<span class="name">
								{#if task.role}<span class="role">{task.role} :</span>{/if}
								{task.name}
								{#if task.handler}<span class="dim">· {t('ansible.run_handler')}</span>{/if}
							</span>
							<span class="hosts">
								{#each task.hosts as h, hi (hi)}
									<span class="host host-{h.status}" title={[h.item ? `item=${h.item}` : '', h.msg ?? ''].filter(Boolean).join(' — ')}>{h.host}</span>
								{/each}
								{#if task.hosts.length === 0 && running}<span class="spinner"></span>{/if}
							</span>
						</div>
						{#each problems(task.hosts) as p, pi2 (pi2)}
							<div class="problem status-{p.status}"><span class="mono">{p.host}</span> {p.msg}</div>
						{/each}
					{/each}
				</div>
			{/each}

			{#if view.recap.length > 0}
				<div class="recap">
					<span class="section-title">{t('ansible.run_recap')}</span>
					<div class="recap-totals">
						<span><b>{view.recap.length}</b> {t('ansible.run_hosts')}</span>
						<span class="c-ok"><b>{totals.ok}</b> ok</span>
						<span class="c-changed"><b>{totals.changed}</b> changed</span>
						<span class="c-failed"><b>{totals.failed}</b> failed</span>
						<span class="c-unreachable"><b>{totals.unreachable}</b> unreachable</span>
						<span class="c-skipped"><b>{totals.skipped}</b> skipped</span>
						{#if totals.ignored > 0}<span class="c-ignored"><b>{totals.ignored}</b> ignored</span>{/if}
					</div>
					{#each view.recap as r (r.host)}
						<div class="recap-row" class:bad={r.failed > 0 || r.unreachable > 0}>
							<span class="mono">{r.host}</span>
							<span class="counts">
								<span class="c-ok">{r.ok}</span>
								<span class="c-changed">{r.changed}</span>
								<span class="c-failed">{r.failed}</span>
								<span class="c-unreachable">{r.unreachable}</span>
								<span class="c-skipped">{r.skipped}</span>
							</span>
						</div>
					{/each}
				</div>
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
		flex-shrink: 0;
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

	.section-title {
		font-size: 0.625rem;
		font-weight: 600;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: var(--color-text-tertiary);
		margin-bottom: 3px;
	}

	.play {
		display: flex;
		flex-direction: column;
		gap: 3px;
	}

	.task {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: 8px;
		padding: 5px 8px;
		border-radius: 6px;
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		font-size: 0.75rem;
	}

	.st {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--color-text-tertiary);
	}

	.status-ok .st {
		background: var(--color-success, #10b981);
	}

	.status-changed .st,
	.status-ignored .st {
		background: var(--color-warning, #f59e0b);
	}

	.status-failed .st,
	.status-unreachable .st {
		background: var(--color-danger);
	}

	.status-pending .st {
		background: var(--color-accent);
		box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-accent) 25%, transparent);
	}

	.name {
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.role {
		color: var(--color-text-tertiary);
		margin-right: 4px;
	}

	.dim {
		color: var(--color-text-tertiary);
	}

	.hosts {
		display: flex;
		gap: 4px;
		flex-wrap: wrap;
		justify-content: flex-end;
		max-width: 50%;
	}

	.host {
		font-family: var(--font-mono, monospace);
		font-size: 0.625rem;
		padding: 1px 6px;
		border-radius: 4px;
		background: color-mix(in srgb, var(--color-success, #10b981) 15%, transparent);
		color: var(--color-success, #10b981);
	}

	.host-changed,
	.host-ignored {
		background: color-mix(in srgb, var(--color-warning, #f59e0b) 15%, transparent);
		color: var(--color-warning, #f59e0b);
	}

	.host-failed,
	.host-unreachable {
		background: color-mix(in srgb, var(--color-danger) 15%, transparent);
		color: var(--color-danger);
	}

	.host-skipped {
		background: var(--color-surface-hover);
		color: var(--color-text-tertiary);
	}

	.problem {
		margin: 0 0 2px 24px;
		padding: 4px 8px;
		border-left: 3px solid var(--color-danger);
		border-radius: 0 4px 4px 0;
		background: color-mix(in srgb, var(--color-danger) 8%, transparent);
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		white-space: pre-wrap;
	}

	.mono {
		font-family: var(--font-mono, monospace);
		color: var(--color-text-primary);
	}

	.diag {
		padding: 6px 10px;
		border-left: 3px solid var(--color-warning, #f59e0b);
		border-radius: 0 6px 6px 0;
		background: color-mix(in srgb, var(--color-warning, #f59e0b) 8%, transparent);
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.diag.error {
		border-left-color: var(--color-danger);
		background: color-mix(in srgb, var(--color-danger) 8%, transparent);
		color: var(--color-text-primary);
	}

	.recap {
		display: flex;
		flex-direction: column;
		gap: 3px;
		border-top: 1px solid var(--color-border);
		padding-top: 10px;
	}

	.recap-totals {
		display: flex;
		gap: 12px;
		flex-wrap: wrap;
		font-family: var(--font-mono, monospace);
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		margin-bottom: 4px;
	}

	.recap-totals b {
		color: var(--color-text-primary);
		font-weight: 500;
	}

	.recap-row {
		display: flex;
		justify-content: space-between;
		gap: 8px;
		padding: 3px 8px;
		font-size: 0.6875rem;
		border-radius: 4px;
	}

	.recap-row.bad {
		background: color-mix(in srgb, var(--color-danger) 6%, transparent);
	}

	.counts {
		display: grid;
		grid-template-columns: repeat(5, 3ch);
		text-align: right;
		font-family: var(--font-mono, monospace);
		font-variant-numeric: tabular-nums;
	}

	.c-ok {
		color: var(--color-success, #10b981);
	}

	.c-changed,
	.c-ignored {
		color: var(--color-warning, #f59e0b);
	}

	.c-failed,
	.c-unreachable {
		color: var(--color-danger);
	}

	.c-skipped {
		color: var(--color-text-tertiary);
	}
</style>
