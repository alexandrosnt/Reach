<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import {
		getWorkspaceInfo,
		isWorkspacesLoading,
		loadWorkspaces,
		createWorkspace,
		selectWorkspace,
		deleteWorkspace
	} from '$lib/state/tofu.svelte';
	import type { TofuExecutionTarget } from '$lib/ipc/tofu';
	import Button from '$lib/components/shared/Button.svelte';

	let { target }: { target: TofuExecutionTarget } = $props();

	let newWorkspaceName = $state('');

	let info = $derived(getWorkspaceInfo());
	let loading = $derived(isWorkspacesLoading());

	onMount(() => {
		loadWorkspaces(target);
	});

	function handleRefresh() {
		loadWorkspaces(target);
	}

	function handleSelect(name: string) {
		selectWorkspace(target, name);
	}

	function handleDelete(name: string) {
		if (name === 'default') return;
		if (!confirm(t('tofu.workspace_delete_confirm'))) return;
		deleteWorkspace(target, name);
	}

	function handleCreate() {
		const name = newWorkspaceName.trim();
		if (!name) return;
		createWorkspace(target, name);
		newWorkspaceName = '';
	}
</script>

<div class="workspace-panel">
	<header class="header">
		<h2 class="title">{t('tofu.workspaces_title')}</h2>
		<div class="header-actions">
			<Button variant="secondary" size="sm" onclick={handleRefresh} disabled={loading}>
				{t('tofu.state_refresh')}
			</Button>
		</div>
	</header>

	{#if loading}
		<div class="loading-state">
			<div class="spinner"></div>
			<p class="loading-text">{t('tofu.workspace_loading')}</p>
		</div>
	{:else if !info}
		<div class="empty-state">
			<svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="empty-icon">
				<path
					d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
			<p class="empty-text">{t('tofu.workspace_empty')}</p>
		</div>
	{:else}
		<!-- Current workspace highlight -->
		<div class="current-workspace">
			<span class="current-label">{t('tofu.workspace_current')}</span>
			<span class="current-name">{info.current}</span>
		</div>

		<!-- Workspace list -->
		{#if info.workspaces.length === 0}
			<div class="empty-state">
				<p class="empty-text">{t('tofu.workspace_empty')}</p>
			</div>
		{:else}
			<div class="workspace-list">
				{#each info.workspaces as ws (ws)}
					<div class="workspace-item" class:active={ws === info.current}>
						<span class="workspace-name">
							{ws}
							{#if ws === info.current}
								<span class="badge current">{t('tofu.workspace_current')}</span>
							{/if}
						</span>
						<div class="workspace-actions">
							<Button
								variant="secondary"
								size="sm"
								onclick={() => handleSelect(ws)}
								disabled={ws === info.current}
							>
								{t('tofu.workspace_select')}
							</Button>
							<Button
								variant="danger"
								size="sm"
								onclick={() => handleDelete(ws)}
								disabled={ws === 'default'}
							>
								{t('tofu.workspace_delete')}
							</Button>
						</div>
					</div>
				{/each}
			</div>
		{/if}

		<!-- Create new workspace -->
		<div class="create-section">
			<h3 class="create-title">{t('tofu.workspace_new')}</h3>
			<div class="create-row">
				<input
					type="text"
					class="create-input"
					placeholder={t('tofu.workspace_name_placeholder')}
					bind:value={newWorkspaceName}
					onkeydown={(e) => { if (e.key === 'Enter') handleCreate(); }}
				/>
				<Button
					variant="primary"
					size="sm"
					onclick={handleCreate}
					disabled={!newWorkspaceName.trim()}
				>
					{t('tofu.workspace_create')}
				</Button>
			</div>
		</div>
	{/if}
</div>

<style>
	.workspace-panel {
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
		gap: 12px;
		flex-wrap: wrap;
	}

	.title {
		margin: 0;
		font-size: 1.25rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.header-actions {
		display: flex;
		gap: 8px;
	}

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
		to { transform: rotate(360deg); }
	}

	.loading-text {
		margin: 0;
		font-size: 0.875rem;
		color: var(--color-text-secondary);
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

	.current-workspace {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px 16px;
		background: color-mix(in srgb, var(--color-accent) 8%, var(--color-bg-elevated));
		border: 1px solid color-mix(in srgb, var(--color-accent) 30%, var(--color-border));
		border-radius: var(--radius-btn);
		margin-bottom: 20px;
	}

	.current-label {
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.current-name {
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--color-accent);
		font-family: monospace;
	}

	.workspace-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin-bottom: 24px;
	}

	.workspace-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 10px 16px;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		transition: border-color 0.12s ease;
	}

	.workspace-item:hover {
		border-color: color-mix(in srgb, var(--color-border) 70%, var(--color-accent));
	}

	.workspace-item.active {
		border-color: color-mix(in srgb, var(--color-accent) 40%, var(--color-border));
		background: color-mix(in srgb, var(--color-accent) 4%, var(--color-bg-elevated));
	}

	.workspace-name {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
		font-family: monospace;
		display: flex;
		align-items: center;
		gap: 8px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}

	.badge.current {
		display: inline-flex;
		align-items: center;
		padding: 1px 6px;
		font-size: 0.625rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		background: color-mix(in srgb, var(--color-accent) 15%, transparent);
		color: var(--color-accent);
		border-radius: 4px;
		white-space: nowrap;
		font-family: var(--font-sans);
	}

	.workspace-actions {
		display: flex;
		gap: 6px;
		flex-shrink: 0;
	}

	.create-section {
		border-top: 1px solid var(--color-border);
		padding-top: 20px;
	}

	.create-title {
		margin: 0 0 12px 0;
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.create-row {
		display: flex;
		gap: 8px;
		align-items: center;
	}

	.create-input {
		flex: 1;
		padding: 6px 10px;
		font-size: 0.8125rem;
		background: var(--color-bg-elevated);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		font-family: monospace;
	}

	.create-input:focus {
		border-color: var(--color-accent);
	}

	.create-input::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.6;
	}
</style>
