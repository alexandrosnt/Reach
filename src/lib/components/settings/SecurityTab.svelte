<script lang="ts">
	import Button from '$lib/components/shared/Button.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import {
		hasMasterPassword,
		setMasterPassword,
		isLocked as checkIsLocked
	} from '$lib/ipc/credentials';
	import { t } from '$lib/state/i18n.svelte';
	import Dropdown from '$lib/components/shared/Dropdown.svelte';
	import Toggle from '$lib/components/shared/Toggle.svelte';
	import { getSettings, updateSetting } from '$lib/state/settings.svelte';
	import { biometricName, biometricPrompt, lock as lockVault } from '$lib/state/vault.svelte';
	import {
		biometricEnable,
		securityKeyAdd,
		unlockMethodRemove,
		unlockMethods,
		type UnlockMethods
	} from '$lib/ipc/vault';

	const settings = getSettings();
	const autoLockOptions = $derived([
		{ label: t('security.auto_lock_never'), value: '0' },
		...[1, 5, 15, 30, 60].map((m) => ({ label: t('security.auto_lock_minutes', { count: m }), value: String(m) }))
	]);

	let hasPassword = $state(false);
	let locked = $state(false);
	let loading = $state(true);

	let showPasswordForm = $state(false);
	let newPassword = $state('');
	let confirmPassword = $state('');
	let error = $state('');
	let saving = $state(false);

	// Device unlock methods: the platform biometric and security keys.
	let methods = $state<UnlockMethods | null>(null);
	let methodBusy = $state(false);
	let methodError = $state('');
	let addingKey = $state(false);
	let keyLabel = $state('');
	let keyPin = $state('');
	const biometric = $derived(methods?.methods.find((m) => m.kind !== 'fido2'));
	const method = $derived(biometricName(methods?.platform));
	const keys = $derived(methods?.methods.filter((m) => m.kind === 'fido2') ?? []);
	const biometricNote = $derived.by(() => {
		if (!methods?.platform_available) return t('security.biometric_unavailable', { method });
		if (!hasPassword) return t('security.biometric_needs_password', { method });
		return t('security.biometric_desc', { method });
	});

	async function changeMethods(change: () => Promise<void>) {
		methodBusy = true;
		methodError = '';
		try {
			await change();
		} catch (e) {
			methodError = String(e);
		}
		try {
			methods = await unlockMethods();
		} catch {
			// Keep the last known state
		}
		methodBusy = false;
	}

	function toggleBiometric(on: boolean) {
		void changeMethods(async () => {
			if (on) await biometricEnable(biometricPrompt(t('security.biometric', { method })));
			else if (biometric) await unlockMethodRemove(biometric.id);
		});
	}

	function addKey() {
		const label = keyLabel.trim() || t('security.keys_default_name');
		const pin = keyPin;
		keyPin = '';
		void changeMethods(async () => {
			await securityKeyAdd(label, methods?.keys_ask_pin ? pin : undefined);
			addingKey = false;
			keyLabel = '';
		});
	}

	$effect(() => {
		loadStatus();
	});

	async function loadStatus() {
		loading = true;
		try {
			hasPassword = await hasMasterPassword();
			locked = await checkIsLocked();
			methods = await unlockMethods().catch(() => null);
		} catch {
			// IPC not available in dev, set safe defaults
			hasPassword = false;
			locked = false;
		}
		loading = false;
	}

	function openPasswordForm() {
		showPasswordForm = true;
		newPassword = '';
		confirmPassword = '';
		error = '';
	}

	function cancelPasswordForm() {
		showPasswordForm = false;
		newPassword = '';
		confirmPassword = '';
		error = '';
	}

	async function savePassword() {
		error = '';

		if (newPassword.length < 8) {
			error = t('security.password_too_short');
			return;
		}

		if (newPassword !== confirmPassword) {
			error = t('security.passwords_mismatch');
			return;
		}

		saving = true;
		try {
			await setMasterPassword(newPassword);
			hasPassword = true;
			showPasswordForm = false;
			newPassword = '';
			confirmPassword = '';
		} catch (e) {
			error = e instanceof Error ? e.message : String(e) || 'Failed to set password';
		}
		saving = false;
	}

	async function handleLock() {
		try {
			// Held: the lock screen shows, and only the user opens it again.
			await lockVault();
			locked = true;
		} catch {
			// Lock failed
		}
	}
</script>

<div class="tab-content">
	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('security.master_password_status')}</span>
			<span class="setting-description">
				{#if loading}
					Checking...
				{:else if hasPassword}
					{t('security.password_set')}
				{:else}
					{t('security.no_password')}
				{/if}
			</span>
		</div>
		<div class="setting-control">
			<div class="status-badge" class:set={hasPassword && !loading}>
				{#if loading}
					...
				{:else if hasPassword}
					<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
						<path d="M2 7L5.5 10.5L12 3.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
					Set
				{:else}
					<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
						<path d="M1 1L13 13M13 1L1 13" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
					</svg>
					Not set
				{/if}
			</div>
		</div>
	</div>

	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('security.lock_status')}</span>
			<span class="setting-description">
				{#if locked}
					Credentials are locked and encrypted
				{:else}
					Credentials are currently accessible
				{/if}
			</span>
		</div>
		<div class="setting-control">
			<div class="lock-badge" class:locked>
				{#if locked}
					<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
						<rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
						<path d="M7 11V7a5 5 0 0 1 10 0v4" />
					</svg>
					{t('security.locked')}
				{:else}
					<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
						<rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
						<path d="M7 11V7a5 5 0 0 1 9.9-1" />
					</svg>
					{t('security.unlocked')}
				{/if}
			</div>
		</div>
	</div>

	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('security.auto_lock')}</span>
			<span class="setting-description">{t('security.auto_lock_desc')}</span>
		</div>
		<div class="setting-control">
			<Dropdown
				options={autoLockOptions}
				selected={String(settings.autoLockMinutes)}
				onchange={(value) => updateSetting('autoLockMinutes', Number(value))}
			/>
		</div>
	</div>

	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('security.lock_on_system_lock')}</span>
			<span class="setting-description">{t('security.lock_on_system_lock_desc')}</span>
		</div>
		<div class="setting-control">
			<Toggle
				hideLabel
				checked={settings.lockOnSystemLock}
				label={t('security.lock_on_system_lock')}
				onchange={(checked) => updateSetting('lockOnSystemLock', checked)}
			/>
		</div>
	</div>

	{#if methods?.platform}
		<div class="setting-row">
			<div class="setting-info">
				<span class="setting-label">{t('security.biometric', { method })}</span>
				<span class="setting-description">{biometricNote}</span>
			</div>
			<div class="setting-control">
				<Toggle
					hideLabel
					checked={!!biometric}
					label={t('security.biometric', { method })}
					disabled={methodBusy || locked || (!biometric && (!methods.platform_available || !hasPassword))}
					onchange={toggleBiometric}
				/>
			</div>
		</div>
	{/if}

	{#if methods?.keys_supported}
		<div class="setting-row keys-row">
			<div class="setting-info">
				<span class="setting-label">{t('security.keys')}</span>
				<span class="setting-description">
					{hasPassword ? t('security.keys_desc') : t('security.keys_needs_password')}
				</span>
				{#if keys.length}
					<ul class="key-list">
						{#each keys as key (key.id)}
							<li>
								<span class="key-name">{key.label}</span>
								<Button
									variant="ghost"
									size="sm"
									disabled={methodBusy || locked}
									onclick={() => void changeMethods(() => unlockMethodRemove(key.id))}
								>
									{t('security.keys_remove')}
								</Button>
							</li>
						{/each}
					</ul>
				{/if}
				{#if addingKey}
					<form
						class="key-form"
						onsubmit={(e) => {
							e.preventDefault();
							addKey();
						}}
					>
						<Input placeholder={t('security.keys_name_placeholder')} bind:value={keyLabel} />
						{#if methods.keys_ask_pin}
							<Input type="password" placeholder={t('lock.key_pin_placeholder')} bind:value={keyPin} />
						{/if}
						<span class="setting-description">{t('security.keys_touch_twice')}</span>
						<div class="form-actions">
							<Button variant="ghost" size="sm" disabled={methodBusy} onclick={() => (addingKey = false)}>
								{t('common.cancel')}
							</Button>
							<Button
								type="submit"
								size="sm"
								disabled={methodBusy || (methods.keys_ask_pin && !keyPin)}
							>
								{t('security.keys_add')}
							</Button>
						</div>
					</form>
				{/if}
				{#if methodError}
					<span class="form-error">{methodError}</span>
				{/if}
			</div>
			{#if !addingKey}
				<div class="setting-control">
					<Button
						variant="secondary"
						size="sm"
						disabled={methodBusy || locked || !hasPassword}
						onclick={() => (addingKey = true)}
					>
						{t('security.keys_add')}
					</Button>
				</div>
			{/if}
		</div>
	{:else if methodError}
		<span class="form-error">{methodError}</span>
	{/if}

	<div class="action-row">
		{#if !showPasswordForm}
			<Button
				variant="secondary"
				size="sm"
				onclick={openPasswordForm}
			>
				{hasPassword ? t('security.change_password') : t('security.set_password')}
			</Button>

			{#if !locked}
				<Button
					variant="danger"
					size="sm"
					onclick={handleLock}
				>
					{t('security.lock_now')}
				</Button>
			{/if}
		{/if}
	</div>

	{#if showPasswordForm}
		<div class="password-form">
			<div class="form-field">
				<Input
					label={t('security.new_password')}
					type="password"
					placeholder={t('settings.new_password_placeholder')}
					bind:value={newPassword}
				/>
			</div>

			<div class="form-field">
				<Input
					label={t('security.confirm_password')}
					type="password"
					placeholder={t('common.reenter_password')}
					bind:value={confirmPassword}
				/>
			</div>

			{#if error}
				<div class="form-error">{error}</div>
			{/if}

			<div class="form-actions">
				<Button
					variant="ghost"
					size="sm"
					onclick={cancelPasswordForm}
				>
					{t('common.cancel')}
				</Button>
				<Button
					variant="primary"
					size="sm"
					disabled={saving || newPassword.length === 0}
					onclick={savePassword}
				>
					{saving ? t('security.saving') : t('security.save_password')}
				</Button>
			</div>
		</div>
	{/if}
</div>

<style>

	.status-badge {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 4px 10px;
		border-radius: 6px;
		font-size: 0.75rem;
		font-weight: 500;
		background-color: rgba(255, 69, 58, 0.12);
		color: var(--color-danger);
	}

	.status-badge.set {
		background-color: rgba(48, 209, 88, 0.12);
		color: var(--color-success);
	}

	.lock-badge {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 4px 10px;
		border-radius: 6px;
		font-size: 0.75rem;
		font-weight: 500;
		background-color: rgba(255, 214, 10, 0.12);
		color: var(--color-warning);
	}

	.lock-badge.locked {
		background-color: rgba(48, 209, 88, 0.12);
		color: var(--color-success);
	}

	.keys-row {
		align-items: flex-start;
	}

	.key-list {
		list-style: none;
		margin: 8px 0 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.key-list li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
	}

	.key-name {
		font-size: 0.9em;
		color: var(--color-text-primary);
	}

	.key-form {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin-top: 8px;
	}

	.password-form {
		display: flex;
		flex-direction: column;
		gap: 12px;
		padding: 16px;
		margin-top: 8px;
		background-color: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-card);
	}

	.form-field {
		width: 100%;
	}

	.form-error {
		font-size: 0.75rem;
		color: var(--color-danger);
		padding: 4px 0;
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		padding-top: 4px;
	}
</style>
