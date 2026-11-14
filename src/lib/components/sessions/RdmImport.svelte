<script lang="ts">
	/**
	 * Import hosts from a Devolutions Remote Desktop Manager export: a .rdm
	 * (XML) or .json file, picked or pasted (see #lib/sessions/rdm-import).
	 * RDP, SSH and VNC entries become sessions in the selected vault, and the
	 * RDM group path is recreated as nested folders, reusing folders already
	 * there, and a linked SSH gateway becomes the session's jump host.
	 * Passwords are never imported: the session asks for them on connect.
	 * An export saved without credentials has no usernames, so the dialog
	 * asks for one to use where the export gives none. Hosts already saved
	 * are shown but not imported again.
	 */
	import Modal from '#lib/components/shared/Modal.svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import { sessionCreate, sessionCreateFolder, sessionList, sessionListFolders, type AuthMethod, type Folder, type JumpHostConfig, type SessionConfig } from '#lib/ipc/sessions.js';
	import { findChildFolder, findExisting, parseRdmExport, type RdmImportHost, type RdmSkipped } from '#lib/sessions/rdm-import.js';
	import { addToast } from '#lib/state/toasts.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';

	interface Props {
		open: boolean;
		vaultId?: string | null;
		onsave?: () => void;
	}

	let { open = $bindable(), vaultId = null, onsave }: Props = $props();

	/** Larger than any real export; stops a wrong pick from freezing the window. */
	const MAX_FILE_BYTES = 50 * 1024 * 1024;

	let raw = $state('');
	let fileName = $state<string | null>(null);
	let fileInput = $state<HTMLInputElement | undefined>();
	let parsed = $state(false);
	let reading = $state(false);
	let importing = $state(false);
	let error = $state<string | undefined>();
	let hosts = $state<RdmImportHost[]>([]);
	let skipped = $state<RdmSkipped[]>([]);
	let failed = $state<{ name: string; error: string }[]>([]);
	let existingSessions = $state<SessionConfig[]>([]);
	let folders = $state<Folder[]>([]);
	let selected = $state<Set<string>>(new Set());
	let filter = $state('');
	/** Used for every host, and gateway, the export gives no username for. */
	let fallbackUser = $state('');

	const target = $derived(vaultId ?? null);
	const busy = $derived(reading || importing);

	const shownHosts = $derived.by(() => {
		const q = filter.trim().toLowerCase();
		if (!q) return hosts;
		return hosts.filter(
			(h) => h.name.toLowerCase().includes(q) || h.host.toLowerCase().includes(q) || folderLabel(h).toLowerCase().includes(q)
		);
	});
	const selectableHosts = $derived(shownHosts.filter((h) => !isSaved(h)));
	const selectedCount = $derived(selected.size);
	/** Selected SSH and RDP hosts that would be saved without a username, which they cannot log in without. */
	const missingUser = $derived(
		fallbackUser.trim() ? 0 : hosts.filter((h) => selected.has(h.id) && h.kind !== 'vnc' && !h.username).length
	);
	const anyWithoutUser = $derived(hosts.some((h) => h.kind !== 'vnc' && !h.username));
	const canImport = $derived(parsed && selectedCount > 0 && missingUser === 0 && !busy);

	function isSaved(host: RdmImportHost): boolean {
		// Compared with the username it would be saved under, fallback included.
		return findExisting({ ...host, username: userFor(host) }, existingSessions, target) !== undefined;
	}

	function folderLabel(host: RdmImportHost): string {
		return host.folder.join(' / ');
	}

	function skipReason(s: RdmSkipped): string {
		if (s.reason === 'bad_port') return t('session.rdp_import_bad_port');
		if (s.reason === 'unsupported') return t('session.rdm_import_unsupported', { type: s.type ?? '?' });
		return t('session.rdp_import_no_host');
	}

	async function loadSaved(): Promise<void> {
		[existingSessions, folders] = await Promise.all([sessionList(), sessionListFolders()]);
	}

	async function pickFile(event: Event): Promise<void> {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (!file) return;
		error = undefined;
		if (file.size > MAX_FILE_BYTES) {
			error = t('session.rdm_import_too_large');
			return;
		}
		try {
			raw = await file.text();
			fileName = file.name;
		} catch (err) {
			error = String(err);
			return;
		}
		await read();
	}

	async function read(): Promise<void> {
		error = undefined;
		const result = parseRdmExport(raw);
		if (!result.ok) {
			if (result.error === 'bad_file') error = t('session.rdm_import_bad_file', { error: result.detail });
			else if (result.error === 'encrypted') error = t('session.rdm_import_encrypted');
			else error = t('session.rdm_import_none');
			return;
		}
		reading = true;
		try {
			await loadSaved();
			hosts = result.hosts;
			skipped = result.skipped;
			selected = new Set(result.hosts.filter((h) => !isSaved(h)).map((h) => h.id));
			parsed = true;
		} catch (err) {
			error = String(err);
		} finally {
			reading = false;
		}
	}

	function toggle(id: string): void {
		const next = new Set(selected);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		selected = next;
	}

	/**
	 * The folder for a group path: each level reused when the vault already
	 * has it, else created once per import.
	 */
	async function folderFor(path: string[], created: Map<string, string>): Promise<string | null> {
		let parent: string | null = null;
		for (let i = 0; i < path.length; i++) {
			const key = path.slice(0, i + 1).join('\u0000');
			const known: string | undefined = created.get(key) ?? findChildFolder(path[i], parent, target, folders)?.id;
			if (known) {
				parent = known;
				continue;
			}
			const folder = await sessionCreateFolder(path[i], parent, target);
			created.set(key, folder.id);
			parent = folder.id;
		}
		return parent;
	}

	function authFor(h: RdmImportHost): AuthMethod {
		return h.keyPath ? { type: 'Key', path: h.keyPath } : { type: 'Password' };
	}

	function userFor(h: RdmImportHost): string {
		return h.username || (h.kind === 'vnc' ? '' : fallbackUser.trim());
	}

	/** The gateway hops; their own credentials are not in the export, so the login asks. */
	function jumpFor(h: RdmImportHost): JumpHostConfig[] | null {
		if (h.jump.length === 0) return null;
		return h.jump.map((g) => ({ host: g.host, port: g.port, username: fallbackUser.trim(), auth_method: { type: 'Password' } }));
	}

	async function handleImport(): Promise<void> {
		if (!canImport) return;
		importing = true;
		error = undefined;
		failed = [];
		const created = new Map<string, string>();
		const failures: { name: string; error: string }[] = [];
		let count = 0;
		// One host that cannot be saved must not cost the others theirs.
		for (const h of hosts) {
			if (!selected.has(h.id)) continue;
			try {
				await sessionCreate({
					name: h.name,
					host: h.host,
					port: h.port,
					username: userFor(h),
					authMethod: authFor(h),
					jumpChain: jumpFor(h),
					folderId: await folderFor(h.folder, created),
					vaultId: target,
					tags: ['rdm-import'],
					kind: h.kind,
					domain: h.kind === 'rdp' ? h.domain : null,
					detectedOs: h.kind === 'rdp' ? 'windows' : null
				});
				count++;
			} catch (err) {
				failures.push({ name: h.name, error: String(err) });
			}
		}
		if (count > 0) {
			addToast(t('session.rdm_import_done', { count: String(count) }), 'success');
			onsave?.();
		}
		if (failures.length === 0) {
			importing = false;
			close();
			return;
		}
		// Keep the dialog open on what failed; what was saved now shows as saved.
		failed = failures;
		try {
			await loadSaved();
		} catch {
			// The list refreshes on the next open; the failures still show.
		}
		selected = new Set(hosts.filter((h) => selected.has(h.id) && !isSaved(h)).map((h) => h.id));
		importing = false;
	}

	function close(): void {
		if (importing) return;
		open = false;
		raw = '';
		fileName = null;
		parsed = false;
		error = undefined;
		hosts = [];
		skipped = [];
		failed = [];
		existingSessions = [];
		folders = [];
		selected = new Set();
		filter = '';
		fallbackUser = '';
	}
</script>

<Modal {open} onclose={close} title={t('session.import_rdm')} maxWidth="600px">
	<div class="import-content">
		<p class="import-desc">{t('session.rdm_import_desc')}</p>

		{#if !parsed}
			<div class="file-row">
				<input
					bind:this={fileInput}
					class="file-input"
					type="file"
					accept=".rdm,.json,.xml"
					onchange={pickFile}
					disabled={busy}
				/>
				<Button variant="secondary" onclick={() => fileInput?.click()} disabled={busy}>{t('session.rdm_import_choose')}</Button>
				<span class="file-name">{fileName ?? t('session.rdm_import_or_paste')}</span>
			</div>
			<textarea
				class="tool-input json-input"
				bind:value={raw}
				oninput={() => (fileName = null)}
				aria-label={t('session.import_rdm')}
				placeholder={'<?xml version="1.0"?>\n<ArrayOfConnection>\n  <Connection>…</Connection>\n</ArrayOfConnection>'}
				spellcheck="false"
				disabled={busy}
			></textarea>
			{#if error}<div class="import-error" role="alert">{error}</div>{/if}
		{:else}
			{#if error}<div class="import-error" role="alert">{error}</div>{/if}
			{#if failed.length > 0}
				<div class="import-error" role="alert">
					<strong>{t('session.rdp_import_failed', { count: String(failed.length) })}</strong>
					<ul>
						{#each failed as f, i (i)}<li>{f.name}: {f.error}</li>{/each}
					</ul>
				</div>
			{/if}
			{#if skipped.length > 0}
				<div class="import-note">
					<strong>{t('session.rdp_import_skipped', { count: String(skipped.length) })}</strong>
					<ul>
						{#each skipped as s, i (i)}<li>{s.label}: {skipReason(s)}</li>{/each}
					</ul>
				</div>
			{/if}

			{#if hosts.length === 0}
				<p class="import-empty">{t('session.rdm_import_none')}</p>
			{:else}
				<input
					class="tool-input filter-input"
					type="search"
					placeholder={t('session.rdp_import_filter')}
					aria-label={t('session.rdp_import_filter')}
					bind:value={filter}
					disabled={busy}
				/>
				{#if anyWithoutUser}
					<label class="user-field">
						<span>{t('session.rdm_import_user')}</span>
						<input class="tool-input" type="text" bind:value={fallbackUser} autocomplete="off" spellcheck="false" disabled={busy} />
					</label>
					<p class="user-hint" class:warn={missingUser > 0}>{t('session.rdm_import_user_hint')}</p>
				{/if}
				<div class="select-actions">
					<button class="select-btn" type="button" onclick={() => (selected = new Set(selectableHosts.map((h) => h.id)))} disabled={busy}>
						{t('session.import_select_all')}
					</button>
					<button class="select-btn" type="button" onclick={() => (selected = new Set())} disabled={busy}>
						{t('session.import_deselect_all')}
					</button>
					<span class="count">{t('session.selected_n', { count: String(selectedCount) })}</span>
				</div>
				<div class="host-list">
					{#each shownHosts as h (h.id)}
						{@const saved = isSaved(h)}
						<label class="host-row" class:disabled={saved} class:on={selected.has(h.id)}>
							<input
								class="host-check"
								type="checkbox"
								checked={selected.has(h.id)}
								onchange={() => toggle(h.id)}
								disabled={saved || busy}
							/>
							<span class="host-info">
								<span class="host-name-row">
									<span class="badge-kind">{h.kind}</span>
									<span class="host-name">{h.name}</span>
									{#if saved}<span class="badge-imported">{t('session.import_already_exists')}</span>{/if}
								</span>
								<span class="host-detail">{h.username ? `${h.username}@` : ''}{h.host}:{h.port}{h.domain ? ` · ${h.domain}` : ''}{h.keyPath ? ` · ${h.keyPath}` : ''}{h.jump.length > 0 ? ` · ${t('session.rdm_import_via', { host: h.jump.map((g) => `${g.host}:${g.port}`).join(' → ') })}` : ''}</span>
							</span>
							{#if h.folder.length > 0}<span class="host-folder">{folderLabel(h)}</span>{/if}
						</label>
					{/each}
				</div>
			{/if}
		{/if}
	</div>

	{#snippet actions()}
		<Button variant="secondary" onclick={close} disabled={importing}>{t('common.cancel')}</Button>
		{#if !parsed}
			<Button variant="primary" onclick={read} disabled={!raw.trim() || busy}>{t('session.rdp_import_parse')}</Button>
		{:else}
			<Button variant="primary" onclick={handleImport} disabled={!canImport}>
				{#if importing}<span class="spinner"></span>{/if}
				{t('session.import_selected', { count: String(selectedCount) })}
			</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	.import-content {
		display: flex;
		flex-direction: column;
		gap: 12px;
		min-width: 0;
	}

	.import-desc {
		margin: 0;
		font-size: 0.8125rem;
		line-height: 1.45;
		color: var(--color-text-secondary);
	}

	.user-field {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.user-hint {
		margin: -6px 0 0;
		font-size: 0.6875rem;
		color: var(--color-text-tertiary);
	}

	.user-hint.warn {
		color: var(--color-warning);
	}

	.file-row {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
	}

	.file-input {
		display: none;
	}

	.file-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.badge-kind {
		flex-shrink: 0;
		padding: 1px 6px;
		font-size: 0.5625rem;
		font-weight: 600;
		color: var(--color-text-secondary);
		border: 1px solid var(--color-border);
		border-radius: 3px;
		text-transform: uppercase;
		letter-spacing: 0.03em;
	}

	.tool-input {
		box-sizing: border-box;
		width: 100%;
		padding: 6px 8px;
		border-radius: 6px;
		border: 1px solid var(--color-border);
		background: var(--color-bg-secondary);
		color: var(--color-text-primary);
		font: inherit;
		font-size: 0.8125rem;
	}

	.json-input {
		min-height: 220px;
		resize: vertical;
		font-family: var(--font-mono);
		font-size: 0.75rem;
	}

	.import-error,
	.import-note {
		padding: 8px 12px;
		font-size: 0.75rem;
		border-radius: var(--radius-btn);
		overflow-wrap: anywhere;
	}

	.import-error {
		color: var(--color-danger);
		background-color: color-mix(in srgb, var(--color-danger) 8%, transparent);
		border: 1px solid color-mix(in srgb, var(--color-danger) 20%, transparent);
	}

	.import-note {
		color: var(--color-text-secondary);
		background-color: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
	}

	.import-error ul,
	.import-note ul {
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

	.count {
		margin-left: auto;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.host-list {
		display: flex;
		flex-direction: column;
		gap: 2px;
		max-height: 44vh;
		overflow-y: auto;
		padding: 4px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
	}

	.host-row {
		display: flex;
		align-items: flex-start;
		gap: 10px;
		padding: 8px 10px;
		border-radius: 6px;
		cursor: pointer;
	}

	.host-row:hover:not(.disabled) {
		background-color: var(--color-surface-hover);
	}

	.host-row.on {
		background-color: var(--color-surface-active);
	}

	.host-row.disabled {
		opacity: 0.55;
		cursor: default;
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
		flex: 1;
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
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
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
		font-family: var(--font-mono);
		color: var(--color-text-secondary);
		overflow-wrap: anywhere;
	}

	.host-folder {
		max-width: 40%;
		overflow: hidden;
		text-overflow: ellipsis;
		font-size: 0.6875rem;
		color: var(--color-text-tertiary);
		white-space: nowrap;
	}

	.spinner {
		display: inline-block;
		width: 0.8em;
		height: 0.8em;
		margin-right: 0.4em;
		border: 2px solid currentColor;
		border-top-color: transparent;
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
		vertical-align: -1px;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.spinner {
			animation: none;
		}
	}
</style>
