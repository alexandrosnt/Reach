<script lang="ts">
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import { sshConnect } from '$lib/ipc/ssh';
	import { createTab } from '$lib/state/tabs.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		open: boolean;
	}

	let { open = $bindable() }: Props = $props();

	let host = $state('');
	let portStr = $state('22');
	let username = $state('root');
	let authMethod = $state<'password' | 'key'>('password');
	let password = $state('');
	let keyPath = $state('');
	let keyPassphrase = $state('');
	let connecting = $state(false);
	let error = $state<string | undefined>();

	let port = $derived(parseInt(portStr, 10) || 22);
	let canConnect = $derived(host.trim().length > 0 && username.trim().length > 0 && !connecting);

	async function handleConnect(): Promise<void> {
		if (!canConnect) return;
		connecting = true;
		error = undefined;

		const id = crypto.randomUUID();

		try {
			await sshConnect({
				id,
				host: host.trim(),
				port,
				username: username.trim(),
				authMethod,
				password: authMethod === 'password' ? password : undefined,
				keyPath: authMethod === 'key' ? keyPath : undefined,
				keyPassphrase: authMethod === 'key' && keyPassphrase ? keyPassphrase : undefined,
				cols: 80,
				rows: 24
			});

			createTab('ssh', `${username.trim()}@${host.trim()}`, id);

			// Reset form
			host = '';
			portStr = '22';
			username = 'root';
			password = '';
			keyPath = '';
			keyPassphrase = '';
			error = undefined;
			open = false;
		} catch (err) {
			error = String(err);
		} finally {
			connecting = false;
		}
	}

	function handleClose(): void {
		if (!connecting) {
			open = false;
		}
	}
</script>

<Modal {open} onclose={handleClose} title={t('session.quick_connect')}>
	<form class="form" onsubmit={(e) => { e.preventDefault(); handleConnect(); }}>
		<div class="row">
			<div class="field-host">
				<Input label={t('session.host')} bind:value={host} placeholder="192.168.1.1" disabled={connecting} />
			</div>
			<div class="field-port">
				<Input label={t('session.port')} bind:value={portStr} type="number" placeholder="22" disabled={connecting} />
			</div>
		</div>

		<Input label={t('session.username')} bind:value={username} placeholder="root" disabled={connecting} />

		<div class="auth-section">
			<span class="auth-label">{t('session.auth_method')}</span>
			<div class="auth-toggle">
				<button
					type="button"
					class="auth-btn"
					class:active={authMethod === 'password'}
					disabled={connecting}
					onclick={() => (authMethod = 'password')}
				>
					{t('session.auth_password')}
				</button>
				<button
					type="button"
					class="auth-btn"
					class:active={authMethod === 'key'}
					disabled={connecting}
					onclick={() => (authMethod = 'key')}
				>
					{t('session.auth_key')}
				</button>
			</div>
		</div>

		{#if authMethod === 'password'}
			<Input label={t('session.password')} bind:value={password} type="password" disabled={connecting} />
		{:else}
			<Input label={t('session.key_path')} bind:value={keyPath} placeholder="~/.ssh/id_rsa" disabled={connecting} />
			<Input label={t('session.passphrase_optional')} bind:value={keyPassphrase} type="password" disabled={connecting} />
		{/if}

		{#if error}
			<div class="error-message">{error}</div>
		{/if}
	</form>

	{#snippet actions()}
		<Button variant="secondary" onclick={handleClose} disabled={connecting}>
			{t('common.cancel')}
		</Button>
		<Button variant="primary" onclick={handleConnect} disabled={!canConnect}>
			{#if connecting}
				<span class="spinner"></span>
				{t('session.connecting')}
			{:else}
				{t('session.connect')}
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

	.auth-btn:first-child {
		border-right: 1px solid var(--color-border);
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
