<script lang="ts">
	/**
	 * What a host's ssh_config does in Reach, line by line: what is in force,
	 * what waits for the user's approval (weaker settings, local commands),
	 * what Reach cannot act on yet, and errors. Nothing is left unsaid.
	 */
	import { weakeningKey, type ReportLine, type SshConfigReport } from '#lib/ipc/sshconfig.js';
	import { t } from '#lib/state/i18n.svelte.js';

	interface Props {
		report: SshConfigReport;
		approvedCommands: string[];
		acceptedWeakenings: string[];
		disabled?: boolean;
		/** The count chips; off where the caller shows its own summary. */
		summary?: boolean;
	}

	let { report, approvedCommands = $bindable(), acceptedWeakenings = $bindable(), disabled = false, summary = true }: Props = $props();

	let showAll = $state(false);

	/** The session's own host, port and user reach the resolver as overrides; they are not config lines. */
	let shown = $derived(report.lines.filter((l) => !(l.at.file === 'command line' && l.support === 'sessionField')));
	let applied = $derived(shown.filter((l) => l.status.kind === 'applied'));
	let notYet = $derived(applied.filter((l) => l.support === 'notYet'));
	let errors = $derived(report.errors);
	/** Weaker settings the user can allow or take back. Lines written in Reach count as allowed. */
	let weakenings = $derived(report.weakenings);
	let pendingCount = $derived(
		weakenings.filter((w) => !w.accepted && !acceptedWeakenings.includes(weakeningKey(w))).length +
			report.commands.filter((c) => !approvedCommands.includes(c)).length
	);
	let other = $derived(shown.filter((l) => l.status.kind !== 'applied'));

	function toggleWeakening(key: string, on: boolean): void {
		acceptedWeakenings = on ? [...new Set([...acceptedWeakenings, key])] : acceptedWeakenings.filter((k) => k !== key);
	}

	function toggleCommand(cmd: string, on: boolean): void {
		approvedCommands = on ? [...new Set([...approvedCommands, cmd])] : approvedCommands.filter((c) => c !== cmd);
	}

	function lineState(l: ReportLine): { text: string; tone: 'ok' | 'warn' | 'muted' | 'bad' } {
		if (l.status.kind === 'error') return { text: l.status.detail, tone: 'bad' };
		if (l.status.kind !== 'applied') return { text: t(`sshopt.status_${l.status.kind}`), tone: 'muted' };
		if (l.support === 'notYet') return { text: t('sshopt.not_yet'), tone: 'warn' };
		if (l.support === 'sessionField') return { text: t('sshopt.session_field'), tone: 'ok' };
		if (l.support === 'structure') return { text: t('sshopt.structure'), tone: 'ok' };
		if (l.used?.kind === 'partly') return { text: l.used.detail, tone: 'warn' };
		if (l.used?.kind === 'notUsed') return { text: l.used.detail, tone: 'warn' };
		return { text: t('sshopt.in_force'), tone: 'ok' };
	}

	function where(l: ReportLine): string {
		const file = l.at.file === 'command line' ? t('sshopt.set_in_reach') : l.at.file.split(/[\\/]/).pop();
		return l.at.line > 0 && l.at.file !== 'command line' ? `${file}:${l.at.line}` : (file ?? '');
	}
</script>

<div class="report">
	{#if summary}
	<div class="chips">
		<span class="chip ok">{t('sshopt.count_applied', { count: String(applied.length) })}</span>
		{#if pendingCount > 0}
			<span class="chip warn">{t('sshopt.count_pending', { count: String(pendingCount) })}</span>
		{/if}
		{#if notYet.length > 0}
			<span class="chip muted">{t('sshopt.count_not_yet', { count: String(notYet.length) })}</span>
		{/if}
		{#if errors.length > 0}
			<span class="chip bad">{t('sshopt.count_errors', { count: String(errors.length) })}</span>
		{/if}
	</div>
	{/if}

	{#if report.refused}
		<div class="box bad">{t('sshopt.refused', { message: report.refused })}</div>
	{/if}

	{#if errors.length > 0}
		<div class="box bad">
			<strong>{t('sshopt.errors_title')}</strong>
			<ul>
				{#each errors as e (e)}<li>{e}</li>{/each}
			</ul>
		</div>
	{/if}

	{#if weakenings.length > 0 || report.commands.length > 0}
		<div class="box warn">
			<strong>{pendingCount > 0 ? t('sshopt.approval_title') : t('sshopt.weaker_title')}</strong>
			<p class="hint">{pendingCount > 0 ? t('sshopt.approval_hint') : t('sshopt.weaker_hint')}</p>
			{#each weakenings as w (weakeningKey(w))}
				{@const key = weakeningKey(w)}
				<label class="approve">
					<input
						type="checkbox"
						checked={w.accepted || acceptedWeakenings.includes(key)}
						{disabled}
						onchange={(e) => toggleWeakening(key, e.currentTarget.checked)}
					/>
					<span class="approve-text">
						<code>{w.keyword} {w.value}</code>
						<span class="reason">{w.reason}</span>
					</span>
				</label>
			{/each}
			{#each report.commands as c (c)}
				<label class="approve">
					<input type="checkbox" checked={approvedCommands.includes(c)} {disabled} onchange={(e) => toggleCommand(c, e.currentTarget.checked)} />
					<span class="approve-text">
						<span>{t('sshopt.allow_command')}</span>
						<code>{c}</code>
					</span>
				</label>
			{/each}
		</div>
	{/if}

	{#if applied.length > 0}
		<ul class="lines">
			{#each applied as l (where(l) + l.keyword)}
				{@const s = lineState(l)}
				<li class="line">
					<code class="kw">{l.keyword} <span class="val">{l.text}</span></code>
					<span class="state {s.tone}">{s.text}</span>
					<span class="where">{where(l)}</span>
				</li>
			{/each}
		</ul>
	{/if}

	{#if other.length > 0}
		<button type="button" class="more" onclick={() => (showAll = !showAll)}>
			{showAll ? t('sshopt.hide_other') : t('sshopt.show_other', { count: String(other.length) })}
		</button>
		{#if showAll}
			<ul class="lines">
				{#each other as l (where(l) + l.keyword + l.text)}
					{@const s = lineState(l)}
					<li class="line">
						<code class="kw">{l.keyword} <span class="val">{l.text}</span></code>
						<span class="state {s.tone}">{s.text}</span>
						<span class="where">{where(l)}</span>
					</li>
				{/each}
			</ul>
		{/if}
	{/if}
	{#if report.otherHostLines > 0}
		<p class="other-hosts">{t('sshopt.other_host_lines', { count: String(report.otherHostLines) })}</p>
	{/if}
</div>

<style>
	.other-hosts {
		margin: 0;
		color: var(--color-text-tertiary);
	}

	.report {
		display: flex;
		flex-direction: column;
		gap: 8px;
		font-size: 0.75rem;
	}

	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	.chip {
		padding: 2px 8px;
		border-radius: 999px;
		font-size: 0.6875rem;
		font-weight: 500;
		border: 1px solid var(--color-border);
	}

	.chip.ok {
		color: var(--color-success);
		border-color: color-mix(in srgb, var(--color-success) 40%, var(--color-border));
	}

	.chip.warn {
		color: var(--color-warning);
		border-color: color-mix(in srgb, var(--color-warning) 45%, var(--color-border));
	}

	.chip.muted {
		color: var(--color-text-secondary);
	}

	.chip.bad {
		color: var(--color-danger);
		border-color: color-mix(in srgb, var(--color-danger) 45%, var(--color-border));
	}

	.box {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 8px 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
	}

	.box.bad {
		color: var(--color-danger);
		background: color-mix(in srgb, var(--color-danger) 7%, transparent);
		border-color: color-mix(in srgb, var(--color-danger) 30%, var(--color-border));
	}

	.box.warn {
		background: color-mix(in srgb, var(--color-warning) 7%, transparent);
		border-color: color-mix(in srgb, var(--color-warning) 35%, var(--color-border));
	}

	.box ul {
		margin: 0;
		padding-left: 16px;
	}

	.hint {
		margin: 0;
		color: var(--color-text-secondary);
	}

	.approve {
		display: flex;
		gap: 8px;
		align-items: flex-start;
		min-height: 28px;
		cursor: pointer;
	}

	.approve input {
		margin-top: 2px;
		accent-color: var(--color-warning);
	}

	.approve-text {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.reason {
		color: var(--color-text-secondary);
	}

	code {
		font-family: var(--font-mono);
		font-size: 0.6875rem;
		overflow-wrap: anywhere;
	}

	.lines {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.line {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		grid-template-areas: 'kw state' 'where where';
		gap: 0 8px;
		padding: 4px 6px;
		border-radius: 4px;
	}

	.line:hover {
		background: var(--color-surface-hover);
	}

	.kw {
		grid-area: kw;
		color: var(--color-text-primary);
	}

	.val {
		color: var(--color-text-secondary);
	}

	.state {
		grid-area: state;
		text-align: right;
		font-size: 0.6875rem;
		max-width: 260px;
	}

	.state.ok {
		color: var(--color-success);
	}

	.state.warn {
		color: var(--color-warning);
	}

	.state.muted {
		color: var(--color-text-tertiary, var(--color-text-secondary));
	}

	.state.bad {
		color: var(--color-danger);
	}

	.where {
		grid-area: where;
		font-size: 0.625rem;
		color: var(--color-text-tertiary, var(--color-text-secondary));
		font-family: var(--font-mono);
	}

	.more {
		align-self: flex-start;
		padding: 2px 0;
		background: none;
		border: none;
		color: var(--color-accent);
		font: inherit;
		font-size: 0.6875rem;
		cursor: pointer;
	}

	@media (max-width: 700px) {
		.line {
			grid-template-columns: 1fr;
			grid-template-areas: 'kw' 'state' 'where';
		}

		.state {
			text-align: left;
			max-width: none;
		}

		.approve {
			min-height: 44px;
		}
	}
</style>
