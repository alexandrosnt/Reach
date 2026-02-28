<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import {
		getActiveResources,
		getResourceCatalog,
		loadResourceCatalog,
		loadCatalog,
		removeResource,
		getActiveProviders,
		getProviderCatalog,
		getDynamicSchema
	} from '$lib/state/tofu.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import TofuResourcePicker from './TofuResourcePicker.svelte';
	import TofuResourceConfigModal from './TofuResourceConfigModal.svelte';

	let showPicker = $state(false);
	let configResourceId = $state<string | null>(null);

	let resources = $derived(getActiveResources());
	let resourceCatalog = $derived(getResourceCatalog());
	let providerCatalog = $derived(getProviderCatalog());
	let schema = $derived(getDynamicSchema());
	let schemaResourceCount = $derived(
		schema ? schema.reduce((sum, p) => sum + p.resourceSchemas.length, 0) : 0
	);

	onMount(() => {
		loadResourceCatalog();
		loadCatalog();
	});

	function catalogEntry(resourceType: string) {
		return resourceCatalog.find((c) => c.id === resourceType);
	}

	function providerName(providerId: string): string {
		const entry = providerCatalog.find((c) => c.id === providerId);
		return entry?.name ?? providerId;
	}

	function handleRemove(resourceId: string) {
		removeResource(resourceId);
	}
</script>

<div class="resource-panel">
	<header class="header">
		<div class="header-left">
			<h2 class="title">{t('tofu.resources_title')}</h2>
			{#if schemaResourceCount > 0}
				<span class="schema-badge">{t('tofu.schema_resource_count', { count: String(schemaResourceCount) })}</span>
			{/if}
		</div>
		<Button variant="primary" size="sm" onclick={() => (showPicker = true)}>
			{t('tofu.add_resource')}
		</Button>
	</header>

	{#if resources.length === 0}
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
			<p class="empty-text">{t('tofu.no_resources')}</p>
		</div>
	{:else}
		<div class="card-list">
			{#each resources as resource (resource.id)}
				<div class="card">
					<div class="card-info">
						<span class="card-name">
							{catalogEntry(resource.resourceType)?.name ?? resource.resourceType}
						</span>
						<span class="card-meta">
							{resource.logicalName} &middot; {resource.resourceType}
						</span>
						<span class="card-provider">{providerName(resource.providerId)}</span>
					</div>
					<div class="card-actions">
						<Button
							variant="secondary"
							size="sm"
							onclick={() => (configResourceId = resource.id)}
						>
							{t('tofu.configure')}
						</Button>
						<Button
							variant="danger"
							size="sm"
							onclick={() => handleRemove(resource.id)}
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
	<TofuResourcePicker onclose={() => (showPicker = false)} />
{/if}

{#if configResourceId}
	<TofuResourceConfigModal
		resourceId={configResourceId}
		onclose={() => (configResourceId = null)}
	/>
{/if}

<style>
	.resource-panel {
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

	.header-left {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.schema-badge {
		font-size: 0.6875rem;
		font-weight: 500;
		color: var(--color-accent);
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
		padding: 2px 8px;
		border-radius: 10px;
		white-space: nowrap;
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
</style>
