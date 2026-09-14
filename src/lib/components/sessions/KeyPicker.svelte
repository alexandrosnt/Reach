<!--
	Which key a session authenticates with: one imported into the vault, or a
	file on this machine.

	An imported key travels with the session, so the same session connects from
	a laptop that has never seen your ~/.ssh (issue #46). A file path is still
	offered because it is what a machine-local setup expects, and because every
	session saved before this existed uses one.

	The picker defaults to whichever the session already has, and to the
	imported list when there is anything in it — that is the choice that keeps
	working everywhere.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { sshKeyList, sshKeyDelete, type StoredKeyInfo } from '$lib/ipc/sshkeys';
	import KeyPathInput from './KeyPathInput.svelte';
	import SshKeyImport from './SshKeyImport.svelte';

	interface Props {
		/** A key file on this machine. */
		path?: string;
		/** An imported key's id. Wins over `path` when set. */
		keyId?: string;
		disabled?: boolean;
	}

	let { path = $bindable(''), keyId = $bindable(''), disabled = false }: Props = $props();

	let keys = $state<StoredKeyInfo[]>([]);
	let loaded = $state(false);
	let showImport = $state(false);
	/** Set while a removal is one click from happening. */
	let confirmRemove = $state(false);

	let mode = $state<'imported' | 'file'>(keyId ? 'imported' : 'file');

	onMount(load);

	async function load(): Promise<void> {
		try {
			keys = await sshKeyList();
		} catch {
			// A locked vault simply means no imported keys to offer.
			keys = [];
		}
		loaded = true;
		// Nothing chosen yet and keys are available: start where the user most
		// likely wants to be. An existing path is left alone.
		if (!keyId && !path.trim() && keys.length > 0) mode = 'imported';
	}

	function choose(mode_: 'imported' | 'file'): void {
		mode = mode_;
		// Only one of the two can be in force, or the backend would have to
		// guess which the user meant.
		if (mode_ === 'imported') path = '';
		else keyId = '';
	}

	/**
	 * Imported keys are hidden from the vault panel like every other internal
	 * vault, so this is the only place one can be taken back out.
	 */
	async function remove(): Promise<void> {
		if (!keyId) return;
		const id = keyId;
		confirmRemove = false;
		keyId = '';
		try {
			await sshKeyDelete(id);
		} finally {
			keys = keys.filter((k) => k.id !== id);
		}
	}

	function onImported(key: StoredKeyInfo): void {
		keys = [...keys, key].sort((a, b) => a.name.localeCompare(b.name));
		keyId = key.id;
		path = '';
		mode = 'imported';
	}

	let selected = $derived(keys.find((k) => k.id === keyId));
	/** An encrypted key with no saved passphrase asks at connect time. */
	let willAsk = $derived(!!selected && selected.encrypted && !selected.hasPassphrase);
	/**
	 * The session names a key this machine does not hold — shared by someone
	 * whose vault we cannot read, or sync has not caught up. Saying so beats
	 * showing an empty picker, which would look like nothing was ever chosen
	 * and would drop the reference on the next save.
	 */
	let missing = $derived(loaded && !!keyId && !selected);
</script>

<div class="key-picker">
	<div class="modes" role="group" aria-label={t('session.key_source')}>
		<button
			type="button"
			class="mode-btn"
			class:active={mode === 'imported'}
			{disabled}
			onclick={() => choose('imported')}
		>
			{t('session.key_source_imported')}
		</button>
		<button
			type="button"
			class="mode-btn"
			class:active={mode === 'file'}
			{disabled}
			onclick={() => choose('file')}
		>
			{t('session.key_source_file')}
		</button>
	</div>

	{#if mode === 'imported'}
		{#if loaded && keys.length === 0 && !keyId}
			<p class="empty">{t('session.key_none_imported')}</p>
		{:else}
			<select class="key-select" bind:value={keyId} {disabled}>
				<option value="">{t('session.key_select')}</option>
				{#if missing}
					<option value={keyId}>{t('session.key_missing_option')}</option>
				{/if}
				{#each keys as k (k.id)}
					<option value={k.id}>{k.name}{k.algo ? ` · ${k.algo}` : ''}</option>
				{/each}
			</select>
			{#if missing}
				<p class="warn">{t('session.key_missing')}</p>
			{/if}
			{#if selected}
				<div class="detail">
					{#if selected.fingerprint}
						<span class="fingerprint mono" title={selected.fingerprint}>{selected.fingerprint}</span>
					{/if}
					{#if willAsk}
						<span class="note">{t('session.key_will_ask')}</span>
					{:else if selected.encrypted}
						<span class="note ok">{t('session.key_unlocked')}</span>
					{/if}
					{#if confirmRemove}
						<span class="note">{t('session.key_forget_confirm')}</span>
						<button type="button" class="link danger" {disabled} onclick={remove}>{t('common.yes')}</button>
						<button type="button" class="link" {disabled} onclick={() => (confirmRemove = false)}>{t('common.cancel')}</button>
					{:else}
						<button type="button" class="link" {disabled} onclick={() => (confirmRemove = true)}>
							{t('session.key_forget')}
						</button>
					{/if}
				</div>
			{/if}
		{/if}
		<button type="button" class="link" {disabled} onclick={() => (showImport = true)}>
			{t('session.key_import_action')}
		</button>
	{:else}
		<KeyPathInput bind:value={path} {disabled} />
	{/if}
</div>

{#if showImport}
	<SshKeyImport onclose={() => (showImport = false)} onimported={onImported} />
{/if}

<style>
	.key-picker {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 8px;
	}

	.modes {
		display: inline-flex;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		overflow: hidden;
	}

	.mode-btn {
		padding: 5px 12px;
		border: none;
		background: transparent;
		color: var(--color-text-secondary);
		font-family: inherit;
		font-size: 0.75rem;
		font-weight: 500;
		cursor: pointer;
		white-space: nowrap;
		transition:
			background-color var(--duration-default, 0.12s) var(--ease-default, ease),
			color var(--duration-default, 0.12s) var(--ease-default, ease);
	}

	.mode-btn + .mode-btn {
		border-left: 1px solid var(--color-border);
	}

	.mode-btn:hover:not(:disabled) {
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
	}

	.mode-btn.active {
		color: var(--color-accent);
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
	}

	.mode-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.key-select {
		width: 100%;
		padding: 8px 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: inherit;
		font-size: 0.8125rem;
	}

	.key-select:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.detail {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 4px 10px;
		min-width: 0;
		max-width: 100%;
	}

	.fingerprint {
		font-size: 0.6875rem;
		color: var(--color-text-tertiary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}

	.mono {
		font-family: var(--font-mono, monospace);
	}

	.note {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.note.ok {
		color: var(--color-success, #30d158);
	}

	.warn {
		margin: 0;
		font-size: 0.6875rem;
		line-height: 1.45;
		color: var(--color-warning, #ffd60a);
	}

	.empty {
		margin: 0;
		font-size: 0.75rem;
		line-height: 1.45;
		color: var(--color-text-secondary);
	}

	.link {
		background: none;
		border: none;
		padding: 0;
		color: var(--color-accent);
		font-family: inherit;
		font-size: 0.75rem;
		cursor: pointer;
	}

	.link:hover:not(:disabled) {
		text-decoration: underline;
	}

	.link:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.link.danger {
		color: var(--color-danger);
	}
</style>
