<script lang="ts">
	import {
		getStateResources,
		setStateResources,
		getSelectedResource,
		setSelectedResource,
		getResourceDetail,
		setResourceDetail,
		getWorkingDir,
		getExecMode,
		getSelectedConnectionId
	} from '$lib/state/terraform.svelte';
	import { terraformStateList, terraformStateShow } from '$lib/ipc/terraform';
	import { t } from '$lib/state/i18n.svelte';
	import { addToast } from '$lib/state/toasts.svelte';

	let loadingList = $state(false);
	let loadingDetail = $state(false);

	let resources = $derived(getStateResources());
	let selectedResource = $derived(getSelectedResource());
	let resourceDetail = $derived(getResourceDetail());

	async function refreshState(): Promise<void> {
		const dir = getWorkingDir();
		if (!dir) {
			addToast(t('terraform.no_working_dir'), 'error');
			return;
		}

		loadingList = true;
		try {
			const result = await terraformStateList(
				dir,
				getExecMode(),
				getSelectedConnectionId() ?? undefined
			);
			setStateResources(result);
			setSelectedResource(null);
			setResourceDetail('');
		} catch (err) {
			addToast(t('terraform.state_error') + ': ' + String(err), 'error');
		} finally {
			loadingList = false;
		}
	}

	async function showResource(resource: string): Promise<void> {
		setSelectedResource(resource);
		loadingDetail = true;
		try {
			const detail = await terraformStateShow(
				getWorkingDir(),
				resource,
				getExecMode(),
				getSelectedConnectionId() ?? undefined
			);
			setResourceDetail(detail);
		} catch (err) {
			setResourceDetail('');
			addToast(t('terraform.state_error') + ': ' + String(err), 'error');
		} finally {
			loadingDetail = false;
		}
	}
</script>

<div class="state-view">
	<div class="toolbar">
		<button class="refresh-btn" onclick={refreshState} disabled={loadingList}>
			{#if loadingList}
				<svg class="spinner" width="14" height="14" viewBox="0 0 24 24" fill="none">
					<circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2" opacity="0.3" />
					<path d="M12 2a10 10 0 019.95 9" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
				</svg>
			{:else}
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
					<path d="M1 4v6h6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
					<path d="M3.51 15a9 9 0 105.64-11.36L1 10" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
			{/if}
			{t('terraform.refresh_state')}
		</button>
	</div>

	<div class="resource-list">
		{#if loadingList}
			<div class="empty-state">{t('terraform.loading')}</div>
		{:else if resources.length === 0}
			<div class="empty-state">{t('terraform.no_state')}</div>
		{:else}
			{#each resources as resource (resource)}
				<button
					class="resource-item"
					class:selected={selectedResource === resource}
					onclick={() => showResource(resource)}
				>
					<span class="resource-type">{resource.split('.').slice(0, -1).join('.')}</span>
					<span class="resource-name">{resource.split('.').at(-1)}</span>
				</button>
			{/each}
		{/if}
	</div>

	{#if selectedResource}
		<div class="detail-section">
			<div class="detail-header">
				<span class="detail-label">{selectedResource}</span>
			</div>
			<div class="detail-body">
				{#if loadingDetail}
					<div class="empty-state">{t('terraform.loading')}</div>
				{:else}
					<pre class="detail-pre">{resourceDetail}</pre>
				{/if}
			</div>
		</div>
	{/if}
</div>

<style>
	.state-view {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	.toolbar {
		display: flex;
		align-items: center;
		padding: 6px 8px;
		border-bottom: 1px solid var(--color-border);
	}

	.refresh-btn {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 4px 10px;
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
	}

	.refresh-btn:hover:not(:disabled) {
		color: var(--color-text-primary);
		border-color: var(--color-accent);
		background-color: rgba(255, 255, 255, 0.04);
	}

	.refresh-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.spinner {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}

	.resource-list {
		flex: 1;
		overflow-y: auto;
		padding: 6px;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.empty-state {
		padding: 16px 8px;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		text-align: center;
		opacity: 0.6;
	}

	.resource-item {
		display: flex;
		flex-direction: column;
		gap: 1px;
		padding: 6px 10px;
		background-color: transparent;
		border: 1px solid transparent;
		border-radius: var(--radius-btn);
		cursor: pointer;
		text-align: left;
		width: 100%;
		transition: all var(--duration-default) var(--ease-default);
	}

	.resource-item:hover {
		background-color: rgba(255, 255, 255, 0.04);
		border-color: var(--color-border);
	}

	.resource-item.selected {
		background-color: rgba(10, 132, 255, 0.08);
		border-color: var(--color-accent);
	}

	.resource-type {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		font-family: var(--font-mono, 'JetBrains Mono', monospace);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.resource-name {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.detail-section {
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--color-border);
		max-height: 50%;
		min-height: 80px;
	}

	.detail-header {
		display: flex;
		align-items: center;
		padding: 6px 10px;
		border-bottom: 1px solid var(--color-border);
		background-color: var(--color-bg-secondary);
	}

	.detail-label {
		font-size: 0.6875rem;
		font-weight: 600;
		color: var(--color-accent);
		font-family: var(--font-mono, 'JetBrains Mono', monospace);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.detail-body {
		flex: 1;
		overflow: auto;
		background-color: var(--color-bg-primary);
	}

	.detail-pre {
		margin: 0;
		padding: 8px 10px;
		font-family: var(--font-mono, 'JetBrains Mono', monospace);
		font-size: 0.75rem;
		line-height: 1.5;
		color: var(--color-text-primary);
		white-space: pre-wrap;
		word-break: break-all;
	}
</style>
