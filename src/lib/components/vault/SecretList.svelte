<script lang="ts">
	import Button from '$lib/components/shared/Button.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import Modal from '$lib/components/shared/Modal.svelte';
	import Dropdown from '$lib/components/shared/Dropdown.svelte';
	import {
		getSecretList,
		createSecret,
		readSecret,
		updateSecret,
		deleteSecret,
		type SecretMetadata
	} from '$lib/state/vault.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';

	// Categories with their display names
	let categories = $derived([
		{ value: 'password', label: t('vault.category_password') },
		{ value: 'ssh_key', label: t('vault.category_ssh_key') },
		{ value: 'api_token', label: t('vault.category_api_token') },
		{ value: 'certificate', label: t('vault.category_certificate') },
		{ value: 'note', label: t('vault.category_note') },
		{ value: 'custom', label: t('vault.category_custom') }
	]);

	// State
	let secretList = $derived(getSecretList());
	let showAddModal = $state(false);
	let showViewModal = $state(false);
	let showEditModal = $state(false);
	let showDeleteModal = $state(false);

	// Add secret form
	let newName = $state('');
	let newCategory = $state('password');
	let newValue = $state('');
	let creating = $state(false);

	// View/Edit secret
	let selectedSecret = $state<SecretMetadata | null>(null);
	let secretValue = $state('');
	let showSecretValue = $state(false);
	let loadingSecret = $state(false);
	let editValue = $state('');
	let updating = $state(false);
	let deleting = $state(false);

	// Format relative time
	function formatRelativeTime(timestamp: number): string {
		const now = Date.now();
		const diff = now - timestamp;
		const seconds = Math.floor(diff / 1000);
		const minutes = Math.floor(seconds / 60);
		const hours = Math.floor(minutes / 60);
		const days = Math.floor(hours / 24);

		if (days > 0) return `${days} day${days !== 1 ? 's' : ''} ago`;
		if (hours > 0) return `${hours} hour${hours !== 1 ? 's' : ''} ago`;
		if (minutes > 0) return `${minutes} minute${minutes !== 1 ? 's' : ''} ago`;
		return 'just now';
	}

	// Get category label
	function getCategoryLabel(category: string): string {
		return categories.find((c) => c.value === category)?.label ?? category;
	}

	// Handle add secret
	async function handleAddSecret(): Promise<void> {
		if (!newName.trim() || !newValue.trim()) {
			addToast(t('vault.name_value_required'), 'error');
			return;
		}

		creating = true;

		try {
			await createSecret(newName.trim(), newCategory, newValue);
			newName = '';
			newCategory = 'password';
			newValue = '';
			showAddModal = false;
			addToast(t('vault.secret_created_toast'), 'success');
		} catch (err) {
			addToast(`Failed to create secret: ${err}`, 'error');
		} finally {
			creating = false;
		}
	}

	// Handle view secret
	async function handleViewSecret(secret: SecretMetadata): Promise<void> {
		selectedSecret = secret;
		secretValue = '';
		showSecretValue = false;
		showViewModal = true;
	}

	// Load secret value
	async function loadSecretValue(): Promise<void> {
		if (!selectedSecret) return;

		loadingSecret = true;

		try {
			secretValue = await readSecret(selectedSecret.id);
			showSecretValue = true;
		} catch (err) {
			addToast(`Failed to read secret: ${err}`, 'error');
		} finally {
			loadingSecret = false;
		}
	}

	// Copy secret value
	async function copySecretValue(): Promise<void> {
		if (!selectedSecret) return;

		try {
			let value = secretValue;
			if (!showSecretValue) {
				value = await readSecret(selectedSecret.id);
			}
			await navigator.clipboard.writeText(value);
			addToast(t('vault.copied_toast'), 'success');
		} catch (err) {
			addToast(`Failed to copy: ${err}`, 'error');
		}
	}

	// Open edit modal
	async function openEditModal(): Promise<void> {
		if (!selectedSecret) return;

		try {
			editValue = secretValue || (await readSecret(selectedSecret.id));
			showViewModal = false;
			showEditModal = true;
		} catch (err) {
			addToast(`Failed to read secret: ${err}`, 'error');
		}
	}

	// Handle update secret
	async function handleUpdateSecret(): Promise<void> {
		if (!selectedSecret || !editValue.trim()) {
			addToast(t('vault.value_required'), 'error');
			return;
		}

		updating = true;

		try {
			await updateSecret(selectedSecret.id, editValue);
			showEditModal = false;
			selectedSecret = null;
			editValue = '';
			addToast(t('vault.secret_updated_toast'), 'success');
		} catch (err) {
			addToast(`Failed to update secret: ${err}`, 'error');
		} finally {
			updating = false;
		}
	}

	// Open delete confirmation
	function openDeleteModal(): void {
		showViewModal = false;
		showDeleteModal = true;
	}

	// Handle delete secret
	async function handleDeleteSecret(): Promise<void> {
		if (!selectedSecret) return;

		deleting = true;

		try {
			await deleteSecret(selectedSecret.id);
			showDeleteModal = false;
			selectedSecret = null;
			addToast(t('vault.secret_deleted_toast'), 'success');
		} catch (err) {
			addToast(`Failed to delete secret: ${err}`, 'error');
		} finally {
			deleting = false;
		}
	}

	// Close all modals
	function closeModals(): void {
		showAddModal = false;
		showViewModal = false;
		showEditModal = false;
		showDeleteModal = false;
		selectedSecret = null;
		secretValue = '';
		showSecretValue = false;
		editValue = '';
	}
</script>

<div class="secret-list">
	<div class="list-header">
		<span class="header-title">{t('vault.secrets')}</span>
		<button class="add-btn" onclick={() => (showAddModal = true)}>
			<svg width="12" height="12" viewBox="0 0 24 24" fill="none">
				<path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
			</svg>
			{t('vault.add_secret')}
		</button>
	</div>

	{#if secretList.length === 0}
		<div class="empty-state">
			<div class="empty-icon">
				<svg width="32" height="32" viewBox="0 0 24 24" fill="none">
					<path d="M12 15v2m0-6V5m0 0L9 8m3-3l3 3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
					<path d="M5 12h2m10 0h2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
					<rect x="3" y="11" width="18" height="11" rx="2" stroke="currentColor" stroke-width="1.5" />
				</svg>
			</div>
			<p class="empty-text">{t('vault.no_secrets')}</p>
			<p class="empty-hint">{t('vault.add_first_secret')}</p>
		</div>
	{:else}
		<div class="secrets">
			{#each secretList as secret (secret.id)}
				<button class="secret-item" onclick={() => handleViewSecret(secret)}>
					<div class="secret-icon">
						{#if secret.category === 'password'}
							<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
								<path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 11-7.778 7.778 5.5 5.5 0 017.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
						{:else if secret.category === 'ssh_key'}
							<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
								<path d="M4 17l6 6m0-6l-6 6m14-6h2m-4 0h-2m6-4V5a2 2 0 00-2-2H6a2 2 0 00-2 2v8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
						{:else if secret.category === 'api_token'}
							<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
								<path d="M16 18l6-6-6-6M8 6l-6 6 6 6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
						{:else if secret.category === 'certificate'}
							<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
								<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
						{:else if secret.category === 'note'}
							<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
								<path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8l-6-6z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
								<path d="M14 2v6h6M16 13H8M16 17H8M10 9H8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
						{:else}
							<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
								<path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
						{/if}
					</div>
					<div class="secret-content">
						<span class="secret-name">{secret.name}</span>
						<span class="secret-meta">
							{getCategoryLabel(secret.category)} &middot; {formatRelativeTime(secret.updatedAt)}
						</span>
					</div>
					<svg class="secret-chevron" width="14" height="14" viewBox="0 0 24 24" fill="none">
						<path d="M9 18l6-6-6-6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
				</button>
			{/each}
		</div>
	{/if}
</div>

<!-- Add Secret Modal -->
<Modal open={showAddModal} onclose={() => (showAddModal = false)} title={t('vault.add_secret')}>
	<div class="form">
		<Input
			label={t('vault.secret_name')}
			placeholder="My API Key"
			bind:value={newName}
			disabled={creating}
		/>

		<div class="form-field">
			<span class="form-label">{t('vault.secret_category')}</span>
			<Dropdown
				options={categories}
				bind:selected={newCategory}
			/>
		</div>

		<div class="form-field">
			<label class="form-label" for="secret-value">{t('vault.secret_value')}</label>
			<textarea
				id="secret-value"
				class="value-textarea"
				placeholder={t('vault.enter_secret_value')}
				bind:value={newValue}
				disabled={creating}
				rows="4"
			></textarea>
		</div>
	</div>

	{#snippet actions()}
		<Button variant="ghost" onclick={() => (showAddModal = false)} disabled={creating}>
			{t('common.cancel')}
		</Button>
		<Button
			variant="primary"
			onclick={handleAddSecret}
			disabled={creating || !newName.trim() || !newValue.trim()}
		>
			{#if creating}{t('vault.creating')}{:else}{t('common.save')}{/if}
		</Button>
	{/snippet}
</Modal>

<!-- View Secret Modal -->
<Modal open={showViewModal} onclose={() => (showViewModal = false)} title={selectedSecret?.name ?? 'Secret'}>
	{#if selectedSecret}
		<div class="view-secret">
			<div class="secret-detail">
				<span class="detail-label">{t('vault.secret_category')}</span>
				<span class="detail-value">{getCategoryLabel(selectedSecret.category)}</span>
			</div>

			<div class="secret-detail">
				<span class="detail-label">{t('vault.last_updated')}</span>
				<span class="detail-value">{formatRelativeTime(selectedSecret.updatedAt)}</span>
			</div>

			<div class="secret-detail">
				<span class="detail-label">{t('vault.secret_value')}</span>
				<div class="value-container">
					{#if showSecretValue}
						<pre class="secret-value-display">{secretValue}</pre>
					{:else}
						<span class="secret-hidden">{t('vault.click_show')}</span>
					{/if}
					<div class="value-actions">
						<button
							class="action-btn"
							onclick={showSecretValue ? () => (showSecretValue = false) : loadSecretValue}
							disabled={loadingSecret}
						>
							{#if loadingSecret}
								{t('common.loading')}
							{:else if showSecretValue}
								{t('vault.hide')}
							{:else}
								{t('vault.show')}
							{/if}
						</button>
						<button class="action-btn" onclick={copySecretValue}>
							{t('vault.copy')}
						</button>
					</div>
				</div>
			</div>
		</div>
	{/if}

	{#snippet actions()}
		<Button variant="danger" onclick={openDeleteModal}>
			{t('common.delete')}
		</Button>
		<Button variant="secondary" onclick={openEditModal}>
			{t('session.edit')}
		</Button>
		<Button variant="ghost" onclick={() => (showViewModal = false)}>
			{t('common.close')}
		</Button>
	{/snippet}
</Modal>

<!-- Edit Secret Modal -->
<Modal open={showEditModal} onclose={() => (showEditModal = false)} title={t('vault.edit_secret')}>
	{#if selectedSecret}
		<div class="form">
			<div class="form-field">
				<span class="form-label">{t('vault.secret_name')}</span>
				<span class="form-value">{selectedSecret.name}</span>
			</div>

			<div class="form-field">
				<label class="form-label" for="edit-value">{t('vault.secret_value')}</label>
				<textarea
					id="edit-value"
					class="value-textarea"
					bind:value={editValue}
					disabled={updating}
					rows="6"
				></textarea>
			</div>
		</div>
	{/if}

	{#snippet actions()}
		<Button variant="ghost" onclick={() => (showEditModal = false)} disabled={updating}>
			{t('common.cancel')}
		</Button>
		<Button
			variant="primary"
			onclick={handleUpdateSecret}
			disabled={updating || !editValue.trim()}
		>
			{#if updating}{t('vault.secret_saving')}{:else}{t('vault.save_changes')}{/if}
		</Button>
	{/snippet}
</Modal>

<!-- Delete Confirmation Modal -->
<Modal open={showDeleteModal} onclose={() => (showDeleteModal = false)} title={t('vault.delete_secret')}>
	<div class="delete-confirm">
		<p class="delete-message">
			{t('vault.delete_secret_confirm')} <strong>{selectedSecret?.name}</strong>
		</p>
		<p class="delete-warning">{t('vault.delete_irreversible')}</p>
	</div>

	{#snippet actions()}
		<Button variant="ghost" onclick={() => (showDeleteModal = false)} disabled={deleting}>
			{t('common.cancel')}
		</Button>
		<Button variant="danger" onclick={handleDeleteSecret} disabled={deleting}>
			{#if deleting}{t('vault.deleting')}{:else}{t('common.delete')}{/if}
		</Button>
	{/snippet}
</Modal>

<style>
	.secret-list {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	.list-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 10px;
		border-bottom: 1px solid var(--color-border);
	}

	.header-title {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
	}

	.add-btn {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 5px 10px;
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

	.add-btn:hover {
		background-color: rgba(10, 132, 255, 0.1);
	}

	.add-btn:active {
		transform: scale(0.98);
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 32px 16px;
		text-align: center;
		flex: 1;
	}

	.empty-icon {
		color: var(--color-text-secondary);
		opacity: 0.5;
		margin-bottom: 12px;
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

	.secrets {
		flex: 1;
		overflow-y: auto;
		padding: 8px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.secret-item {
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

	.secret-item:hover {
		background-color: rgba(255, 255, 255, 0.05);
		border-color: var(--color-accent);
	}

	.secret-icon {
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

	.secret-item:hover .secret-icon {
		color: var(--color-accent);
	}

	.secret-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 2px;
		overflow: hidden;
	}

	.secret-name {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.secret-meta {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.secret-chevron {
		color: var(--color-text-secondary);
		opacity: 0.5;
		flex-shrink: 0;
		transition: opacity var(--duration-default) var(--ease-default);
	}

	.secret-item:hover .secret-chevron {
		opacity: 1;
	}

	/* Form styles */
	.form {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.form-field {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.form-label {
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-text-secondary);
	}

	.form-value {
		font-size: 0.875rem;
		color: var(--color-text-primary);
	}

	.value-textarea {
		width: 100%;
		padding: 12px;
		font-family: var(--font-mono, 'JetBrains Mono', monospace);
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		resize: vertical;
		min-height: 80px;
		outline: none;
		transition: border-color var(--duration-default) var(--ease-default);
		box-sizing: border-box;
	}

	.value-textarea:focus {
		border-color: var(--color-accent);
	}

	.value-textarea:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	/* View secret styles */
	.view-secret {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.secret-detail {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.detail-label {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
	}

	.detail-value {
		font-size: 0.875rem;
		color: var(--color-text-primary);
	}

	.value-container {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.secret-value-display {
		margin: 0;
		padding: 12px;
		font-family: var(--font-mono, 'JetBrains Mono', monospace);
		font-size: 0.75rem;
		color: var(--color-text-primary);
		background-color: rgba(0, 0, 0, 0.2);
		border-radius: 6px;
		white-space: pre-wrap;
		word-break: break-all;
		max-height: 200px;
		overflow-y: auto;
	}

	.secret-hidden {
		padding: 12px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		background-color: rgba(0, 0, 0, 0.2);
		border-radius: 6px;
		font-style: italic;
	}

	.value-actions {
		display: flex;
		gap: 8px;
	}

	.action-btn {
		padding: 6px 12px;
		font-family: var(--font-sans);
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-text-primary);
		background-color: rgba(255, 255, 255, 0.06);
		border: 1px solid var(--color-border);
		border-radius: 6px;
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
	}

	.action-btn:hover:not(:disabled) {
		background-color: rgba(255, 255, 255, 0.1);
		border-color: var(--color-text-secondary);
	}

	.action-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	/* Delete confirmation */
	.delete-confirm {
		text-align: center;
	}

	.delete-message {
		margin: 0 0 8px;
		font-size: 0.875rem;
		color: var(--color-text-primary);
	}

	.delete-warning {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-danger);
	}
</style>
