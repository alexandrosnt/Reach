<script lang="ts">
	import { onMount } from 'svelte';
	import type { ProjectTemplate } from '$lib/ipc/tofu';
	import { getTemplateCatalog, loadTemplates } from '$lib/state/tofu.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';

	interface Props {
		open: boolean;
		onclose: () => void;
		onapply: (templateId: string) => void;
	}

	let { open, onclose, onapply }: Props = $props();

	let templates = $derived(getTemplateCatalog());
	let activeCategory = $state<string>('All');
	let searchQuery = $state('');
	let categories = $derived(['All', ...new Set(templates.map((tpl) => tpl.category))]);

	let filteredTemplates = $derived.by(() => {
		let items = templates;

		if (activeCategory !== 'All') {
			items = items.filter((tpl) => tpl.category === activeCategory);
		}

		if (searchQuery.trim()) {
			const q = searchQuery.trim().toLowerCase();
			items = items.filter(
				(tpl) =>
					tpl.name.toLowerCase().includes(q) ||
					tpl.description.toLowerCase().includes(q)
			);
		}

		return items;
	});

	onMount(() => {
		loadTemplates();
	});

	function handleSelect(template: ProjectTemplate) {
		onapply(template.id);
		onclose();
	}

	function categoryBadgeClass(category: string): string {
		switch (category) {
			case 'AWS':
				return 'badge-aws';
			case 'Docker':
				return 'badge-docker';
			case 'Kubernetes':
				return 'badge-kubernetes';
			default:
				return 'badge-default';
		}
	}
</script>

<Modal {open} {onclose} title={t('tofu.templates')} maxWidth="700px">
	<div class="picker-body">
		<input
			class="search-input"
			type="text"
			placeholder={t('tofu.template_search')}
			bind:value={searchQuery}
		/>

		<div class="category-pills">
			{#each categories as cat (cat)}
				<button
					type="button"
					class="pill"
					class:active={activeCategory === cat}
					onclick={() => (activeCategory = cat)}
				>
					{cat}
				</button>
			{/each}
		</div>

		<div class="template-grid">
			{#each filteredTemplates as tpl (tpl.id)}
				<button
					type="button"
					class="template-card"
					onclick={() => handleSelect(tpl)}
				>
					<div class="card-header">
						<span class="template-name">{tpl.name}</span>
						<span class="badge {categoryBadgeClass(tpl.category)}">{tpl.category}</span>
					</div>
					<div class="template-desc">{tpl.description}</div>
				</button>
			{/each}
		</div>
	</div>

	{#snippet actions()}
		<Button variant="secondary" onclick={onclose}>{t('common.close')}</Button>
	{/snippet}
</Modal>

<style>
	.picker-body {
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

	.template-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 10px;
	}

	.template-card {
		display: flex;
		flex-direction: column;
		gap: 8px;
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

	.template-card:hover {
		border-color: var(--color-accent);
		background-color: rgba(255, 255, 255, 0.03);
	}

	.card-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
	}

	.template-name {
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.badge {
		font-size: 0.6875rem;
		font-weight: 600;
		padding: 2px 8px;
		border-radius: 9999px;
		white-space: nowrap;
		flex-shrink: 0;
	}

	.badge-aws {
		background-color: rgba(251, 146, 60, 0.15);
		color: #fb923c;
	}

	.badge-docker {
		background-color: rgba(34, 211, 238, 0.15);
		color: #22d3ee;
	}

	.badge-kubernetes {
		background-color: rgba(168, 85, 247, 0.15);
		color: #a855f7;
	}

	.badge-default {
		background-color: rgba(156, 163, 175, 0.15);
		color: #9ca3af;
	}

	.template-desc {
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		line-height: 1.4;
	}
</style>
