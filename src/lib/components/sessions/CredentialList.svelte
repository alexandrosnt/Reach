<script lang="ts">
	/**
	 * Saved credentials: a login kept once and used by many sessions in the
	 * same vault, linked from a session's editor or set on a folder for the
	 * sessions inside. Shown as a tab of the sessions sidebar.
	 */
	import Modal from '#lib/components/shared/Modal.svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import Input from '#lib/components/shared/Input.svelte';
	import KeyPicker from './KeyPicker.svelte';
	import FaIcon from '#lib/components/shared/FaIcon.svelte';
	import { faPlus } from '@fortawesome/free-solid-svg-icons';
	import { t } from '#lib/state/i18n.svelte.js';
	import { ALL_VAULTS, vaultState, type VaultFilter } from '#lib/state/vault.svelte.js';
	import type { AuthMethod, Folder, SessionConfig } from '#lib/ipc/sessions.js';
	import {
		savedCredentialList,
		savedCredentialCreate,
		savedCredentialUpdate,
		savedCredentialDelete,
		type SavedCredential
	} from '#lib/ipc/savedCredentials.js';

	interface Props {
		/** What the sidebar shows: all vaults, this device's own (null), or one. */
		vaultFilter: VaultFilter;
		/** Where a new credential goes. */
		targetVaultId: string | null;
		sessions: SessionConfig[];
		folders: Folder[];
		/** Something the session list shows changed. */
		onchange?: () => void;
	}

	let { vaultFilter, targetVaultId, sessions, folders, onchange }: Props = $props();

	let all = $state<SavedCredential[]>([]);
	let loading = $state(true);
	let error = $state<string | undefined>();

	async function load(): Promise<void> {
		try {
			all = await savedCredentialList();
			error = undefined;
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		void load();
	});

	let shown = $derived(
		all
			.filter((c) => vaultFilter === ALL_VAULTS || (c.vault_id ?? null) === (vaultFilter ?? null))
			.sort((a, b) => a.name.localeCompare(b.name))
	);

	/** Sessions whose login comes from a credential, directly or from a folder. */
	function usedBy(id: string): SessionConfig[] {
		return sessions.filter((s) => s.credential?.id === id && !s.credential.missing);
	}

	function foldersUsing(id: string): Folder[] {
		return folders.filter((f) => f.credential_id === id);
	}

	function vaultName(c: SavedCredential): string {
		return c.vault_id ? (vaultState.vaults.get(c.vault_id)?.name ?? t('vault.private')) : t('vault.private');
	}

	// ---- Editor -------------------------------------------------------------

	let editorOpen = $state(false);
	let editing = $state<SavedCredential | null>(null);
	let name = $state('');
	let username = $state('');
	let domain = $state('');
	let authType = $state<'Password' | 'Key' | 'Agent'>('Password');
	let password = $state('');
	let keyPath = $state('');
	let keyId = $state('');
	let passphrase = $state('');
	let saving = $state(false);
	let editorError = $state<string | undefined>();

	function openEditor(c: SavedCredential | null): void {
		editing = c;
		name = c?.name ?? '';
		username = c?.username ?? '';
		domain = c?.domain ?? '';
		const a = c?.auth_method;
		authType = a?.type ?? 'Password';
		password = a?.password ?? '';
		keyPath = a?.path ?? '';
		keyId = a?.key_id ?? '';
		passphrase = a?.passphrase ?? '';
		editorError = undefined;
		editorOpen = true;
	}

	let canSave = $derived(name.trim().length > 0 && username.trim().length > 0 && !saving);

	async function save(): Promise<void> {
		if (!canSave) return;
		const authMethod: AuthMethod =
			authType === 'Password'
				? { type: 'Password', password: password || undefined }
				: authType === 'Key'
					? { type: 'Key', path: keyId ? undefined : keyPath.trim() || undefined, key_id: keyId || undefined, passphrase: keyId ? undefined : passphrase || undefined }
					: { type: 'Agent' };
		saving = true;
		editorError = undefined;
		try {
			if (editing) {
				await savedCredentialUpdate({ ...editing, name: name.trim(), username: username.trim(), domain: domain.trim() || null, auth_method: authMethod });
			} else {
				await savedCredentialCreate({ name, username, domain, authMethod, vaultId: targetVaultId });
			}
			editorOpen = false;
			await load();
			onchange?.();
		} catch (e) {
			editorError = String(e);
		} finally {
			saving = false;
		}
	}

	// ---- Delete -------------------------------------------------------------

	let deleting = $state<SavedCredential | null>(null);
	let deleteBusy = $state(false);

	async function confirmDelete(): Promise<void> {
		if (!deleting) return;
		deleteBusy = true;
		try {
			await savedCredentialDelete(deleting.id);
			deleting = null;
			await load();
			onchange?.();
		} catch (e) {
			error = String(e);
			deleting = null;
		} finally {
			deleteBusy = false;
		}
	}
</script>

<div class="cred-list">
	<div class="cred-head">
		<p class="cred-intro">{t('credential.intro')}</p>
		<button class="add-btn" title={t('credential.new')} aria-label={t('credential.new')} onclick={() => openEditor(null)}>
			<FaIcon icon={faPlus} size={13} />
		</button>
	</div>

	{#if error}
		<p class="cred-error" role="alert">{error}</p>
	{/if}

	{#if loading}
		<div class="cred-empty"><span class="spinner"></span></div>
	{:else if shown.length === 0}
		<p class="cred-empty">{t('credential.none')}</p>
	{:else}
		<div class="cred-scroll">
			{#each shown as c (c.id)}
				{@const used = usedBy(c.id).length}
				<div class="cred-row">
					<button class="cred-main" onclick={() => openEditor(c)} title={t('common.edit')}>
						<span class="cred-name">{c.name}</span>
						<span class="cred-detail">
							{c.domain ? `${c.domain}\\` : ''}{c.username} · {t(`credential.auth_${c.auth_method.type.toLowerCase()}`)}
							{#if vaultFilter === ALL_VAULTS} · {vaultName(c)}{/if}
						</span>
						<span class="cred-usage">{t('credential.used_by', { count: String(used) })}</span>
					</button>
					<button class="cred-delete" onclick={() => (deleting = c)} title={t('common.delete')} aria-label={t('common.delete')}>×</button>
				</div>
			{/each}
		</div>
	{/if}
</div>

<Modal open={editorOpen} onclose={() => (editorOpen = false)} title={editing ? t('credential.edit') : t('credential.new')} maxWidth="440px">
	<form class="cred-form" onsubmit={(e) => { e.preventDefault(); void save(); }}>
		<Input label={t('credential.name')} bind:value={name} placeholder={t('credential.name_placeholder')} disabled={saving} />
		<Input label={t('session.username')} bind:value={username} placeholder="root" disabled={saving} />
		<div class="auth-toggle" role="radiogroup" aria-label={t('session.auth_method')}>
			{#each ['Password', 'Key', 'Agent'] as const as kind (kind)}
				<button type="button" class="auth-btn" class:active={authType === kind} disabled={saving} onclick={() => (authType = kind)}>
					{t(`credential.auth_${kind.toLowerCase()}`)}
				</button>
			{/each}
		</div>
		{#if authType === 'Password'}
			<Input label={t('session.password_optional')} type="password" bind:value={password} placeholder={t('session.stored_encrypted')} disabled={saving} />
		{:else if authType === 'Key'}
			<KeyPicker bind:path={keyPath} bind:keyId disabled={saving} />
			{#if !keyId}
				<Input label={t('session.passphrase_optional')} type="password" bind:value={passphrase} placeholder={t('session.stored_encrypted')} disabled={saving} />
			{/if}
		{/if}
		<Input label={t('credential.domain')} bind:value={domain} placeholder="CORP" disabled={saving} />
		<p class="cred-hint">{editing ? t('credential.edit_hint', { count: String(usedBy(editing.id).length) }) : t('credential.vault_hint')}</p>
		{#if editorError}<p class="cred-error" role="alert">{editorError}</p>{/if}
	</form>
	{#snippet actions()}
		<Button variant="ghost" onclick={() => (editorOpen = false)}>{t('common.cancel')}</Button>
		<Button variant="primary" disabled={!canSave} onclick={() => void save()}>{t('common.save')}</Button>
	{/snippet}
</Modal>

<Modal open={deleting !== null} onclose={() => (deleting = null)} title={t('credential.delete_title', { name: deleting?.name ?? '' })} maxWidth="420px">
	{#if deleting}
		{@const users = usedBy(deleting.id)}
		{@const inFolders = foldersUsing(deleting.id)}
		<p class="cred-hint">
			{users.length + inFolders.length > 0
				? t('credential.delete_used', { sessions: String(users.length), folders: String(inFolders.length) })
				: t('credential.delete_unused')}
		</p>
		{#if users.length > 0}
			<ul class="cred-users">
				{#each users.slice(0, 8) as s (s.id)}<li>{s.name}</li>{/each}
				{#if users.length > 8}<li>…</li>{/if}
			</ul>
		{/if}
	{/if}
	{#snippet actions()}
		<Button variant="ghost" onclick={() => (deleting = null)}>{t('common.cancel')}</Button>
		<Button variant="danger" disabled={deleteBusy} onclick={() => void confirmDelete()}>{t('common.delete')}</Button>
	{/snippet}
</Modal>

<style>
	.cred-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
		min-height: 0;
		flex: 1;
	}

	.cred-head {
		display: flex;
		align-items: flex-start;
		gap: 6px;
	}

	.cred-intro {
		flex: 1;
		margin: 0;
		font-size: var(--text-2xs);
		color: var(--color-text-secondary);
		line-height: 1.4;
	}

	.add-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 30px;
		height: 30px;
		flex-shrink: 0;
		padding: 0;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		background: transparent;
		color: var(--color-accent);
		cursor: pointer;
	}

	.add-btn:hover {
		background-color: rgba(0, 122, 255, 0.1);
	}

	.cred-scroll {
		display: flex;
		flex-direction: column;
		gap: 2px;
		overflow-y: auto;
		min-height: 0;
	}

	.cred-row {
		display: flex;
		align-items: stretch;
		border-radius: 6px;
	}

	.cred-row:hover {
		background-color: var(--color-surface-hover);
	}

	.cred-main {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 1px;
		flex: 1;
		min-width: 0;
		padding: 6px 8px;
		border: none;
		background: transparent;
		text-align: left;
		cursor: pointer;
		color: inherit;
	}

	.cred-name {
		font-size: var(--text-sm);
		font-weight: 500;
		color: var(--color-text-primary);
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.cred-detail,
	.cred-usage {
		font-size: var(--text-2xs);
		color: var(--color-text-secondary);
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.cred-delete {
		flex-shrink: 0;
		width: 26px;
		border: none;
		background: transparent;
		color: var(--color-text-secondary);
		font-size: 16px;
		cursor: pointer;
		opacity: 0;
	}

	.cred-row:hover .cred-delete,
	.cred-delete:focus-visible {
		opacity: 1;
	}

	.cred-delete:hover {
		color: var(--color-danger, #ef4444);
	}

	.cred-empty {
		margin: 12px 0;
		text-align: center;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.cred-error {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-danger, #ef4444);
	}

	.cred-form {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.auth-toggle {
		display: flex;
		gap: 4px;
	}

	.auth-btn {
		flex: 1;
		padding: 6px 8px;
		font-size: var(--text-xs);
		border: 1px solid var(--color-border);
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.auth-btn.active {
		border-color: var(--color-accent);
		color: var(--color-accent);
		background-color: rgba(0, 122, 255, 0.08);
	}

	.cred-hint {
		margin: 0;
		font-size: var(--text-2xs);
		color: var(--color-text-secondary);
		line-height: 1.4;
	}

	.cred-users {
		margin: 8px 0 0;
		padding-left: 18px;
		font-size: var(--text-xs);
		color: var(--color-text-primary);
	}
</style>
