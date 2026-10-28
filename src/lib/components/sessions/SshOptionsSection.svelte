<script lang="ts">
	/**
	 * A session's ssh_config settings in the editor: lines written as in
	 * ~/.ssh/config, a few common ones a click away, the file the session
	 * came from, and what Reach makes of all of it, checked as you type.
	 */
	import { sshOptionsReport, sshconfigScan, weakeningKey, type SshConfigReport } from '#lib/ipc/sshconfig.js';
	import type { SshOptions } from '#lib/ipc/sessions.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import SshOptionsReport from './SshOptionsReport.svelte';
	import { untrack } from 'svelte';

	interface Props {
		options: SshOptions;
		host: string;
		port: number;
		username: string;
		disabled?: boolean;
	}

	let { options = $bindable(), host, port, username, disabled = false }: Props = $props();

	let open = $state(false);
	let text = $state((options.lines ?? []).join('\n'));
	let report = $state<SshConfigReport | null>(null);
	let checking = $state(false);
	let rereadError = $state<string | null>(null);
	// This person's approvals, as the backend verified them; it signs them
	// into the stored lists on save.
	let approvedCommands = $state<string[]>(options.my_approved_commands ?? []);
	let acceptedWeakenings = $state<string[]>(options.my_accepted_weakenings ?? []);

	let hasAny = $derived(!!options.imported || (options.lines ?? []).length > 0);
	$effect(() => {
		if (hasAny) open = true;
	});

	/** Common needs, as the lines OpenSSH users would write. */
	const PRESETS: { label: string; lines: string[] }[] = [
		{
			label: 'sshopt.preset_legacy',
			lines: [
				'KexAlgorithms +diffie-hellman-group14-sha1,diffie-hellman-group1-sha1',
				'HostKeyAlgorithms +ssh-rsa',
				'Ciphers +aes128-cbc,aes256-cbc,3des-cbc',
				'MACs +hmac-sha1'
			]
		},
		{ label: 'sshopt.preset_keepalive', lines: ['ServerAliveInterval 30', 'ServerAliveCountMax 3'] },
		{ label: 'sshopt.preset_compression', lines: ['Compression yes'] },
		{ label: 'sshopt.preset_timeout', lines: ['ConnectTimeout 10', 'ConnectionAttempts 3'] }
	];

	function addLines(lines: string[]): void {
		const have = text.split('\n').map((l) => l.trim().toLowerCase());
		const add = lines.filter((l) => !have.includes(l.toLowerCase()));
		if (add.length === 0) return;
		text = [text.trimEnd(), ...add].filter(Boolean).join('\n');
	}

	// The lines as stored: one per non-empty, non-comment line.
	$effect(() => {
		const lines = text
			.split('\n')
			.map((l) => l.trim())
			.filter((l) => l && !l.startsWith('#'));
		const approved = approvedCommands;
		const accepted = acceptedWeakenings;
		untrack(() => {
			options = { ...options, lines, my_approved_commands: approved, my_accepted_weakenings: accepted };
		});
	});

	// What Reach makes of it, re-checked shortly after each change.
	let timer: ReturnType<typeof setTimeout> | undefined;
	$effect(() => {
		const snapshot = { ...options };
		const h = host.trim() || 'host';
		const u = username;
		const p = port;
		clearTimeout(timer);
		if (!snapshot.imported && (snapshot.lines ?? []).length === 0) {
			report = null;
			return;
		}
		timer = setTimeout(async () => {
			checking = true;
			try {
				report = await sshOptionsReport(h, p, u, snapshot);
			} catch (e) {
				report = { host: h, lines: [], weakenings: [], commands: [], refused: null, errors: [String(e)], otherHostLines: 0 };
			} finally {
				checking = false;
			}
		}, 350);
		return () => clearTimeout(timer);
	});

	/** Read ~/.ssh/config again for this session's host, keeping lines set in Reach. */
	async function reread(): Promise<void> {
		rereadError = null;
		const alias = options.imported?.alias;
		if (!alias) return;
		try {
			const scan = await sshconfigScan();
			const found = scan.hosts.find((h) => h.alias === alias);
			if (!found?.options.imported) {
				rereadError = t('sshopt.reread_missing', { alias });
				return;
			}
			options = { ...options, imported: found.options.imported };
		} catch (e) {
			rereadError = String(e);
		}
	}

	function forget(): void {
		options = { ...options, imported: null };
	}

	/** The weakenings in force, so the session list can show a warning. */
	export function acceptedNow(): string[] {
		const fromReport = (report?.weakenings ?? []).filter((w) => w.accepted || acceptedWeakenings.includes(weakeningKey(w))).map(weakeningKey);
		return [...new Set(fromReport)];
	}
</script>

<div class="section">
	<button type="button" class="toggle" onclick={() => (open = !open)} aria-expanded={open}>
		<span class="chev" class:open>›</span>
		<span class="title">{t('sshopt.title')}</span>
		{#if report && report.weakenings.some((w) => w.accepted || acceptedWeakenings.includes(weakeningKey(w)))}
			<span class="badge">{t('sshopt.weaker_badge')}</span>
		{/if}
	</button>

	{#if open}
		<div class="body">
			<p class="hint">{t('sshopt.intro')}</p>

			{#if options.imported}
				<div class="imported">
					<span>{t('sshopt.imported_from', { alias: options.imported.alias, date: new Date(options.imported.at * 1000).toLocaleDateString() })}</span>
					<span class="imported-actions">
						<button type="button" class="link" onclick={reread} {disabled}>{t('sshopt.reread')}</button>
						<button type="button" class="link" onclick={forget} {disabled}>{t('sshopt.forget')}</button>
					</span>
				</div>
				{#if rereadError}<p class="err">{rereadError}</p>{/if}
			{/if}

			<div class="presets">
				{#each PRESETS as p (p.label)}
					<button type="button" class="preset" onclick={() => addLines(p.lines)} {disabled}>+ {t(p.label)}</button>
				{/each}
			</div>

			<label class="lines-label" for="ssh-option-lines">{t('sshopt.lines_label')}</label>
			<textarea
				id="ssh-option-lines"
				class="lines"
				bind:value={text}
				rows="5"
				spellcheck="false"
				autocapitalize="off"
				autocomplete="off"
				{disabled}
			></textarea>
			<p class="hint">{t('sshopt.lines_hint')}</p>

			{#if checking && !report}
				<p class="hint">{t('sshopt.checking')}</p>
			{/if}
			{#if report}
				<SshOptionsReport {report} bind:approvedCommands bind:acceptedWeakenings {disabled} />
			{/if}
		</div>
	{/if}
</div>

<style>
	.section {
		display: flex;
		flex-direction: column;
		gap: 8px;
		border-top: 1px solid var(--color-border);
		padding-top: 10px;
	}

	.toggle {
		display: flex;
		align-items: center;
		gap: 6px;
		min-height: 32px;
		padding: 0;
		background: none;
		border: none;
		color: var(--color-text-primary);
		font: inherit;
		cursor: pointer;
		text-align: left;
	}

	.chev {
		display: inline-block;
		transition: transform var(--duration-default) var(--ease-default);
		color: var(--color-text-secondary);
	}

	.chev.open {
		transform: rotate(90deg);
	}

	.title {
		font-size: 0.8125rem;
		font-weight: 600;
	}

	.badge {
		padding: 1px 6px;
		border-radius: 999px;
		font-size: 0.625rem;
		color: var(--color-warning);
		border: 1px solid color-mix(in srgb, var(--color-warning) 45%, var(--color-border));
	}

	.body {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.hint {
		margin: 0;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.err {
		margin: 0;
		font-size: 0.6875rem;
		color: var(--color-danger);
	}

	.imported {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		gap: 6px;
		padding: 6px 8px;
		font-size: 0.6875rem;
		border-radius: var(--radius-btn);
		background: var(--color-surface-hover);
	}

	.imported-actions {
		display: flex;
		gap: 10px;
	}

	.link {
		padding: 0;
		background: none;
		border: none;
		color: var(--color-accent);
		font: inherit;
		cursor: pointer;
	}

	.presets {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	.preset {
		min-height: 28px;
		padding: 0 10px;
		font: inherit;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px dashed var(--color-border);
		border-radius: 999px;
		cursor: pointer;
	}

	.preset:hover:not(:disabled) {
		color: var(--color-text-primary);
		border-color: var(--color-accent);
	}

	.lines-label {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
	}

	.lines {
		width: 100%;
		box-sizing: border-box;
		padding: 8px 10px;
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		resize: vertical;
	}

	.lines:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	@media (max-width: 700px) {
		.preset,
		.toggle {
			min-height: 44px;
		}
	}
</style>
