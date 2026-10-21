<script lang="ts">
	/**
	 * Bulk-import RDP hosts from pasted JSON: a Termix export or a plain array
	 * (see $lib/sessions/rdp-import). Each host becomes an RDP session in the
	 * selected vault, and folders named in the JSON are created there once,
	 * or reused when the vault already has them. Passwords are never imported;
	 * RDP asks for them on connect. Hosts already saved are shown but not
	 * imported again, like the ssh_config import.
	 */
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import { sessionCreate, sessionCreateFolder, sessionList, sessionListFolders, type Folder, type SessionConfig } from '$lib/ipc/sessions';
	import { findExisting, findFolder, parseRdpHosts, type RdpImportHost, type RdpSkipped } from '$lib/sessions/rdp-import';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		open: boolean;
		vaultId?: string | null;
		onsave?: () => void;
	}

	let { open = $bindable(), vaultId = null, onsave }: Props = $props();

	let raw = $state('');
	let parsed = $state(false);
	let reading = $state(false);
	let importing = $state(false);
	let error = $state<string | undefined>();
	let hosts = $state<RdpImportHost[]>([]);
	let skipped = $state<RdpSkipped[]>([]);
	let failed = $state<{ name: string; error: string }[]>([]);
	let existingSessions = $state<SessionConfig[]>([]);
	let folders = $state<Folder[]>([]);
	let selected = $state<Set<string>>(new Set());
	let filter = $state('');

	const target = $derived(vaultId ?? null);
	const busy = $derived(reading || importing);

	const shownHosts = $derived.by(() => {
		const q = filter.trim().toLowerCase();
		if (!q) return hosts;
		return hosts.filter(
			(h) => h.name.toLowerCase().includes(q) || h.host.toLowerCase().includes(q) || (h.folder ?? '').toLowerCase().includes(q)
		);
	});
	const selectableHosts = $derived(shownHosts.filter((h) => !isSaved(h)));
	const selectedCount = $derived(selected.size);
	const canImport = $derived(parsed && selectedCount > 0 && !busy);

	function isSaved(host: RdpImportHost): boolean {
		return findExisting(host, existingSessions) !== undefined;
	}

	function skipReason(s: RdpSkipped): string {
		return s.reason === 'bad_port' ? t('session.rdp_import_bad_port') : t('session.rdp_import_no_host');
	}

	async function loadSaved(): Promise<void> {
		[existingSessions, folders] = await Promise.all([sessionList(), sessionListFolders()]);
	}

	async function read(): Promise<void> {
		error = undefined;
		const result = parseRdpHosts(raw);
		if (!result.ok) {
			error = result.error === 'bad_json' ? t('session.rdp_import_bad_json', { error: result.detail }) : t('session.rdp_import_none');
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

	/** The folder for a host: one already in the vault, else created once per import. */
	async function folderFor(name: string, created: Map<string, string>): Promise<string> {
		const known = created.get(name) ?? findFolder(name, target, folders)?.id;
		if (known) return known;
		const folder = await sessionCreateFolder(name, null, target);
		created.set(name, folder.id);
		return folder.id;
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
					username: h.username,
					authMethod: { type: 'Password' },
					folderId: h.folder ? await folderFor(h.folder, created) : null,
					vaultId: target,
					tags: ['rdp-import'],
					kind: 'rdp',
					domain: h.domain,
					detectedOs: 'windows'
				});
				count++;
			} catch (err) {
				failures.push({ name: h.name, error: String(err) });
			}
		}
		if (count > 0) {
			addToast(t('session.rdp_import_done', { count: String(count) }), 'success');
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
		parsed = false;
		error = undefined;
		hosts = [];
		skipped = [];
		failed = [];
		existingSessions = [];
		folders = [];
		selected = new Set();
		filter = '';
	}
</script>

<Modal {open} onclose={close} title={t('session.import_rdp')}>
	<div class="import-content">
		<p class="import-desc">{t('session.rdp_import_desc')}</p>

		{#if !parsed}
			<textarea
				class="tool-input json-input"
				bind:value={raw}
				aria-label={t('session.import_rdp')}
				placeholder={'[\n  { "name": "DC01", "ip": "10.0.0.10", "port": 3389, "username": "admin", "domain": "CORP", "folder": "HQ" }\n]'}
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
				<p class="import-empty">{t('session.rdp_import_none')}</p>
			{:else}
				<input
					class="tool-input filter-input"
					type="search"
					placeholder={t('session.rdp_import_filter')}
					aria-label={t('session.rdp_import_filter')}
					bind:value={filter}
					disabled={busy}
				/>
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
									<span class="host-name">{h.name}</span>
									{#if saved}<span class="badge-imported">{t('session.import_already_exists')}</span>{/if}
								</span>
								<span class="host-detail">{h.username ? `${h.username}@` : ''}{h.host}:{h.port}{h.domain ? ` · ${h.domain}` : ''}</span>
							</span>
							{#if h.folder}<span class="host-folder">{h.folder}</span>{/if}
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
		min-width: min(540px, 80vw);
	}

	.import-desc {
		margin: 0;
		font-size: 0.8125rem;
		line-height: 1.45;
		color: var(--color-text-secondary);
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
		font-family: var(--font-mono);
		color: var(--color-text-secondary);
		overflow-wrap: anywhere;
	}

	.host-folder {
		margin-left: auto;
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
