<script lang="ts">
	/**
	 * Report a problem: a GitHub issue with what a maintainer needs, and the
	 * log, cleaned of the user's hosts, accounts and folders, to attach.
	 *
	 * The user reads the cleaned log here before anything leaves the machine.
	 * GitHub lets no app attach a file to a new issue without the user's token,
	 * so the issue opens in the browser, filled in, and the folder holding the
	 * cleaned copy opens beside it to drag the file in.
	 */
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import { t } from '$lib/state/i18n.svelte';
	import { sessionList } from '$lib/ipc/sessions';
	import { reportPrepare, reportReveal, type Private, type Report } from '$lib/ipc/report';

	interface Props {
		open: boolean;
		onclose: () => void;
	}

	let { open, onclose }: Props = $props();

	const NEW_ISSUE = 'https://github.com/alexandrosnt/Reach/issues/new';
	/** GitHub refuses longer links; the log goes as a file, not in the link. */
	const MAX_URL = 7000;

	let report = $state<Report | null>(null);
	let error = $state('');
	let attach = $state(true);
	let sent = $state(false);

	$effect(() => {
		if (open) void prepare();
	});

	/** The saved hosts, accounts and names, to take out of the log. */
	async function privateTerms(): Promise<Private[]> {
		const sessions = await Promise.race([
			sessionList(),
			new Promise<never>((_, reject) =>
				setTimeout(() => reject(new Error('Could not load saved sessions in time; report redaction was not prepared.')), 3000)
			)
		]);
		return sessions.flatMap((s) => [
			{ text: s.host, label: 'host' as const },
			{ text: s.username, label: 'user' as const },
			{ text: s.name, label: 'session' as const }
		]);
	}

	async function prepare(): Promise<void> {
		report = null;
		error = '';
		sent = false;
		try {
			report = await reportPrepare(await privateTerms());
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	function issueUrl(r: Report): string {
		const system = [`Reach ${r.version}`, r.os, r.desktop].filter(Boolean).join(' · ');
		const build = (problems: string[]) => {
			const body = [
				'**What happened**',
				'',
				'',
				'**What you expected**',
				'',
				'',
				'**Steps to reproduce**',
				'1. ',
				'',
				'---',
				system,
				...(problems.length ? ['', 'Recent warnings and errors (cleaned):', '```', ...problems, '```'] : []),
				...(attach ? ['', '_Log: drag reach-report.log from the folder Reach opened into this box._'] : [])
			].join('\n');
			return `${NEW_ISSUE}?body=${encodeURIComponent(body)}`;
		};
		let problems = [...r.problems];
		let url = build(problems);
		while (url.length > MAX_URL && problems.length) {
			problems = problems.slice(1);
			url = build(problems);
		}
		return url;
	}

	async function send(): Promise<void> {
		if (!report) return;
		try {
			await openUrl(issueUrl(report));
			if (attach) await reportReveal();
			sent = true;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}
</script>

<Modal {open} {onclose} title={t('report.title')} maxWidth="760px">
	<div class="report">
		{#if error}
			<p class="error" role="alert">{error}</p>
		{:else if !report}
			<p class="muted">{t('report.preparing')}</p>
		{:else if sent}
			<p>{t('report.after')}</p>
		{:else}
			<p class="muted">{t('report.cleaned')}</p>
			<pre class="log" aria-label={t('report.log_label')}>{report.log || t('report.empty')}</pre>
			<label class="attach">
				<input type="checkbox" bind:checked={attach} />
				{t('report.include_log')}
			</label>
		{/if}
	</div>

	{#snippet actions()}
		<Button variant="secondary" size="sm" onclick={onclose}>{sent ? t('common.close') : t('common.cancel')}</Button>
		{#if !sent}
			<Button variant="primary" size="sm" onclick={send} disabled={!report}>{t('report.continue')}</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	.report {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.muted {
		color: var(--color-text-secondary);
		font-size: 0.8125rem;
		line-height: 1.5;
	}

	.error {
		color: var(--color-danger);
	}

	.log {
		max-height: 40vh;
		overflow: auto;
		margin: 0;
		padding: 10px 12px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background: var(--color-surface-sunken);
		font-family: var(--font-mono, monospace);
		font-size: 0.75rem;
		line-height: 1.45;
		white-space: pre-wrap;
		word-break: break-all;
	}

	.attach {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 0.8125rem;
	}
</style>
