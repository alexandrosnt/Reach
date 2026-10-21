<script lang="ts">
	/**
	 * Import hosts from ~/.ssh/config. Each host is read as ssh reads it, and
	 * the dialog says what Reach will do with every line: in force, waiting
	 * for approval (weaker settings, local commands), not supported yet, or
	 * an error. The files are kept with the session.
	 */
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import SshOptionsReport from './SshOptionsReport.svelte';
	import { sshconfigScan, weakeningKey, type HostImport } from '$lib/ipc/sshconfig';
	import { sessionCreate, sessionList, sessionListFolders, sessionCreateFolder, type SessionConfig, type AuthMethod, type JumpHostConfig, type Folder } from '$lib/ipc/sessions';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { folderPath } from '$lib/sessions/folder-tree';

	interface Props {
		open: boolean;
		vaultId?: string | null;
		onsave?: () => void;
	}

	let { open = $bindable(), vaultId = null, onsave }: Props = $props();

	let hosts = $state<HostImport[]>([]);
	let files = $state<string[]>([]);
	let existingSessions = $state<SessionConfig[]>([]);
	let selected = $state<Set<string>>(new Set());
	let expanded = $state<string | null>(null);
	/** Per host: the commands and weaker settings the user allowed. */
	let approvals = $state<Record<string, { commands: string[]; weakenings: string[] }>>({});
	let loading = $state(false);
	let importing = $state(false);
	let error = $state<string | undefined>();

	/** Where the imported sessions go: '' no folder, a folder id, or NEW. */
	const NEW = '__new__';
	let folders = $state<Folder[]>([]);
	let folderChoice = $state('');
	let newFolder = $state('');

	/** Narrows the list (as the session search does, /regex/ included), so
	 *  "Select all" can pick a group of a large file at a time. */
	let filter = $state('');
	let filterRe = $derived.by((): { re: RegExp | null; error: string | null } => {
		const m = /^\/(.+)\/([a-z]*)$/.exec(filter.trim());
		if (!m) return { re: null, error: null };
		try {
			return { re: new RegExp(m[1], (m[2] || 'i').replace(/[gy]/g, '')), error: null };
		} catch (e) {
			return { re: null, error: e instanceof Error ? e.message : String(e) };
		}
	});
	let shownHosts = $derived.by(() => {
		const raw = filter.trim();
		if (!raw) return hosts;
		if (filterRe.error) return [];
		const fields = (h: HostImport) => [h.alias, h.hostname, h.user, `${h.user ? h.user + '@' : ''}${h.hostname}:${h.port}`];
		if (filterRe.re) {
			const re = filterRe.re;
			return hosts.filter((h) => fields(h).some((f) => re.test(f)));
		}
		const q = raw.toLowerCase();
		return hosts.filter((h) => fields(h).some((f) => f.toLowerCase().includes(q)));
	});

	let selectableHosts = $derived(shownHosts.filter((h) => !isAlreadyImported(h)));
	let selectedCount = $derived(selected.size);
	let canImport = $derived(selectedCount > 0 && !importing);
	/** Errors in the files themselves: the same for every host, shown once. */
	let fileErrors = $derived([...new Set(hosts.flatMap((h) => h.report.errors))]);

	function isAlreadyImported(host: HostImport): boolean {
		// An imported session knows its Host alias: two Host blocks for the
		// same machine (different options) are different sessions. Only a
		// session saved without one is matched by address.
		return existingSessions.some((s) =>
			s.ssh_options?.imported
				? s.ssh_options.imported.alias === host.alias
				: s.host === host.hostname && s.port === host.port && s.username === host.user
		);
	}

	function pending(host: HostImport): number {
		const a = approvals[host.alias] ?? { commands: [], weakenings: [] };
		return (
			host.report.weakenings.filter((w) => !w.accepted && !a.weakenings.includes(weakeningKey(w))).length +
			host.report.commands.filter((c) => !a.commands.includes(c)).length
		);
	}

	function notYet(host: HostImport): number {
		return host.report.lines.filter((l) => l.status.kind === 'applied' && l.support === 'notYet').length;
	}

	function applied(host: HostImport): number {
		return host.report.lines.filter((l) => l.status.kind === 'applied').length;
	}

	$effect(() => {
		if (open) loadHosts();
	});

	async function loadHosts(): Promise<void> {
		loading = true;
		error = undefined;
		selected = new Set();
		expanded = null;
		filter = '';
		folderChoice = '';
		newFolder = '';
		try {
			const [scan, sessions, folderList] = await Promise.all([sshconfigScan(), sessionList(), sessionListFolders()]);
			folders = folderList;
			hosts = scan.hosts;
			files = scan.files;
			existingSessions = sessions;
			approvals = Object.fromEntries(scan.hosts.map((h) => [h.alias, { commands: [], weakenings: [] }]));
		} catch (err) {
			error = String(err);
		} finally {
			loading = false;
		}
	}

	function toggleHost(alias: string): void {
		const next = new Set(selected);
		if (next.has(alias)) next.delete(alias);
		else next.add(alias);
		selected = next;
	}

	function keyAuth(files: string[]): AuthMethod {
		return files.length > 0 ? { type: 'Key', path: files[0] } : { type: 'Password' };
	}

	async function handleImport(): Promise<void> {
		if (!canImport) return;
		importing = true;
		error = undefined;
		let count = 0;
		try {
			let folderId: string | null = folderChoice && folderChoice !== NEW ? folderChoice : null;
			if (folderChoice === NEW) {
				const name = newFolder.trim();
				if (!name) throw new Error(t('session.import_folder_name_needed'));
				folderId = (await sessionCreateFolder(name, null, vaultId)).id;
			}
			for (const host of hosts) {
				if (!selected.has(host.alias)) continue;
				const a = approvals[host.alias] ?? { commands: [], weakenings: [] };
				const jumpChain: JumpHostConfig[] | null =
					host.proxyJump.length > 0
						? host.proxyJump.map((j) => ({ host: j.host, port: j.port, username: j.user, auth_method: keyAuth(j.identityFiles) }))
						: null;
				await sessionCreate({
					name: host.alias,
					host: host.hostname,
					port: host.port,
					username: host.user,
					authMethod: keyAuth(host.identityFiles),
					folderId,
					vaultId,
					tags: ['ssh-config'],
					jumpChain,
					// What this person ticked; the backend signs it as theirs.
					sshOptions: { ...host.options, my_approved_commands: a.commands, my_accepted_weakenings: a.weakenings }
				});
				count++;
			}
			addToast(t('session.import_success', { count: String(count) }), 'success');
			onsave?.();
			open = false;
		} catch (err) {
			error = String(err);
		} finally {
			importing = false;
		}
	}

	function handleClose(): void {
		if (!importing) open = false;
	}
</script>

<Modal {open} onclose={handleClose} title={t('session.import_title')}>
	<div class="import-content">
		<p class="import-desc">{t('sshopt.import_desc')}</p>

		{#if loading}
			<div class="import-loading">
				<span class="spinner"></span>
				<span>{t('session.import_parsing')}</span>
			</div>
		{:else if error}
			<div class="import-error">{error}</div>
		{:else if hosts.length === 0}
			<p class="import-empty">{t('session.import_no_hosts')}</p>
		{:else}
			{#if fileErrors.length > 0}
				<div class="import-error">
					<strong>{t('sshopt.file_errors_title')}</strong>
					<ul>
						{#each fileErrors as e (e)}<li>{e}</li>{/each}
					</ul>
				</div>
			{/if}

			<input
				class="tool-input filter-input"
				type="search"
				placeholder={t('session.import_filter')}
				aria-label={t('session.import_filter')}
				bind:value={filter}
				disabled={importing}
			/>
			{#if filterRe.error}
				<p class="filter-error" role="alert">{t('session.regex_error', { error: filterRe.error })}</p>
			{/if}

			<div class="select-actions">
				<button class="select-btn" onclick={() => (selected = new Set(selectableHosts.map((h) => h.alias)))} disabled={importing}>
					{t('session.import_select_all')}
				</button>
				<button class="select-btn" onclick={() => (selected = new Set())} disabled={importing}>
					{t('session.import_deselect_all')}
				</button>
				{#if filter.trim() && !filterRe.error}
					<span class="filter-count">{t('session.matches_n', { count: String(shownHosts.length) })}</span>
				{/if}
				<span class="files" title={files.join('\n')}>{t('sshopt.files_read', { count: String(files.length) })}</span>
			</div>

			<div class="import-tools">
				<label class="folder-pick">
					<span>{t('session.import_into')}</span>
					<select class="tool-input" bind:value={folderChoice} disabled={importing}>
						<option value="">{t('session.import_no_folder')}</option>
						{#each folders as f (f.id)}<option value={f.id}>{folderPath(f.id, folders)}</option>{/each}
						<option value={NEW}>{t('session.new_folder')}</option>
					</select>
				</label>
				{#if folderChoice === NEW}
					<input class="tool-input new-folder-input" type="text" placeholder={t('session.folder_name')} aria-label={t('session.folder_name')} bind:value={newFolder} disabled={importing} />
				{/if}
			</div>

			<div class="host-list">
				{#each shownHosts as host (host.alias)}
					{@const already = isAlreadyImported(host)}
					{@const waiting = pending(host)}
					{@const missing = notYet(host)}
					<div class="host-row" class:disabled={already}>
						<label class="host-main">
							<input
								type="checkbox"
								class="host-check"
								checked={selected.has(host.alias)}
								disabled={already || importing}
								onchange={() => toggleHost(host.alias)}
							/>
							<div class="host-info">
								<div class="host-name-row">
									<span class="host-name">{host.alias}</span>
									{#if already}
										<span class="badge-imported">{t('session.import_already_exists')}</span>
									{/if}
								</div>
								<span class="host-detail">
									{host.user}@{host.hostname}:{host.port}
									{#if host.proxyJump.length > 0}
										<span class="proxy-chain">{t('session.import_proxy_chain', { chain: host.proxyJump.map((j) => j.host).join(' → ') })}</span>
									{/if}
								</span>
								<span class="summary">
									<span class="ok">{t('sshopt.count_applied', { count: String(applied(host)) })}</span>
									{#if waiting > 0}<span class="warn">{t('sshopt.count_pending', { count: String(waiting) })}</span>{/if}
									{#if missing > 0}<span class="muted">{t('sshopt.count_not_yet', { count: String(missing) })}</span>{/if}
								</span>
							</div>
						</label>
						<button type="button" class="details-btn" onclick={() => (expanded = expanded === host.alias ? null : host.alias)}>
							{expanded === host.alias ? t('sshopt.hide_details') : t('sshopt.details')}
						</button>
						{#if expanded === host.alias && approvals[host.alias]}
							<div class="details">
								<SshOptionsReport
									report={host.report}
									bind:approvedCommands={approvals[host.alias].commands}
									bind:acceptedWeakenings={approvals[host.alias].weakenings}
									disabled={importing}
									summary={false}
								/>
							</div>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	</div>

	{#snippet actions()}
		<Button variant="secondary" onclick={handleClose} disabled={importing}>
			{t('common.cancel')}
		</Button>
		<Button variant="primary" onclick={handleImport} disabled={!canImport}>
			{#if importing}
				<span class="spinner"></span>
			{/if}
			{t('session.import_selected', { count: String(selectedCount) })}
		</Button>
	{/snippet}
</Modal>

<style>
	.import-content {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.import-desc {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
	}

	.import-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		padding: 24px 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
	}

	.import-error {
		padding: 8px 12px;
		font-size: 0.75rem;
		color: var(--color-danger);
		background-color: rgba(255, 69, 58, 0.08);
		border: 1px solid rgba(255, 69, 58, 0.2);
		border-radius: var(--radius-btn);
	}

	.import-error ul {
		margin: 4px 0 0;
		padding-left: 16px;
	}

	.import-empty {
		margin: 0;
		padding: 24px 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		text-align: center;
	}

	.select-actions {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.import-tools {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
	}

	.tool-input {
		min-height: 30px;
		padding: 4px 8px;
		border-radius: 6px;
		border: 1px solid var(--color-border);
		background: var(--color-bg-secondary);
		color: var(--color-text-primary);
		font: inherit;
		font-size: 0.8125rem;
	}

	.filter-input {
		width: 100%;
		box-sizing: border-box;
	}

	.new-folder-input {
		flex: 1 1 160px;
		min-width: 0;
	}

	.folder-pick {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.filter-error,
	.filter-count {
		margin: 0;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.filter-error {
		color: var(--color-danger);
		overflow-wrap: anywhere;
	}

	.files {
		margin-left: auto;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.select-btn {
		padding: 4px 10px;
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		font-weight: 500;
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: 4px;
		cursor: pointer;
	}

	.select-btn:hover:not(:disabled) {
		background-color: var(--color-surface-hover);
		color: var(--color-text-primary);
	}

	.select-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.host-list {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.host-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 4px 8px;
		padding: 8px 10px;
		border-radius: 6px;
	}

	.host-row:hover:not(.disabled) {
		background-color: var(--color-surface-hover);
	}

	.host-row.disabled {
		opacity: 0.55;
	}

	.host-main {
		display: flex;
		align-items: flex-start;
		gap: 10px;
		cursor: pointer;
		min-width: 0;
	}

	.host-check {
		width: 14px;
		height: 14px;
		margin-top: 2px;
		accent-color: var(--color-accent);
		flex-shrink: 0;
	}

	.host-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.host-name-row {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.host-name {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.badge-imported {
		padding: 1px 6px;
		font-size: 0.5625rem;
		font-weight: 500;
		color: var(--color-text-secondary);
		background-color: var(--color-surface-hover);
		border-radius: 3px;
		text-transform: uppercase;
		letter-spacing: 0.03em;
	}

	.host-detail {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		font-family: var(--font-mono);
		overflow-wrap: anywhere;
	}

	.proxy-chain {
		margin-left: 6px;
		color: var(--color-accent);
		font-family: var(--font-sans);
		font-style: italic;
	}

	.summary {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		font-size: 0.6875rem;
	}

	.summary .ok {
		color: var(--color-success);
	}

	.summary .warn {
		color: var(--color-warning);
	}

	.summary .muted {
		color: var(--color-text-secondary);
	}

	.details-btn {
		align-self: start;
		min-height: 28px;
		padding: 0 8px;
		font: inherit;
		font-size: 0.6875rem;
		color: var(--color-accent);
		background: none;
		border: none;
		cursor: pointer;
	}

	.details {
		grid-column: 1 / -1;
		padding: 6px 0 4px 24px;
	}

	.spinner {
		display: inline-block;
		width: 14px;
		height: 14px;
		border: 2px solid var(--color-border);
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	@media (max-width: 700px) {
		.host-row {
			grid-template-columns: 1fr;
		}

		.details {
			padding-left: 0;
		}

		.details-btn {
			min-height: 44px;
			justify-self: start;
		}
	}
</style>
