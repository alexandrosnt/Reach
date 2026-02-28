<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import { getProviderCatalog, addProvider, getActiveProviders } from '$lib/state/tofu.svelte';
	import Button from '$lib/components/shared/Button.svelte';

	interface Props {
		onclose: () => void;
	}

	let { onclose }: Props = $props();

	type Category = 'All' | 'Cloud' | 'Container' | 'CDN & DNS' | 'VCS' | 'Utility';

	let searchQuery = $state('');
	let selectedCategory = $state<Category>('All');

	const categories: { key: Category; i18nKey: string }[] = [
		{ key: 'All', i18nKey: 'tofu.provider_category_all' },
		{ key: 'Cloud', i18nKey: 'tofu.provider_category_cloud' },
		{ key: 'Container', i18nKey: 'tofu.provider_category_container' },
		{ key: 'CDN & DNS', i18nKey: 'tofu.provider_category_cdn_dns' },
		{ key: 'VCS', i18nKey: 'tofu.provider_category_vcs' },
		{ key: 'Utility', i18nKey: 'tofu.provider_category_utility' }
	];

	let catalog = $derived(getProviderCatalog());
	let activeProviders = $derived(getActiveProviders());
	let activeProviderIds = $derived(new Set(activeProviders.map((p) => p.providerId)));

	let filteredCatalog = $derived.by(() => {
		let items = catalog;

		if (selectedCategory !== 'All') {
			items = items.filter((entry) => entry.category === selectedCategory);
		}

		if (searchQuery.trim()) {
			const q = searchQuery.trim().toLowerCase();
			items = items.filter(
				(entry) =>
					entry.name.toLowerCase().includes(q) ||
					entry.source.toLowerCase().includes(q) ||
					entry.description.toLowerCase().includes(q)
			);
		}

		return items;
	});

	function handleSelect(entry: (typeof catalog)[number]) {
		if (activeProviderIds.has(entry.id)) return;
		addProvider({ providerId: entry.id, source: entry.source, version: '', fields: {} });
		onclose();
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
	<div class="modal" role="dialog" aria-modal="true" aria-label={t('tofu.add_provider')}>
		<header class="modal-header">
			<h2 class="modal-title">{t('tofu.add_provider')}</h2>
			<button class="close-btn" onclick={onclose} aria-label={t('common.close_dialog')}>
				<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
					<path d="M1 1L13 13M13 1L1 13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
				</svg>
			</button>
		</header>

		<div class="modal-body">
			<input
				class="search-input"
				type="text"
				placeholder={t('tofu.provider_search')}
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

			<div class="provider-grid">
				{#each filteredCatalog as entry (entry.id)}
					{@const isAdded = activeProviderIds.has(entry.id)}
					<button
						type="button"
						class="provider-card"
						class:disabled={isAdded}
						disabled={isAdded}
						onclick={() => handleSelect(entry)}
					>
						<div class="provider-name">{entry.name}</div>
						<div class="provider-source">{entry.source}</div>
						<div class="provider-desc">{entry.description}</div>
					</button>
				{/each}
			</div>
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

	.provider-grid {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.provider-card {
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

	.provider-card:hover:not(:disabled) {
		border-color: var(--color-accent);
		background-color: rgba(255, 255, 255, 0.03);
	}

	.provider-card.disabled {
		opacity: 0.45;
		cursor: not-allowed;
	}

	.provider-name {
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.provider-source {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		font-family: monospace;
	}

	.provider-desc {
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		line-height: 1.4;
		margin-top: 2px;
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
