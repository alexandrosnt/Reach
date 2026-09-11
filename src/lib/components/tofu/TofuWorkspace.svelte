<!--
	The OpenTofu workspace: a run you can read.

	Three tabs a person thinks in. Overview is the run: a bar that says which
	environment, on which machine, with Validate and Plan; the result below,
	and when a plan is waiting, the gate that applies exactly that plan. Code
	holds the HCL builders as sections. State holds state, graph, workspaces
	and backend, and at its foot the one action that must never be a
	sibling of Plan: Destroy, behind a typed confirmation.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { faEllipsis } from '@fortawesome/free-solid-svg-icons';
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
		getCodeSection,
		setCodeSection,
		getStateSection,
		setStateSection,
		getActiveEnvironmentName,
		getActiveBackend,
		getActiveEncryption,
		isPlanPending,
		discardPlan,
		formatFiles,
		isFmtRunning,
		loadPlanSummary,
		loadBinaryStatus,
		type TofuCodeSection,
		type TofuStateSection
	} from '$lib/state/tofu.svelte';
	import { tofuReadFile } from '$lib/ipc/tofu';
	import type { TofuCommandRequest, TofuExecutionTarget } from '$lib/ipc/tofu';
	import { sshListConnections, type ConnectionInfo } from '$lib/ipc/ssh';
	import Button from '$lib/components/shared/Button.svelte';
	import FaIcon from '$lib/components/shared/FaIcon.svelte';
	import TofuRunView from './TofuRunView.svelte';
	import TofuVersionControl from './TofuVersionControl.svelte';
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
	let fmtRunning = $derived(isFmtRunning());

	let connections = $state<ConnectionInfo[]>([]);
	// 'local' or an SSH connection id.
	let engine = $state('local');
	let autoApprove = $state(false);

	let activeTab = $derived(getWorkspaceTab());
	let codeSection = $derived(getCodeSection());
	let stateSection = $derived(getStateSection());
	let activeEnvName = $derived(getActiveEnvironmentName());
	let backend = $derived(getActiveBackend());
	let encryption = $derived(getActiveEncryption());
	let planPending = $derived(isPlanPending());

	let showHclPreview = $state(false);
	let showPlanViewer = $state(false);
	let moreOpen = $state(false);
	let moreEl = $state<HTMLDivElement | undefined>();
	let destroyConfirm = $state('');

	let selectedFile = $state<string | null>(null);
	let fileContent = $state<string | null>(null);
	let fileLoading = $state(false);
	let leftPanelCollapsed = $state(false);

	const CODE_SECTIONS: { id: TofuCodeSection; key: string }[] = [
		{ id: 'providers', key: 'tofu.tab_providers' },
		{ id: 'variables', key: 'tofu.tab_variables' },
		{ id: 'resources', key: 'tofu.tab_resources' },
		{ id: 'data_sources', key: 'tofu.tab_data_sources' },
		{ id: 'locals', key: 'tofu.tab_locals' },
		{ id: 'modules', key: 'tofu.tab_modules' },
		{ id: 'outputs', key: 'tofu.tab_outputs' },
		{ id: 'environments', key: 'tofu.tab_environments' }
	];

	const STATE_SECTIONS: { id: TofuStateSection; key: string }[] = [
		{ id: 'state', key: 'tofu.tab_state' },
		{ id: 'graph', key: 'tofu.tab_graph' },
		{ id: 'workspaces', key: 'tofu.tab_workspaces' },
		{ id: 'backend', key: 'tofu.tab_backend' }
	];

	onMount(() => {
		refreshFiles();
		loadConnections();
		loadBinaryStatus();
	});

	$effect(() => {
		if (!moreOpen) return;
		const onDown = (e: MouseEvent) => {
			if (moreEl && !moreEl.contains(e.target as Node)) moreOpen = false;
		};
		const onKey = (e: KeyboardEvent) => {
			if (e.key === 'Escape') moreOpen = false;
		};
		window.addEventListener('mousedown', onDown);
		window.addEventListener('keydown', onKey);
		return () => {
			window.removeEventListener('mousedown', onDown);
			window.removeEventListener('keydown', onKey);
		};
	});

	async function loadConnections() {
		try {
			connections = await sshListConnections();
		} catch {
			connections = [];
		}
	}

	function buildTarget(): TofuExecutionTarget {
		if (engine !== 'local' && connections.some((c) => c.id === engine)) {
			return { type: 'ssh', connectionId: engine };
		}
		return { type: 'local' };
	}

	/** Every run shows in Overview, whichever tab started it. */
	function handleRunCommand(command: TofuCommandRequest['command'], approve = autoApprove) {
		if (!project) return;
		moreOpen = false;
		showPlanViewer = false;
		closeFileViewer();
		setWorkspaceTab('overview');
		const request: TofuCommandRequest = {
			projectId: project.id,
			command,
			target: buildTarget(),
			autoApprove: approve,
			varFile: activeEnvName ? `${activeEnvName}.tfvars` : null,
			extraArgs: []
		};
		runCommand(request);
	}

	function handleDestroy() {
		if (!project || destroyConfirm !== project.name) return;
		destroyConfirm = '';
		handleRunCommand('destroy', true);
	}

	async function handleFormat() {
		moreOpen = false;
		await formatFiles(buildTarget());
	}

	async function handleViewPlan() {
		moreOpen = false;
		await loadPlanSummary(buildTarget());
		showPlanViewer = true;
	}

	async function handleFileClick(filename: string) {
		if (!project) return;
		selectedFile = filename;
		fileLoading = true;
		fileContent = null;
		setWorkspaceTab('overview');
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

	let whatLine = $derived.by(() => {
		const parts: string[] = [];
		parts.push(activeEnvName ? `${activeEnvName}.tfvars` : t('tofu.no_var_file'));
		if (encryption?.enabled) parts.push(t('tofu.state_encrypted'));
		parts.push(t('tofu.backend_is', { backend: backend?.backendType ?? 'local' }));
		return parts.join(' · ');
	});
</script>

<div class="workspace">
	{#if leftPanelCollapsed}
		<button type="button" class="panel-expand-btn" onclick={() => leftPanelCollapsed = false} title={t('tofu.show_files')}>
			<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
				<path d="M9 18l6-6-6-6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
		</button>
	{/if}

	<aside class="left-panel" class:collapsed={leftPanelCollapsed}>
		<div class="panel-header">
			<h2 class="project-name">{project?.name ?? ''}</h2>
			<button type="button" class="panel-collapse-btn" onclick={() => leftPanelCollapsed = true} title={t('tofu.hide_files')}>
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
									<path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8l-6-6z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
									<path d="M14 2v6h6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
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
					<path d="M19 12H5M12 19l-7-7 7-7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
				{t('tofu.back_to_projects')}
			</button>
		</div>
	</aside>

	<main class="right-panel">
		<div class="tab-bar">
			<button class="tab" class:active={activeTab === 'overview'} onclick={() => setWorkspaceTab('overview')}>{t('tofu.tab_overview')}</button>
			<button class="tab" class:active={activeTab === 'code'} onclick={() => setWorkspaceTab('code')}>{t('tofu.tab_code')}</button>
			<button class="tab" class:active={activeTab === 'state'} onclick={() => setWorkspaceTab('state')}>{t('tofu.tab_state')}</button>
			<div class="tab-spacer"></div>
			<div class="tab-side"><TofuVersionControl /></div>
		</div>

		{#if activeTab === 'overview'}
			<!-- The run bar: what will run, where, and the two buttons that matter. -->
			<div class="run-bar">
				<div class="what">
					<span class="what-name">{activeEnvName ?? t('tofu.env_default')}</span>
					<span class="what-detail mono">{whatLine}</span>
					<select class="engine-select" bind:value={engine} onfocus={loadConnections} title={t('tofu.execution_target')}>
						<option value="local">{t('tofu.target_local')}</option>
						{#each connections as conn (conn.id)}
							<option value={conn.id}>{t('tofu.target_remote')} · {conn.username}@{conn.host}</option>
						{/each}
					</select>
				</div>
				<div class="acts">
					<Button variant="secondary" size="sm" disabled={running} onclick={() => handleRunCommand('validate')}>
						{t('tofu.validate')}
					</Button>
					<Button variant="primary" size="sm" disabled={running} onclick={() => handleRunCommand('plan')}>
						{t('tofu.plan')}
					</Button>
					<div class="more-wrap" bind:this={moreEl}>
						<button
							type="button"
							class="more-btn"
							class:open={moreOpen}
							title={t('tofu.more_actions')}
							aria-label={t('tofu.more_actions')}
							aria-haspopup="menu"
							aria-expanded={moreOpen}
							onclick={() => (moreOpen = !moreOpen)}
						>
							<FaIcon icon={faEllipsis} size={13} />
						</button>
						{#if moreOpen}
							<div class="menu" role="menu">
								<button class="menu-item" role="menuitem" disabled={running} onclick={() => handleRunCommand('init')}><span>{t('tofu.init')}</span></button>
								<button class="menu-item" role="menuitem" disabled={running} onclick={() => handleRunCommand('test')}><span>{t('tofu.test')}</span></button>
								<button class="menu-item" role="menuitem" disabled={running || fmtRunning} onclick={handleFormat}><span>{t('tofu.format')}</span></button>
								<div class="menu-sep"></div>
								<button class="menu-item" role="menuitem" disabled={running} onclick={() => handleRunCommand('apply')}><span>{t('tofu.apply')}</span></button>
								<button class="menu-item" role="menuitem" disabled={running || !planPending} onclick={handleViewPlan}><span>{t('tofu.plan_view')}</span></button>
								<div class="menu-sep"></div>
								<label class="menu-item menu-toggle">
									<span>{t('tofu.auto_approve')}</span>
									<input type="checkbox" bind:checked={autoApprove} />
								</label>
							</div>
						{/if}
					</div>
				</div>
			</div>

			<div class="output-area">
				{#if selectedFile !== null}
					<div class="file-viewer">
						<div class="file-viewer-header">
							<span class="file-viewer-title">{selectedFile}</span>
							<button type="button" class="close-viewer-btn" title={t('tofu.close')} onclick={closeFileViewer}>
								<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
									<path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
								</svg>
							</button>
						</div>
						<div class="file-viewer-content">
							{#if fileLoading}
								<span class="loading-text">{t('tofu.loading')}</span>
							{:else}
								<pre>{fileContent ?? ''}</pre>
							{/if}
						</div>
					</div>
				{:else}
					{#if planPending && !running}
						<!-- The review gate. Apply here applies the saved plan, never a fresh one. -->
						<div class="gate">
							<span class="gate-text">{t('tofu.plan_gate')}</span>
							<div class="gate-acts">
								<button type="button" class="gate-link" onclick={showPlanViewer ? () => (showPlanViewer = false) : handleViewPlan}>
									{showPlanViewer ? t('tofu.plan_gate_result') : t('tofu.plan_gate_full')}
								</button>
								<Button variant="secondary" size="sm" onclick={discardPlan}>{t('tofu.plan_discard')}</Button>
								<Button variant="primary" size="sm" onclick={() => handleRunCommand('apply', false)}>{t('tofu.plan_apply_this')}</Button>
							</div>
						</div>
					{/if}
					{#if showPlanViewer}
						<TofuPlanViewer />
					{:else}
						<TofuRunView />
					{/if}
				{/if}
			</div>
		{:else if activeTab === 'code'}
			<div class="section-nav">
				{#each CODE_SECTIONS as s (s.id)}
					<button class="section-btn" class:active={codeSection === s.id} onclick={() => setCodeSection(s.id)}>{t(s.key)}</button>
				{/each}
				<div class="tab-spacer"></div>
				<Button variant="secondary" size="sm" onclick={() => showHclPreview = true}>{t('tofu.generate_hcl')}</Button>
			</div>
			<div class="tab-content">
				{#if codeSection === 'providers'}
					<TofuProviderPanel target={buildTarget()} />
				{:else if codeSection === 'variables'}
					<TofuVariablePanel />
				{:else if codeSection === 'resources'}
					<TofuResourcePanel />
				{:else if codeSection === 'data_sources'}
					<TofuDataSourcePanel />
				{:else if codeSection === 'locals'}
					<TofuLocalsPanel />
				{:else if codeSection === 'modules'}
					<TofuModulePanel />
				{:else if codeSection === 'outputs'}
					<TofuOutputPanel target={buildTarget()} />
				{:else if codeSection === 'environments'}
					<TofuEnvironmentPanel />
				{/if}
			</div>
		{:else}
			<div class="section-nav">
				{#each STATE_SECTIONS as s (s.id)}
					<button class="section-btn" class:active={stateSection === s.id} onclick={() => setStateSection(s.id)}>{t(s.key)}</button>
				{/each}
			</div>
			<div class="tab-content">
				{#if stateSection === 'state'}
					<TofuStatePanel target={buildTarget()} />
					<!-- Destroy lives with the state it erases, behind the project's own name. -->
					<section class="danger">
						<h3 class="danger-title">{t('tofu.danger_zone')}</h3>
						<p class="danger-text">{t('tofu.destroy_explain')}</p>
						<div class="danger-row">
							<input
								type="text"
								class="danger-input mono"
								placeholder={t('tofu.destroy_type_name', { name: project?.name ?? '' })}
								bind:value={destroyConfirm}
								autocomplete="off"
								spellcheck="false"
							/>
							<Button variant="danger" size="sm" disabled={running || !project || destroyConfirm !== project.name} onclick={handleDestroy}>
								{t('tofu.destroy')}
							</Button>
						</div>
					</section>
				{:else if stateSection === 'graph'}
					<TofuGraphPanel />
				{:else if stateSection === 'workspaces'}
					<TofuWorkspacePanel target={buildTarget()} />
				{:else if stateSection === 'backend'}
					<TofuBackendPanel />
				{/if}
			</div>
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
		/* Breakpoints on this box, not the window: the workspace sits beside
		   the rail and the sessions sidebar, so a 1300px window can hand it
		   700px. */
		container-type: inline-size;
		container-name: tofu;
	}

	/* Left panel */
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
		background: var(--color-surface-hover);
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
		background: var(--color-surface-hover);
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
		font-family: var(--font-mono, monospace);
		font-size: 0.75rem;
		cursor: pointer;
		text-align: left;
		transition: background-color 0.12s ease, color 0.12s ease;
	}

	.file-item:hover {
		background: var(--color-surface-hover);
		color: var(--color-text-primary);
	}

	.file-item.active {
		background: var(--color-surface-active);
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

	/* Right panel */
	.right-panel {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-width: 0;
		overflow: hidden;
	}

	.tab-bar {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0 2px;
		padding: 0 12px;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		flex-shrink: 0;
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

	.tab-side {
		display: flex;
		align-items: center;
		padding: 6px 4px;
		min-width: 0;
		max-width: 100%;
	}

	/* Run bar */
	.run-bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		flex-wrap: wrap;
		gap: 10px 16px;
		padding: 10px 16px;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		flex-shrink: 0;
	}

	.what {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 6px 10px;
		min-width: 0;
		flex: 1 1 320px;
	}

	.what-name {
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--color-text-primary);
		white-space: nowrap;
	}

	.what-detail {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.mono {
		font-family: var(--font-mono, monospace);
	}

	.engine-select {
		padding: 5px 10px;
		font-size: 0.8125rem;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		font-family: inherit;
		max-width: 260px;
		outline: none;
		cursor: pointer;
	}

	.engine-select:focus {
		border-color: var(--color-accent);
	}

	.acts {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-left: auto;
	}

	.more-wrap {
		position: relative;
	}

	.more-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 30px;
		height: 30px;
		padding: 0;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: background-color 0.12s ease, border-color 0.12s ease, color 0.12s ease;
	}

	.more-btn:hover,
	.more-btn.open {
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
	}

	.more-btn:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 1px;
	}

	.menu {
		position: absolute;
		right: 0;
		top: calc(100% + 4px);
		z-index: 30;
		min-width: 200px;
		padding: 4px;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: 8px;
		box-shadow: var(--shadow-elevated, 0 8px 32px rgba(0, 0, 0, 0.35));
	}

	.menu-item {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		padding: 7px 10px;
		border: none;
		border-radius: 5px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		text-align: left;
		font-family: inherit;
		font-size: 0.8125rem;
		white-space: nowrap;
	}

	.menu-item:hover:not(:disabled),
	.menu-item:focus-visible {
		background: var(--color-surface-hover);
		color: var(--color-text-primary);
		outline: none;
	}

	.menu-item:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.menu-item span {
		flex: 1;
	}

	.menu-toggle input {
		accent-color: var(--color-accent);
		margin: 0;
	}

	.menu-sep {
		height: 1px;
		margin: 4px 6px;
		background: var(--color-border);
	}

	/* Section nav inside Code and State */
	.section-nav {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 4px;
		padding: 8px 12px;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		flex-shrink: 0;
	}

	.section-btn {
		padding: 5px 10px;
		border: 1px solid transparent;
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		font-family: inherit;
		font-size: 0.75rem;
		font-weight: 500;
		cursor: pointer;
		white-space: nowrap;
		transition: color 0.12s ease, background-color 0.12s ease;
	}

	.section-btn:hover {
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
	}

	.section-btn.active {
		color: var(--color-accent);
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
	}

	.tab-content {
		flex: 1;
		overflow-y: auto;
		padding: 16px;
	}

	/* Output */
	.output-area {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 16px;
		overflow: hidden;
		min-height: 0;
	}

	.gate {
		display: flex;
		align-items: center;
		justify-content: space-between;
		flex-wrap: wrap;
		gap: 8px 12px;
		padding: 10px 12px;
		border: 1px solid color-mix(in srgb, var(--color-accent) 40%, var(--color-border));
		border-radius: var(--radius-btn);
		background: color-mix(in srgb, var(--color-accent) 8%, var(--color-bg-elevated));
		flex-shrink: 0;
	}

	.gate-text {
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		flex: 1 1 200px;
	}

	.gate-acts {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}

	.gate-link {
		background: transparent;
		border: none;
		padding: 4px 6px;
		color: var(--color-accent);
		font-family: inherit;
		font-size: 0.8125rem;
		cursor: pointer;
	}

	.gate-link:hover {
		text-decoration: underline;
	}

	/* Danger zone */
	.danger {
		margin-top: 24px;
		padding: 14px 16px;
		border: 1px solid color-mix(in srgb, var(--color-danger, #ff453a) 40%, var(--color-border));
		border-radius: var(--radius-btn);
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.danger-title {
		margin: 0;
		font-size: 0.6875rem;
		font-weight: 600;
		color: var(--color-danger, #ff453a);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.danger-text {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		max-width: 60ch;
	}

	.danger-row {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 8px;
	}

	.danger-input {
		flex: 1 1 200px;
		max-width: 320px;
		padding: 6px 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: 0.8125rem;
	}

	.danger-input:focus {
		outline: none;
		border-color: var(--color-danger, #ff453a);
	}

	/* File viewer */
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
		font-family: var(--font-mono, monospace);
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
		background: var(--color-surface-hover);
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
	@container tofu (max-width: 900px) {
		.left-panel {
			width: 200px;
			min-width: 200px;
		}

		.tab {
			padding: 8px 10px;
			font-size: 0.75rem;
		}

		.run-bar {
			padding: 8px 12px;
		}

		.output-area,
		.tab-content {
			padding: 12px;
		}
	}

	@container tofu (max-width: 640px) {
		/* The file list and the work cannot both fit; the file list yields
		   and comes back through its expand button. */
		.left-panel:not(.collapsed) {
			width: 0;
			min-width: 0;
			overflow: hidden;
			opacity: 0;
			border-right: none;
		}

		.tab {
			padding: 8px 8px;
			font-size: 0.6875rem;
		}

		.acts {
			margin-left: 0;
		}

		.output-area,
		.tab-content {
			padding: 8px;
		}
	}
</style>
