<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import {
		getActiveEnvironments,
		getActiveEnvironmentName,
		getActiveVariables,
		getActiveProviders,
		getProviderCatalog,
		saveEnvironments,
		loadCatalog
	} from '$lib/state/tofu.svelte';
	import Button from '$lib/components/shared/Button.svelte';

	let newEnvName = $state('');
	let selectedEnvName = $state<string | null>(null);
	let localValues = $state<Record<string, string>>({});

	let environments = $derived(getActiveEnvironments());
	let activeEnvName = $derived(getActiveEnvironmentName());
	let variables = $derived(getActiveVariables());
	let providers = $derived(getActiveProviders());
	let catalog = $derived(getProviderCatalog());

	let sensitiveProviderFields = $derived.by(() => {
		const fields: Array<{ varName: string; label: string }> = [];
		for (const provider of providers) {
			const entry = catalog.find((c) => c.id === provider.providerId);
			if (!entry) continue;
			for (const field of entry.fields) {
				if (field.fieldType === 'sensitive') {
					const varName = `${provider.providerId}_${field.name}`;
					fields.push({ varName, label: `${entry.name} - ${field.label}` });
				}
			}
		}
		return fields;
	});

	let selectedEnv = $derived(environments.find((e) => e.name === selectedEnvName) ?? null);

	onMount(() => {
		loadCatalog();
	});

	function syncLocalValues(envName: string | null) {
		const env = environments.find((e) => e.name === envName);
		localValues = env ? { ...env.values } : {};
	}

	function selectEnvironment(envName: string | null) {
		selectedEnvName = envName;
		syncLocalValues(envName);
	}

	function handleAddEnvironment() {
		const trimmed = newEnvName.trim();
		if (!trimmed) return;
		if (environments.some((e) => e.name === trimmed)) return;

		const updated = [...environments, { name: trimmed, values: {} }];
		saveEnvironments(updated, activeEnvName);
		selectEnvironment(trimmed);
		newEnvName = '';
	}

	function handleDeleteEnvironment() {
		if (!selectedEnvName) return;
		if (!confirm(t('tofu.delete_environment_confirm'))) return;

		const updated = environments.filter((e) => e.name !== selectedEnvName);
		const newActive = activeEnvName === selectedEnvName ? null : activeEnvName;
		saveEnvironments(updated, newActive);
		selectEnvironment(updated.length > 0 ? updated[0].name : null);
	}

	function handleSaveValues() {
		if (!selectedEnvName) return;
		const updated = environments.map((e) =>
			e.name === selectedEnvName ? { ...e, values: { ...localValues } } : e
		);
		saveEnvironments(updated, activeEnvName);
	}

	function handleSetActive() {
		if (!selectedEnvName) return;
		saveEnvironments(environments, selectedEnvName);
	}

	function handleValueChange(key: string, value: string) {
		localValues[key] = value;
	}
</script>

<div class="env-panel">
	<header class="header">
		<h2 class="title">{t('tofu.environments')}</h2>
	</header>

	<div class="env-controls">
		<div class="selector-row">
			{#if environments.length > 0}
				<select
					class="env-select"
					value={selectedEnvName ?? ''}
					onchange={(e) => {
						const val = e.currentTarget.value;
						selectEnvironment(val || null);
					}}
				>
					<option value="" disabled>{t('tofu.select_environment')}</option>
					{#each environments as env (env.name)}
						<option value={env.name}>
							{env.name}{activeEnvName === env.name ? ' *' : ''}
						</option>
					{/each}
				</select>

				{#if selectedEnvName}
					<Button variant="danger" size="sm" onclick={handleDeleteEnvironment}>
						{t('tofu.delete_environment')}
					</Button>
				{/if}
			{/if}
		</div>

		<div class="add-row">
			<input
				type="text"
				class="env-input"
				placeholder={t('tofu.environment_name_placeholder')}
				bind:value={newEnvName}
				onkeydown={(e) => {
					if (e.key === 'Enter') handleAddEnvironment();
				}}
			/>
			<Button variant="primary" size="sm" onclick={handleAddEnvironment} disabled={!newEnvName.trim()}>
				{t('tofu.add_environment')}
			</Button>
		</div>
	</div>

	{#if environments.length === 0}
		<div class="empty-state">
			<svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="empty-icon">
				<path
					d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2z"
					stroke="currentColor"
					stroke-width="1.5"
				/>
				<path d="M8 12h8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
				<path d="M12 8v8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
			</svg>
			<p class="empty-text">{t('tofu.no_environments')}</p>
		</div>
	{:else if selectedEnvName && selectedEnv}
		<div class="values-section">
			<div class="values-header">
				<h3 class="values-title">{t('tofu.variable_values')} - {selectedEnvName}</h3>
				<div class="values-actions">
					{#if activeEnvName === selectedEnvName}
						<span class="active-badge">{t('tofu.active')}</span>
					{:else}
						<Button variant="secondary" size="sm" onclick={handleSetActive}>
							{t('tofu.set_active')}
						</Button>
					{/if}
					<Button variant="primary" size="sm" onclick={handleSaveValues}>
						{t('tofu.save_values')}
					</Button>
				</div>
			</div>

			{#if variables.length === 0 && sensitiveProviderFields.length === 0}
				<p class="no-vars-text">{t('tofu.no_variables_defined')}</p>
			{:else}
				<div class="values-table">
					{#each variables as variable (variable.name)}
						<div class="value-row">
							<label class="value-label" for="var-{variable.name}">
								<span class="var-name">{variable.name}</span>
								{#if variable.description}
									<span class="var-desc">{variable.description}</span>
								{/if}
							</label>
							<input
								id="var-{variable.name}"
								type={variable.sensitive ? 'password' : 'text'}
								class="value-input"
								value={localValues[variable.name] ?? ''}
								oninput={(e) => handleValueChange(variable.name, e.currentTarget.value)}
								placeholder={variable.defaultValue ?? ''}
							/>
						</div>
					{/each}

					{#each sensitiveProviderFields as field (field.varName)}
						<div class="value-row">
							<label class="value-label" for="prov-{field.varName}">
								<span class="var-name">{field.varName}</span>
								<span class="var-desc">{field.label}</span>
							</label>
							<input
								id="prov-{field.varName}"
								type="password"
								class="value-input"
								value={localValues[field.varName] ?? ''}
								oninput={(e) => handleValueChange(field.varName, e.currentTarget.value)}
							/>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	.env-panel {
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

	.env-controls {
		display: flex;
		flex-direction: column;
		gap: 12px;
		margin-bottom: 24px;
	}

	.selector-row {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.env-select {
		flex: 1;
		padding: 7px 10px;
		font-size: 0.8125rem;
		background: var(--color-bg-elevated);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		cursor: pointer;
	}

	.env-select:focus {
		border-color: var(--color-accent);
	}

	.add-row {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.env-input {
		flex: 1;
		padding: 7px 10px;
		font-size: 0.8125rem;
		background: var(--color-bg-elevated);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		font-family: var(--font-sans);
	}

	.env-input::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.6;
	}

	.env-input:focus {
		border-color: var(--color-accent);
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

	.values-section {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.values-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		flex-wrap: wrap;
	}

	.values-title {
		margin: 0;
		font-size: 0.9375rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.values-actions {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.active-badge {
		display: inline-flex;
		align-items: center;
		padding: 4px 10px;
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--color-accent);
		background: rgba(255, 255, 255, 0.06);
		border: 1px solid var(--color-accent);
		border-radius: var(--radius-btn);
		user-select: none;
	}

	.no-vars-text {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		font-style: italic;
		opacity: 0.6;
	}

	.values-table {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.value-row {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 12px 16px;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
	}

	.value-label {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.var-name {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
		font-family: monospace;
	}

	.var-desc {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.value-input {
		width: 100%;
		padding: 7px 10px;
		font-size: 0.8125rem;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		font-family: var(--font-sans);
		box-sizing: border-box;
	}

	.value-input:focus {
		border-color: var(--color-accent);
	}

	.value-input::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}
</style>
