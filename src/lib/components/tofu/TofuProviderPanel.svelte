<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import type { TofuExecutionTarget } from '$lib/ipc/tofu';
	import {
		getActiveProviders,
		getProviderCatalog,
		loadCatalog,
		removeProvider,
		fetchSchema,
		isSchemaLoading,
		getDynamicSchema,
		getSchemaError
	} from '$lib/state/tofu.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import TofuProviderPicker from './TofuProviderPicker.svelte';
	import TofuProviderConfigModal from './TofuProviderConfigModal.svelte';

	let { target }: { target: TofuExecutionTarget } = $props();

	let showPicker = $state(false);
	let configProviderId = $state<string | null>(null);

	let providers = $derived(getActiveProviders());
	let catalog = $derived(getProviderCatalog());
	let schemaLoading = $derived(isSchemaLoading());
	let dynamicSchema = $derived(getDynamicSchema());
	let schemaError = $derived(getSchemaError());
	let resourceCount = $derived(
		dynamicSchema ? dynamicSchema.reduce((sum, s) => sum + (s.resourceSchemas?.length ?? 0), 0) : 0
	);

	onMount(() => {
		loadCatalog();
	});

	function catalogName(providerId: string): string {
		const entry = catalog.find((c) => c.id === providerId);
		return entry?.name ?? providerId;
	}

	function handleRemove(providerId: string) {
		removeProvider(providerId);
	}
</script>

<div class="provider-panel">
	<header class="header">
		<h2 class="title">{t('tofu.providers')}</h2>
		<div class="header-actions">
			<div class="schema-section">
				<Button
					variant="secondary"
					size="sm"
					disabled={schemaLoading || providers.length === 0}
					onclick={() => fetchSchema(target)}
				>
					{schemaLoading ? t('tofu.schema_fetching') : t('tofu.schema_fetch')}
				</Button>
				{#if dynamicSchema && resourceCount > 0}
					<span class="schema-badge">
						{t('tofu.schema_resource_count', { count: String(resourceCount) })}
					</span>
				{/if}
			</div>
			<Button variant="primary" size="sm" onclick={() => (showPicker = true)}>
				{t('tofu.add_provider')}
			</Button>
		</div>
	</header>

	{#if schemaError}
		<div class="schema-error">
			<span class="schema-error-label">{t('tofu.schema_error')}:</span>
			{schemaError}
		</div>
	{/if}

	{#if dynamicSchema && !schemaError}
		<p class="schema-hint">{t('tofu.schema_hint')}</p>
	{/if}

	{#if providers.length === 0}
		<div class="empty-state">
			<svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="empty-icon">
				<path
					d="M12 2L3 7v10l9 5 9-5V7l-9-5z"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linejoin="round"
				/>
				<path d="M12 22V12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
				<path
					d="M3 7l9 5 9-5"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linejoin="round"
				/>
			</svg>
			<p class="empty-text">{t('tofu.no_providers')}</p>
		</div>
	{:else}
		<div class="card-list">
			{#each providers as provider (provider.providerId)}
				<div class="card">
					<div class="card-info">
						<span class="card-name">{catalogName(provider.providerId)}</span>
						<span class="card-meta">{provider.source} &middot; {provider.version}</span>
					</div>
					<div class="card-actions">
						<Button
							variant="secondary"
							size="sm"
							onclick={() => (configProviderId = provider.providerId)}
						>
							{t('tofu.configure')}
						</Button>
						<Button
							variant="danger"
							size="sm"
							onclick={() => handleRemove(provider.providerId)}
						>
							{t('tofu.remove')}
						</Button>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

{#if showPicker}
	<TofuProviderPicker onclose={() => (showPicker = false)} />
{/if}

{#if configProviderId}
	<TofuProviderConfigModal
		providerId={configProviderId}
		onclose={() => (configProviderId = null)}
	/>
{/if}

<style>
	.provider-panel {
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
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.card-actions {
		display: flex;
		gap: 8px;
		flex-shrink: 0;
	}

	.header-actions {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.schema-section {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.schema-badge {
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--color-accent);
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
		padding: 2px 8px;
		border-radius: var(--radius-btn);
		white-space: nowrap;
	}

	.schema-error {
		margin-bottom: 16px;
		padding: 10px 14px;
		font-size: 0.8125rem;
		color: var(--color-danger, #ef4444);
		background: color-mix(in srgb, var(--color-danger, #ef4444) 8%, transparent);
		border: 1px solid color-mix(in srgb, var(--color-danger, #ef4444) 25%, transparent);
		border-radius: var(--radius-btn);
		line-height: 1.4;
	}

	.schema-error-label {
		font-weight: 600;
	}

	.schema-hint {
		margin: 0 0 16px;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		line-height: 1.4;
	}
</style>
