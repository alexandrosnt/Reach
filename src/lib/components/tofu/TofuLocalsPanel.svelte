<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import { getActiveLocals, saveLocals } from '$lib/state/tofu.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import Modal from '$lib/components/shared/Modal.svelte';
	import type { TofuLocal } from '$lib/ipc/tofu';

	let showModal = $state(false);
	let editing = $state<TofuLocal | null>(null);

	let locals = $derived(getActiveLocals());

	// Form state
	let name = $state('');
	let expression = $state('');
	let nameTouched = $state(false);

	const nameRegex = /^[a-zA-Z_][a-zA-Z0-9_]*$/;

	let existingNames = $derived(locals.map((l) => l.name));
	let nameInvalid = $derived(nameTouched && name.length > 0 && !nameRegex.test(name));
	let nameDuplicate = $derived(
		nameTouched &&
			name.length > 0 &&
			nameRegex.test(name) &&
			existingNames
				.filter((n) => (editing ? n !== editing.name : true))
				.includes(name)
	);

	let nameError = $derived(
		nameInvalid
			? t('tofu.local_name_invalid')
			: nameDuplicate
				? t('tofu.local_name_exists')
				: ''
	);

	let canSave = $derived(
		name.trim().length > 0 &&
			nameRegex.test(name) &&
			!nameDuplicate &&
			expression.trim().length > 0
	);

	let modalTitle = $derived(editing ? t('tofu.edit_local') : t('tofu.add_local'));

	function resetForm() {
		name = '';
		expression = '';
		nameTouched = false;
	}

	function handleAdd() {
		editing = null;
		resetForm();
		showModal = true;
	}

	function handleEdit(local: TofuLocal) {
		editing = local;
		name = local.name;
		expression = local.expression;
		nameTouched = false;
		showModal = true;
	}

	function handleRemove(localName: string) {
		const updated = locals.filter((l) => l.name !== localName);
		saveLocals(updated);
	}

	function handleClose() {
		showModal = false;
		editing = null;
	}

	function handleSave() {
		nameTouched = true;
		if (!canSave) return;

		const entry: TofuLocal = {
			name: name.trim(),
			expression: expression.trim()
		};

		let updated: TofuLocal[];
		if (editing) {
			updated = locals.map((l) => (l.name === editing!.name ? entry : l));
		} else {
			updated = [...locals, entry];
		}

		saveLocals(updated);
		showModal = false;
		editing = null;
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSave();
		}
	}
</script>

<div class="locals-panel">
	<header class="header">
		<h2 class="title">{t('tofu.locals_title')}</h2>
		<Button variant="primary" size="sm" onclick={handleAdd}>
			{t('tofu.add_local')}
		</Button>
	</header>

	{#if locals.length === 0}
		<div class="empty-state">
			<svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="empty-icon">
				<path
					d="M4 7V4h16v3"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
				<path
					d="M9 20h6"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
				<path
					d="M12 4v16"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
			<p class="empty-text">{t('tofu.no_locals')}</p>
		</div>
	{:else}
		<div class="card-list">
			{#each locals as local (local.name)}
				<div class="card">
					<div class="card-info">
						<span class="card-name">{local.name}</span>
						<span class="card-expression">{local.expression}</span>
					</div>
					<div class="card-actions">
						<button
							type="button"
							class="action-btn edit"
							onclick={() => handleEdit(local)}
							title={t('tofu.edit_local')}
						>
							<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
								<path
									d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"
									stroke="currentColor"
									stroke-width="1.5"
									stroke-linecap="round"
									stroke-linejoin="round"
								/>
								<path
									d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"
									stroke="currentColor"
									stroke-width="1.5"
									stroke-linecap="round"
									stroke-linejoin="round"
								/>
							</svg>
						</button>
						<button
							type="button"
							class="action-btn remove"
							onclick={() => handleRemove(local.name)}
							title={t('tofu.remove')}
						>
							<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
								<path
									d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6"
									stroke="currentColor"
									stroke-width="1.5"
									stroke-linecap="round"
									stroke-linejoin="round"
								/>
							</svg>
						</button>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

<Modal open={showModal} onclose={handleClose} title={modalTitle} maxWidth="480px">
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<form class="form" onsubmit={(e) => { e.preventDefault(); handleSave(); }} onkeydown={handleKeydown}>
		<div class="form-field">
			<label class="form-label" for="local-name">{t('tofu.local_name')}</label>
			<input
				id="local-name"
				class="form-input"
				class:error={nameError.length > 0}
				type="text"
				bind:value={name}
				oninput={() => { nameTouched = true; }}
				placeholder="my_local"
			/>
			{#if nameError}
				<span class="form-error">{nameError}</span>
			{/if}
		</div>

		<div class="form-field">
			<label class="form-label" for="local-expression">{t('tofu.local_expression')}</label>
			<textarea
				id="local-expression"
				class="form-textarea"
				bind:value={expression}
				placeholder={t('tofu.local_expression_placeholder')}
				rows="4"
			></textarea>
		</div>
	</form>

	{#snippet actions()}
		<Button variant="secondary" size="sm" onclick={handleClose}>
			{t('tofu.cancel')}
		</Button>
		<Button variant="primary" size="sm" onclick={handleSave} disabled={!canSave}>
			{t('tofu.save')}
		</Button>
	{/snippet}
</Modal>

<style>
	.locals-panel {
		width: 100%;
		height: 100%;
		overflow-y: auto;
		background: var(--color-bg-primary);
		padding: 24px;
	}

	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 24px;
	}

	.title {
		margin: 0;
		font-size: 1.25rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 16px;
		padding: 64px 32px;
		text-align: center;
	}

	.empty-icon {
		color: var(--color-text-secondary);
		opacity: 0.4;
	}

	.empty-text {
		margin: 0;
		font-size: 0.875rem;
		color: var(--color-text-secondary);
		max-width: 400px;
		line-height: 1.5;
	}

	.card-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.card {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 12px;
		padding: 12px 16px;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		transition: border-color 0.12s ease;
	}

	.card:hover {
		border-color: color-mix(in srgb, var(--color-border) 70%, var(--color-accent));
	}

	.card-info {
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 0;
		overflow: hidden;
	}

	.card-name {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
		font-family: monospace;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.card-expression {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		font-family: monospace;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.card-actions {
		display: flex;
		gap: 4px;
		flex-shrink: 0;
		margin-top: 2px;
	}

	.action-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 4px;
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition:
			background-color 0.12s ease,
			color 0.12s ease,
			border-color 0.12s ease;
	}

	.action-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.action-btn.edit {
		color: var(--color-accent);
	}

	.action-btn.edit:hover:not(:disabled) {
		background: rgba(255, 255, 255, 0.04);
		border-color: var(--color-accent);
	}

	.action-btn.remove {
		color: var(--color-danger, #ef4444);
	}

	.action-btn.remove:hover:not(:disabled) {
		background: rgba(239, 68, 68, 0.08);
		border-color: var(--color-danger, #ef4444);
	}

	/* Modal form styles */
	.form {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.form-field {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.form-label {
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-text-secondary);
	}

	.form-input {
		width: 100%;
		padding: 10px 12px;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		transition: border-color var(--duration-default) var(--ease-default);
		box-sizing: border-box;
	}

	.form-input:focus {
		border-color: var(--color-accent);
	}

	.form-input.error {
		border-color: var(--color-danger);
	}

	.form-input::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.form-textarea {
		width: 100%;
		padding: 10px 12px;
		font-family: var(--font-mono, monospace);
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		resize: vertical;
		min-height: 80px;
		line-height: 1.5;
		transition: border-color var(--duration-default) var(--ease-default);
		box-sizing: border-box;
	}

	.form-textarea:focus {
		border-color: var(--color-accent);
	}

	.form-textarea::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.form-error {
		font-size: 0.6875rem;
		color: var(--color-danger);
	}
</style>
