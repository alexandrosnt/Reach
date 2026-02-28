<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import {
		getActiveProject,
		getProjectFiles,
		isCommandRunning,
		refreshFiles,
		closeProject,
		getWorkspaceTab,
		setWorkspaceTab
	} from '$lib/state/ansible.svelte';
	import { ansibleReadFile } from '$lib/ipc/ansible';
	import type { AnsibleExecutionTarget } from '$lib/ipc/ansible';
	import { sshListConnections, type ConnectionInfo } from '$lib/ipc/ssh';
	import Button from '$lib/components/shared/Button.svelte';
	import AnsibleCommandOutput from './AnsibleCommandOutput.svelte';
	import AnsiblePlaybookPanel from './AnsiblePlaybookPanel.svelte';
	import AnsibleInventoryPanel from './AnsibleInventoryPanel.svelte';
	import AnsibleRolesPanel from './AnsibleRolesPanel.svelte';
	import AnsibleCollectionsPanel from './AnsibleCollectionsPanel.svelte';
	import AnsibleAdHocPanel from './AnsibleAdHocPanel.svelte';
	import AnsibleVaultPanel from './AnsibleVaultPanel.svelte';

	let project = $derived(getActiveProject());
	let files = $derived(getProjectFiles());
	let running = $derived(isCommandRunning());

	let connections = $state<ConnectionInfo[]>([]);
	let targetType = $state<'local' | 'ssh'>('local');
	let selectedConnectionId = $state<string | null>(null);

	let activeTab = $derived(getWorkspaceTab());

	let selectedFile = $state<string | null>(null);
	let fileContent = $state<string | null>(null);
	let fileLoading = $state(false);
	let leftPanelCollapsed = $state(false);

	onMount(() => {
		refreshFiles();
		loadConnections();
	});

	async function loadConnections() {
		try {
			connections = await sshListConnections();
		} catch {
			connections = [];
		}
	}

	function buildTarget(): AnsibleExecutionTarget {
		if (targetType === 'ssh' && selectedConnectionId) {
			return { type: 'ssh', connectionId: selectedConnectionId };
		}
		return { type: 'local' };
	}

	async function handleFileClick(filename: string) {
		if (!project) return;
		selectedFile = filename;
		fileLoading = true;
		fileContent = null;
		try {
			fileContent = await ansibleReadFile(project.id, filename);
		} catch {
			fileContent = '-- Error reading file --';
		} finally {
			fileLoading = false;
		}
	}

	function closeFileViewer() {
		selectedFile = null;
		fileContent = null;
	}
</script>

<div class="workspace">
	<!-- Left Panel Toggle (visible when collapsed) -->
	{#if leftPanelCollapsed}
		<button type="button" class="panel-expand-btn" onclick={() => leftPanelCollapsed = false} title="Show files">
			<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
				<path d="M9 18l6-6-6-6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
		</button>
	{/if}

	<!-- Left Panel -->
	<aside class="left-panel" class:collapsed={leftPanelCollapsed}>
		<div class="panel-header">
			<h2 class="project-name">{project?.name ?? ''}</h2>
			<button type="button" class="panel-collapse-btn" onclick={() => leftPanelCollapsed = true} title="Hide files">
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
					<path d="M15 18l-6-6 6-6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</button>
		</div>

		<div class="file-section">
			<h3 class="section-label">{t('ansible.files')}</h3>

			{#if files.length === 0}
				<p class="no-files">{t('ansible.no_files')}</p>
			{:else}
				<ul class="file-list">
					{#each files as filename (filename)}
						<li>
							<button
								type="button"
								class="file-item"
								class:active={selectedFile === filename}
								onclick={() => handleFileClick(filename)}
							>
								<svg width="14" height="14" viewBox="0 0 24 24" fill="none" class="file-icon">
									<path
										d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8l-6-6z"
										stroke="currentColor"
										stroke-width="1.5"
										stroke-linecap="round"
										stroke-linejoin="round"
									/>
									<path
										d="M14 2v6h6"
										stroke="currentColor"
										stroke-width="1.5"
										stroke-linecap="round"
										stroke-linejoin="round"
									/>
								</svg>
								<span class="file-name">{filename}</span>
							</button>
						</li>
					{/each}
				</ul>
			{/if}
		</div>

		<div class="panel-footer">
			<button type="button" class="back-link" onclick={closeProject}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
					<path
						d="M19 12H5M12 19l-7-7 7-7"
						stroke="currentColor"
						stroke-width="1.5"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
				{t('ansible.back_to_projects')}
			</button>
		</div>
	</aside>

	<!-- Right Panel -->
	<main class="right-panel">
		<!-- Workspace Tabs -->
		<div class="tab-bar">
			<button class="tab" class:active={activeTab === 'playbooks'} onclick={() => setWorkspaceTab('playbooks')}>{t('ansible.tab_playbooks')}</button>
			<button class="tab" class:active={activeTab === 'inventory'} onclick={() => setWorkspaceTab('inventory')}>{t('ansible.tab_inventory')}</button>
			<button class="tab" class:active={activeTab === 'roles'} onclick={() => setWorkspaceTab('roles')}>{t('ansible.tab_roles')}</button>
			<button class="tab" class:active={activeTab === 'collections'} onclick={() => setWorkspaceTab('collections')}>{t('ansible.tab_collections')}</button>
			<button class="tab" class:active={activeTab === 'adhoc'} onclick={() => setWorkspaceTab('adhoc')}>{t('ansible.tab_adhoc')}</button>
			<button class="tab" class:active={activeTab === 'vault'} onclick={() => setWorkspaceTab('vault')}>{t('ansible.tab_vault')}</button>
			<div class="tab-spacer"></div>
		</div>

		<!-- Target Selector -->
		<div class="target-bar">
			<div class="target-row">
				<span class="target-label">{t('ansible.execution_target')}</span>
				<div class="target-controls">
					<select
						class="target-select"
						bind:value={targetType}
						onchange={() => { if (targetType === 'local') selectedConnectionId = null; }}
					>
						<option value="local">{t('ansible.target_local')}</option>
						<option value="ssh">{t('ansible.target_ssh')}</option>
					</select>

					{#if targetType === 'ssh'}
						<select class="target-select" bind:value={selectedConnectionId}>
							<option value={null}>{t('ansible.select_connection')}</option>
							{#each connections as conn (conn.id)}
								<option value={conn.id}>{conn.username}@{conn.host}:{conn.port}</option>
							{/each}
						</select>
					{/if}
				</div>
			</div>
		</div>

		<!-- Tab Content -->
		{#if activeTab === 'playbooks'}
			<div class="tab-content-split">
				<div class="tab-panel"><AnsiblePlaybookPanel target={buildTarget()} /></div>
				<div class="output-area">
					{#if selectedFile !== null}
						<div class="file-viewer">
							<div class="file-viewer-header">
								<span class="file-viewer-title">{selectedFile}</span>
								<button type="button" class="close-viewer-btn" title="Close" onclick={closeFileViewer}>
									<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
										<path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
									</svg>
								</button>
							</div>
							<div class="file-viewer-content">
								{#if fileLoading}
									<span class="loading-text">Loading...</span>
								{:else}
									<pre>{fileContent ?? ''}</pre>
								{/if}
							</div>
						</div>
					{:else}
						<AnsibleCommandOutput />
					{/if}
				</div>
			</div>
		{:else if activeTab === 'inventory'}
			<div class="tab-content"><AnsibleInventoryPanel /></div>
		{:else if activeTab === 'roles'}
			<div class="tab-content"><AnsibleRolesPanel target={buildTarget()} /></div>
		{:else if activeTab === 'collections'}
			<div class="tab-content"><AnsibleCollectionsPanel target={buildTarget()} /></div>
		{:else if activeTab === 'adhoc'}
			<div class="tab-content-split">
				<div class="tab-panel"><AnsibleAdHocPanel target={buildTarget()} /></div>
				<div class="output-area"><AnsibleCommandOutput /></div>
			</div>
		{:else if activeTab === 'vault'}
			<div class="tab-content-split">
				<div class="tab-panel"><AnsibleVaultPanel target={buildTarget()} /></div>
				<div class="output-area"><AnsibleCommandOutput /></div>
			</div>
		{/if}
	</main>
</div>

<style>
	.workspace {
		display: flex;
		width: 100%;
		height: 100%;
		background: var(--color-bg-primary);
	}

	/* Left Panel */
	.left-panel {
		width: 240px;
		min-width: 240px;
		display: flex;
		flex-direction: column;
		border-right: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		transition: width 0.2s ease, min-width 0.2s ease, opacity 0.2s ease;
	}

	.left-panel.collapsed {
		width: 0;
		min-width: 0;
		overflow: hidden;
		opacity: 0;
		border-right: none;
	}

	.panel-expand-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		min-width: 28px;
		background: var(--color-bg-elevated);
		border: none;
		border-right: 1px solid var(--color-border);
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: color 0.12s ease, background-color 0.12s ease;
	}

	.panel-expand-btn:hover {
		color: var(--color-accent);
		background: rgba(255, 255, 255, 0.04);
	}

	.panel-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		padding: 16px;
		border-bottom: 1px solid var(--color-border);
	}

	.panel-collapse-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		background: transparent;
		border: none;
		border-radius: var(--radius-btn);
		color: var(--color-text-secondary);
		cursor: pointer;
		flex-shrink: 0;
		transition: color 0.12s ease, background-color 0.12s ease;
	}

	.panel-collapse-btn:hover {
		color: var(--color-accent);
		background: rgba(255, 255, 255, 0.06);
	}

	.project-name {
		margin: 0;
		font-size: 0.9375rem;
		font-weight: 600;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}

	.file-section {
		flex: 1;
		overflow-y: auto;
		padding: 12px 0;
	}

	.section-label {
		margin: 0 0 8px 0;
		padding: 0 16px;
		font-size: 0.6875rem;
		font-weight: 600;
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.no-files {
		margin: 0;
		padding: 0 16px;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		font-style: italic;
		opacity: 0.6;
	}

	.file-list {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.file-item {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 6px 16px;
		background: transparent;
		border: none;
		color: var(--color-text-secondary);
		font-family: monospace;
		font-size: 0.75rem;
		cursor: pointer;
		text-align: left;
		transition: background-color 0.12s ease, color 0.12s ease;
	}

	.file-item:hover {
		background: rgba(255, 255, 255, 0.04);
		color: var(--color-text-primary);
	}

	.file-item.active {
		background: rgba(255, 255, 255, 0.08);
		color: var(--color-accent);
	}

	.file-icon {
		flex-shrink: 0;
		opacity: 0.6;
	}

	.file-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.panel-footer {
		padding: 12px 16px;
		border-top: 1px solid var(--color-border);
	}

	.back-link {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		background: transparent;
		border: none;
		color: var(--color-text-secondary);
		font-size: 0.8125rem;
		cursor: pointer;
		padding: 0;
		transition: color 0.12s ease;
	}

	.back-link:hover {
		color: var(--color-accent);
	}

	/* Right Panel */
	.right-panel {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		min-width: 0;
	}

	.tab-bar {
		display: flex;
		align-items: center;
		gap: 0;
		padding: 0 16px;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		flex-shrink: 0;
		overflow-x: auto;
	}

	.tab {
		padding: 10px 14px;
		background: transparent;
		border: none;
		border-bottom: 2px solid transparent;
		color: var(--color-text-secondary);
		font-size: 0.8125rem;
		font-weight: 500;
		cursor: pointer;
		white-space: nowrap;
		transition: color 0.12s ease, border-color 0.12s ease;
	}

	.tab:hover {
		color: var(--color-text-primary);
	}

	.tab.active {
		color: var(--color-accent);
		border-bottom-color: var(--color-accent);
	}

	.tab-spacer {
		flex: 1;
	}

	.target-bar {
		padding: 10px 16px;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		flex-shrink: 0;
	}

	.target-row {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.target-label {
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.04em;
		white-space: nowrap;
	}

	.target-controls {
		display: flex;
		gap: 8px;
		flex: 1;
	}

	.target-select {
		padding: 6px 8px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: 0.8125rem;
		font-family: inherit;
	}

	.target-select:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.tab-content {
		flex: 1;
		overflow-y: auto;
	}

	.tab-content-split {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.tab-panel {
		flex-shrink: 0;
		overflow-y: auto;
		max-height: 50%;
		border-bottom: 1px solid var(--color-border);
	}

	.output-area {
		flex: 1;
		padding: 12px 16px;
		overflow-y: auto;
		min-height: 0;
	}

	.file-viewer {
		display: flex;
		flex-direction: column;
		gap: 8px;
		height: 100%;
	}

	.file-viewer-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		flex-shrink: 0;
	}

	.file-viewer-title {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
		font-family: monospace;
	}

	.close-viewer-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		padding: 0;
		background: transparent;
		border: none;
		border-radius: var(--radius-btn);
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: color 0.12s ease, background-color 0.12s ease;
	}

	.close-viewer-btn:hover {
		color: var(--color-text-primary);
		background: rgba(255, 255, 255, 0.06);
	}

	.file-viewer-content {
		flex: 1;
		overflow: auto;
		padding: 10px 12px;
		border-radius: var(--radius-btn);
		background: color-mix(in srgb, var(--color-bg-primary) 90%, black);
		border: 1px solid var(--color-border);
	}

	.file-viewer-content pre {
		margin: 0;
		font-family: monospace;
		font-size: 0.75rem;
		line-height: 1.6;
		color: var(--color-text-primary);
		white-space: pre-wrap;
		word-break: break-all;
	}

	.loading-text {
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		font-style: italic;
	}
</style>
