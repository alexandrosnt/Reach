<script lang="ts">
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import { sessionCreate, sessionUpdate, type SessionConfig, type AuthMethod } from '$lib/ipc/sessions';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		open: boolean;
		editSession?: SessionConfig;
		vaultId?: string | null; // Which vault to save to (null = private)
		onsave?: () => void;
	}

	let { open = $bindable(), editSession, vaultId = null, onsave }: Props = $props();

	let name = $state('');
	let host = $state('');
	let portStr = $state('22');
	let username = $state('root');
	let authType = $state<'Password' | 'Key' | 'Agent'>('Password');
	let password = $state('');
	let keyPath = $state('');
	let keyPassphrase = $state('');
	let tagsStr = $state('');
	let saving = $state(false);
	let error = $state<string | undefined>();

	let isEditing = $derived(!!editSession);
	let canSave = $derived(name.trim().length > 0 && host.trim().length > 0 && username.trim().length > 0 && !saving);

	// Populate fields when editing, reset when creating
	$effect(() => {
		if (editSession) {
			name = editSession.name;
			host = editSession.host;
			portStr = String(editSession.port);
			username = editSession.username;
			authType = editSession.auth_method.type;
			password = editSession.auth_method.password ?? '';
			keyPath = editSession.auth_method.path ?? '';
			keyPassphrase = editSession.auth_method.passphrase ?? '';
			tagsStr = editSession.tags.join(', ');
		} else {
			name = '';
			host = '';
			portStr = '22';
			username = 'root';
			authType = 'Password';
			password = '';
			keyPath = '';
			keyPassphrase = '';
			tagsStr = '';
		}
		error = undefined;
	});

	async function handleSave(): Promise<void> {
		if (!canSave) return;
		saving = true;
		error = undefined;

		const port = parseInt(portStr, 10) || 22;
		const authMethod: AuthMethod = authType === 'Password'
			? { type: 'Password', password: password || undefined }
			: authType === 'Key'
				? { type: 'Key', path: keyPath, passphrase: keyPassphrase || undefined }
				: { type: 'Agent' };
		const tags = tagsStr.split(',').map(t => t.trim()).filter(Boolean);

		try {
			if (isEditing && editSession) {
				await sessionUpdate({
					...editSession,
					name: name.trim(),
					host: host.trim(),
					port,
					username: username.trim(),
					auth_method: authMethod,
					tags,
				});
			} else {
				await sessionCreate({
					name: name.trim(),
					host: host.trim(),
					port,
					username: username.trim(),
					authMethod: authMethod,
					folderId: null,
					tags,
					vaultId,
				});
			}
			onsave?.();
			open = false;
		} catch (err) {
			error = String(err);
		} finally {
			saving = false;
		}
	}

	function handleClose(): void {
		if (!saving) {
			open = false;
		}
	}
</script>

<Modal {open} onclose={handleClose} title={isEditing ? t('session.edit_session') : t('session.new')}>
	<form class="form" onsubmit={(e) => { e.preventDefault(); handleSave(); }}>
		<Input label={t('session.name')} bind:value={name} placeholder="My Server" disabled={saving} />

		<div class="row">
			<div class="field-host">
				<Input label={t('session.host')} bind:value={host} placeholder="192.168.1.1" disabled={saving} />
			</div>
			<div class="field-port">
				<Input label={t('session.port')} bind:value={portStr} type="number" placeholder="22" disabled={saving} />
			</div>
		</div>

		<Input label={t('session.username')} bind:value={username} placeholder="root" disabled={saving} />

		<div class="auth-section">
			<span class="auth-label">{t('session.auth_method')}</span>
			<div class="auth-toggle">
				<button
					type="button"
					class="auth-btn"
					class:active={authType === 'Password'}
					disabled={saving}
					onclick={() => (authType = 'Password')}
				>
					{t('session.auth_password')}
				</button>
				<button
					type="button"
					class="auth-btn"
					class:active={authType === 'Key'}
					disabled={saving}
					onclick={() => (authType = 'Key')}
				>
					{t('session.auth_key')}
				</button>
				<button
					type="button"
					class="auth-btn"
					class:active={authType === 'Agent'}
					disabled={saving}
					onclick={() => (authType = 'Agent')}
				>
					{t('session.auth_agent')}
				</button>
			</div>
		</div>

		{#if authType === 'Password'}
			<Input label={t('session.password_optional')} bind:value={password} type="password" placeholder="Stored encrypted in vault" disabled={saving} />
		{:else if authType === 'Key'}
			<Input label={t('session.key_path')} bind:value={keyPath} placeholder="~/.ssh/id_rsa" disabled={saving} />
			<Input label={t('session.passphrase_optional')} bind:value={keyPassphrase} type="password" placeholder="Stored encrypted in vault" disabled={saving} />
		{/if}

		<Input label={t('session.tags')} bind:value={tagsStr} placeholder="production, web, linux" disabled={saving} />

		{#if error}
			<div class="error-message">{error}</div>
		{/if}
	</form>

	{#snippet actions()}
		<Button variant="secondary" onclick={handleClose} disabled={saving}>
			{t('common.cancel')}
		</Button>
		<Button variant="primary" onclick={handleSave} disabled={!canSave}>
			{#if saving}
				<span class="spinner"></span>
				{t('session.saving')}
			{:else}
				{isEditing ? t('session.update_session') : t('session.save_session')}
			{/if}
		</Button>
	{/snippet}
</Modal>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.row {
		display: flex;
		gap: 10px;
		align-items: flex-start;
	}

	.field-host {
		flex: 1;
		min-width: 0;
	}

	.field-port {
		width: 80px;
		flex-shrink: 0;
	}

	.auth-section {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.auth-label {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
	}

	.auth-toggle {
		display: flex;
		gap: 0;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		overflow: hidden;
	}

	.auth-btn {
		flex: 1;
		padding: 7px 12px;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		font-weight: 500;
		border: none;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition:
			background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.auth-btn:hover:not(:disabled) {
		background-color: rgba(255, 255, 255, 0.04);
	}

	.auth-btn.active {
		background-color: var(--color-accent);
		color: #fff;
	}

	.auth-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.auth-btn + .auth-btn {
		border-left: 1px solid var(--color-border);
	}

	.error-message {
		padding: 8px 12px;
		font-size: 0.8125rem;
		color: var(--color-danger);
		background-color: rgba(255, 69, 58, 0.08);
		border: 1px solid rgba(255, 69, 58, 0.2);
		border-radius: var(--radius-btn);
	}

	.spinner {
		display: inline-block;
		width: 14px;
		height: 14px;
		border: 2px solid rgba(255, 255, 255, 0.3);
		border-top-color: #fff;
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
