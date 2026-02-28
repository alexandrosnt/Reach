<script lang="ts">
	import { SvelteSet } from 'svelte/reactivity';
	import { t } from '$lib/state/i18n.svelte';
	import { getPlanSummary, isPlanSummaryLoading } from '$lib/state/tofu.svelte';
	import type {
		TofuPlanSummary,
		TofuResourceChange,
		TofuChangeAction
	} from '$lib/ipc/tofu';

	let summary = $derived(getPlanSummary());
	let loading = $derived(isPlanSummaryLoading());

	let expandedResources = new SvelteSet<string>();

	let createCount = $derived(
		summary?.resourceChanges.filter((r) => r.action === 'create').length ?? 0
	);
	let updateCount = $derived(
		summary?.resourceChanges.filter((r) => r.action === 'update').length ?? 0
	);
	let deleteCount = $derived(
		summary?.resourceChanges.filter((r) => r.action === 'delete').length ?? 0
	);
	let replaceCount = $derived(
		summary?.resourceChanges.filter((r) => r.action === 'replace').length ?? 0
	);

	function toggleResource(address: string) {
		if (expandedResources.has(address)) {
			expandedResources.delete(address);
		} else {
			expandedResources.add(address);
		}
	}

	function actionColorClass(action: TofuChangeAction): string {
		switch (action) {
			case 'create':
				return 'action-create';
			case 'update':
				return 'action-update';
			case 'delete':
				return 'action-delete';
			case 'replace':
				return 'action-replace';
			case 'read':
			case 'noOp':
			default:
				return 'action-noop';
		}
	}

	function actionLabel(action: TofuChangeAction): string {
		switch (action) {
			case 'create':
				return t('tofu.plan_create');
			case 'update':
				return t('tofu.plan_update');
			case 'delete':
				return t('tofu.plan_delete');
			case 'replace':
				return t('tofu.plan_replace');
			default:
				return action;
		}
	}

	function formatValue(value: unknown): string {
		if (value === null || value === undefined) return '';
		if (typeof value === 'string') return value;
		return JSON.stringify(value);
	}
</script>

<div class="plan-viewer">
	<header class="header">
		<h2 class="title">{t('tofu.plan_summary')}</h2>
	</header>

	{#if loading}
		<div class="loading-state">
			<div class="spinner"></div>
			<p class="loading-text">{t('tofu.plan_loading')}</p>
		</div>
	{:else if !summary}
		<div class="empty-state">
			<svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="empty-icon">
				<path
					d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
				<rect
					x="9"
					y="3"
					width="6"
					height="4"
					rx="1"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
			<p class="empty-text">{t('tofu.plan_no_changes')}</p>
		</div>
	{:else if !summary.hasChanges}
		<div class="empty-state">
			<svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="empty-icon">
				<path
					d="M22 11.08V12a10 10 0 11-5.93-9.14"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
				<polyline
					points="22 4 12 14.01 9 11.01"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
			<p class="empty-text">{t('tofu.plan_no_changes')}</p>
		</div>
	{:else}
		<!-- Summary bar -->
		<div class="summary-bar">
			{#if createCount > 0}
				<span class="count-badge badge-create">
					{t('tofu.plan_to_create', { count: createCount })}
				</span>
			{/if}
			{#if updateCount > 0}
				<span class="count-badge badge-update">
					{t('tofu.plan_to_update', { count: updateCount })}
				</span>
			{/if}
			{#if deleteCount > 0}
				<span class="count-badge badge-delete">
					{t('tofu.plan_to_destroy', { count: deleteCount })}
				</span>
			{/if}
			{#if replaceCount > 0}
				<span class="count-badge badge-replace">
					{t('tofu.plan_to_replace', { count: replaceCount })}
				</span>
			{/if}
		</div>

		<!-- Resource change cards -->
		<div class="resource-list">
			{#each summary.resourceChanges as change (change.address)}
				{@const colorClass = actionColorClass(change.action)}
				{@const isExpanded = expandedResources.has(change.address)}
				<div class="resource-card {colorClass}">
					<button
						type="button"
						class="resource-header"
						onclick={() => toggleResource(change.address)}
					>
						<div class="resource-header-left">
							<span class="action-badge {colorClass}">
								{actionLabel(change.action)}
							</span>
							<span class="resource-address">{change.address}</span>
						</div>
						<div class="resource-header-right">
							<span class="provider-name">{change.provider}</span>
							<svg
								width="16"
								height="16"
								viewBox="0 0 24 24"
								fill="none"
								class="chevron"
								class:chevron-open={isExpanded}
							>
								<path
									d="M6 9l6 6 6-6"
									stroke="currentColor"
									stroke-width="1.5"
									stroke-linecap="round"
									stroke-linejoin="round"
								/>
							</svg>
						</div>
					</button>

					{#if isExpanded && change.attributeChanges.length > 0}
						<div class="attribute-section">
							<div class="attribute-header-row">
								<span class="attr-col-name">{t('tofu.plan_attributes')}</span>
								<span class="attr-col-old">{t('tofu.plan_old_value')}</span>
								<span class="attr-col-new">{t('tofu.plan_new_value')}</span>
							</div>
							{#each change.attributeChanges as attr (attr.attribute)}
								<div class="attribute-row">
									<span class="attr-name">{attr.attribute}</span>
									<span class="attr-old">
										{#if attr.sensitive}
											<span class="sensitive-value">{t('tofu.plan_sensitive_value')}</span>
										{:else if attr.oldValue !== null && attr.oldValue !== undefined}
											<span class="value-removed">{formatValue(attr.oldValue)}</span>
										{:else}
											<span class="value-empty">-</span>
										{/if}
									</span>
									<span class="attr-new">
										{#if attr.sensitive}
											<span class="sensitive-value">{t('tofu.plan_sensitive_value')}</span>
										{:else if attr.newValue !== null && attr.newValue !== undefined}
											<span class="value-added">{formatValue(attr.newValue)}</span>
										{:else}
											<span class="value-known-after">{t('tofu.plan_known_after_apply')}</span>
										{/if}
									</span>
								</div>
							{/each}
						</div>
					{:else if isExpanded}
						<div class="attribute-section">
							<p class="no-attributes">{t('tofu.plan_attributes')}: --</p>
						</div>
					{/if}
				</div>
			{/each}
		</div>

		<!-- Output changes -->
		{#if summary.outputChanges.length > 0}
			<div class="output-changes">
				<h3 class="section-title">Output Changes</h3>
				<div class="output-change-list">
					{#each summary.outputChanges as outputChange (outputChange.name)}
						<div class="output-change-item">
							<span class="output-change-name">{outputChange.name}</span>
							<span class="action-badge {actionColorClass(outputChange.action)}">
								{actionLabel(outputChange.action)}
							</span>
						</div>
					{/each}
				</div>
			</div>
		{/if}
	{/if}
</div>

<style>
	.plan-viewer {
		width: 100%;
		height: 100%;
		overflow-y: auto;
		background: var(--color-bg-primary);
		padding: 24px;
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
	}

	.title {
		margin: 0;
		font-size: 1.25rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	/* Loading */
	.loading-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 16px;
		padding: 64px 32px;
	}

	.spinner {
		width: 24px;
		height: 24px;
		border: 2px solid var(--color-border);
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.loading-text {
		margin: 0;
		font-size: 0.875rem;
		color: var(--color-text-secondary);
	}

	/* Empty */
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

	/* Summary bar */
	.summary-bar {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}

	.count-badge {
		display: inline-flex;
		align-items: center;
		padding: 4px 12px;
		font-size: 0.75rem;
		font-weight: 600;
		border-radius: 9999px;
		white-space: nowrap;
	}

	.badge-create {
		background: rgba(48, 209, 88, 0.12);
		color: var(--color-success);
	}

	.badge-update {
		background: rgba(255, 214, 10, 0.12);
		color: var(--color-warning);
	}

	.badge-delete {
		background: rgba(255, 69, 58, 0.12);
		color: var(--color-danger);
	}

	.badge-replace {
		background: rgba(10, 132, 255, 0.12);
		color: var(--color-accent);
	}

	/* Resource cards */
	.resource-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.resource-card {
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		overflow: hidden;
		transition: border-color 0.12s ease;
		border-left: 3px solid transparent;
	}

	.resource-card.action-create {
		border-left-color: var(--color-success);
	}

	.resource-card.action-update {
		border-left-color: var(--color-warning);
	}

	.resource-card.action-delete {
		border-left-color: var(--color-danger);
	}

	.resource-card.action-replace {
		border-left-color: var(--color-accent);
	}

	.resource-card.action-noop {
		border-left-color: var(--color-text-secondary);
	}

	.resource-card:hover {
		border-color: color-mix(in srgb, var(--color-border) 70%, var(--color-accent));
	}

	/* Resource header */
	.resource-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		width: 100%;
		padding: 10px 16px;
		background: transparent;
		border: none;
		cursor: pointer;
		color: var(--color-text-primary);
		font-family: inherit;
		text-align: left;
	}

	.resource-header:hover {
		background: rgba(255, 255, 255, 0.02);
	}

	.resource-header-left {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
		overflow: hidden;
	}

	.resource-header-right {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-shrink: 0;
	}

	.action-badge {
		display: inline-flex;
		align-items: center;
		padding: 2px 8px;
		font-size: 0.625rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		border-radius: 4px;
		white-space: nowrap;
		flex-shrink: 0;
	}

	.action-badge.action-create {
		background: rgba(48, 209, 88, 0.12);
		color: var(--color-success);
	}

	.action-badge.action-update {
		background: rgba(255, 214, 10, 0.12);
		color: var(--color-warning);
	}

	.action-badge.action-delete {
		background: rgba(255, 69, 58, 0.12);
		color: var(--color-danger);
	}

	.action-badge.action-replace {
		background: rgba(10, 132, 255, 0.12);
		color: var(--color-accent);
	}

	.action-badge.action-noop {
		background: rgba(255, 255, 255, 0.04);
		color: var(--color-text-secondary);
	}

	.resource-address {
		font-size: 0.8125rem;
		font-weight: 500;
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.provider-name {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		white-space: nowrap;
	}

	.chevron {
		color: var(--color-text-secondary);
		transition: transform 0.15s ease;
		flex-shrink: 0;
	}

	.chevron-open {
		transform: rotate(180deg);
	}

	/* Attribute section */
	.attribute-section {
		border-top: 1px solid var(--color-border);
		padding: 12px 16px;
	}

	.attribute-header-row {
		display: grid;
		grid-template-columns: 1fr 1fr 1fr;
		gap: 8px;
		padding-bottom: 8px;
		margin-bottom: 8px;
		border-bottom: 1px solid var(--color-border);
	}

	.attr-col-name,
	.attr-col-old,
	.attr-col-new {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--color-text-secondary);
	}

	.attribute-row {
		display: grid;
		grid-template-columns: 1fr 1fr 1fr;
		gap: 8px;
		padding: 4px 0;
	}

	.attribute-row:not(:last-child) {
		border-bottom: 1px solid rgba(255, 255, 255, 0.03);
	}

	.attr-name {
		font-size: 0.75rem;
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.attr-old,
	.attr-new {
		font-size: 0.75rem;
		font-family: var(--font-mono);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.value-removed {
		color: var(--color-danger);
	}

	.value-added {
		color: var(--color-success);
	}

	.value-empty {
		color: var(--color-text-secondary);
		opacity: 0.4;
	}

	.value-known-after {
		font-size: 0.6875rem;
		font-style: italic;
		color: var(--color-text-secondary);
		font-family: var(--font-sans);
	}

	.sensitive-value {
		font-size: 0.6875rem;
		font-style: italic;
		color: var(--color-danger);
		opacity: 0.7;
		font-family: var(--font-sans);
	}

	.no-attributes {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		font-style: italic;
	}

	/* Output changes */
	.output-changes {
		margin-top: 8px;
	}

	.section-title {
		margin: 0 0 12px;
		font-size: 1rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.output-change-list {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.output-change-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 8px 16px;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
	}

	.output-change-name {
		font-size: 0.8125rem;
		font-weight: 500;
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	/* Responsive */
	@media (max-width: 700px) {
		.plan-viewer {
			padding: 16px;
			gap: 16px;
		}

		.attribute-header-row,
		.attribute-row {
			grid-template-columns: 1fr;
			gap: 4px;
		}

		.attr-col-old,
		.attr-col-new {
			display: none;
		}

		.attr-old,
		.attr-new {
			white-space: normal;
			word-break: break-all;
		}

		.attr-old::before {
			content: 'Old: ';
			font-size: 0.625rem;
			color: var(--color-text-secondary);
			font-family: var(--font-sans);
			text-transform: uppercase;
			letter-spacing: 0.04em;
		}

		.attr-new::before {
			content: 'New: ';
			font-size: 0.625rem;
			color: var(--color-text-secondary);
			font-family: var(--font-sans);
			text-transform: uppercase;
			letter-spacing: 0.04em;
		}

		.resource-header {
			padding: 8px 12px;
			flex-wrap: wrap;
		}

		.resource-header-left {
			flex-wrap: wrap;
			gap: 6px;
		}

		.resource-address {
			font-size: 0.75rem;
		}

		.attribute-section {
			padding: 8px 12px;
		}

		.output-change-item {
			padding: 8px 12px;
			flex-wrap: wrap;
			gap: 6px;
		}
	}
</style>
