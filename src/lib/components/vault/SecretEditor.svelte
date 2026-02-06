<script lang="ts">
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import Dropdown from '$lib/components/shared/Dropdown.svelte';
	import { createSecret, updateSecret, readSecret } from '$lib/state/vault.svelte';
	import { addToast } from '$lib/state/toasts.svelte';

	interface Props {
		open: boolean;
		mode: 'create' | 'edit';
		secretId?: string;
		initialName?: string;
		initialCategory?: string;
		onclose: () => void;
		onsave: () => void;
	}

	let {
		open,
		mode,
		secretId,
		initialName = '',
		initialCategory = 'password',
		onclose,
		onsave
	}: Props = $props();

	// Categories
	const categories = [
		{ value: 'password', label: 'Password' },
		{ value: 'ssh_key', label: 'SSH Key' },
		{ value: 'api_token', label: 'API Token' },
		{ value: 'certificate', label: 'Certificate' },
		{ value: 'note', label: 'Note' },
		{ value: 'custom', label: 'Custom' }
	];

	// Form state
	let name = $state('');
	let category = $state('password');
	let value = $state('');
	let showValue = $state(false);
	let saving = $state(false);
	let loading = $state(false);
	let textareaEl: HTMLTextAreaElement | undefined = $state();

	// Derived
	let isEditMode = $derived(mode === 'edit');
	let isMonospace = $derived(
		category === 'ssh_key' || category === 'api_token' || category === 'certificate'
	);
	let canSave = $derived(
		name.trim().length > 0 && value.trim().length > 0 && !saving && !loading
	);
	let modalTitle = $derived(isEditMode ? 'Edit Secret' : 'New Secret');

	// Reset form when modal opens/mode changes
	$effect(() => {
		if (open) {
			if (mode === 'edit') {
				name = initialName;
				category = initialCategory;
				value = '';
				showValue = false;
				// Load existing value
				loadSecretValue();
			} else {
				name = '';
				category = 'password';
				value = '';
				showValue = false;
			}
		}
	});

	// Auto-grow textarea
	$effect(() => {
		if (textareaEl && value) {
			textareaEl.style.height = 'auto';
			textareaEl.style.height = `${Math.min(textareaEl.scrollHeight, 300)}px`;
		}
	});

	async function loadSecretValue(): Promise<void> {
		if (!secretId) return;

		loading = true;
		try {
			value = await readSecret(secretId);
		} catch (err) {
			addToast(`Failed to load secret: ${err}`, 'error');
		} finally {
			loading = false;
		}
	}

	async function handleSave(): Promise<void> {
		if (!canSave) return;

		saving = true;

		try {
			if (isEditMode && secretId) {
				await updateSecret(secretId, value.trim());
				addToast('Secret updated', 'success');
			} else {
				await createSecret(name.trim(), category, value.trim());
				addToast('Secret created', 'success');
			}
			onsave();
			onclose();
		} catch (err) {
			addToast(`Failed to save secret: ${err}`, 'error');
		} finally {
			saving = false;
		}
	}

	function handleClose(): void {
		if (!saving) {
			onclose();
		}
	}

	function generatePassword(): void {
		const chars = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()_+-=[]{}|;:,.<>?';
		const array = new Uint32Array(20);
		crypto.getRandomValues(array);
		value = Array.from(array, (n) => chars[n % chars.length]).join('');
		showValue = true;
		addToast('Password generated', 'info');
	}

	async function copyValue(): Promise<void> {
		try {
			await navigator.clipboard.writeText(value);
			addToast('Copied to clipboard', 'success');
		} catch (err) {
			addToast(`Failed to copy: ${err}`, 'error');
		}
	}

	function toggleShowValue(): void {
		showValue = !showValue;
	}
</script>

<Modal {open} onclose={handleClose} title={modalTitle}>
	<form class="form" onsubmit={(e) => { e.preventDefault(); handleSave(); }}>
		<Input
			label="Name"
			placeholder="My Secret"
			bind:value={name}
			disabled={isEditMode || saving}
		/>

		<div class="form-field">
			<span class="form-label">Category</span>
			{#if isEditMode}
				<span class="form-value">{categories.find(c => c.value === category)?.label ?? category}</span>
			{:else}
				<Dropdown
					options={categories}
					bind:selected={category}
				/>
			{/if}
		</div>

		<div class="form-field">
			<div class="value-header">
				<label class="form-label" for="secret-value">Value</label>
				<div class="value-actions">
					{#if category === 'password'}
						<button
							type="button"
							class="action-btn"
							onclick={generatePassword}
							disabled={saving || loading}
							title="Generate random password"
						>
							<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
								<path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 11-7.778 7.778 5.5 5.5 0 017.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
							Generate
						</button>
					{/if}
					<button
						type="button"
						class="action-btn"
						onclick={toggleShowValue}
						disabled={saving || loading}
						title={showValue ? 'Hide value' : 'Show value'}
					>
						{#if showValue}
							<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
								<path d="M17.94 17.94A10.07 10.07 0 0112 20c-7 0-11-8-11-8a18.45 18.45 0 015.06-5.94M9.9 4.24A9.12 9.12 0 0112 4c7 0 11 8 11 8a18.5 18.5 0 01-2.16 3.19m-6.72-1.07a3 3 0 11-4.24-4.24" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
								<path d="M1 1l22 22" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
							Hide
						{:else}
							<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
								<path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
								<circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
							Show
						{/if}
					</button>
					<button
						type="button"
						class="action-btn"
						onclick={copyValue}
						disabled={!value || saving || loading}
						title="Copy to clipboard"
					>
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
							<rect x="9" y="9" width="13" height="13" rx="2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
							<path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
						</svg>
						Copy
					</button>
				</div>
			</div>
			{#if loading}
				<div class="loading-placeholder">
					<span class="spinner"></span>
					Loading secret...
				</div>
			{:else}
				<textarea
					id="secret-value"
					class="value-textarea"
					class:monospace={isMonospace}
					class:masked={!showValue}
					placeholder="Enter secret value..."
					bind:value
					bind:this={textareaEl}
					disabled={saving}
					rows="4"
				></textarea>
			{/if}
		</div>
	</form>

	{#snippet actions()}
		<Button variant="secondary" onclick={handleClose} disabled={saving}>
			Cancel
		</Button>
		<Button variant="primary" onclick={handleSave} disabled={!canSave}>
			{#if saving}
				<span class="spinner"></span>
				Saving...
			{:else}
				{isEditMode ? 'Update' : 'Save'}
			{/if}
		</Button>
	{/snippet}
</Modal>

<style>
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

	.value-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.value-actions {
		display: flex;
		gap: 6px;
	}

	.action-btn {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 4px 8px;
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		font-weight: 500;
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: 4px;
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
	}

	.action-btn:hover:not(:disabled) {
		color: var(--color-text-primary);
		background-color: rgba(255, 255, 255, 0.06);
		border-color: var(--color-text-secondary);
	}

	.action-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.value-textarea {
		width: 100%;
		padding: 12px;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		resize: none;
		min-height: 100px;
		max-height: 300px;
		outline: none;
		transition: border-color var(--duration-default) var(--ease-default);
		box-sizing: border-box;
	}

	.value-textarea.monospace {
		font-family: var(--font-mono, 'JetBrains Mono', monospace);
		font-size: 0.75rem;
	}

	.value-textarea.masked {
		-webkit-text-security: disc;
	}

	.value-textarea:focus {
		border-color: var(--color-accent);
	}

	.value-textarea:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.value-textarea::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.loading-placeholder {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		padding: 24px;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
	}

	.spinner {
		display: inline-block;
		width: 14px;
		height: 14px;
		border: 2px solid rgba(255, 255, 255, 0.2);
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
