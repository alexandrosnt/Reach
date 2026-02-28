<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import {
		getActiveProject,
		getProjectFiles,
		isCommandRunning,
		runCommand,
		refreshFiles,
		closeProject,
		getWorkspaceTab,
		setWorkspaceTab,
		getActiveEnvironmentName,
		loadCatalog,
		generateHcl,
		formatFiles,
		isFmtRunning,
		loadPlanSummary,
		getPlanSummary
	} from '$lib/state/tofu.svelte';
	import { tofuReadFile } from '$lib/ipc/tofu';
	import type { TofuCommandRequest, TofuExecutionTarget } from '$lib/ipc/tofu';
	import { sshListConnections, type ConnectionInfo } from '$lib/ipc/ssh';
	import Button from '$lib/components/shared/Button.svelte';
	import TofuCommandOutput from './TofuCommandOutput.svelte';
	import TofuProviderPanel from './TofuProviderPanel.svelte';
	import TofuVariablePanel from './TofuVariablePanel.svelte';
	import TofuResourcePanel from './TofuResourcePanel.svelte';
	import TofuEnvironmentPanel from './TofuEnvironmentPanel.svelte';
	import TofuStatePanel from './TofuStatePanel.svelte';
	import TofuGraphPanel from './TofuGraphPanel.svelte';
	import TofuOutputPanel from './TofuOutputPanel.svelte';
	import TofuBackendPanel from './TofuBackendPanel.svelte';
	import TofuDataSourcePanel from './TofuDataSourcePanel.svelte';
	import TofuLocalsPanel from './TofuLocalsPanel.svelte';
	import TofuModulePanel from './TofuModulePanel.svelte';
	import TofuWorkspacePanel from './TofuWorkspacePanel.svelte';
	import TofuPlanViewer from './TofuPlanViewer.svelte';
	import TofuHclPreview from './TofuHclPreview.svelte';

	let project = $derived(getActiveProject());
	let files = $derived(getProjectFiles());
	let running = $derived(isCommandRunning());

	let connections = $state<ConnectionInfo[]>([]);
	let targetType = $state<'local' | 'ssh'>('local');
	let selectedConnectionId = $state<string | null>(null);
	let autoApprove = $state(false);

	let activeTab = $derived(getWorkspaceTab());
	let activeEnvName = $derived(getActiveEnvironmentName());
	let showHclPreview = $state(false);
	let fmtRunning = $derived(isFmtRunning());
	let showPlanViewer = $state(false);
	let planDone = $state(false);

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

	function buildTarget(): TofuExecutionTarget {
		if (targetType === 'ssh' && selectedConnectionId) {
			return { type: 'ssh', connectionId: selectedConnectionId };
		}
		return { type: 'local' };
	}

	function handleRunCommand(command: TofuCommandRequest['command']) {
		if (!project) return;
		showPlanViewer = false;
		planDone = false;
		const request: TofuCommandRequest = {
			projectId: project.id,
			command,
			target: buildTarget(),
			autoApprove,
			varFile: activeEnvName ? `${activeEnvName}.tfvars` : null,
			extraArgs: []
		};
		runCommand(request).then(() => {
			if (command === 'plan') {
				planDone = true;
			}
		});
	}

	async function handleFormat() {
		await formatFiles(buildTarget());
	}

	async function handleViewPlan() {
		await loadPlanSummary(buildTarget());
		showPlanViewer = true;
	}

	async function handleFileClick(filename: string) {
		if (!project) return;
		selectedFile = filename;
		fileLoading = true;
		fileContent = null;
		try {
			fileContent = await tofuReadFile(project.id, filename);
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
			<h3 class="section-label">{t('tofu.files')}</h3>

			{#if files.length === 0}
				<p class="no-files">{t('tofu.no_files')}</p>
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
				{t('tofu.back_to_projects')}
			</button>
		</div>
	</aside>

	<!-- Right Panel -->
	<main class="right-panel">
		<!-- Workspace Tabs -->
		<div class="tab-bar">
			<button class="tab" class:active={activeTab === 'actions'} onclick={() => setWorkspaceTab('actions')}>{t('tofu.tab_actions')}</button>
			<button class="tab" class:active={activeTab === 'providers'} onclick={() => setWorkspaceTab('providers')}>{t('tofu.tab_providers')}</button>
			<button class="tab" class:active={activeTab === 'variables'} onclick={() => setWorkspaceTab('variables')}>{t('tofu.tab_variables')}</button>
			<button class="tab" class:active={activeTab === 'resources'} onclick={() => setWorkspaceTab('resources')}>{t('tofu.tab_resources')}</button>
			<button class="tab" class:active={activeTab === 'data_sources'} onclick={() => setWorkspaceTab('data_sources')}>{t('tofu.tab_data_sources')}</button>
			<button class="tab" class:active={activeTab === 'environments'} onclick={() => setWorkspaceTab('environments')}>{t('tofu.tab_environments')}</button>
			<button class="tab" class:active={activeTab === 'backend'} onclick={() => setWorkspaceTab('backend')}>{t('tofu.tab_backend')}</button>
			<button class="tab" class:active={activeTab === 'locals'} onclick={() => setWorkspaceTab('locals')}>{t('tofu.tab_locals')}</button>
			<button class="tab" class:active={activeTab === 'modules'} onclick={() => setWorkspaceTab('modules')}>{t('tofu.tab_modules')}</button>
			<button class="tab" class:active={activeTab === 'state'} onclick={() => setWorkspaceTab('state')}>{t('tofu.tab_state')}</button>
			<button class="tab" class:active={activeTab === 'graph'} onclick={() => setWorkspaceTab('graph')}>{t('tofu.tab_graph')}</button>
			<button class="tab" class:active={activeTab === 'outputs'} onclick={() => setWorkspaceTab('outputs')}>{t('tofu.tab_outputs')}</button>
			<button class="tab" class:active={activeTab === 'workspaces'} onclick={() => setWorkspaceTab('workspaces')}>{t('tofu.tab_workspaces')}</button>
			<div class="tab-spacer"></div>
			<Button variant="secondary" size="sm" onclick={() => showHclPreview = true}>{t('tofu.generate_hcl')}</Button>
		</div>

		{#if activeTab === 'actions'}
			<!-- Action Buttons Bar -->
			<div class="action-bar">
				<div class="action-buttons">
					<Button variant="secondary" size="sm" disabled={running} onclick={() => handleRunCommand('init')}>
						{t('tofu.init')}
					</Button>
					<Button variant="primary" size="sm" disabled={running} onclick={() => handleRunCommand('plan')}>
						{t('tofu.plan')}
					</Button>
					<Button variant="primary" size="sm" disabled={running} onclick={() => handleRunCommand('apply')}>
						{t('tofu.apply')}
					</Button>
					<Button variant="danger" size="sm" disabled={running} onclick={() => handleRunCommand('destroy')}>
						{t('tofu.destroy')}
					</Button>
					<Button variant="secondary" size="sm" disabled={running} onclick={() => handleRunCommand('validate')}>
						{t('tofu.validate')}
					</Button>
					<Button variant="secondary" size="sm" disabled={running || fmtRunning} onclick={handleFormat}>
						{t('tofu.format')}
					</Button>
					{#if planDone && !running}
						<Button variant="secondary" size="sm" onclick={handleViewPlan}>
							{t('tofu.plan_view')}
						</Button>
					{/if}
				</div>
			</div>

			<!-- Execution Target Selector -->
			<div class="target-bar">
				<div class="target-row">
					<span class="target-label">{t('tofu.execution_target')}</span>
					<div class="target-controls">
						<select
							class="target-select"
							bind:value={targetType}
							onchange={() => { if (targetType === 'local') selectedConnectionId = null; }}
						>
							<option value="local">{t('tofu.target_local')}</option>
							<option value="ssh">{t('tofu.target_ssh')}</option>
						</select>

						{#if targetType === 'ssh'}
							<select class="target-select" bind:value={selectedConnectionId}>
								<option value={null}>{t('tofu.select_connection')}</option>
								{#each connections as conn (conn.id)}
									<option value={conn.id}>{conn.username}@{conn.host}:{conn.port}</option>
								{/each}
							</select>
						{/if}
					</div>
				</div>

				<div class="auto-approve-row">
					<label class="toggle-label">
						<span class="toggle-wrapper">
							<input
								type="checkbox"
								class="toggle-input"
								bind:checked={autoApprove}
							/>
							<span class="toggle-track">
								<span class="toggle-thumb"></span>
							</span>
						</span>
						<span class="toggle-text">{t('tofu.auto_approve')}</span>
					</label>
				</div>
			</div>

			<!-- File Viewer or Command Output -->
			<div class="output-area">
				{#if selectedFile !== null}
					<div class="file-viewer">
						<div class="file-viewer-header">
							<span class="file-viewer-title">{selectedFile}</span>
							<button type="button" class="close-viewer-btn" title="Close" onclick={closeFileViewer}>
								<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
									<path
										d="M18 6L6 18M6 6l12 12"
										stroke="currentColor"
										stroke-width="1.5"
										stroke-linecap="round"
										stroke-linejoin="round"
									/>
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
				{:else if showPlanViewer}
					<TofuPlanViewer />
				{:else}
					<TofuCommandOutput />
				{/if}
			</div>
		{:else if activeTab === 'providers'}
			<div class="tab-content"><TofuProviderPanel target={buildTarget()} /></div>
		{:else if activeTab === 'variables'}
			<div class="tab-content"><TofuVariablePanel /></div>
		{:else if activeTab === 'resources'}
			<div class="tab-content"><TofuResourcePanel /></div>
		{:else if activeTab === 'environments'}
			<div class="tab-content"><TofuEnvironmentPanel /></div>
		{:else if activeTab === 'data_sources'}
			<div class="tab-content"><TofuDataSourcePanel /></div>
		{:else if activeTab === 'backend'}
			<div class="tab-content"><TofuBackendPanel /></div>
		{:else if activeTab === 'locals'}
			<div class="tab-content"><TofuLocalsPanel /></div>
		{:else if activeTab === 'modules'}
			<div class="tab-content"><TofuModulePanel /></div>
		{:else if activeTab === 'state'}
			<div class="tab-content"><TofuStatePanel target={buildTarget()} /></div>
		{:else if activeTab === 'graph'}
			<div class="tab-content"><TofuGraphPanel /></div>
		{:else if activeTab === 'outputs'}
			<div class="tab-content"><TofuOutputPanel target={buildTarget()} /></div>
		{:else if activeTab === 'workspaces'}
			<div class="tab-content"><TofuWorkspacePanel target={buildTarget()} /></div>
		{/if}
	</main>
</div>

{#if showHclPreview}
	<TofuHclPreview onclose={() => showHclPreview = false} />
{/if}

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
		padding: 4px 0;
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
		min-width: 0;
		overflow: hidden;
	}

	.action-bar {
		display: flex;
		align-items: center;
		padding: 12px 16px;
		border-bottom: 1px solid var(--color-border);
	}

	.action-buttons {
		display: flex;
		gap: 8px;
		flex-wrap: wrap;
	}

	.target-bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 10px 16px;
		border-bottom: 1px solid var(--color-border);
		flex-wrap: wrap;
	}

	.target-row {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.target-label {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-secondary);
		white-space: nowrap;
	}

	.target-controls {
		display: flex;
		gap: 8px;
	}

	.target-select {
		padding: 5px 10px;
		font-size: 0.8125rem;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		cursor: pointer;
	}

	.target-select:focus {
		border-color: var(--color-accent);
	}

	.auto-approve-row {
		display: flex;
		align-items: center;
	}

	.toggle-label {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		cursor: pointer;
		user-select: none;
	}

	.toggle-wrapper {
		position: relative;
		display: inline-flex;
		align-items: center;
	}

	.toggle-input {
		position: absolute;
		opacity: 0;
		width: 0;
		height: 0;
		pointer-events: none;
	}

	.toggle-track {
		display: inline-block;
		width: 34px;
		height: 18px;
		background: var(--color-border);
		border-radius: 9px;
		position: relative;
		transition: background-color 0.2s ease;
	}

	.toggle-input:checked + .toggle-track {
		background: var(--color-accent);
	}

	.toggle-thumb {
		position: absolute;
		top: 2px;
		left: 2px;
		width: 14px;
		height: 14px;
		background: #fff;
		border-radius: 50%;
		transition: transform 0.2s ease;
	}

	.toggle-input:checked + .toggle-track .toggle-thumb {
		transform: translateX(16px);
	}

	.toggle-text {
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		font-weight: 500;
	}

	/* Tab Bar */
	.tab-bar {
		display: flex;
		align-items: center;
		gap: 0;
		padding: 0 12px;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		overflow-x: auto;
		flex-shrink: 0;
		scrollbar-width: none;
	}

	.tab-bar::-webkit-scrollbar {
		display: none;
	}

	.tab {
		padding: 10px 12px;
		background: transparent;
		border: none;
		border-bottom: 2px solid transparent;
		color: var(--color-text-secondary);
		font-size: 0.8125rem;
		font-weight: 500;
		cursor: pointer;
		transition: color 0.12s ease, border-color 0.12s ease;
		white-space: nowrap;
		flex-shrink: 0;
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

	.tab-content {
		flex: 1;
		overflow-y: auto;
		padding: 16px;
	}

	/* Output Area */
	.output-area {
		flex: 1;
		display: flex;
		flex-direction: column;
		padding: 16px;
		overflow: hidden;
	}

	/* File Viewer */
	.file-viewer {
		display: flex;
		flex-direction: column;
		height: 100%;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		overflow: hidden;
	}

	.file-viewer-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 12px;
		background: var(--color-bg-elevated);
		border-bottom: 1px solid var(--color-border);
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
		width: 24px;
		height: 24px;
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
		padding: 12px;
		background: color-mix(in srgb, var(--color-bg-primary) 90%, black);
	}

	.file-viewer-content pre {
		margin: 0;
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
		opacity: 0.6;
	}

	/* Responsive */
	@media (max-width: 800px) {
		.left-panel {
			width: 200px;
			min-width: 200px;
		}

		.tab {
			padding: 8px 10px;
			font-size: 0.75rem;
		}

		.action-bar {
			padding: 10px 12px;
		}

		.target-bar {
			padding: 8px 12px;
			flex-direction: column;
			align-items: flex-start;
		}

		.target-row {
			flex-wrap: wrap;
		}

		.output-area {
			padding: 12px;
		}

		.tab-content {
			padding: 12px;
		}
	}

	@media (max-width: 600px) {
		.left-panel:not(.collapsed) {
			width: 180px;
			min-width: 180px;
		}

		.tab {
			padding: 8px 8px;
			font-size: 0.6875rem;
		}

		.action-buttons {
			gap: 4px;
		}

		.tab-content {
			padding: 8px;
		}

		.output-area {
			padding: 8px;
		}
	}
</style>
