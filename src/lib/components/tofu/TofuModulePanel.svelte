<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import { getActiveModules, saveModules } from '$lib/state/tofu.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import type { TofuModuleConfig } from '$lib/ipc/tofu';

	let modules = $derived(getActiveModules());

	let editing = $state<TofuModuleConfig | null>(null);
	let showModal = $state(false);

	// Form state
	let formName = $state('');
	let formSource = $state('');
	let formVersion = $state('');
	let formInputs = $state<Array<{ key: string; value: string }>>([]);
	let nameError = $state('');

	function resetForm() {
		formName = '';
		formSource = '';
		formVersion = '';
		formInputs = [];
		nameError = '';
	}

	function handleAdd() {
		editing = null;
		resetForm();
		showModal = true;
	}

	function handleEdit(mod: TofuModuleConfig) {
		editing = mod;
		formName = mod.name;
		formSource = mod.source;
		formVersion = mod.version;
		formInputs = Object.entries(mod.inputs).map(([key, value]) => ({
			key,
			value: String(value)
		}));
		nameError = '';
		showModal = true;
	}

	async function handleRemove(moduleId: string) {
		const newList = modules.filter((m) => m.id !== moduleId);
		await saveModules(newList);
	}

	function validateName(name: string): boolean {
		if (!name) {
			nameError = t('tofu.module_name_invalid');
			return false;
		}
		if (!/^[a-zA-Z_]/.test(name)) {
			nameError = t('tofu.module_name_invalid');
			return false;
		}
		const isDuplicate = modules.some(
			(m) => m.name === name && m.id !== (editing?.id ?? '')
		);
		if (isDuplicate) {
			nameError = t('tofu.module_name_invalid');
			return false;
		}
		nameError = '';
		return true;
	}

	async function handleSave() {
		if (!validateName(formName)) return;

		const inputs: Record<string, unknown> = {};
		for (const entry of formInputs) {
			if (entry.key.trim()) {
				inputs[entry.key.trim()] = entry.value;
			}
		}

		const mod: TofuModuleConfig = {
			id: editing?.id ?? crypto.randomUUID(),
			name: formName.trim(),
			source: formSource.trim(),
			version: formVersion.trim(),
			inputs
		};

		let newList: TofuModuleConfig[];
		if (editing) {
			newList = modules.map((m) => (m.id === editing!.id ? mod : m));
		} else {
			newList = [...modules, mod];
		}

		await saveModules(newList);
		showModal = false;
		editing = null;
	}

	function handleClose() {
		showModal = false;
		editing = null;
	}

	function addInput() {
		formInputs = [...formInputs, { key: '', value: '' }];
	}

	function removeInput(index: number) {
		formInputs = formInputs.filter((_, i) => i !== index);
	}

	function updateInputKey(index: number, key: string) {
		formInputs = formInputs.map((entry, i) => (i === index ? { ...entry, key } : entry));
	}

	function updateInputValue(index: number, value: string) {
		formInputs = formInputs.map((entry, i) => (i === index ? { ...entry, value } : entry));
	}

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			handleClose();
		}
	}

	$effect(() => {
		function onKeydown(e: KeyboardEvent) {
			if (e.key === 'Escape' && showModal) {
				handleClose();
			}
		}

		document.addEventListener('keydown', onKeydown);

		return () => {
			document.removeEventListener('keydown', onKeydown);
		};
	});
</script>

<div class="module-panel">
	<header class="header">
		<h2 class="title">{t('tofu.modules_title')}</h2>
		<Button variant="primary" size="sm" onclick={handleAdd}>
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
				<path
					d="M12 5v14M5 12h14"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
			{t('tofu.add_module')}
		</Button>
	</header>

	{#if modules.length === 0}
		<div class="empty-state">
			<svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="empty-icon">
				<path
					d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linejoin="round"
				/>
				<path d="M3.27 6.96L12 12.01l8.73-5.05" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
				<path d="M12 22.08V12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
			</svg>
			<p class="empty-text">{t('tofu.no_modules')}</p>
		</div>
	{:else}
		<div class="card-list">
			{#each modules as mod (mod.id)}
				<div class="card">
					<div class="card-info">
						<span class="card-name">{mod.name}</span>
						<span class="card-meta">{mod.source}</span>
						{#if mod.version}
							<span class="card-version">{mod.version}</span>
						{/if}
					</div>
					<div class="card-actions">
						<Button
							variant="secondary"
							size="sm"
							onclick={() => handleEdit(mod)}
						>
							{t('tofu.edit_module')}
						</Button>
						<Button
							variant="danger"
							size="sm"
							onclick={() => handleRemove(mod.id)}
						>
							{t('tofu.remove')}
						</Button>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

{#if showModal}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="overlay" onclick={handleBackdropClick} onkeydown={() => {}}>
		<div class="modal" role="dialog" aria-modal="true" aria-label={editing ? t('tofu.edit_module') : t('tofu.add_module')}>
			<header class="modal-header">
				<h2 class="modal-title">
					{editing ? t('tofu.edit_module') : t('tofu.add_module')}
				</h2>
				<button type="button" class="modal-close" onclick={handleClose} aria-label={t('tofu.cancel')}>
					<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
						<path d="M1 1L13 13M13 1L1 13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
					</svg>
				</button>
			</header>

			<div class="modal-body">
				<div class="field-group">
					<label class="field-label" for="module-name">
						{t('tofu.module_name')}
						<span class="required">*</span>
					</label>
					<input
						id="module-name"
						type="text"
						class="field-input"
						class:field-error={nameError}
						placeholder="my_module"
						bind:value={formName}
						oninput={() => validateName(formName)}
					/>
					{#if nameError}
						<p class="field-error-text">{nameError}</p>
					{/if}
				</div>

				<div class="field-group">
					<label class="field-label" for="module-source">
						{t('tofu.module_source')}
						<span class="required">*</span>
					</label>
					<input
						id="module-source"
						type="text"
						class="field-input"
						placeholder={t('tofu.module_source_placeholder')}
						bind:value={formSource}
					/>
				</div>

				<div class="field-group">
					<label class="field-label" for="module-version">
						{t('tofu.module_version')}
					</label>
					<input
						id="module-version"
						type="text"
						class="field-input"
						placeholder="~> 1.0"
						bind:value={formVersion}
					/>
				</div>

				<div class="field-group">
					<div class="inputs-header">
						<span class="field-label">{t('tofu.module_inputs')}</span>
						<button type="button" class="add-input-btn" onclick={addInput}>
							<svg width="12" height="12" viewBox="0 0 24 24" fill="none">
								<path
									d="M12 5v14M5 12h14"
									stroke="currentColor"
									stroke-width="2"
									stroke-linecap="round"
									stroke-linejoin="round"
								/>
							</svg>
							{t('tofu.module_add_input')}
						</button>
					</div>

					{#if formInputs.length > 0}
						<div class="inputs-list">
							{#each formInputs as entry, index (index)}
								<div class="input-row">
									<input
										type="text"
										class="field-input input-key"
										placeholder={t('tofu.module_input_key')}
										value={entry.key}
										oninput={(e) => updateInputKey(index, e.currentTarget.value)}
									/>
									<input
										type="text"
										class="field-input input-value"
										placeholder={t('tofu.module_input_value')}
										value={entry.value}
										oninput={(e) => updateInputValue(index, e.currentTarget.value)}
									/>
									<button
										type="button"
										class="remove-input-btn"
										onclick={() => removeInput(index)}
										aria-label={t('tofu.remove')}
									>
										<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
											<path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
										</svg>
									</button>
								</div>
							{/each}
						</div>
					{/if}
				</div>
			</div>

			<footer class="modal-actions">
				<Button variant="secondary" size="sm" onclick={handleClose}>
					{t('tofu.cancel')}
				</Button>
				<Button variant="primary" size="sm" onclick={handleSave} disabled={!formName || !formSource}>
					{t('tofu.save')}
				</Button>
			</footer>
		</div>
	</div>
{/if}

<style>
	.module-panel {
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

	/* Empty state */
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

	/* Card list */
	.card-list {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.card {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 16px;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		transition:
			border-color 0.15s ease,
			box-shadow 0.15s ease;
	}

	.card:hover {
		border-color: var(--color-accent);
		box-shadow: var(--shadow-elevated);
	}

	.card-info {
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 0;
	}

	.card-name {
		font-size: 0.9375rem;
		font-weight: 600;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.card-meta {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		font-family: monospace;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.card-version {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
	}

	.card-actions {
		display: flex;
		gap: 8px;
		flex-shrink: 0;
	}

	/* Overlay / Modal */
	.overlay {
		position: fixed;
		inset: 0;
		z-index: 100;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(0, 0, 0, 0.5);
		backdrop-filter: blur(4px);
		animation: fadeIn var(--duration-default) var(--ease-default);
	}

	.modal {
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-modal);
		box-shadow: var(--shadow-elevated);
		min-width: 320px;
		max-width: 560px;
		width: 90%;
		max-height: 85vh;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		animation: scaleIn var(--duration-default) var(--ease-default);
	}

	.modal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 16px 20px;
		border-bottom: 1px solid var(--color-border);
	}

	.modal-title {
		font-size: 1rem;
		font-weight: 600;
		color: var(--color-text-primary);
		margin: 0;
	}

	.modal-close {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.modal-close:hover {
		background-color: rgba(255, 255, 255, 0.08);
		color: var(--color-text-primary);
	}

	.modal-body {
		padding: 20px;
		overflow-y: auto;
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.modal-actions {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: 8px;
		padding: 16px 20px;
		border-top: 1px solid var(--color-border);
	}

	/* Form fields */
	.field-group {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.field-label {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-secondary);
	}

	.required {
		color: var(--color-danger);
		margin-left: 2px;
	}

	.field-input {
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		color: var(--color-text-primary);
		border-radius: var(--radius-btn);
		padding: 8px 10px;
		font-size: 0.8125rem;
		font-family: var(--font-sans);
		outline: none;
		transition: border-color var(--duration-default) var(--ease-default);
		width: 100%;
		box-sizing: border-box;
	}

	.field-input:focus {
		border-color: var(--color-accent);
	}

	.field-input.field-error {
		border-color: var(--color-danger);
	}

	.field-error-text {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-danger);
		line-height: 1.4;
	}

	/* Inputs section */
	.inputs-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.add-input-btn {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 4px 10px;
		font-size: 0.75rem;
		font-weight: 500;
		font-family: var(--font-sans);
		color: var(--color-accent);
		background: transparent;
		border: 1px solid var(--color-accent);
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition:
			background-color 0.15s ease,
			color 0.15s ease;
	}

	.add-input-btn:hover {
		background: rgba(var(--color-accent-rgb, 99, 102, 241), 0.1);
	}

	.inputs-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.input-row {
		display: flex;
		gap: 8px;
		align-items: center;
	}

	.input-key {
		flex: 1;
	}

	.input-value {
		flex: 1;
	}

	.remove-input-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		flex-shrink: 0;
		padding: 0;
		background: transparent;
		border: none;
		border-radius: var(--radius-btn);
		color: var(--color-text-secondary);
		cursor: pointer;
		transition:
			color 0.12s ease,
			background-color 0.12s ease;
	}

	.remove-input-btn:hover {
		color: var(--color-danger);
		background: rgba(255, 255, 255, 0.06);
	}

	@keyframes fadeIn {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	@keyframes scaleIn {
		from {
			opacity: 0;
			transform: scale(0.95);
		}
		to {
			opacity: 1;
			transform: scale(1);
		}
	}
</style>
