<!--
	The Ansible workspace: a run you can read.

	Four tabs a person thinks in — Playbooks, Inventory, Dependencies,
	Vault — under one run bar that always says which engine will run and,
	on Playbooks, what. The result has the whole page below it. Ad hoc is a
	one-off by definition, so it is an action on the run bar, not a tab.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { faTerminal } from '@fortawesome/free-solid-svg-icons';
	import { t } from '$lib/state/i18n.svelte';
	import {
		getActiveProject,
		getProjectFiles,
		refreshFiles,
		closeProject,
		getWorkspaceTab,
		setWorkspaceTab
	} from '$lib/state/ansible.svelte';
	import { ansibleReadFile } from '$lib/ipc/ansible';
	import type { AnsibleExecutionTarget } from '$lib/ipc/ansible';
	import FaIcon from '$lib/components/shared/FaIcon.svelte';
	import Modal from '$lib/components/shared/Modal.svelte';
	import AnsibleEnginePicker from './AnsibleEnginePicker.svelte';
	import AnsibleRunView from './AnsibleRunView.svelte';
	import AnsiblePlaybookPanel from './AnsiblePlaybookPanel.svelte';
	import AnsibleInventoryPanel from './AnsibleInventoryPanel.svelte';
	import AnsibleRolesPanel from './AnsibleRolesPanel.svelte';
	import AnsibleCollectionsPanel from './AnsibleCollectionsPanel.svelte';
	import AnsibleAdHocPanel from './AnsibleAdHocPanel.svelte';
	import AnsibleVaultPanel from './AnsibleVaultPanel.svelte';

	let project = $derived(getActiveProject());
	let files = $derived(getProjectFiles());

	// Null until the picker settles on something that can run.
	let target = $state<AnsibleExecutionTarget | null>(null);

	let activeTab = $derived(getWorkspaceTab());
	let showAdHoc = $state(false);

	let selectedFile = $state<string | null>(null);
	let fileContent = $state<string | null>(null);
	let fileLoading = $state(false);
	let leftPanelCollapsed = $state(false);

	onMount(() => {
		refreshFiles();
	});

	async function handleFileClick(filename: string) {
		if (!project) return;
		selectedFile = filename;
		fileLoading = true;
		fileContent = null;
		setWorkspaceTab('playbooks');
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

	/** A run started from the ad hoc dialog shows where every run shows. */
	function onAdHocRun() {
		showAdHoc = false;
		closeFileViewer();
		setWorkspaceTab('playbooks');
	}
</script>

<div class="workspace">
	{#if leftPanelCollapsed}
		<button type="button" class="panel-expand-btn" onclick={() => leftPanelCollapsed = false} title={t('ansible.show_files')}>
			<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
				<path d="M9 18l6-6-6-6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
		</button>
	{/if}

	<aside class="left-panel" class:collapsed={leftPanelCollapsed}>
		<div class="panel-header">
			<h2 class="project-name">{project?.name ?? ''}</h2>
			<button type="button" class="panel-collapse-btn" onclick={() => leftPanelCollapsed = true} title={t('ansible.hide_files')}>
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
				{t('ansible.back_to_projects')}
			</button>
		</div>
	</aside>

	<main class="right-panel">
		<div class="tab-bar">
			<button class="tab" class:active={activeTab === 'playbooks'} onclick={() => setWorkspaceTab('playbooks')}>{t('ansible.tab_playbooks')}</button>
			<button class="tab" class:active={activeTab === 'inventory'} onclick={() => setWorkspaceTab('inventory')}>{t('ansible.tab_inventory')}</button>
			<button class="tab" class:active={activeTab === 'dependencies'} onclick={() => setWorkspaceTab('dependencies')}>{t('ansible.tab_dependencies')}</button>
			<button class="tab" class:active={activeTab === 'vault'} onclick={() => setWorkspaceTab('vault')}>{t('ansible.tab_vault')}</button>
		</div>

		<!-- The run bar: engine first, because nothing runs without one. -->
		<div class="run-bar">
			<div class="engine-row">
				<span class="run-label">{t('ansible.engine')}</span>
				<AnsibleEnginePicker onchange={(t2) => (target = t2)} />
				<button
					type="button"
					class="adhoc-btn"
					title={t('ansible.adhoc_open')}
					aria-label={t('ansible.adhoc_open')}
					disabled={!target}
					onclick={() => (showAdHoc = true)}
				>
					<FaIcon icon={faTerminal} size={12} />
					<span>{t('ansible.adhoc')}</span>
				</button>
			</div>
			{#if activeTab === 'playbooks'}
				<AnsiblePlaybookPanel {target} />
			{/if}
		</div>

		{#if activeTab === 'playbooks'}
			<div class="output-area">
				{#if selectedFile !== null}
					<div class="file-viewer">
						<div class="file-viewer-header">
							<span class="file-viewer-title">{selectedFile}</span>
							<button type="button" class="close-viewer-btn" title={t('ansible.close')} onclick={closeFileViewer}>
								<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
									<path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
								</svg>
							</button>
						</div>
						<div class="file-viewer-content">
							{#if fileLoading}
								<span class="loading-text">{t('ansible.loading')}</span>
							{:else}
								<pre>{fileContent ?? ''}</pre>
							{/if}
						</div>
					</div>
				{:else}
					<AnsibleRunView />
				{/if}
			</div>
		{:else if activeTab === 'inventory'}
			<div class="tab-content"><AnsibleInventoryPanel /></div>
		{:else if activeTab === 'dependencies'}
			<div class="tab-content">
				<section class="dep-section">
					<h3 class="dep-title">{t('ansible.roles_title')}</h3>
					<AnsibleRolesPanel {target} />
				</section>
				<section class="dep-section">
					<h3 class="dep-title">{t('ansible.collections_title')}</h3>
					<AnsibleCollectionsPanel {target} />
				</section>
			</div>
		{:else if activeTab === 'vault'}
			<div class="tab-content-split">
				<div class="tab-panel"><AnsibleVaultPanel {target} /></div>
				<div class="output-area"><AnsibleRunView /></div>
			</div>
		{/if}
	</main>
</div>

{#if showAdHoc}
	<Modal open={showAdHoc} title={t('ansible.adhoc_title')} maxWidth="520px" onclose={() => (showAdHoc = false)}>
		<AnsibleAdHocPanel {target} onrun={onAdHocRun} />
	</Modal>
{/if}

<style>
	.workspace {
		display: flex;
		width: 100%;
		height: 100%;
		background: var(--color-bg-primary);
		/* Breakpoints on this box, not the window: the workspace sits beside
		   the rail and the sessions sidebar. */
		container-type: inline-size;
		container-name: ansible;
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
		padding: 0;
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
		overflow: hidden;
		min-width: 0;
	}

	.tab-bar {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0 2px;
		padding: 0 16px;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		flex-shrink: 0;
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

	/* Run bar */
	.run-bar {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 10px 16px 12px;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		flex-shrink: 0;
	}

	/* Label, picker, Ad hoc: the button keeps the right edge whatever the
	   picker's hint does in the middle. */
	.engine-row {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: 10px;
		min-width: 0;
	}

	.run-label {
		font-size: 0.6875rem;
		font-weight: 600;
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.04em;
		white-space: nowrap;
	}

	/* Ad hoc: the same box as the sessions "+" — an action, not a tab. */
	.adhoc-btn {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		align-self: start;
		height: 30px;
		padding: 0 10px;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		background: transparent;
		color: var(--color-accent);
		font-family: inherit;
		font-size: 0.75rem;
		font-weight: 600;
		cursor: pointer;
		white-space: nowrap;
		transition: background-color var(--duration-default, 0.12s) var(--ease-default, ease),
			border-color var(--duration-default, 0.12s) var(--ease-default, ease);
	}

	.adhoc-btn:hover:not(:disabled) {
		background-color: rgba(0, 122, 255, 0.1);
		border-color: color-mix(in srgb, var(--color-accent) 55%, var(--color-border));
	}

	.adhoc-btn:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 1px;
	}

	.adhoc-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	/* Content */
	.tab-content {
		flex: 1;
		overflow-y: auto;
		min-height: 0;
	}

	.dep-section + .dep-section {
		border-top: 1px solid var(--color-border);
	}

	.dep-title {
		margin: 16px 16px 0;
		font-size: 0.6875rem;
		font-weight: 600;
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
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
		max-height: 40%;
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
		font-family: var(--font-mono, monospace);
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
		background: var(--color-surface-hover);
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
		font-family: var(--font-mono, monospace);
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

	@container ansible (max-width: 640px) {
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
			padding: 8px 10px;
			font-size: 0.75rem;
		}

		.run-bar {
			padding: 8px 12px 10px;
		}

		.output-area {
			padding: 8px 12px;
		}
	}
</style>
