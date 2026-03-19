<script lang="ts">
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import { sessionCreate, sessionUpdate, type SessionConfig, type AuthMethod, type JumpHostConfig } from '$lib/ipc/sessions';
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
	let jumpEnabled = $state(false);
	let jumpHops = $state<Array<{host: string; port: string; username: string; authType: 'Password' | 'Key' | 'Agent'; password: string; keyPath: string; keyPassphrase: string}>>([]);
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
			if (editSession.jump_chain && editSession.jump_chain.length > 0) {
				jumpEnabled = true;
				jumpHops = editSession.jump_chain.map(j => ({
					host: j.host,
					port: String(j.port),
					username: j.username,
					authType: j.auth_method.type,
					password: j.auth_method.type === 'Password' ? (j.auth_method.password ?? '') : '',
					keyPath: j.auth_method.type === 'Key' ? (j.auth_method.path ?? '') : '',
					keyPassphrase: j.auth_method.type === 'Key' ? (j.auth_method.passphrase ?? '') : '',
				}));
			} else {
				jumpEnabled = false;
				jumpHops = [];
			}
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
			jumpEnabled = false;
			jumpHops = [];
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

		const jumpChain: JumpHostConfig[] | undefined = jumpEnabled && jumpHops.length > 0
			? jumpHops.map(h => {
				const hopAuth: AuthMethod = h.authType === 'Password'
					? { type: 'Password', password: h.password || undefined }
					: h.authType === 'Key'
						? { type: 'Key', path: h.keyPath, passphrase: h.keyPassphrase || undefined }
						: { type: 'Agent' };
				return {
					host: h.host.trim(),
					port: parseInt(h.port, 10) || 22,
					username: h.username.trim(),
					auth_method: hopAuth,
				};
			})
			: undefined;

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
					jump_chain: jumpChain ?? editSession.jump_chain ?? null,
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
					jumpChain: jumpChain ?? null,
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

	function addHop(): void {
		jumpHops = [...jumpHops, { host: '', port: '22', username: 'root', authType: 'Password', password: '', keyPath: '', keyPassphrase: '' }];
	}

	function removeHop(index: number): void {
		jumpHops = jumpHops.filter((_, i) => i !== index);
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

		<div class="jump-section">
			<label class="jump-toggle">
				<input type="checkbox" bind:checked={jumpEnabled} disabled={saving} />
				<span class="jump-toggle-text">{t('session.jump_host_enable')}</span>
				<span class="beta-badge">BETA</span>
			</label>

			{#if jumpEnabled}
				<p class="jump-hint">{t('session.jump_host_hint')}</p>

				{#each jumpHops as hop, i (i)}
					<div class="jump-hop">
						<div class="jump-hop-header">
							<span class="jump-hop-label">{t('session.jump_hop', { n: String(i + 1) })}</span>
							<button type="button" class="jump-hop-remove" onclick={() => removeHop(i)} disabled={saving}>
								{t('session.jump_remove_hop')}
							</button>
						</div>
						<div class="row">
							<div class="field-host">
								<Input label={t('session.host')} bind:value={hop.host} placeholder="bastion.example.com" disabled={saving} />
							</div>
							<div class="field-port">
								<Input label={t('session.port')} bind:value={hop.port} type="number" placeholder="22" disabled={saving} />
							</div>
						</div>
						<Input label={t('session.username')} bind:value={hop.username} placeholder="root" disabled={saving} />

						<div class="auth-section">
							<span class="auth-label">{t('session.auth_method')}</span>
							<div class="auth-toggle">
								<button type="button" class="auth-btn" class:active={hop.authType === 'Password'} disabled={saving} onclick={() => (hop.authType = 'Password')}>
									{t('session.auth_password')}
								</button>
								<button type="button" class="auth-btn" class:active={hop.authType === 'Key'} disabled={saving} onclick={() => (hop.authType = 'Key')}>
									{t('session.auth_key')}
								</button>
								<button type="button" class="auth-btn" class:active={hop.authType === 'Agent'} disabled={saving} onclick={() => (hop.authType = 'Agent')}>
									{t('session.auth_agent')}
								</button>
							</div>
						</div>

						{#if hop.authType === 'Password'}
							<Input label={t('session.password_optional')} bind:value={hop.password} type="password" disabled={saving} />
						{:else if hop.authType === 'Key'}
							<Input label={t('session.key_path')} bind:value={hop.keyPath} placeholder="~/.ssh/id_rsa" disabled={saving} />
							<Input label={t('session.passphrase_optional')} bind:value={hop.keyPassphrase} type="password" disabled={saving} />
						{/if}
					</div>
				{/each}

				<button type="button" class="jump-add-btn" onclick={addHop} disabled={saving}>
					+ {t('session.jump_add_hop')}
				</button>
			{/if}
		</div>

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

	.jump-section {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding-top: 4px;
	}

	.jump-toggle {
		display: flex;
		align-items: center;
		gap: 8px;
		cursor: pointer;
	}

	.jump-toggle input {
		width: 14px;
		height: 14px;
		accent-color: var(--color-accent);
	}

	.jump-toggle-text {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.beta-badge {
		padding: 1px 5px;
		font-size: 0.5rem;
		font-weight: 700;
		letter-spacing: 0.05em;
		color: #fff;
		background: linear-gradient(135deg, #ff6b35, #f7c948);
		border-radius: 3px;
		line-height: 1.4;
	}

	.jump-hint {
		margin: 0;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.jump-hop {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 10px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background-color: rgba(255, 255, 255, 0.02);
	}

	.jump-hop-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.jump-hop-label {
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.jump-hop-remove {
		padding: 2px 8px;
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		color: var(--color-danger);
		background: transparent;
		border: 1px solid rgba(255, 69, 58, 0.3);
		border-radius: 4px;
		cursor: pointer;
	}

	.jump-hop-remove:hover:not(:disabled) {
		background-color: rgba(255, 69, 58, 0.08);
	}

	.jump-hop-remove:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.jump-add-btn {
		padding: 6px 12px;
		font-family: var(--font-sans);
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-accent);
		background: transparent;
		border: 1px dashed var(--color-accent);
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.jump-add-btn:hover:not(:disabled) {
		background-color: rgba(0, 122, 255, 0.08);
	}

	.jump-add-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
</style>
