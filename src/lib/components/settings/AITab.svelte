<script lang="ts">
	import Toggle from '$lib/components/shared/Toggle.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import {
		getAISettings,
		updateAISetting,
		getModels,
		getModelsLoading,
		getModelsError,
		fetchModels,
		loadSecureAISettings
	} from '$lib/state/ai.svelte';
	import { isLocked } from '$lib/state/vault.svelte';
	import { t } from '$lib/state/i18n.svelte';

	const aiSettings = getAISettings();

	let searchQuery = $state('');
	let savingApiKey = $state(false);
	let apiKeyError = $state<string | null>(null);

	// Load secure settings when vault is unlocked
	$effect(() => {
		if (!isLocked()) {
			loadSecureAISettings();
		}
	});

	const filteredModels = $derived.by(() => {
		const models = getModels();
		if (!searchQuery.trim()) return models;
		const q = searchQuery.toLowerCase();
		return models.filter(
			(m) => m.id.toLowerCase().includes(q) || m.name.toLowerCase().includes(q)
		);
	});

	async function onEnabledChange(checked: boolean) {
		await updateAISetting('enabled', checked);
	}

	async function onApiKeyInput(e: Event & { currentTarget: HTMLInputElement }) {
		const value = e.currentTarget.value;
		savingApiKey = true;
		apiKeyError = null;
		try {
			await updateAISetting('apiKey', value);
		} catch (err) {
			apiKeyError = err instanceof Error ? err.message : String(err);
		} finally {
			savingApiKey = false;
		}
	}

	async function onValidate() {
		await fetchModels();
	}

	async function selectModel(id: string) {
		await updateAISetting('selectedModel', id);
	}

	function formatContext(length: number): string {
		if (length >= 1_000_000) return `${Math.round(length / 1_000_000)}M`;
		if (length >= 1000) return `${Math.round(length / 1000)}K`;
		return String(length);
	}

	function formatPricing(prompt: string, completion: string): string {
		const p = parseFloat(prompt) * 1_000_000;
		const c = parseFloat(completion) * 1_000_000;
		if (p === 0 && c === 0) return 'free';
		const fmt = (v: number) => (v < 0.01 ? v.toFixed(4) : v.toFixed(2));
		return `$${fmt(p)} / $${fmt(c)}`;
	}
</script>

<div class="tab-content">
	<!-- Enable AI row -->
	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('ai_settings.enable')}</span>
			<span class="setting-description">{t('ai_settings.enable_desc')}</span>
		</div>
		<div class="setting-control">
			<Toggle
				checked={aiSettings.enabled}
				label={t('ai_settings.enable')}
				onchange={onEnabledChange}
			/>
		</div>
	</div>

	<!-- API Key section -->
	<div class="ai-section" class:disabled-section={!aiSettings.enabled}>
		<div class="api-key-section">
			<span class="setting-label">{t('ai_settings.api_key')}</span>
			<span class="setting-description">{t('ai_settings.api_key_desc')}</span>
			<div class="api-key-row">
				<Input
					type="password"
					value={aiSettings.apiKey}
					placeholder="sk-or-..."
					disabled={!aiSettings.enabled}
					oninput={onApiKeyInput}
				/>
				<button
					class="validate-btn"
					disabled={!aiSettings.enabled || !aiSettings.apiKey || getModelsLoading()}
					onclick={onValidate}
				>
					{#if getModelsLoading()}
						{t('ai_settings.validating')}
					{:else}
						{t('ai_settings.validate')}
					{/if}
				</button>
			</div>
			<div class="api-status">
				{#if apiKeyError}
					<span class="status-error">{apiKeyError}</span>
				{:else if savingApiKey}
					<span class="status-hint">{t('ai_settings.saving_encrypted')}</span>
				{:else if getModelsError()}
					<span class="status-error">{getModelsError()}</span>
				{:else if getModels().length > 0}
					<span class="status-success">{t('ai_settings.model_count', { count: getModels().length })}</span>
				{:else if isLocked()}
					<span class="status-hint">{t('ai_settings.unlock_vault')}</span>
				{:else}
					<span class="status-hint">{t('ai_settings.enter_api_key')}</span>
				{/if}
			</div>
		</div>
	</div>

	<!-- Model Browser section -->
	<div class="ai-section" class:disabled-section={!aiSettings.enabled}>
		<div class="section-header">
			<span class="setting-label">{t('ai_settings.model_browser')}</span>
		</div>

		<div class="model-search">
			<input
				class="search-input"
				type="text"
				placeholder={t('ai_settings.search_models')}
				disabled={!aiSettings.enabled}
				bind:value={searchQuery}
			/>
		</div>

		<div class="model-list">
			{#if getModels().length === 0}
				<div class="model-empty">
					{t('ai_settings.no_models')}
				</div>
			{:else if filteredModels.length === 0}
				<div class="model-empty">
					{t('ai_settings.no_match')}
				</div>
			{:else}
				{#each filteredModels as model (model.id)}
					<button
						class="model-row"
						class:selected={aiSettings.selectedModel === model.id}
						disabled={!aiSettings.enabled}
						onclick={() => selectModel(model.id)}
					>
						<div class="model-info">
							<span class="model-name">{model.name || model.id}</span>
							<span class="model-meta">
								{formatContext(model.context_length)} ctx
							</span>
						</div>
						<span class="model-pricing">
							{formatPricing(model.pricing.prompt, model.pricing.completion)}
						</span>
					</button>
				{/each}
			{/if}
		</div>
	</div>

	<!-- Selected model indicator -->
	{#if aiSettings.selectedModel}
		<div class="selected-indicator">
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
				<polyline points="20 6 9 17 4 12" />
			</svg>
			<span>{aiSettings.selectedModel}</span>
		</div>
	{/if}
</div>

<style>
	.tab-content {
		display: flex;
		flex-direction: column;
	}

	.setting-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 10px 0;
		border-bottom: 1px solid var(--color-border);
		gap: 24px;
	}

	.setting-row:last-child {
		border-bottom: none;
	}

	.setting-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.setting-label {
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.setting-description {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.setting-control {
		flex-shrink: 0;
	}

	/* AI sections with reduced opacity when disabled */
	.ai-section {
		transition: opacity var(--duration-default) var(--ease-default);
	}

	.ai-section.disabled-section {
		opacity: 0.4;
		pointer-events: none;
	}

	/* API Key section — stacked layout */
	.api-key-section {
		display: flex;
		flex-direction: column;
		gap: 3px;
		padding: 10px 0;
		border-bottom: 1px solid var(--color-border);
	}

	.api-key-row {
		display: flex;
		gap: 8px;
		align-items: stretch;
		margin-top: 4px;
	}

	.validate-btn {
		padding: 0 12px;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		font-weight: 500;
		color: #fff;
		background-color: var(--color-accent);
		border: none;
		border-radius: var(--radius-btn);
		cursor: pointer;
		white-space: nowrap;
		transition:
			opacity var(--duration-default) var(--ease-default),
			background-color var(--duration-default) var(--ease-default);
		flex-shrink: 0;
	}

	.validate-btn:hover:not(:disabled) {
		opacity: 0.85;
	}

	.validate-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.api-status {
		margin-top: 4px;
		font-size: 0.75rem;
	}

	.status-hint {
		color: var(--color-text-secondary);
	}

	.status-success {
		color: #34c759;
	}

	.status-error {
		color: #ff453a;
	}

	/* Model Browser */
	.section-header {
		padding: 10px 0 6px;
	}

	.model-search {
		padding-bottom: 6px;
	}

	.search-input {
		width: 100%;
		padding: 8px 12px;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		box-sizing: border-box;
		transition: border-color var(--duration-default) var(--ease-default);
	}

	.search-input:focus {
		border-color: var(--color-accent);
	}

	.search-input::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.search-input:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.model-list {
		max-height: 180px;
		overflow-y: auto;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background-color: var(--color-bg-secondary);
	}

	.model-empty {
		padding: 16px;
		text-align: center;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
	}

	.model-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		padding: 8px 12px;
		border: none;
		border-bottom: 1px solid var(--color-border);
		background: transparent;
		cursor: pointer;
		text-align: left;
		font-family: var(--font-sans);
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.model-row:last-child {
		border-bottom: none;
	}

	.model-row:hover:not(:disabled) {
		background-color: rgba(255, 255, 255, 0.04);
	}

	.model-row.selected {
		background-color: color-mix(in srgb, var(--color-accent) 15%, transparent);
	}

	.model-row:disabled {
		cursor: not-allowed;
	}

	.model-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.model-name {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.model-meta {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.model-pricing {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		white-space: nowrap;
		flex-shrink: 0;
		margin-left: 12px;
	}

	/* Selected model indicator */
	.selected-indicator {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 8px 0;
		font-size: 0.75rem;
		color: var(--color-accent);
	}

	.selected-indicator svg {
		flex-shrink: 0;
	}

	.selected-indicator span {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
