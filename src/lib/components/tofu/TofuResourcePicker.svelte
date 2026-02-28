<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import {
		getResourceCatalog,
		getActiveProviders,
		getActiveResources,
		addResource,
		getProviderCatalog
	} from '$lib/state/tofu.svelte';
	import Button from '$lib/components/shared/Button.svelte';

	interface Props {
		onclose: () => void;
	}

	let { onclose }: Props = $props();

	type Category = 'All' | 'Compute' | 'Storage' | 'Network' | 'Database' | 'Container' | 'Utility';

	let searchQuery = $state('');
	let selectedCategory = $state<Category>('All');
	let selectedEntry = $state<string | null>(null);
	let logicalName = $state('');
	let nameError = $state<string | null>(null);

	const categories: { key: Category; i18nKey: string }[] = [
		{ key: 'All', i18nKey: 'tofu.resource_category_all' },
		{ key: 'Compute', i18nKey: 'tofu.resource_category_compute' },
		{ key: 'Storage', i18nKey: 'tofu.resource_category_storage' },
		{ key: 'Network', i18nKey: 'tofu.resource_category_network' },
		{ key: 'Database', i18nKey: 'tofu.resource_category_database' },
		{ key: 'Container', i18nKey: 'tofu.resource_category_container' },
		{ key: 'Utility', i18nKey: 'tofu.resource_category_utility' }
	];

	let catalog = $derived(getResourceCatalog());
	let activeProviders = $derived(getActiveProviders());
	let providerCatalog = $derived(getProviderCatalog());
	let activeProviderIds = $derived(new Set(activeProviders.map((p) => p.providerId)));
	let existingResources = $derived(getActiveResources());

	let filteredCatalog = $derived.by(() => {
		let items = catalog.filter((entry) => activeProviderIds.has(entry.providerId));

		if (selectedCategory !== 'All') {
			items = items.filter((entry) => entry.category === selectedCategory);
		}

		if (searchQuery.trim()) {
			const q = searchQuery.trim().toLowerCase();
			items = items.filter(
				(entry) =>
					entry.name.toLowerCase().includes(q) ||
					entry.resourceType.toLowerCase().includes(q) ||
					entry.description.toLowerCase().includes(q)
			);
		}

		return items;
	});

	function handleCardClick(entryId: string) {
		selectedEntry = entryId;
		logicalName = '';
		nameError = null;
	}

	function validateName(): boolean {
		if (!/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(logicalName)) {
			nameError = t('tofu.resource_name_invalid');
			return false;
		}

		const entry = catalog.find((e) => e.id === selectedEntry);
		if (entry) {
			const duplicate = existingResources.some(
				(r) => r.resourceType === entry.id && r.logicalName === logicalName
			);
			if (duplicate) {
				nameError = t('tofu.resource_name_exists');
				return false;
			}
		}

		nameError = null;
		return true;
	}

	function handleAdd() {
		if (!validateName()) return;

		const entry = catalog.find((e) => e.id === selectedEntry);
		if (!entry) return;

		addResource({
			id: crypto.randomUUID(),
			resourceType: entry.id,
			logicalName,
			providerId: entry.providerId,
			fields: {}
		});

		onclose();
	}

	function providerDisplayName(providerId: string): string {
		const provider = providerCatalog.find((p) => p.id === providerId);
		return provider?.name ?? providerId;
	}

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			onclose();
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onclose();
		}
	}

	$effect(() => {
		document.addEventListener('keydown', handleKeydown);
		return () => {
			document.removeEventListener('keydown', handleKeydown);
		};
	});
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" onclick={handleBackdropClick} onkeydown={() => {}}>
	<div class="modal" role="dialog" aria-modal="true" aria-label={t('tofu.add_resource')}>
		<header class="modal-header">
			<h2 class="modal-title">{t('tofu.add_resource')}</h2>
			<button class="close-btn" onclick={onclose} aria-label={t('common.close_dialog')}>
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
					bind:value={searchQuery}
				/>

				<div class="category-pills">
					{#each categories as cat (cat.key)}
						<button
							type="button"
							class="pill"
							class:active={selectedCategory === cat.key}
							onclick={() => (selectedCategory = cat.key)}
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
							class:selected={selectedEntry === entry.id}
							onclick={() => handleCardClick(entry.id)}
						>
							<div class="resource-name">{entry.name}</div>
							<div class="resource-type">{entry.resourceType}</div>
							<div class="resource-desc">{entry.description}</div>
							<span class="provider-badge">{providerDisplayName(entry.providerId)}</span>
						</button>

						{#if selectedEntry === entry.id}
							<div class="name-form">
								<input
									class="name-input"
									type="text"
									placeholder={t('tofu.resource_name_placeholder')}
									bind:value={logicalName}
									onkeydown={(e) => {
										if (e.key === 'Enter') handleAdd();
									}}
								/>
								{#if nameError}
									<p class="name-error">{nameError}</p>
								{/if}
								<p class="name-help">{t('tofu.resource_logical_name_help')}</p>
								<div class="name-actions">
									<Button variant="primary" size="sm" onclick={handleAdd}>
										{t('tofu.add_resource')}
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

<style>
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
