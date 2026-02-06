<script lang="ts">
	import { vaultState, createVault, deleteVault, refreshVaults, type VaultInfo } from '$lib/state/vault.svelte';
	import { inviteMember, listMembers, removeMember, type MemberInfo, type InviteInfo } from '$lib/ipc/vault';
	import { addToast } from '$lib/state/toasts.svelte';

	interface Props {
		onvaultselect?: (vaultId: string | null) => void;
		onrefresh?: () => void;
	}

	let { onvaultselect, onrefresh }: Props = $props();

	let showCreateDialog = $state(false);
	let newVaultName = $state('');
	let newVaultType = $state<'private' | 'shared'>('private');
	let creating = $state(false);
	let refreshing = $state(false);
	let error = $state('');

	async function handleRefresh() {
		refreshing = true;
		try {
			await refreshVaults();
			onrefresh?.();
		} catch (e) {
			addToast(`Failed to refresh: ${e}`, 'error');
		} finally {
			refreshing = false;
		}
	}

	// Delete confirmation
	let showDeleteDialog = $state(false);
	let vaultToDelete = $state<VaultInfo | null>(null);
	let deleting = $state(false);

	// Invite dialog state
	let showInviteDialog = $state(false);
	let inviteVault = $state<VaultInfo | null>(null);
	let inviteeUuid = $state('');
	let inviteePublicKey = $state('');
	let inviteeRole = $state<'admin' | 'member' | 'readonly'>('member');
	let inviting = $state(false);
	let inviteResult = $state<InviteInfo | null>(null);
	let members = $state<MemberInfo[]>([]);
	let loadingMembers = $state(false);

	// User vaults (excludes internal __xxx__ vaults)
	let userVaults = $derived(vaultState.vaultList.filter(v => !v.name.startsWith('__')));
	let selectedVaultId = $state<string | null>(null);

	async function handleCreateVault() {
		if (!newVaultName.trim()) {
			error = 'Vault name is required';
			return;
		}

		creating = true;
		error = '';

		try {
			// Shared vaults auto-create Turso database via Platform API
			const vault = await createVault(newVaultName.trim(), newVaultType);
			await refreshVaults();
			selectedVaultId = vault.id;
			onvaultselect?.(vault.id);
			showCreateDialog = false;
			newVaultName = '';
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			creating = false;
		}
	}

	function selectVault(vaultId: string | null) {
		selectedVaultId = vaultId;
		onvaultselect?.(vaultId);
	}

	function confirmDelete(vault: VaultInfo, e: Event) {
		e.stopPropagation();
		vaultToDelete = vault;
		showDeleteDialog = true;
	}

	function openInviteDialog(vault: VaultInfo, e: Event) {
		e.stopPropagation();
		inviteVault = vault;
		inviteeUuid = '';
		inviteePublicKey = '';
		inviteeRole = 'member';
		inviteResult = null;
		error = '';
		showInviteDialog = true;
		loadMembers(vault.id);
	}

	async function loadMembers(vaultId: string) {
		loadingMembers = true;
		try {
			members = await listMembers(vaultId);
		} catch (e) {
			console.error('Failed to load members:', e);
		} finally {
			loadingMembers = false;
		}
	}

	async function handleInvite() {
		if (!inviteVault) return;
		if (!inviteeUuid.trim()) {
			error = 'Recipient UUID is required';
			return;
		}
		if (!inviteePublicKey.trim()) {
			error = 'Recipient public key is required';
			return;
		}

		inviting = true;
		error = '';

		try {
			const result = await inviteMember(inviteVault.id, inviteePublicKey.trim(), inviteeUuid.trim(), inviteeRole);
			inviteResult = result;
			addToast('Member invited successfully', 'success');
			await loadMembers(inviteVault.id);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			inviting = false;
		}
	}

	function copyToClipboard(text: string, label: string) {
		navigator.clipboard.writeText(text);
		addToast(`${label} copied`, 'success');
	}

	function copyInviteInfo() {
		if (!inviteResult || !inviteVault) return;
		const info = `Vault Invite: ${inviteVault.name}

Sync URL: ${inviteResult.syncUrl}
Token: ${inviteResult.token}

Go to Settings > Sync > Accept Vault Invite`;
		navigator.clipboard.writeText(info);
		addToast('Invite info copied', 'success');
	}

	async function handleDelete() {
		if (!vaultToDelete) return;
		deleting = true;
		try {
			await deleteVault(vaultToDelete.id);
			if (selectedVaultId === vaultToDelete.id) {
				selectedVaultId = null;
				onvaultselect?.(null);
			}
			showDeleteDialog = false;
			vaultToDelete = null;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			deleting = false;
		}
	}
</script>

<div class="vault-selector">
	<div class="vault-header">
		<span class="vault-title">Vaults</span>
		<div class="header-actions">
			<button class="header-btn" onclick={handleRefresh} disabled={refreshing} title="Refresh vaults">
				<svg class:spinning={refreshing} width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
					<path d="M1 4v6h6" stroke-linecap="round" stroke-linejoin="round" />
					<path d="M3.51 15a9 9 0 105.64-9.94L1 10" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
			</button>
			<button class="add-vault-btn" onclick={() => (showCreateDialog = true)} title="Create Vault">
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M12 5v14M5 12h14"/>
				</svg>
			</button>
		</div>
	</div>

	<div class="vault-list">
		<!-- Private (default) -->
		<button
			class="vault-item"
			class:selected={selectedVaultId === null}
			onclick={() => selectVault(null)}
		>
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
				<rect x="3" y="11" width="18" height="11" rx="2"/>
				<path d="M7 11V7a5 5 0 0 1 10 0v4"/>
			</svg>
			<span class="vault-name">Private</span>
		</button>

		<!-- User vaults -->
		{#each userVaults as vault}
			<div class="vault-row">
				<button
					class="vault-item"
					class:selected={selectedVaultId === vault.id}
					class:shared={vault.vaultType === 'shared'}
					onclick={() => selectVault(vault.id)}
				>
					{#if vault.vaultType === 'shared'}
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
							<path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/>
							<circle cx="9" cy="7" r="4"/>
							<path d="M23 21v-2a4 4 0 0 0-3-3.87"/>
							<path d="M16 3.13a4 4 0 0 1 0 7.75"/>
						</svg>
					{:else}
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
							<rect x="3" y="11" width="18" height="11" rx="2"/>
							<path d="M7 11V7a5 5 0 0 1 10 0v4"/>
						</svg>
					{/if}
					<span class="vault-name">{vault.name}</span>
					{#if vault.vaultType === 'shared' && vault.memberCount}
						<span class="member-count">{vault.memberCount}</span>
					{/if}
				</button>
				{#if vault.vaultType === 'shared'}
					<button class="invite-btn" onclick={(e) => openInviteDialog(vault, e)} title="Invite members">
						<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M16 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/>
							<circle cx="8.5" cy="7" r="4"/>
							<path d="M20 8v6M23 11h-6"/>
						</svg>
					</button>
				{/if}
				<button class="delete-btn" onclick={(e) => confirmDelete(vault, e)} title="Delete vault">
					<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
					</svg>
				</button>
			</div>
		{/each}
	</div>
</div>

<!-- Create Vault Dialog -->
{#if showCreateDialog}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="dialog-overlay" onclick={() => (showCreateDialog = false)} onkeydown={(e) => { if (e.key === 'Escape') showCreateDialog = false; }}>
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="dialog" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
			<h3>Create Vault</h3>

			<div class="form-group">
				<label for="vault-name">Name</label>
				<input
					id="vault-name"
					type="text"
					bind:value={newVaultName}
					placeholder="DevOps Team"
					disabled={creating}
				/>
			</div>

			<div class="form-group" role="group" aria-labelledby="vault-type-label">
				<span id="vault-type-label" class="field-label">Type</span>
				<div class="type-toggle">
					<button
						type="button"
						class="type-btn"
						class:active={newVaultType === 'private'}
						disabled={creating}
						onclick={() => (newVaultType = 'private')}
					>
						Private
					</button>
					<button
						type="button"
						class="type-btn"
						class:active={newVaultType === 'shared'}
						disabled={creating}
						onclick={() => (newVaultType = 'shared')}
					>
						Shared
					</button>
				</div>
				<p class="type-hint">
					{#if newVaultType === 'private'}
						Only you can access this vault.
					{:else}
						Team members can be invited to access this vault.
					{/if}
				</p>
			</div>

			{#if error}
				<p class="error">{error}</p>
			{/if}

			<div class="dialog-actions">
				<button class="btn-secondary" onclick={() => (showCreateDialog = false)} disabled={creating}>
					Cancel
				</button>
				<button class="btn-primary" onclick={handleCreateVault} disabled={creating || !newVaultName.trim()}>
					{#if creating}Creating...{:else}Create{/if}
				</button>
			</div>
		</div>
	</div>
{/if}

<!-- Invite Member Dialog -->
{#if showInviteDialog && inviteVault}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="dialog-overlay" onclick={() => (showInviteDialog = false)} onkeydown={(e) => { if (e.key === 'Escape') showInviteDialog = false; }}>
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="dialog invite-dialog" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
			<h3>Invite to {inviteVault.name}</h3>

			{#if inviteResult}
				<!-- Show result -->
				<div class="invite-success">
					<p class="success-msg">Invite created! Share this info with the invitee:</p>
					<div class="invite-field">
						<span class="field-label">Sync URL</span>
						<div class="field-row">
							<code class="field-value">{inviteResult.syncUrl}</code>
							<button class="copy-btn" onclick={() => copyToClipboard(inviteResult!.syncUrl, 'URL')}>Copy</button>
						</div>
					</div>
					<div class="invite-field">
						<span class="field-label">Token</span>
						<div class="field-row">
							<code class="field-value truncate">{inviteResult.token}</code>
							<button class="copy-btn" onclick={() => copyToClipboard(inviteResult!.token, 'Token')}>Copy</button>
						</div>
					</div>
					<div class="dialog-actions">
						<button class="btn-primary" onclick={copyInviteInfo}>Copy All</button>
						<button class="btn-secondary" onclick={() => { inviteResult = null; inviteeUuid = ''; inviteePublicKey = ''; }}>Invite Another</button>
					</div>
				</div>
			{:else}
				<!-- Show form -->
				<div class="invite-section">
					<p class="section-label">Your info (share with invitee)</p>
					<div class="info-row">
						<span class="info-label">UUID:</span>
						<code class="info-value">{vaultState.userUuid ?? 'N/A'}</code>
						{#if vaultState.userUuid}
							<button class="copy-btn-sm" onclick={() => copyToClipboard(vaultState.userUuid!, 'UUID')}>Copy</button>
						{/if}
					</div>
					<div class="info-row">
						<span class="info-label">Key:</span>
						<code class="info-value truncate">{vaultState.publicKey ?? 'N/A'}</code>
						{#if vaultState.publicKey}
							<button class="copy-btn-sm" onclick={() => copyToClipboard(vaultState.publicKey!, 'Key')}>Copy</button>
						{/if}
					</div>
				</div>

				<div class="invite-section">
					<p class="section-label">Invitee info (get from them)</p>
					<div class="form-group">
						<input type="text" placeholder="Invitee UUID" bind:value={inviteeUuid} disabled={inviting} />
					</div>
					<div class="form-group">
						<input type="text" placeholder="Invitee Public Key" bind:value={inviteePublicKey} disabled={inviting} />
					</div>
					<div class="form-group">
						<select bind:value={inviteeRole} disabled={inviting}>
							<option value="member">Member (read/write)</option>
							<option value="admin">Admin (can invite)</option>
							<option value="readonly">Read Only</option>
						</select>
					</div>
				</div>

				{#if error}
					<p class="error">{error}</p>
				{/if}

				<div class="dialog-actions">
					<button class="btn-secondary" onclick={() => (showInviteDialog = false)} disabled={inviting}>Cancel</button>
					<button class="btn-primary" onclick={handleInvite} disabled={inviting || !inviteeUuid.trim() || !inviteePublicKey.trim()}>
						{#if inviting}Inviting...{:else}Send Invite{/if}
					</button>
				</div>
			{/if}
		</div>
	</div>
{/if}

<!-- Delete Vault Confirmation -->
{#if showDeleteDialog && vaultToDelete}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="dialog-overlay" onclick={() => (showDeleteDialog = false)} onkeydown={(e) => { if (e.key === 'Escape') showDeleteDialog = false; }}>
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="dialog" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
			<h3>Delete Vault</h3>
			<p class="delete-warning">
				Are you sure you want to delete <strong>{vaultToDelete.name}</strong>?
				This will permanently delete all secrets in this vault.
			</p>
			{#if error}
				<p class="error">{error}</p>
			{/if}
			<div class="dialog-actions">
				<button class="btn-secondary" onclick={() => (showDeleteDialog = false)} disabled={deleting}>
					Cancel
				</button>
				<button class="btn-danger" onclick={handleDelete} disabled={deleting}>
					{#if deleting}Deleting...{:else}Delete{/if}
				</button>
			</div>
		</div>
	</div>
{/if}

<style>
	.vault-selector {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 8px 0;
		border-bottom: 1px solid var(--color-border);
		margin-bottom: 8px;
	}

	.vault-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0 8px;
	}

	.vault-title {
		font-size: 0.625rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-tertiary);
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
		width: 20px;
		height: 20px;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.header-btn:hover:not(:disabled) {
		background: rgba(255, 255, 255, 0.1);
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

	.add-vault-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 20px;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.add-vault-btn:hover {
		background: rgba(255, 255, 255, 0.1);
		color: var(--color-text-primary);
	}

	.vault-list {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.vault-item {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 8px;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		text-align: left;
		font-family: inherit;
		font-size: 0.75rem;
		transition: background-color 0.15s, color 0.15s;
	}

	.vault-item:hover {
		background: rgba(255, 255, 255, 0.05);
		color: var(--color-text-primary);
	}

	.vault-item.selected {
		background: rgba(59, 130, 246, 0.15);
		color: var(--color-accent);
	}

	.vault-item.shared {
		color: #10b981;
	}

	.vault-item.shared.selected {
		background: rgba(16, 185, 129, 0.15);
	}

	.vault-name {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.member-count {
		font-size: 0.625rem;
		padding: 1px 5px;
		border-radius: 10px;
		background: rgba(255, 255, 255, 0.1);
	}

	/* Dialog */
	.dialog-overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.6);
		backdrop-filter: blur(4px);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
	}

	.dialog {
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: 12px;
		padding: 20px;
		width: 100%;
		max-width: 360px;
	}

	.dialog h3 {
		margin: 0 0 16px;
		font-size: 1rem;
		font-weight: 600;
	}

	.form-group {
		margin-bottom: 16px;
	}

	.form-group label,
	.form-group .field-label {
		display: block;
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-text-secondary);
		margin-bottom: 6px;
	}

	.form-group input {
		width: 100%;
		padding: 8px 12px;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: 0.875rem;
	}

	.form-group input:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.type-toggle {
		display: flex;
		gap: 8px;
	}

	.type-btn {
		flex: 1;
		padding: 8px;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		font-size: 0.8125rem;
		transition: all 0.15s;
	}

	.type-btn:hover {
		border-color: var(--color-text-tertiary);
	}

	.type-btn.active {
		background: var(--color-accent);
		border-color: var(--color-accent);
		color: white;
	}

	.type-hint {
		margin: 8px 0 0;
		font-size: 0.6875rem;
		color: var(--color-text-tertiary);
	}

	.error {
		margin: 0 0 12px;
		font-size: 0.75rem;
		color: var(--color-danger);
	}

	.dialog-actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
	}

	.btn-primary, .btn-secondary {
		padding: 8px 16px;
		border-radius: 6px;
		font-size: 0.8125rem;
		font-weight: 500;
		cursor: pointer;
	}

	.btn-primary {
		background: var(--color-accent);
		border: none;
		color: white;
	}

	.btn-primary:hover:not(:disabled) {
		opacity: 0.9;
	}

	.btn-primary:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.btn-secondary {
		background: transparent;
		border: 1px solid var(--color-border);
		color: var(--color-text-primary);
	}

	.btn-secondary:hover:not(:disabled) {
		background: rgba(255, 255, 255, 0.05);
	}

	.btn-danger {
		background: var(--color-danger);
		border: none;
		color: white;
		padding: 8px 16px;
		border-radius: 6px;
		font-size: 0.8125rem;
		font-weight: 500;
		cursor: pointer;
	}

	.btn-danger:hover:not(:disabled) {
		opacity: 0.9;
	}

	.btn-danger:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.vault-row {
		display: flex;
		align-items: center;
		gap: 2px;
	}

	.vault-row .vault-item {
		flex: 1;
	}

	.delete-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-tertiary);
		cursor: pointer;
		opacity: 0;
		transition: opacity 0.15s, color 0.15s;
	}

	.vault-row:hover .delete-btn {
		opacity: 1;
	}

	.delete-btn:hover {
		color: var(--color-danger);
		background: rgba(255, 69, 58, 0.1);
	}

	.invite-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-accent);
		cursor: pointer;
		opacity: 0;
		transition: opacity 0.15s, background 0.15s;
	}

	.vault-row:hover .invite-btn {
		opacity: 1;
	}

	.invite-btn:hover {
		background: rgba(59, 130, 246, 0.15);
	}

	.invite-dialog {
		max-width: 420px;
	}

	.invite-success {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.success-msg {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-success);
	}

	.invite-field {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.field-row {
		display: flex;
		gap: 8px;
		align-items: center;
	}

	.field-value {
		flex: 1;
		padding: 6px 8px;
		background: var(--color-bg-primary);
		border-radius: 4px;
		font-size: 0.6875rem;
		word-break: break-all;
	}

	.field-value.truncate {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.copy-btn {
		padding: 4px 8px;
		font-size: 0.625rem;
		background: transparent;
		border: 1px solid var(--color-accent);
		border-radius: 4px;
		color: var(--color-accent);
		cursor: pointer;
	}

	.copy-btn:hover {
		background: rgba(59, 130, 246, 0.1);
	}

	.invite-section {
		margin-bottom: 16px;
		padding: 12px;
		background: rgba(255, 255, 255, 0.02);
		border-radius: 8px;
	}

	.section-label {
		margin: 0 0 8px;
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		color: var(--color-text-tertiary);
	}

	.info-row {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-bottom: 6px;
	}

	.info-label {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		width: 40px;
	}

	.info-value {
		flex: 1;
		font-size: 0.6875rem;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.copy-btn-sm {
		padding: 2px 6px;
		font-size: 0.5625rem;
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: 3px;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.copy-btn-sm:hover {
		border-color: var(--color-accent);
		color: var(--color-accent);
	}

	.form-group select {
		width: 100%;
		padding: 8px 12px;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: 0.875rem;
	}

	.delete-warning {
		margin: 0 0 16px;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		line-height: 1.5;
	}

	.delete-warning strong {
		color: var(--color-text-primary);
	}
</style>
