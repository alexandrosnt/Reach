<script lang="ts">
	import Button from '$lib/components/shared/Button.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import Modal from '$lib/components/shared/Modal.svelte';
	import {
		vaultState,
		unlock,
		initIdentity,
		createVault,
		setActiveVault,
		checkState,
		lock,
		refreshVaults,
		importBackup,
		previewBackup,
		type VaultInfo,
		type BackupPreview
	} from '$lib/state/vault.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { untrack } from 'svelte';
	import { open, message } from '@tauri-apps/plugin-dialog';
	import { relaunch } from '@tauri-apps/plugin-process';
	import SecretList from './SecretList.svelte';
	import InviteMemberDialog from './InviteMemberDialog.svelte';

	// State
	let password = $state('');
	let confirmPassword = $state('');
	let loading = $state(false);
	let error = $state('');
	let showCreateVault = $state(false);
	let newVaultName = $state('');
	let newVaultType = $state<'private' | 'shared'>('private');
	let creatingVault = $state(false);
	let showInviteDialog = $state(false);
	let refreshing = $state(false);

	// Import backup state
	let showImportBackup = $state(false);
	let importFilePath = $state('');
	let importExportPassword = $state('');
	let importMasterPassword = $state('');
	let importPreview = $state<BackupPreview | null>(null);
	let importVerifying = $state(false);
	let importLoading = $state(false);
	let importError = $state('');

	// Direct access to reactive state object
	let locked = $derived(vaultState.locked);
	let hasIdentity = $derived(vaultState.hasIdentity);
	let vaultList = $derived(vaultState.vaultList);
	let activeVault = $derived(vaultState.activeVault);
	let activeVaultId = $derived(vaultState.activeVaultId);

	// Check initial state on mount
	$effect(() => {
		untrack(() => {
			checkState().catch((err) => {
				console.error('Failed to check vault state:', err);
			});
		});
	});

	async function handleUnlock(): Promise<void> {
		if (!password) {
			error = t('vault.password_required');
			return;
		}

		loading = true;
		error = '';

		try {
			const success = await unlock(password);
			if (success) {
				password = '';
				addToast(t('vault.unlocked_toast'), 'success');
			} else {
				error = t('vault.invalid_password');
			}
		} catch (err) {
			error = String(err);
		} finally {
			loading = false;
		}
	}

	async function handleCreateIdentity(): Promise<void> {
		if (!password) {
			error = t('vault.password_required');
			return;
		}

		if (password.length < 8) {
			error = t('vault.password_min_chars');
			return;
		}

		if (password !== confirmPassword) {
			error = t('vault.passwords_mismatch');
			return;
		}

		loading = true;
		error = '';

		try {
			await initIdentity(password);
			password = '';
			confirmPassword = '';
			addToast(t('vault.identity_created_toast'), 'success');
		} catch (err) {
			error = String(err);
		} finally {
			loading = false;
		}
	}

	async function handleRefresh(): Promise<void> {
		refreshing = true;
		try {
			await refreshVaults();
			addToast(t('vault.vaults_refreshed_toast'), 'success');
		} catch (err) {
			addToast(`Failed to refresh: ${err}`, 'error');
		} finally {
			refreshing = false;
		}
	}

	async function handleLock(): Promise<void> {
		try {
			await lock();
			addToast(t('vault.locked_toast'), 'info');
		} catch (err) {
			addToast(`Failed to lock: ${err}`, 'error');
		}
	}

	async function handleCreateVault(): Promise<void> {
		if (!newVaultName.trim()) {
			return;
		}

		creatingVault = true;

		try {
			await createVault(newVaultName.trim(), newVaultType);
			newVaultName = '';
			newVaultType = 'private';
			showCreateVault = false;
			addToast(t('vault.vault_created_toast'), 'success');
		} catch (err) {
			addToast(`Failed to create vault: ${err}`, 'error');
		} finally {
			creatingVault = false;
		}
	}

	async function handleSelectVault(vault: VaultInfo): Promise<void> {
		try {
			await setActiveVault(vault.id);
		} catch (err) {
			addToast(`Failed to open vault: ${err}`, 'error');
		}
	}

	async function handleBackToList(): Promise<void> {
		try {
			await setActiveVault(null);
		} catch (err) {
			console.error('Failed to close vault:', err);
		}
	}

	async function handleSelectBackupFile(): Promise<void> {
		const path = await open({
			filters: [{ name: 'Reach Backup', extensions: ['reachbackup'] }],
			multiple: false
		});
		if (path) {
			importFilePath = path as string;
			importPreview = null;
			importError = '';
		}
	}

	async function handleVerifyBackup(): Promise<void> {
		if (!importFilePath || !importExportPassword) return;
		importVerifying = true;
		importError = '';
		try {
			importPreview = await previewBackup(importFilePath, importExportPassword);
		} catch (e) {
			importError = e instanceof Error ? e.message : String(e);
		} finally {
			importVerifying = false;
		}
	}

	async function handleImportBackup(): Promise<void> {
		if (!importFilePath || !importExportPassword) return;
		importLoading = true;
		importError = '';
		try {
			await importBackup(importFilePath, importExportPassword, importMasterPassword);
			addToast(t('vault.backup_restored_toast'), 'success');
			await message('Backup restored successfully. The app will now restart to apply all settings.', { title: 'Import Complete', kind: 'info' });
			await relaunch();
		} catch (e) {
			importError = e instanceof Error ? e.message : String(e);
		} finally {
			importLoading = false;
		}
	}

	function handleKeydown(e: KeyboardEvent): void {
		if (e.key === 'Enter') {
			if (hasIdentity) {
				handleUnlock();
			} else {
				handleCreateIdentity();
			}
		}
	}
</script>

<div class="vault-panel">
	{#if locked}
		<!-- Lock Screen -->
		<div class="lock-screen">
			<div class="lock-icon">
				<svg width="48" height="48" viewBox="0 0 24 24" fill="none">
					<rect x="3" y="11" width="18" height="11" rx="2" stroke="currentColor" stroke-width="1.5" />
					<path d="M7 11V7a5 5 0 0110 0v4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
					<circle cx="12" cy="16" r="1.5" fill="currentColor" />
				</svg>
			</div>

			{#if hasIdentity}
				<!-- Unlock existing identity -->
				<h2 class="lock-title">{t('vault.unlock')}</h2>
				<p class="lock-description">{t('vault.enter_password')}</p>

				<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div class="lock-form" onkeydown={handleKeydown}>
					<Input
						type="password"
						label={t('vault.master_password')}
						placeholder={t('vault.enter_password')}
						bind:value={password}
						disabled={loading}
					/>

					{#if error}
						<div class="error-message">{error}</div>
					{/if}

					<Button variant="primary" onclick={handleUnlock} disabled={loading || !password}>
						{#if loading}{t('vault.unlocking')}{:else}{t('vault.unlock')}{/if}
					</Button>
				</div>
			{:else}
				<!-- Create new identity -->
				<h2 class="lock-title">{t('vault.create_identity')}</h2>
				<p class="lock-description">
					{t('vault.create_identity_desc')}
				</p>

				<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div class="lock-form" onkeydown={handleKeydown}>
					<Input
						type="password"
						label={t('vault.master_password')}
						placeholder={t('vault.min_chars')}
						bind:value={password}
						disabled={loading}
					/>

					<Input
						type="password"
						label={t('vault.confirm_password')}
						placeholder={t('vault.confirm_password')}
						bind:value={confirmPassword}
						disabled={loading}
					/>

					{#if error}
						<div class="error-message">{error}</div>
					{/if}

					<Button
						variant="primary"
						onclick={handleCreateIdentity}
						disabled={loading || !password || !confirmPassword}
					>
						{#if loading}{t('vault.creating')}{:else}{t('vault.create_identity')}{/if}
					</Button>

					<button class="import-link" onclick={() => (showImportBackup = true)}>
						{t('vault.or_restore')}
					</button>
				</div>
			{/if}
		</div>
	{:else if activeVault}
		<!-- Active Vault View -->
		<div class="vault-view">
			<div class="vault-header">
				<button class="back-btn" onclick={handleBackToList} aria-label={t('vault.back_to_list')}>
					<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
						<path d="M15 18l-6-6 6-6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
				</button>
				<div class="vault-info">
					<span class="vault-name">{activeVault.name}</span>
					<span class="vault-badge" class:shared={activeVault.vaultType === 'shared'}>
						{activeVault.vaultType}
					</span>
				</div>
				{#if activeVault.vaultType === 'shared'}
					<button class="invite-btn" onclick={() => (showInviteDialog = true)} title={t('vault.manage_members')}>
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
							<path d="M16 21v-2a4 4 0 00-4-4H5a4 4 0 00-4 4v2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
							<circle cx="8.5" cy="7" r="4" stroke="currentColor" stroke-width="1.5" />
							<path d="M20 8v6M23 11h-6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
						</svg>
					</button>
				{/if}
			</div>

			<div class="secret-list-container">
				<SecretList />
			</div>
		</div>
	{:else}
		<!-- Vault List -->
		<div class="vault-list-view">
			<div class="list-header">
				<div class="header-row">
					<span class="header-title">{t('vault.vaults')}</span>
					<div class="header-actions">
						<button class="header-btn" onclick={handleRefresh} disabled={refreshing} title={t('vault.refresh_vaults')}>
							<svg class:spinning={refreshing} width="14" height="14" viewBox="0 0 24 24" fill="none">
								<path d="M1 4v6h6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
								<path d="M3.51 15a9 9 0 105.64-9.94L1 10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
						</button>
						<button class="header-btn" onclick={handleLock} title={t('vault.lock')}>
							<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
								<rect x="3" y="11" width="18" height="11" rx="2" stroke="currentColor" stroke-width="1.5" />
								<path d="M7 11V7a5 5 0 0110 0v4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
							</svg>
						</button>
					</div>
				</div>
				<button class="create-vault-btn" onclick={() => (showCreateVault = true)}>
					<svg width="12" height="12" viewBox="0 0 24 24" fill="none">
						<path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
					</svg>
					{t('vault.new_vault')}
				</button>
			</div>

			{#if vaultList.length === 0}
				<div class="empty-state">
					<p class="empty-text">{t('vault.no_vaults')}</p>
					<p class="empty-hint">{t('vault.create_prompt')}</p>
				</div>
			{:else}
				<div class="vault-list">
					{#each vaultList as vault (vault.id)}
						<div class="vault-card-wrapper">
							<button
								class="vault-card"
								class:active={activeVaultId === vault.id}
								onclick={() => handleSelectVault(vault)}
							>
								<div class="vault-card-icon">
									{#if vault.vaultType === 'shared'}
										<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
											<path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4 4v2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
											<circle cx="9" cy="7" r="4" stroke="currentColor" stroke-width="1.5" />
											<path d="M23 21v-2a4 4 0 00-3-3.87M16 3.13a4 4 0 010 7.75" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
										</svg>
									{:else}
										<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
											<rect x="3" y="11" width="18" height="11" rx="2" stroke="currentColor" stroke-width="1.5" />
											<path d="M7 11V7a5 5 0 0110 0v4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
										</svg>
									{/if}
								</div>
								<div class="vault-card-content">
									<span class="vault-card-name">{vault.name}</span>
									<span class="vault-card-meta">
										{t('vault.n_secrets', { count: vault.secretCount })}
										{#if vault.vaultType === 'shared' && vault.memberCount}
											&middot; {t('vault.n_members', { count: vault.memberCount })}
										{/if}
									</span>
								</div>
								<div class="vault-card-badge" class:shared={vault.vaultType === 'shared'}>
									{vault.vaultType === 'shared' ? t('vault.shared') : t('vault.private')}
								</div>
							</button>
							{#if vault.vaultType === 'shared'}
								<button
									class="vault-card-invite"
									onclick={(e) => { e.stopPropagation(); handleSelectVault(vault).then(() => { showInviteDialog = true; }); }}
									title={t('vault.invite_members')}
								>
									<svg width="12" height="12" viewBox="0 0 24 24" fill="none">
										<path d="M16 21v-2a4 4 0 00-4-4H5a4 4 0 00-4 4v2" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
										<circle cx="8.5" cy="7" r="4" stroke="currentColor" stroke-width="2" />
										<path d="M20 8v6M23 11h-6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
									</svg>
								</button>
							{/if}
						</div>
					{/each}
				</div>
			{/if}
		</div>
	{/if}
</div>

<!-- Create Vault Modal -->
<Modal
	open={showCreateVault}
	onclose={() => (showCreateVault = false)}
	title={t('vault.create_new_vault')}
>
	<div class="create-vault-form">
		<Input
			label={t('vault.vault_name')}
			placeholder="My Secrets"
			bind:value={newVaultName}
			disabled={creatingVault}
		/>

		<div class="type-selector">
			<span class="type-label">{t('vault.vault_type')}</span>
			<div class="type-options" role="radiogroup" aria-label={t('vault.type_selection')}>
				<button
					class="type-option"
					class:active={newVaultType === 'private'}
					onclick={() => (newVaultType = 'private')}
					disabled={creatingVault}
				>
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
						<rect x="3" y="11" width="18" height="11" rx="2" stroke="currentColor" stroke-width="1.5" />
						<path d="M7 11V7a5 5 0 0110 0v4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
					</svg>
					<span class="type-name">{t('vault.private')}</span>
					<span class="type-desc">{t('vault.type_private_desc')}</span>
				</button>
				<button
					class="type-option"
					class:active={newVaultType === 'shared'}
					onclick={() => (newVaultType = 'shared')}
					disabled={creatingVault}
				>
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
						<path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4 4v2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
						<circle cx="9" cy="7" r="4" stroke="currentColor" stroke-width="1.5" />
						<path d="M23 21v-2a4 4 0 00-3-3.87M16 3.13a4 4 0 010 7.75" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
					<span class="type-name">{t('vault.shared')}</span>
					<span class="type-desc">{t('vault.type_shared_desc')}</span>
				</button>
			</div>
		</div>
	</div>

	{#snippet actions()}
		<Button variant="ghost" onclick={() => (showCreateVault = false)} disabled={creatingVault}>
			{t('common.cancel')}
		</Button>
		<Button
			variant="primary"
			onclick={handleCreateVault}
			disabled={creatingVault || !newVaultName.trim()}
		>
			{#if creatingVault}{t('vault.creating')}{:else}{t('vault.create_vault')}{/if}
		</Button>
	{/snippet}
</Modal>

<!-- Invite Member Dialog -->
{#if activeVault}
	<InviteMemberDialog
		open={showInviteDialog}
		vaultId={activeVault.id}
		vaultName={activeVault.name}
		onclose={() => (showInviteDialog = false)}
	/>
{/if}

<!-- Import Backup Modal -->
<Modal
	open={showImportBackup}
	onclose={() => (showImportBackup = false)}
	title={t('vault.import_backup')}
>
	{#snippet children()}
		<div class="import-backup-form">
			<p class="import-desc">{t('vault.import_backup_desc')}</p>

			<div class="import-step">
				<Button variant="secondary" size="sm" onclick={handleSelectBackupFile}>
					{t('vault.select_backup_file')}
				</Button>
				{#if importFilePath}
					<span class="import-file-path">{importFilePath.split(/[\\/]/).pop()}</span>
				{/if}
			</div>

			{#if importFilePath}
				<Input
					type="password"
					label={t('vault.export_password')}
					placeholder="Password used during export"
					bind:value={importExportPassword}
					disabled={importVerifying || importLoading}
				/>

				{#if !importPreview}
					<Button
						variant="secondary"
						size="sm"
						onclick={handleVerifyBackup}
						disabled={importVerifying || !importExportPassword}
					>
						{importVerifying ? t('vault.verifying') : t('vault.verify_backup')}
					</Button>
				{/if}
			{/if}

			{#if importPreview}
				<div class="import-preview">
					<div class="preview-row">
						<span class="preview-label">{t('vault.backup_preview_date')}</span>
						<span class="preview-value">{new Date(importPreview.exportedAt * 1000).toLocaleString()}</span>
					</div>
					<div class="preview-row">
						<span class="preview-label">{t('vault.backup_preview_vaults')}</span>
						<span class="preview-value">{importPreview.vaultCount}</span>
					</div>
					<div class="preview-row">
						<span class="preview-label">{t('vault.backup_preview_secrets')}</span>
						<span class="preview-value">{importPreview.secretCount}</span>
					</div>
					<div class="preview-row">
						<span class="preview-label">{t('vault.backup_preview_sync')}</span>
						<span class="preview-value">{importPreview.hasSyncConfig ? t('vault.yes') : t('vault.no')}</span>
					</div>
				</div>

				<Input
					type="password"
					label={t('vault.master_password_optional')}
					placeholder="Leave blank if you used TLS-style init"
					bind:value={importMasterPassword}
					disabled={importLoading}
				/>

				<p class="import-warning">{t('vault.import_warning')}</p>
			{/if}

			{#if importError}
				<div class="error-message">{importError}</div>
			{/if}
		</div>
	{/snippet}

	{#snippet actions()}
		<Button variant="ghost" onclick={() => (showImportBackup = false)} disabled={importLoading}>
			{t('common.cancel')}
		</Button>
		{#if importPreview}
			<Button
				variant="danger"
				onclick={handleImportBackup}
				disabled={importLoading}
			>
				{importLoading ? t('vault.importing') : t('vault.import_backup')}
			</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	.vault-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	/* Lock Screen */
	.lock-screen {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 24px 16px;
		text-align: center;
		flex: 1;
	}

	.lock-icon {
		color: var(--color-text-secondary);
		margin-bottom: 16px;
		opacity: 0.6;
	}

	.lock-title {
		margin: 0 0 8px;
		font-size: 1rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.lock-description {
		margin: 0 0 20px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		max-width: 240px;
		line-height: 1.5;
	}

	.lock-form {
		display: flex;
		flex-direction: column;
		gap: 12px;
		width: 100%;
		max-width: 240px;
	}

	.error-message {
		padding: 8px 10px;
		font-size: 0.6875rem;
		color: var(--color-danger);
		background-color: rgba(255, 69, 58, 0.08);
		border-radius: 6px;
		text-align: left;
	}

	/* Vault View */
	.vault-view {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	.vault-header {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 10px;
		border-bottom: 1px solid var(--color-border);
	}

	.back-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		padding: 0;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
	}

	.back-btn:hover {
		background-color: rgba(255, 255, 255, 0.08);
		color: var(--color-text-primary);
	}

	.vault-info {
		display: flex;
		align-items: center;
		gap: 8px;
		flex: 1;
		overflow: hidden;
	}

	.vault-name {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.vault-badge {
		padding: 2px 6px;
		font-size: 0.5625rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		color: var(--color-text-secondary);
		background-color: rgba(255, 255, 255, 0.06);
		border-radius: 4px;
	}

	.vault-badge.shared {
		color: var(--color-accent);
		background-color: rgba(10, 132, 255, 0.12);
	}

	.invite-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		padding: 0;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--color-accent);
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
	}

	.invite-btn:hover {
		background-color: rgba(10, 132, 255, 0.1);
	}

	.secret-list-container {
		flex: 1;
		overflow-y: auto;
		padding: 8px;
	}

	/* Vault List View */
	.vault-list-view {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	.list-header {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 8px 10px;
		border-bottom: 1px solid var(--color-border);
	}

	.header-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.header-title {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
	}

	.header-actions {
		display: flex;
		align-items: center;
		gap: 2px;
	}

	.header-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		padding: 0;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
	}

	.header-btn:hover:not(:disabled) {
		background-color: rgba(255, 255, 255, 0.08);
		color: var(--color-text-primary);
	}

	.header-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}

	.spinning {
		animation: spin 0.8s linear infinite;
	}

	.create-vault-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
		width: 100%;
		padding: 6px 10px;
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		font-weight: 500;
		color: var(--color-accent);
		background: transparent;
		border: 1px solid var(--color-accent);
		border-radius: 6px;
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
	}

	.create-vault-btn:hover {
		background-color: rgba(10, 132, 255, 0.1);
	}

	.create-vault-btn:active {
		transform: scale(0.98);
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 32px 16px;
		text-align: center;
	}

	.empty-text {
		margin: 0 0 4px;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
	}

	.empty-hint {
		margin: 0;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
	}

	.vault-list {
		flex: 1;
		overflow-y: auto;
		padding: 8px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.vault-card {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		padding: 10px 12px;
		text-align: left;
		background-color: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--color-border);
		border-radius: 8px;
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
	}

	.vault-card:hover {
		background-color: rgba(255, 255, 255, 0.05);
		border-color: var(--color-accent);
	}

	.vault-card.active {
		background-color: rgba(10, 132, 255, 0.08);
		border-color: var(--color-accent);
	}

	.vault-card-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		background-color: rgba(255, 255, 255, 0.04);
		border-radius: 6px;
		color: var(--color-text-secondary);
		flex-shrink: 0;
	}

	.vault-card:hover .vault-card-icon {
		color: var(--color-accent);
	}

	.vault-card-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 2px;
		overflow: hidden;
	}

	.vault-card-name {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.vault-card-meta {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.vault-card-badge {
		padding: 3px 8px;
		font-size: 0.5625rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		color: var(--color-text-secondary);
		background-color: rgba(255, 255, 255, 0.06);
		border-radius: 4px;
		flex-shrink: 0;
	}

	.vault-card-badge.shared {
		color: var(--color-accent);
		background-color: rgba(10, 132, 255, 0.12);
	}

	.vault-card-wrapper {
		display: flex;
		gap: 4px;
	}

	.vault-card-wrapper .vault-card {
		flex: 1;
	}

	.vault-card-invite {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 36px;
		padding: 0;
		background-color: rgba(10, 132, 255, 0.1);
		border: 1px solid var(--color-accent);
		border-radius: 8px;
		color: var(--color-accent);
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
		flex-shrink: 0;
	}

	.vault-card-invite:hover {
		background-color: rgba(10, 132, 255, 0.2);
	}

	/* Create Vault Form */
	.create-vault-form {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.type-selector {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.type-label {
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-text-secondary);
	}

	.type-options {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 8px;
	}

	.type-option {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 6px;
		padding: 14px 12px;
		background-color: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: 8px;
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
		color: var(--color-text-secondary);
	}

	.type-option:hover:not(:disabled) {
		background-color: rgba(255, 255, 255, 0.04);
		border-color: var(--color-text-secondary);
	}

	.type-option.active {
		background-color: rgba(10, 132, 255, 0.08);
		border-color: var(--color-accent);
		color: var(--color-accent);
	}

	.type-option:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.type-name {
		font-size: 0.8125rem;
		font-weight: 600;
		color: inherit;
	}

	.type-desc {
		font-size: 0.625rem;
		color: var(--color-text-secondary);
		text-align: center;
	}

	/* Import Backup */
	.import-link {
		padding: 6px 0;
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		font-weight: 500;
		color: var(--color-accent);
		background: transparent;
		border: none;
		cursor: pointer;
		text-decoration: none;
		transition: opacity var(--duration-default) var(--ease-default);
	}

	.import-link:hover {
		opacity: 0.8;
	}

	.import-backup-form {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.import-desc {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		line-height: 1.5;
	}

	.import-step {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.import-file-path {
		font-size: 0.6875rem;
		font-family: var(--font-mono);
		color: var(--color-text-secondary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.import-preview {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 10px 12px;
		background-color: rgba(48, 209, 88, 0.06);
		border: 1px solid rgba(48, 209, 88, 0.2);
		border-radius: 8px;
	}

	.preview-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.preview-label {
		font-size: 0.6875rem;
		font-weight: 500;
		color: var(--color-text-secondary);
	}

	.preview-value {
		font-size: 0.6875rem;
		font-family: var(--font-mono);
		color: var(--color-text-primary);
	}

	.import-warning {
		margin: 0;
		font-size: 0.6875rem;
		font-weight: 500;
		color: var(--color-warning);
		padding: 8px 10px;
		background: rgba(255, 214, 10, 0.08);
		border-radius: 6px;
	}
</style>
