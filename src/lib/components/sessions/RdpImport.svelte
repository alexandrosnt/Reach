<script lang="ts">
	/**
	 * Bulk-import RDP hosts from pasted JSON. Accepts either a Termix export
	 * ({ hosts: [...] }, RDP entries are picked out) or a plain array of
	 * { name, ip|host, port, username, domain?, folder? } objects. Each host
	 * becomes an RDP session in the selected vault; folders named in the JSON
	 * are recreated (flat) in that vault. Passwords are never imported — RDP
	 * logons prompt on connect.
	 */
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import { sessionCreate, sessionCreateFolder } from '$lib/ipc/sessions';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		open: boolean;
		vaultId?: string | null;
		onsave?: () => void;
	}

	let { open = $bindable(), vaultId = null, onsave }: Props = $props();

	interface RdpHost {
		id: string;
		name: string;
		host: string;
		port: number;
		username: string;
		domain: string | null;
		folder: string | null;
	}

	let raw = $state('');
	let hosts = $state<RdpHost[]>([]);
	let selected = $state<Set<string>>(new Set());
	let filter = $state('');
	let parsed = $state(false);
	let importing = $state(false);
	let error = $state<string | undefined>();

	function num(v: unknown, fallback: number): number {
		const n = Number(v);
		return Number.isFinite(n) && n > 0 ? n : fallback;
	}

	function parse(): void {
		error = undefined;
		let data: unknown;
		try {
			data = JSON.parse(raw);
		} catch (e) {
			error = t('session.rdp_import_bad_json', { error: String(e) });
			return;
		}
		const arr: any[] = Array.isArray(data) ? data : ((data as any)?.hosts ?? []);
		if (!Array.isArray(arr)) {
			error = t('session.rdp_import_none');
			return;
		}
		const list: RdpHost[] = arr
			// Keep RDP only when a type is present; assume RDP when it is absent.
			.filter((h) => !h?.connectionType || h.connectionType === 'rdp')
			.map((h, i) => {
				const rdpPort = h?.pluginSettings?.['remote-desktop']?.rdpPort;
				return {
					id: `${i}:${h?.name ?? ''}`,
					name: String(h?.name ?? h?.ip ?? h?.host ?? `rdp-${i}`),
					host: String(h?.ip ?? h?.host ?? h?.address ?? '').trim(),
					port: num(h?.port ?? rdpPort, 3389),
					username: String(h?.username ?? '').trim(),
					domain: (h?.domain ?? null) || null,
					folder: (h?.folder ?? null) || null
				};
			})
			.filter((h) => h.host);
		hosts = list;
		selected = new Set(list.map((h) => h.id));
		parsed = true;
		if (list.length === 0) error = t('session.rdp_import_none');
	}

	const shownHosts = $derived(
		filter.trim()
			? hosts.filter((h) => {
					const q = filter.toLowerCase();
					return (
						h.name.toLowerCase().includes(q) ||
						h.host.toLowerCase().includes(q) ||
						(h.folder ?? '').toLowerCase().includes(q)
					);
				})
			: hosts
	);
	const selectedCount = $derived(selected.size);
	const canImport = $derived(parsed && selected.size > 0 && !importing);

	function toggle(id: string): void {
		const next = new Set(selected);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		selected = next;
	}

	async function handleImport(): Promise<void> {
		if (!canImport) return;
		importing = true;
		error = undefined;
		let count = 0;
		try {
			const folderCache = new Map<string, string>();
			for (const h of hosts) {
				if (!selected.has(h.id)) continue;
				let folderId: string | null = null;
				if (h.folder) {
					if (!folderCache.has(h.folder)) {
						const f = await sessionCreateFolder(h.folder, null, vaultId);
						folderCache.set(h.folder, f.id);
					}
					folderId = folderCache.get(h.folder) ?? null;
				}
				await sessionCreate({
					name: h.name,
					host: h.host,
					port: h.port,
					username: h.username,
					authMethod: { type: 'Password' }, // no password imported; prompted on connect
					folderId,
					vaultId,
					tags: ['rdp-import'],
					kind: 'rdp',
					domain: h.domain,
					detectedOs: 'windows',
					sharePath: null
				});
				count++;
			}
			addToast(t('session.rdp_import_done', { count: String(count) }), 'success');
			onsave?.();
			close();
		} catch (err) {
			error = String(err);
		} finally {
			importing = false;
		}
	}

	function close(): void {
		if (importing) return;
		open = false;
		raw = '';
		hosts = [];
		selected = new Set();
		filter = '';
		parsed = false;
		error = undefined;
	}
</script>

<Modal {open} onclose={close} title={t('session.import_rdp')}>
	<div class="rdp-import">
		<p class="desc">{t('session.rdp_import_desc')}</p>

		{#if !parsed}
			<textarea
				class="json-input"
				bind:value={raw}
				placeholder={'[\n  { "name": "DC01", "ip": "10.0.0.10", "port": 3389, "username": "admin", "domain": "CORP", "folder": "HQ" }\n]'}
				spellcheck="false"
				disabled={importing}
			></textarea>
			{#if error}<p class="err">{error}</p>{/if}
		{:else}
			<input
				class="filter-input"
				type="search"
				placeholder={t('session.rdp_import_filter')}
				bind:value={filter}
				disabled={importing}
			/>
			<div class="actions-row">
				<button class="link-btn" type="button" onclick={() => (selected = new Set(shownHosts.map((h) => h.id)))} disabled={importing}>
					{t('session.select_all_n', { count: String(shownHosts.length) })}
				</button>
				<button class="link-btn" type="button" onclick={() => (selected = new Set())} disabled={importing}>
					{t('session.import_deselect_all')}
				</button>
				<span class="count">{t('session.selected_n', { count: String(selectedCount) })}</span>
			</div>
			{#if error}<p class="err">{error}</p>{/if}
			<div class="host-list">
				{#each shownHosts as h (h.id)}
					<label class="host-row" class:on={selected.has(h.id)}>
						<input type="checkbox" checked={selected.has(h.id)} onchange={() => toggle(h.id)} disabled={importing} />
						<span class="host-main">
							<span class="host-name">{h.name}</span>
							<span class="host-sub">{h.username ? `${h.username}@` : ''}{h.host}:{h.port}{h.domain ? ` · ${h.domain}` : ''}</span>
						</span>
						{#if h.folder}<span class="host-folder">{h.folder}</span>{/if}
					</label>
				{/each}
			</div>
		{/if}
	</div>

	{#snippet actions()}
		<Button variant="secondary" onclick={close} disabled={importing}>{t('common.cancel')}</Button>
		{#if !parsed}
			<Button variant="primary" onclick={parse} disabled={!raw.trim() || importing}>{t('session.rdp_import_parse')}</Button>
		{:else}
			<Button variant="primary" onclick={handleImport} disabled={!canImport}>
				{#if importing}<span class="spinner"></span>{/if}
				{t('session.import_selected', { count: String(selectedCount) })}
			</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	.rdp-import {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		min-width: min(540px, 80vw);
	}
	.desc {
		margin: 0;
		font-size: 0.85rem;
		opacity: 0.8;
		line-height: 1.4;
	}
	.json-input {
		width: 100%;
		min-height: 220px;
		resize: vertical;
		font-family: var(--font-mono, monospace);
		font-size: 0.8rem;
		padding: 0.6rem;
		border-radius: 6px;
		border: 1px solid var(--border, rgba(128, 128, 128, 0.35));
		background: var(--input-bg, rgba(128, 128, 128, 0.08));
		color: inherit;
	}
	.filter-input {
		width: 100%;
		padding: 0.45rem 0.6rem;
		border-radius: 6px;
		border: 1px solid var(--border, rgba(128, 128, 128, 0.35));
		background: var(--input-bg, rgba(128, 128, 128, 0.08));
		color: inherit;
	}
	.actions-row {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		font-size: 0.82rem;
	}
	.link-btn {
		background: none;
		border: none;
		color: var(--color-accent, #4a9eff);
		cursor: pointer;
		padding: 0;
		font-size: 0.82rem;
	}
	.link-btn:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.count {
		margin-left: auto;
		opacity: 0.7;
	}
	.host-list {
		display: flex;
		flex-direction: column;
		gap: 2px;
		max-height: 44vh;
		overflow-y: auto;
		border: 1px solid var(--border, rgba(128, 128, 128, 0.25));
		border-radius: 6px;
		padding: 4px;
	}
	.host-row {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		padding: 0.4rem 0.5rem;
		border-radius: 5px;
		cursor: pointer;
	}
	.host-row:hover {
		background: rgba(128, 128, 128, 0.1);
	}
	.host-row.on {
		background: rgba(74, 158, 255, 0.12);
	}
	.host-main {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}
	.host-name {
		font-size: 0.86rem;
	}
	.host-sub {
		font-size: 0.75rem;
		opacity: 0.65;
	}
	.host-folder {
		margin-left: auto;
		font-size: 0.72rem;
		opacity: 0.6;
		white-space: nowrap;
	}
	.err {
		margin: 0;
		color: var(--color-error, #ff6b6b);
		font-size: 0.82rem;
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
</style>
