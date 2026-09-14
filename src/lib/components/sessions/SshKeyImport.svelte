<!--
	Import a private key into the vault.

	Two ways in, because people have the key in two places: a file on this
	machine, or the clipboard. Either way what lands in the vault is the key
	material, so the session that uses it works on a machine that has never
	seen the file (issue #46).

	The passphrase is saved with the key, not with the session: unlock a key
	once and every session that uses it connects without asking again.
-->
<script lang="ts">
	import { open as openDialog } from '@tauri-apps/plugin-dialog';
	import { t } from '$lib/state/i18n.svelte';
	import { sshKeyImport, type StoredKeyInfo } from '$lib/ipc/sshkeys';
	import Modal from '$lib/components/shared/Modal.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import Button from '$lib/components/shared/Button.svelte';

	interface Props {
		onclose: () => void;
		onimported: (key: StoredKeyInfo) => void;
	}
	let { onclose, onimported }: Props = $props();

	let name = $state('');
	let privateKey = $state('');
	let passphrase = $state('');
	let saving = $state(false);
	let error = $state<string | undefined>();
	/** True once the user picked a file, so the textarea stays out of the way. */
	let fromFile = $state<string | undefined>();

	let ready = $derived(
		!saving && name.trim().length > 0 && (privateKey.trim().length > 0 || !!fromFile)
	);

	async function pickFile(): Promise<void> {
		try {
			const selected = await openDialog({
				multiple: false,
				directory: false,
				title: t('session.select_key_file'),
				filters: [
					{ name: t('session.ssh_private_key_filter'), extensions: ['pem', 'key', 'ppk', 'rsa', 'ed25519', 'ecdsa', 'dsa'] },
					{ name: 'All Files', extensions: ['*'] }
				]
			});
			if (typeof selected !== 'string') return;
			error = undefined;
			// The file is read in Rust at save time; the key never passes
			// through here.
			fromFile = selected;
			// A key named after its file is a key you recognise later.
			if (!name.trim()) {
				name = selected.replace(/\\/g, '/').split('/').pop() ?? '';
			}
		} catch (e) {
			error = String(e);
		}
	}

	function usePaste(): void {
		fromFile = undefined;
		privateKey = '';
	}

	async function save(): Promise<void> {
		if (!ready) return;
		saving = true;
		error = undefined;
		try {
			const key = await sshKeyImport({
				name: name.trim(),
				privateKey: fromFile ? undefined : privateKey,
				path: fromFile,
				passphrase: passphrase || undefined
			});
			onimported(key);
			onclose();
		} catch (e) {
			error = String(e);
		} finally {
			saving = false;
		}
	}
</script>

<Modal open title={t('session.key_import_title')} maxWidth="520px" {onclose}>
	<div class="import">
		<p class="explain">{t('session.key_import_explain')}</p>

		<Input label={t('session.key_import_name')} bind:value={name} placeholder={t('session.key_import_name_placeholder')} disabled={saving} />

		<div class="source">
			<span class="field-label">{t('session.key_import_source')}</span>
			{#if fromFile}
				<div class="picked">
					<span class="picked-path mono" title={fromFile}>{fromFile}</span>
					<button type="button" class="link" onclick={usePaste} disabled={saving}>{t('session.key_import_paste_instead')}</button>
				</div>
			{:else}
				<textarea
					class="key-area mono"
					bind:value={privateKey}
					rows="6"
					spellcheck="false"
					autocomplete="off"
					disabled={saving}
					placeholder={t('session.key_import_placeholder')}
				></textarea>
				<Button variant="secondary" size="sm" onclick={pickFile} disabled={saving}>
					{t('session.key_import_from_file')}
				</Button>
			{/if}
		</div>

		<Input
			label={t('session.passphrase_optional')}
			bind:value={passphrase}
			type="password"
			placeholder={t('session.key_import_passphrase_hint')}
			disabled={saving}
		/>

		{#if error}
			<div class="error">{error}</div>
		{/if}

		<div class="actions">
			<Button variant="secondary" size="sm" onclick={onclose} disabled={saving}>{t('common.cancel')}</Button>
			<Button variant="primary" size="sm" onclick={save} disabled={!ready}>
				{saving ? t('session.key_import_saving') : t('session.key_import_save')}
			</Button>
		</div>
	</div>
</Modal>

<style>
	.import {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.explain {
		margin: 0;
		font-size: 0.8125rem;
		line-height: 1.5;
		color: var(--color-text-secondary);
	}

	.source {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 6px;
	}

	.field-label {
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-text-secondary);
	}

	.key-area {
		width: 100%;
		padding: 8px 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: 0.75rem;
		line-height: 1.5;
		resize: vertical;
	}

	.key-area:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.mono {
		font-family: var(--font-mono, monospace);
	}

	.picked {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		min-width: 0;
	}

	.picked-path {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 0.75rem;
		color: var(--color-text-primary);
	}

	.link {
		background: none;
		border: none;
		padding: 0;
		color: var(--color-accent);
		font-family: inherit;
		font-size: 0.75rem;
		cursor: pointer;
		white-space: nowrap;
	}

	.link:hover {
		text-decoration: underline;
	}

	.error {
		font-size: 0.8125rem;
		line-height: 1.45;
		color: var(--color-danger);
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
	}
</style>
