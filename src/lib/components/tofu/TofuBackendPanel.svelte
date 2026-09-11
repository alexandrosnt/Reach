<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import {
		getActiveBackend,
		getBackendCatalog,
		loadBackendCatalog,
		saveBackend
	} from '$lib/state/tofu.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import Toggle from '$lib/components/shared/Toggle.svelte';
	import type { TofuBackendConfig, BackendCatalogEntry } from '$lib/ipc/tofu';
	import { getActiveEncryption, saveEncryption } from '$lib/state/tofu.svelte';

	let backend = $derived(getActiveBackend());
	let catalog = $derived(getBackendCatalog());

	let selectedType = $state('');
	let fields = $state<Record<string, unknown>>({});
	let saved = $state(false);

	// ---- State encryption ---------------------------------------------------
	// OpenTofu's own feature (1.7+): the state file and saved plans encrypted
	// with AES-256-GCM under a key derived from a passphrase. The passphrase
	// lives in Reach's vault with the project and reaches OpenTofu as an
	// environment variable; the generated HCL only ever names the variable.
	let encryption = $derived(getActiveEncryption());
	let encEnabled = $state(false);
	let encPassphrase = $state('');
	let encShow = $state(false);
	let encSaved = $state(false);
	$effect(() => {
		encEnabled = encryption?.enabled ?? false;
		encPassphrase = encryption?.passphrase ?? '';
	});
	let encDirty = $derived(
		encEnabled !== (encryption?.enabled ?? false) || encPassphrase !== (encryption?.passphrase ?? '')
	);

	function generatePassphrase() {
		const bytes = new Uint8Array(24);
		crypto.getRandomValues(bytes);
		encPassphrase = btoa(String.fromCharCode(...bytes)).replace(/[+/=]/g, '').slice(0, 32);
		encShow = true;
	}

	async function handleSaveEncryption() {
		await saveEncryption(encEnabled || encPassphrase ? { enabled: encEnabled, passphrase: encPassphrase } : null);
		encSaved = true;
		setTimeout(() => (encSaved = false), 1500);
	}

	onMount(() => {
		loadBackendCatalog();
	});

	function selectedCatalogEntry(): BackendCatalogEntry | undefined {
		return catalog.find((c) => c.id === selectedType);
	}

	function backendCatalogEntry(): BackendCatalogEntry | undefined {
		if (!backend) return undefined;
		return catalog.find((c) => c.id === backend.backendType);
	}

	function handleTypeChange(value: string) {
		selectedType = value;
		const entry = catalog.find((c) => c.id === value);
		if (entry) {
			const defaults: Record<string, unknown> = {};
			for (const field of entry.fields) {
				if (field.defaultValue !== null) {
					if (field.fieldType === 'number') {
						defaults[field.name] = Number(field.defaultValue);
					} else if (field.fieldType === 'bool') {
						defaults[field.name] = field.defaultValue === 'true';
					} else {
						defaults[field.name] = field.defaultValue;
					}
				} else if (field.fieldType === 'bool') {
					defaults[field.name] = false;
				} else if (field.fieldType === 'number') {
					defaults[field.name] = 0;
				} else {
					defaults[field.name] = '';
				}
			}
			fields = defaults;
		} else {
			fields = {};
		}
	}

	function setField(name: string, value: unknown) {
		fields = { ...fields, [name]: value };
	}

	async function handleSave() {
		if (!selectedType) return;
		await saveBackend({ backendType: selectedType, fields });
		selectedType = '';
		fields = {};
		saved = true;
		setTimeout(() => {
			saved = false;
		}, 2000);
	}

	async function handleRemove() {
		await saveBackend(null);
		saved = false;
	}
</script>

<div class="backend-panel">
	<header class="header">
		<h2 class="title">{t('tofu.backend_title')}</h2>
	</header>

	{#if backend}
		<div class="card">
			<div class="card-info">
				<span class="card-name">
					{backendCatalogEntry()?.name ?? backend.backendType}
				</span>
				<span class="card-meta">{t('tofu.backend_type')}: {backend.backendType}</span>
				{#if backendCatalogEntry()}
					<div class="field-values">
						{#each backendCatalogEntry()?.fields ?? [] as schema (schema.name)}
							{#if backend.fields[schema.name] !== undefined && backend.fields[schema.name] !== ''}
								<div class="field-value-row">
									<span class="field-value-label">{schema.label}:</span>
									<span class="field-value-text">
										{#if schema.fieldType === 'sensitive'}
											********
										{:else if schema.fieldType === 'bool'}
											{backend.fields[schema.name] ? 'true' : 'false'}
										{:else}
											{backend.fields[schema.name]}
										{/if}
									</span>
								</div>
							{/if}
						{/each}
					</div>
				{/if}
			</div>
			<div class="card-actions">
				<Button variant="danger" size="sm" onclick={handleRemove}>
					{t('tofu.backend_remove')}
				</Button>
			</div>
		</div>
	{:else}
		<div class="empty-state">
			<svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="empty-icon">
				<rect
					x="2"
					y="3"
					width="20"
					height="18"
					rx="2"
					stroke="currentColor"
					stroke-width="1.5"
				/>
				<path d="M2 9h20" stroke="currentColor" stroke-width="1.5" />
				<circle cx="6" cy="6" r="1" fill="currentColor" />
				<circle cx="9.5" cy="6" r="1" fill="currentColor" />
				<path d="M7 14h10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
				<path d="M7 17h6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
			</svg>
			<p class="empty-text">{t('tofu.backend_none')}</p>
		</div>

		<div class="select-section">
			<label class="field-label" for="backend-type-select">{t('tofu.backend_select')}</label>
			<select
				id="backend-type-select"
				class="field-input"
				value={selectedType}
				onchange={(e) => handleTypeChange(e.currentTarget.value)}
			>
				<option value="">-- {t('tofu.backend_select')} --</option>
				{#each catalog as entry (entry.id)}
					<option value={entry.id}>{entry.name}</option>
				{/each}
			</select>
		</div>

		{#if selectedType && selectedCatalogEntry()}
			<div class="config-form">
				<p class="form-description">{selectedCatalogEntry()?.description}</p>

				{#each selectedCatalogEntry()?.fields ?? [] as schema (schema.name)}
					<div class="field-group">
						<label class="field-label" for="backend-field-{schema.name}">
							{schema.label}
							{#if schema.required}
								<span class="required">*</span>
							{/if}
						</label>

						{#if schema.fieldType === 'string'}
							<input
								id="backend-field-{schema.name}"
								type="text"
								class="field-input"
								value={fields[schema.name] ?? schema.defaultValue ?? ''}
								oninput={(e) => setField(schema.name, e.currentTarget.value)}
							/>
						{:else if schema.fieldType === 'number'}
							<input
								id="backend-field-{schema.name}"
								type="number"
								class="field-input"
								value={fields[schema.name] ?? schema.defaultValue ?? ''}
								oninput={(e) => setField(schema.name, Number(e.currentTarget.value))}
							/>
						{:else if schema.fieldType === 'bool'}
							<label class="toggle-label">
								<span class="toggle-wrapper">
									<input
										type="checkbox"
										class="toggle-input"
										checked={fields[schema.name] === true}
										onchange={(e) => setField(schema.name, e.currentTarget.checked)}
									/>
									<span class="toggle-track">
										<span class="toggle-thumb"></span>
									</span>
								</span>
							</label>
						{:else if schema.fieldType === 'select'}
							<select
								id="backend-field-{schema.name}"
								class="field-input"
								value={fields[schema.name] ?? schema.defaultValue ?? ''}
								onchange={(e) => setField(schema.name, e.currentTarget.value)}
							>
								{#each schema.options as opt (opt.value)}
									<option value={opt.value}>{opt.label}</option>
								{/each}
							</select>
						{:else if schema.fieldType === 'sensitive'}
							<input
								id="backend-field-{schema.name}"
								type="password"
								class="field-input"
								value={fields[schema.name] ?? ''}
								oninput={(e) => setField(schema.name, e.currentTarget.value)}
							/>
						{/if}

						{#if schema.helpText}
							<p class="field-help">{schema.helpText}</p>
						{/if}
					</div>
				{/each}

				<div class="form-actions">
					<Button variant="primary" size="sm" onclick={handleSave}>
						{t('tofu.backend_save')}
					</Button>
				</div>
			</div>
		{/if}

		{#if saved}
			<p class="saved-message">{t('tofu.backend_saved')}</p>
		{/if}
	{/if}
	<section class="encryption">
		<header class="header">
			<h2 class="title">{t('tofu.encryption_title')}</h2>
		</header>
		<p class="enc-desc">{t('tofu.encryption_desc')}</p>
		<div class="enc-row">
			<Toggle hideLabel checked={encEnabled} label={t('tofu.encryption_enable')} onchange={(v) => (encEnabled = v)} />
			<span class="enc-label">{t('tofu.encryption_enable')}</span>
		</div>
		{#if encEnabled}
			<div class="enc-row">
				<label class="enc-field">
					<span class="enc-label">{t('tofu.encryption_passphrase')}</span>
					<span class="enc-input-row">
						<input
							class="enc-input"
							type={encShow ? 'text' : 'password'}
							bind:value={encPassphrase}
							autocomplete="off"
							spellcheck="false"
						/>
						<Button variant="ghost" size="sm" onclick={() => (encShow = !encShow)}>{encShow ? t('vault.hide') : t('vault.show')}</Button>
						<Button variant="secondary" size="sm" onclick={generatePassphrase}>{t('vault.generate')}</Button>
					</span>
				</label>
			</div>
			<p class="enc-warn">{t('tofu.encryption_warning')}</p>
		{/if}
		<div class="enc-actions">
			<Button variant="primary" size="sm" disabled={!encDirty || (encEnabled && !encPassphrase)} onclick={handleSaveEncryption}>
				{encSaved ? t('tofu.encryption_saved') : t('tofu.encryption_save')}
			</Button>
		</div>
	</section>
</div>

<style>
	.encryption {
		display: flex;
		flex-direction: column;
		gap: 10px;
		margin-top: 24px;
		padding-top: 18px;
		border-top: 1px solid var(--color-border);
	}

	.enc-desc,
	.enc-warn {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		max-width: 68ch;
	}

	.enc-warn {
		color: var(--color-warning, #f59e0b);
	}

	.enc-row {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.enc-label {
		font-size: 0.8125rem;
		color: var(--color-text-primary);
	}

	.enc-field {
		display: flex;
		flex-direction: column;
		gap: 4px;
		width: 100%;
		max-width: 560px;
	}

	.enc-input-row {
		display: flex;
		gap: 6px;
		align-items: center;
	}

	.enc-input {
		flex: 1;
		min-width: 0;
		padding: 6px 8px;
		font-family: var(--font-mono, monospace);
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
	}

	.enc-actions {
		display: flex;
		justify-content: flex-end;
	}

	.backend-panel {
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

	.card {
		display: flex;
		align-items: flex-start;
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
		flex: 1;
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

	.card-actions {
		display: flex;
		gap: 8px;
		flex-shrink: 0;
	}

	.field-values {
		display: flex;
		flex-direction: column;
		gap: 4px;
		margin-top: 8px;
		padding-top: 8px;
		border-top: 1px solid var(--color-border);
	}

	.field-value-row {
		display: flex;
		gap: 8px;
		font-size: 0.75rem;
	}

	.field-value-label {
		color: var(--color-text-secondary);
		white-space: nowrap;
	}

	.field-value-text {
		color: var(--color-text-primary);
		font-family: monospace;
		word-break: break-all;
	}

	.select-section {
		display: flex;
		flex-direction: column;
		gap: 6px;
		margin-top: 24px;
		max-width: 400px;
	}

	.config-form {
		display: flex;
		flex-direction: column;
		gap: 16px;
		margin-top: 20px;
		padding: 20px;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
	}

	.form-description {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		line-height: 1.5;
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		padding-top: 8px;
		border-top: 1px solid var(--color-border);
	}

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
		transition: border-color 0.15s ease;
		width: 100%;
		box-sizing: border-box;
	}

	.field-input:focus {
		border-color: var(--color-accent);
	}

	.field-help {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
		line-height: 1.4;
	}

	.toggle-label {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		cursor: pointer;
		user-select: none;
	}

	.toggle-wrapper {
		position: relative;
		display: inline-flex;
		align-items: center;
	}

	.toggle-input {
		position: absolute;
		opacity: 0;
		width: 0;
		height: 0;
		pointer-events: none;
	}

	.toggle-track {
		display: inline-block;
		width: 34px;
		height: 18px;
		background: var(--color-border);
		border-radius: 9px;
		position: relative;
		transition: background-color 0.2s ease;
	}

	.toggle-input:checked + .toggle-track {
		background: var(--color-accent);
	}

	.toggle-thumb {
		position: absolute;
		top: 2px;
		left: 2px;
		width: 14px;
		height: 14px;
		background: #fff;
		border-radius: 50%;
		transition: transform 0.2s ease;
	}

	.toggle-input:checked + .toggle-track .toggle-thumb {
		transform: translateX(16px);
	}

	.saved-message {
		margin: 16px 0 0;
		font-size: 0.8125rem;
		color: var(--color-accent);
		font-weight: 500;
	}

	@media (max-width: 700px) {
		.backend-panel {
			padding: 16px;
		}

		.card {
			flex-direction: column;
			gap: 12px;
		}

		.card-actions {
			width: 100%;
			justify-content: flex-end;
		}

		.select-section {
			max-width: 100%;
		}

		.config-form {
			padding: 16px;
		}
	}
</style>
