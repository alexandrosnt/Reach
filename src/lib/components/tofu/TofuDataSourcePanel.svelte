<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import {
		getActiveDataSources,
		getDataSourceCatalog,
		loadDataSourceCatalog,
		saveDataSources,
		getActiveProviders,
		getProviderCatalog,
		loadCatalog
	} from '$lib/state/tofu.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import type { TofuDataSource, DataSourceCatalogEntry } from '$lib/ipc/tofu';

	type Category = 'All' | 'Compute' | 'Network' | 'Storage' | 'Container' | 'Utility';

	let showPicker = $state(false);
	let configDataSourceId = $state<string | null>(null);

	let dataSources = $derived(getActiveDataSources());
	let dsCatalog = $derived(getDataSourceCatalog());
	let providerCatalog = $derived(getProviderCatalog());

	onMount(() => {
		loadDataSourceCatalog();
		loadCatalog();
	});

	function catalogEntry(dataType: string): DataSourceCatalogEntry | undefined {
		return dsCatalog.find((c) => c.id === dataType);
	}

	function providerName(providerId: string): string {
		const entry = providerCatalog.find((c) => c.id === providerId);
		return entry?.name ?? providerId;
	}

	function handleRemove(dsId: string) {
		const updated = dataSources.filter((d) => d.id !== dsId);
		saveDataSources(updated);
	}

	// --- Picker state ---
	let pickerSearch = $state('');
	let pickerCategory = $state<Category>('All');
	let pickerSelectedEntry = $state<string | null>(null);
	let pickerLogicalName = $state('');
	let pickerNameError = $state<string | null>(null);

	const categories: { key: Category; i18nKey: string }[] = [
		{ key: 'All', i18nKey: 'tofu.resource_category_all' },
		{ key: 'Compute', i18nKey: 'tofu.resource_category_compute' },
		{ key: 'Network', i18nKey: 'tofu.resource_category_network' },
		{ key: 'Storage', i18nKey: 'tofu.resource_category_storage' },
		{ key: 'Container', i18nKey: 'tofu.resource_category_container' },
		{ key: 'Utility', i18nKey: 'tofu.resource_category_utility' }
	];

	let activeProviders = $derived(getActiveProviders());
	let activeProviderIds = $derived(new Set(activeProviders.map((p) => p.providerId)));

	let filteredCatalog = $derived.by(() => {
		let items = dsCatalog.filter((entry) => activeProviderIds.has(entry.providerId));

		if (pickerCategory !== 'All') {
			items = items.filter((entry) => entry.category === pickerCategory);
		}

		if (pickerSearch.trim()) {
			const q = pickerSearch.trim().toLowerCase();
			items = items.filter(
				(entry) =>
					entry.name.toLowerCase().includes(q) ||
					entry.dataType.toLowerCase().includes(q) ||
					entry.description.toLowerCase().includes(q)
			);
		}

		return items;
	});

	function openPicker() {
		pickerSearch = '';
		pickerCategory = 'All';
		pickerSelectedEntry = null;
		pickerLogicalName = '';
		pickerNameError = null;
		showPicker = true;
	}

	function closePicker() {
		showPicker = false;
	}

	function handlePickerCardClick(entryId: string) {
		pickerSelectedEntry = entryId;
		pickerLogicalName = '';
		pickerNameError = null;
	}

	function validatePickerName(): boolean {
		if (!/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(pickerLogicalName)) {
			pickerNameError = t('tofu.data_source_name_invalid');
			return false;
		}

		const entry = dsCatalog.find((e) => e.id === pickerSelectedEntry);
		if (entry) {
			const duplicate = dataSources.some(
				(d) => d.dataType === entry.id && d.logicalName === pickerLogicalName
			);
			if (duplicate) {
				pickerNameError = t('tofu.data_source_name_invalid');
				return false;
			}
		}

		pickerNameError = null;
		return true;
	}

	function handlePickerAdd() {
		if (!validatePickerName()) return;

		const entry = dsCatalog.find((e) => e.id === pickerSelectedEntry);
		if (!entry) return;

		const newDs: TofuDataSource = {
			id: crypto.randomUUID(),
			dataType: entry.id,
			logicalName: pickerLogicalName,
			providerId: entry.providerId,
			fields: {}
		};

		saveDataSources([...dataSources, newDs]);
		closePicker();
	}

	function pickerProviderDisplayName(providerId: string): string {
		const provider = providerCatalog.find((p) => p.id === providerId);
		return provider?.name ?? providerId;
	}

	// --- Config modal state ---
	let configFields = $state<Record<string, unknown>>({});
	let configLogicalName = $state('');
	let configNameError = $state<string | null>(null);

	let configDataSource = $derived(
		configDataSourceId ? dataSources.find((d) => d.id === configDataSourceId) : undefined
	);
	let configCatalogEntry = $derived(
		configDataSource ? dsCatalog.find((c) => c.id === configDataSource.dataType) : undefined
	);

	function openConfig(dsId: string) {
		const ds = dataSources.find((d) => d.id === dsId);
		if (ds) {
			configFields = { ...ds.fields };
			configLogicalName = ds.logicalName;
			configNameError = null;
		}
		configDataSourceId = dsId;
	}

	function closeConfig() {
		configDataSourceId = null;
	}

	function setConfigField(name: string, value: unknown) {
		configFields = { ...configFields, [name]: value };
	}

	function validateConfigName(): boolean {
		if (!/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(configLogicalName)) {
			configNameError = t('tofu.data_source_name_invalid');
			return false;
		}

		const duplicate = dataSources.find(
			(d) =>
				d.id !== configDataSourceId &&
				d.dataType === configDataSource?.dataType &&
				d.logicalName === configLogicalName
		);
		if (duplicate) {
			configNameError = t('tofu.data_source_name_invalid');
			return false;
		}

		configNameError = null;
		return true;
	}

	async function handleConfigSave() {
		if (!configDataSource) return;
		if (!validateConfigName()) return;

		const updated = dataSources.map((d) =>
			d.id === configDataSourceId
				? {
						...d,
						logicalName: configLogicalName,
						fields: configFields
					}
				: d
		);

		await saveDataSources(updated);
		closeConfig();
	}

	// --- Shared modal handlers ---
	function handlePickerBackdrop(e: MouseEvent) {
		if (e.target === e.currentTarget) closePicker();
	}

	function handleConfigBackdrop(e: MouseEvent) {
		if (e.target === e.currentTarget) closeConfig();
	}

	$effect(() => {
		function onKeydown(e: KeyboardEvent) {
			if (e.key === 'Escape') {
				if (configDataSourceId) {
					closeConfig();
				} else if (showPicker) {
					closePicker();
				}
			}
		}

		document.addEventListener('keydown', onKeydown);
		return () => {
			document.removeEventListener('keydown', onKeydown);
		};
	});
</script>

<div class="ds-panel">
	<header class="header">
		<h2 class="title">{t('tofu.data_sources_title')}</h2>
		<Button variant="primary" size="sm" onclick={openPicker}>
			{t('tofu.add_data_source')}
		</Button>
	</header>

	{#if dataSources.length === 0}
		<div class="empty-state">
			<svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="empty-icon">
				<path
					d="M4 6h16M4 6v12a2 2 0 002 2h12a2 2 0 002-2V6M4 6l2-4h12l2 4"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linejoin="round"
					stroke-linecap="round"
				/>
				<path d="M9 14l2 2 4-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
			</svg>
			<p class="empty-text">{t('tofu.no_data_sources')}</p>
		</div>
	{:else}
		<div class="card-list">
			{#each dataSources as ds (ds.id)}
				<div class="card">
					<div class="card-info">
						<span class="card-name">
							{catalogEntry(ds.dataType)?.name ?? ds.dataType}
						</span>
						<span class="card-meta">
							{ds.logicalName} &middot; {ds.dataType}
						</span>
						<span class="card-provider">{providerName(ds.providerId)}</span>
					</div>
					<div class="card-actions">
						<Button
							variant="secondary"
							size="sm"
							onclick={() => openConfig(ds.id)}
						>
							{t('tofu.configure')}
						</Button>
						<Button
							variant="danger"
							size="sm"
							onclick={() => handleRemove(ds.id)}
						>
							{t('tofu.remove')}
						</Button>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

<!-- Picker Modal -->
{#if showPicker}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="overlay" onclick={handlePickerBackdrop} onkeydown={() => {}}>
		<div class="modal" role="dialog" aria-modal="true" aria-label={t('tofu.add_data_source')}>
			<header class="modal-header">
				<h2 class="modal-title">{t('tofu.add_data_source')}</h2>
				<button type="button" class="close-btn" onclick={closePicker} aria-label={t('tofu.cancel')}>
					<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
						<path
							d="M1 1L13 13M13 1L1 13"
							stroke="currentColor"
							stroke-width="1.5"
							stroke-linecap="round"
						/>
					</svg>
				</button>
			</header>

			<div class="modal-body">
				{#if activeProviderIds.size === 0}
					<div class="empty-message">{t('tofu.no_providers_for_resources')}</div>
				{:else}
					<input
						class="search-input"
						type="text"
						placeholder={t('tofu.resource_search')}
						bind:value={pickerSearch}
					/>

					<div class="category-pills">
						{#each categories as cat (cat.key)}
							<button
								type="button"
								class="pill"
								class:active={pickerCategory === cat.key}
								onclick={() => (pickerCategory = cat.key)}
							>
								{t(cat.i18nKey)}
							</button>
						{/each}
					</div>

					<div class="resource-grid">
						{#each filteredCatalog as entry (entry.id)}
							<button
								type="button"
								class="resource-card"
								class:selected={pickerSelectedEntry === entry.id}
								onclick={() => handlePickerCardClick(entry.id)}
							>
								<div class="resource-name">{entry.name}</div>
								<div class="resource-type">{entry.dataType}</div>
								<div class="resource-desc">{entry.description}</div>
								<span class="provider-badge">{pickerProviderDisplayName(entry.providerId)}</span>
							</button>

							{#if pickerSelectedEntry === entry.id}
								<div class="name-form">
									<input
										class="name-input"
										type="text"
										placeholder={t('tofu.data_source_name')}
										bind:value={pickerLogicalName}
										onkeydown={(e) => {
											if (e.key === 'Enter') handlePickerAdd();
										}}
									/>
									{#if pickerNameError}
										<p class="name-error">{pickerNameError}</p>
									{/if}
									<p class="name-help">{t('tofu.resource_logical_name_help')}</p>
									<div class="name-actions">
										<Button variant="primary" size="sm" onclick={handlePickerAdd}>
											{t('tofu.add_data_source')}
										</Button>
									</div>
								</div>
							{/if}
						{/each}
					</div>
				{/if}
			</div>
		</div>
	</div>
{/if}

<!-- Config Modal -->
{#if configDataSourceId && configDataSource}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="overlay" onclick={handleConfigBackdrop} onkeydown={() => {}}>
		<div class="modal config-modal" role="dialog" aria-modal="true" aria-label={t('tofu.edit_data_source')}>
			<header class="modal-header">
				<h2 class="modal-title">{configCatalogEntry?.name ?? configDataSource.dataType}</h2>
				<button type="button" class="close-btn" onclick={closeConfig} aria-label={t('tofu.cancel')}>
					<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
						<path d="M1 1L13 13M13 1L1 13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
					</svg>
				</button>
			</header>

			<div class="modal-body">
				<div class="field-group">
					<label class="field-label" for="ds-logical-name">{t('tofu.data_source_name')}</label>
					<input
						id="ds-logical-name"
						type="text"
						class="field-input"
						bind:value={configLogicalName}
					/>
					{#if configNameError}
						<p class="field-error">{configNameError}</p>
					{/if}
					<p class="field-help">{t('tofu.resource_logical_name_help')}</p>
				</div>

				{#if configCatalogEntry}
					{#each configCatalogEntry.fields as schema (schema.name)}
						<div class="field-group">
							<label class="field-label" for="ds-field-{schema.name}">
								{schema.label}
								{#if schema.required}
									<span class="required">*</span>
								{/if}
							</label>

							{#if schema.fieldType === 'string'}
								<input
									id="ds-field-{schema.name}"
									type="text"
									class="field-input"
									value={configFields[schema.name] ?? schema.defaultValue ?? ''}
									oninput={(e) => setConfigField(schema.name, e.currentTarget.value)}
								/>
							{:else if schema.fieldType === 'number'}
								<input
									id="ds-field-{schema.name}"
									type="number"
									class="field-input"
									value={configFields[schema.name] ?? schema.defaultValue ?? ''}
									oninput={(e) => setConfigField(schema.name, Number(e.currentTarget.value))}
								/>
							{:else if schema.fieldType === 'bool'}
								<label class="toggle-label">
									<span class="toggle-wrapper">
										<input
											type="checkbox"
											class="toggle-input"
											checked={configFields[schema.name] === true}
											onchange={(e) => setConfigField(schema.name, e.currentTarget.checked)}
										/>
										<span class="toggle-track">
											<span class="toggle-thumb"></span>
										</span>
									</span>
								</label>
							{:else if schema.fieldType === 'select'}
								<select
									id="ds-field-{schema.name}"
									class="field-input"
									value={configFields[schema.name] ?? schema.defaultValue ?? ''}
									onchange={(e) => setConfigField(schema.name, e.currentTarget.value)}
								>
									{#each schema.options as opt (opt.value)}
										<option value={opt.value}>{opt.label}</option>
									{/each}
								</select>
							{:else if schema.fieldType === 'sensitive'}
								<input
									id="ds-field-{schema.name}"
									type="password"
									class="field-input"
									value={configFields[schema.name] ?? ''}
									oninput={(e) => setConfigField(schema.name, e.currentTarget.value)}
								/>
							{/if}

							{#if schema.helpText}
								<p class="field-help">{schema.helpText}</p>
							{/if}
						</div>
					{/each}
				{/if}
			</div>

			<footer class="modal-actions">
				<Button variant="secondary" size="sm" onclick={closeConfig}>
					{t('tofu.cancel')}
				</Button>
				<Button variant="primary" size="sm" onclick={handleConfigSave}>
					{t('tofu.save')}
				</Button>
			</footer>
		</div>
	</div>
{/if}

<style>
	.ds-panel {
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

	.card-provider {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
	}

	.card-actions {
		display: flex;
		gap: 8px;
		flex-shrink: 0;
	}

	/* --- Overlay & Modal --- */
	.overlay {
		position: fixed;
		inset: 0;
		z-index: 100;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(0, 0, 0, 0.6);
		animation: fadeIn var(--duration-default) var(--ease-default);
	}

	.modal {
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-modal);
		box-shadow: var(--shadow-elevated);
		width: 90%;
		max-width: 700px;
		max-height: 80vh;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		animation: scaleIn var(--duration-default) var(--ease-default);
	}

	.config-modal {
		max-width: 500px;
		max-height: 85vh;
		backdrop-filter: blur(4px);
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

	.close-btn {
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

	.close-btn:hover {
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

	/* --- Picker --- */
	.search-input {
		width: 100%;
		padding: 10px 14px;
		font-size: 0.875rem;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		transition: border-color var(--duration-default) var(--ease-default);
		box-sizing: border-box;
	}

	.search-input::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.6;
	}

	.search-input:focus {
		border-color: var(--color-accent);
	}

	.category-pills {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}

	.pill {
		padding: 5px 14px;
		font-size: 0.8125rem;
		font-weight: 500;
		border: 1px solid var(--color-border);
		border-radius: 9999px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition:
			background-color var(--duration-default) var(--ease-default),
			border-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
		white-space: nowrap;
	}

	.pill:hover {
		background-color: rgba(255, 255, 255, 0.06);
		color: var(--color-text-primary);
	}

	.pill.active {
		background-color: var(--color-accent);
		border-color: var(--color-accent);
		color: #fff;
	}

	.resource-grid {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.resource-card {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 14px 16px;
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		cursor: pointer;
		text-align: left;
		transition:
			border-color var(--duration-default) var(--ease-default),
			background-color var(--duration-default) var(--ease-default);
	}

	.resource-card:hover {
		border-color: var(--color-accent);
		background-color: rgba(255, 255, 255, 0.03);
	}

	.resource-card.selected {
		border-color: var(--color-accent);
		background-color: rgba(255, 255, 255, 0.03);
	}

	.resource-name {
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.resource-type {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		font-family: monospace;
	}

	.resource-desc {
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		line-height: 1.4;
		margin-top: 2px;
	}

	.provider-badge {
		display: inline-block;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		background: rgba(255, 255, 255, 0.06);
		padding: 2px 8px;
		border-radius: 9999px;
		margin-top: 4px;
	}

	.name-form {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 12px 16px 4px;
		border-top: 1px solid var(--color-border);
	}

	.name-input {
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		color: var(--color-text-primary);
		border-radius: var(--radius-btn);
		padding: 8px 10px;
		font-size: 0.8125rem;
		font-family: monospace;
		outline: none;
		transition: border-color var(--duration-default) var(--ease-default);
		width: 100%;
		box-sizing: border-box;
	}

	.name-input:focus {
		border-color: var(--color-accent);
	}

	.name-error {
		font-size: 0.75rem;
		color: var(--color-danger);
		margin: 0;
	}

	.name-help {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
		margin: 0;
		line-height: 1.4;
	}

	.name-actions {
		display: flex;
		justify-content: flex-end;
		padding-bottom: 8px;
	}

	.empty-message {
		text-align: center;
		padding: 32px 16px;
		color: var(--color-text-secondary);
		font-size: 0.875rem;
	}

	/* --- Config form fields --- */
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

	.field-help {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
		line-height: 1.4;
	}

	.field-error {
		font-size: 0.75rem;
		color: var(--color-danger);
		margin: 0;
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
