<script lang="ts">
	import {
		getWorkingDir,
		setWorkingDir,
		setExecMode,
		setSelectedConnectionId,
		getActiveRun,
		setActiveRun,
		getOutputBuffer,
		appendOutput,
		clearOutput,
		getIsRunning,
		getSavedWorkspaces,
		setSavedWorkspaces,
		getToolStatus,
		setToolStatus,
		getToolVersion,
		setToolVersion,
		getInstallLog,
		appendInstallLog,
		clearInstallLog
	} from '$lib/state/terraform.svelte';
	import {
		terraformRun,
		terraformCancel,
		terraformListWorkspaces,
		terraformSaveWorkspace,
		terraformDeleteWorkspace,
		type TerraformOutputEvent,
		type TerraformCompleteEvent,
		type TerraformAction
	} from '$lib/ipc/terraform';
	import { toolchainCheck, toolchainInstall, type ToolInstallEvent } from '$lib/ipc/toolchain';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import TerraformStateView from './TerraformStateView.svelte';
	import TerraformOutputView from './TerraformOutputView.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { addToast } from '$lib/state/toasts.svelte';

	let selectedWorkspaceId = $state<string | null>(null);
	let activeBottomTab = $state<'state' | 'outputs'>('state');
	let outputEl: HTMLPreElement | undefined = $state();
	let installLogEl: HTMLPreElement | undefined = $state();

	let workingDir = $derived(getWorkingDir());
	let isRunning = $derived(getIsRunning());
	let outputBuffer = $derived(getOutputBuffer());
	let savedWorkspaces = $derived(getSavedWorkspaces());

	let toolSetupStatus = $derived(getToolStatus());
	let toolVersion = $derived(getToolVersion());
	let installLogText = $derived(getInstallLog());

	let unlistenOutput: UnlistenFn | null = null;
	let unlistenComplete: UnlistenFn | null = null;

	// Auto-detect on mount (not auto-install)
	$effect(() => {
		checkTool();
	});

	$effect(() => {
		loadWorkspaces();
	});

	$effect(() => {
		// Auto-scroll output to bottom
		const _buffer = outputBuffer;
		if (outputEl) {
			requestAnimationFrame(() => {
				if (outputEl) {
					outputEl.scrollTop = outputEl.scrollHeight;
				}
			});
		}
	});

	// Auto-scroll install log
	$effect(() => {
		const _log = installLogText;
		if (installLogEl) {
			requestAnimationFrame(() => {
				if (installLogEl) {
					installLogEl.scrollTop = installLogEl.scrollHeight;
				}
			});
		}
	});

	$effect(() => {
		return () => {
			cleanupListeners();
		};
	});

	async function loadWorkspaces(): Promise<void> {
		try {
			const workspaces = await terraformListWorkspaces();
			setSavedWorkspaces(workspaces);
		} catch {
			// Ignore
		}
	}

	function cleanupListeners(): void {
		if (unlistenOutput) {
			unlistenOutput();
			unlistenOutput = null;
		}
		if (unlistenComplete) {
			unlistenComplete();
			unlistenComplete = null;
		}
	}

	async function checkTool(): Promise<void> {
		try {
			setToolStatus('checking');
			const status = await toolchainCheck('terraform');
			if (status.installed) {
				setToolStatus('installed');
				setToolVersion(status.version);
			} else {
				setToolStatus('not_installed');
			}
		} catch {
			setToolStatus('not_installed');
		}
	}

	async function installTool(): Promise<void> {
		setToolStatus('installing');
		clearInstallLog();
		const unlisten = await listen<ToolInstallEvent>('toolchain-install-terraform', (event) => {
			appendInstallLog(event.payload.message);
		});
		try {
			const result = await toolchainInstall('terraform');
			if (result.installed) {
				setToolStatus('installed');
				setToolVersion(result.version);
				addToast(t('toolchain.install_success', { tool: 'Terraform' }), 'success');
			} else {
				setToolStatus('install_failed');
			}
		} catch {
			setToolStatus('install_failed');
		} finally {
			unlisten();
		}
	}

	async function runAction(action: TerraformAction): Promise<void> {
		const dir = getWorkingDir();
		if (!dir) {
			addToast(t('terraform.working_dir_placeholder'), 'error');
			return;
		}

		if (action === 'apply') {
			if (!confirm(t('terraform.confirm_apply'))) return;
		}
		if (action === 'destroy') {
			if (!confirm(t('terraform.confirm_destroy'))) return;
		}

		try {
			clearOutput();
			const run = await terraformRun(action, dir, 'local');
			setActiveRun(run);

			cleanupListeners();

			unlistenOutput = await listen<TerraformOutputEvent>(
				`tf-output-${run.id}`,
				(event) => {
					appendOutput(event.payload.data);
				}
			);

			unlistenComplete = await listen<TerraformCompleteEvent>(
				`tf-complete-${run.id}`,
				(event) => {
					setActiveRun(null);
					cleanupListeners();
					if (event.payload.status === 'Completed') {
						addToast(t('terraform.completed'), 'success');
					} else {
						addToast(t('terraform.failed'), 'error');
					}
				}
			);
		} catch (err) {
			setActiveRun(null);
			addToast(`${t('terraform.failed')}: ${err}`, 'error');
		}
	}

	async function cancelRun(): Promise<void> {
		const run = getActiveRun();
		if (run) {
			try {
				await terraformCancel(run.id);
			} catch (err) {
				addToast(`Cancel failed: ${err}`, 'error');
			}
		}
	}

	async function saveWorkspace(): Promise<void> {
		const dir = getWorkingDir();
		if (!dir) return;

		const name = prompt(t('terraform.save_workspace'));
		if (!name) return;

		try {
			await terraformSaveWorkspace(
				name,
				dir,
				'local',
				undefined,
				selectedWorkspaceId ?? undefined
			);
			await loadWorkspaces();
			addToast(t('terraform.workspace_saved'), 'success');
		} catch (err) {
			addToast(`Save failed: ${err}`, 'error');
		}
	}

	async function deleteWorkspace(): Promise<void> {
		if (!selectedWorkspaceId) return;
		try {
			await terraformDeleteWorkspace(selectedWorkspaceId);
			selectedWorkspaceId = null;
			await loadWorkspaces();
			addToast(t('terraform.workspace_deleted'), 'info');
		} catch (err) {
			addToast(`Delete failed: ${err}`, 'error');
		}
	}

	async function copyOutput(): Promise<void> {
		const text = getOutputBuffer();
		if (text) {
			await navigator.clipboard.writeText(text);
			addToast(t('common.copy'), 'success');
		}
	}

	function selectWorkspace(id: string): void {
		selectedWorkspaceId = id;
		const ws = getSavedWorkspaces().find((w) => w.id === id);
		if (ws) {
			setWorkingDir(ws.workingDir);
		}
	}
</script>

<div class="terraform-panel">
	{#if toolSetupStatus === 'checking'}
		<div class="tool-setup-screen">
			<div class="setup-spinner-row">
				<div class="spinner"></div>
				<span class="setup-checking-text">{t('toolchain.checking', { tool: 'Terraform' })}</span>
			</div>
		</div>
	{:else if toolSetupStatus === 'not_installed' || toolSetupStatus === 'installing' || toolSetupStatus === 'install_failed'}
		<div class="tool-setup-screen">
			<div class="setup-card">
				<!-- Icon -->
				<div class="setup-icon">
					<svg width="32" height="32" viewBox="0 0 24 24" fill="none">
						<path d="M1 5.5l7 4v8l-7-4v-8z" fill="currentColor" opacity="0.3" />
						<path d="M8.5 1.5l7 4v8l-7-4v-8z" fill="currentColor" opacity="0.5" />
						<path d="M8.5 13.5l7 4v8l-7-4v-8z" fill="currentColor" opacity="0.3" />
						<path d="M16 5.5l7 4v8l-7-4v-8z" fill="currentColor" opacity="0.5" />
					</svg>
				</div>

				<!-- Title -->
				<h3 class="setup-title">{t('toolchain.not_installed', { tool: 'Terraform' })}</h3>

				<!-- Description -->
				<p class="setup-desc">{t('toolchain.not_installed_desc_terraform')}</p>

				<!-- Install button or installing state -->
				{#if toolSetupStatus === 'not_installed'}
					<button class="install-btn" onclick={installTool}>
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
							<path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
							<polyline points="7 10 12 15 17 10" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
							<line x1="12" y1="15" x2="12" y2="3" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
						</svg>
						{t('toolchain.install', { tool: 'Terraform' })}
					</button>
				{:else if toolSetupStatus === 'installing'}
					<div class="installing-row">
						<div class="spinner"></div>
						<span class="installing-text">{t('toolchain.installing', { tool: 'Terraform' })}</span>
					</div>
				{:else if toolSetupStatus === 'install_failed'}
					<div class="failed-row">
						<span class="failed-text">{t('toolchain.install_failed', { tool: 'Terraform' })}</span>
					</div>
					<button class="install-btn" onclick={installTool}>
						{t('toolchain.retry')}
					</button>
				{/if}

				<!-- Install log -->
				{#if installLogText}
					<pre class="install-log" bind:this={installLogEl}>{installLogText}</pre>
				{/if}
			</div>
		</div>
	{:else}
		<!-- Tool is installed — normal panel -->
		{#if toolVersion}
			<div class="tool-version-bar">
				<span class="version-text">{t('toolchain.installed', { tool: 'Terraform', version: toolVersion })}</span>
			</div>
		{/if}
		<div class="section-body">
			<!-- Working directory -->
			<div class="section-row">
				<input
					class="tf-input"
					type="text"
					value={workingDir}
					oninput={(e) => setWorkingDir(e.currentTarget.value)}
					placeholder={t('terraform.working_dir_placeholder')}
				/>
			</div>

			<!-- Saved workspaces -->
			<div class="section-row workspace-row">
				<select
					class="tf-select workspace-select"
					value={selectedWorkspaceId ?? ''}
					onchange={(e) => {
						const val = e.currentTarget.value;
						if (val) selectWorkspace(val);
						else selectedWorkspaceId = null;
					}}
				>
					<option value="">{t('terraform.select_workspace')}</option>
					{#each savedWorkspaces as ws (ws.id)}
						<option value={ws.id}>{ws.name}</option>
					{/each}
				</select>
				<button class="icon-btn" onclick={saveWorkspace} title={t('terraform.save_workspace')}>
					<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
						<path d="M19 21H5a2 2 0 01-2-2V5a2 2 0 012-2h11l5 5v11a2 2 0 01-2 2z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
						<polyline points="17 21 17 13 7 13 7 21" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
						<polyline points="7 3 7 8 15 8" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
				</button>
				<button
					class="icon-btn danger"
					onclick={deleteWorkspace}
					disabled={!selectedWorkspaceId}
					title="Delete workspace"
				>
					<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
						<polyline points="3 6 5 6 21 6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
						<path d="M19 6l-1 14a2 2 0 01-2 2H8a2 2 0 01-2-2L5 6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
						<path d="M10 11v6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
						<path d="M14 11v6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
				</button>
			</div>

			<!-- Action buttons -->
			<div class="section-row action-row">
				<button class="action-btn" disabled={isRunning} onclick={() => runAction('init')}>
					{t('terraform.init')}
				</button>
				<button class="action-btn" disabled={isRunning} onclick={() => runAction('plan')}>
					{t('terraform.plan')}
				</button>
				<button class="action-btn accent" disabled={isRunning} onclick={() => runAction('apply')}>
					{t('terraform.apply')}
				</button>
				<button class="action-btn danger" disabled={isRunning} onclick={() => runAction('destroy')}>
					{t('terraform.destroy')}
				</button>
			</div>

			<!-- Output console -->
			<div class="output-section">
				<div class="output-bar">
					{#if isRunning}
						<span class="running-label">{t('terraform.running')}</span>
						<div class="output-bar-actions">
							<button class="cancel-btn" onclick={cancelRun}>
								{t('terraform.cancel')}
							</button>
						</div>
					{:else}
						<span class="output-bar-title"></span>
						<div class="output-bar-actions">
							{#if outputBuffer}
								<button class="icon-btn" onclick={copyOutput} title={t('common.copy')}>
									<svg width="13" height="13" viewBox="0 0 24 24" fill="none">
										<rect x="9" y="9" width="13" height="13" rx="2" stroke="currentColor" stroke-width="2" />
										<path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" stroke="currentColor" stroke-width="2" />
									</svg>
								</button>
							{/if}
						</div>
					{/if}
				</div>
				<pre class="output-console" bind:this={outputEl}>{outputBuffer}</pre>
			</div>

			<!-- Bottom tabs -->
			<div class="bottom-tabs">
				<button
					class="tab-btn"
					class:active={activeBottomTab === 'state'}
					onclick={() => (activeBottomTab = 'state')}
				>
					{t('terraform.state_tab')}
				</button>
				<button
					class="tab-btn"
					class:active={activeBottomTab === 'outputs'}
					onclick={() => (activeBottomTab = 'outputs')}
				>
					{t('terraform.outputs_tab')}
				</button>
			</div>

			<div class="bottom-content">
				{#if activeBottomTab === 'state'}
					<TerraformStateView />
				{:else}
					<TerraformOutputView />
				{/if}
			</div>
		</div>
	{/if}
</div>

<style>
	.terraform-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	/* ── Tool setup screen ── */

	.tool-setup-screen {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		flex: 1;
		padding: 24px 16px;
	}

	.setup-spinner-row {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.setup-checking-text {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.setup-card {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 10px;
		max-width: 260px;
		text-align: center;
	}

	.setup-icon {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.setup-title {
		margin: 0;
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.setup-desc {
		margin: 0;
		font-size: 0.6875rem;
		line-height: 1.5;
		color: var(--color-text-secondary);
	}

	.install-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
		padding: 6px 16px;
		font-size: 0.6875rem;
		font-weight: 600;
		color: #fff;
		background-color: var(--color-accent);
		border: none;
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
	}

	.install-btn:hover {
		filter: brightness(1.15);
	}

	.installing-row {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.installing-text {
		font-size: 0.6875rem;
		color: var(--color-accent);
		font-weight: 600;
	}

	.failed-row {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.failed-text {
		font-size: 0.6875rem;
		color: var(--color-danger);
		font-weight: 600;
	}

	.spinner {
		width: 16px;
		height: 16px;
		border: 2px solid var(--color-border);
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
		flex-shrink: 0;
	}

	@keyframes spin {
		to { transform: rotate(360deg); }
	}

	.install-log {
		width: 100%;
		max-height: 160px;
		overflow-y: auto;
		margin: 0;
		padding: 8px;
		font-family: var(--font-mono);
		font-size: 0.5625rem;
		line-height: 1.4;
		color: var(--color-text-secondary);
		background-color: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		white-space: pre-wrap;
		word-break: break-all;
		text-align: left;
	}

	/* ── Version bar ── */

	.tool-version-bar {
		padding: 4px 8px;
		border-bottom: 1px solid var(--color-border);
		background-color: rgba(48, 209, 88, 0.06);
	}

	.version-text {
		font-size: 0.625rem;
		color: var(--color-text-secondary);
	}

	/* ── Normal panel ── */

	.section-body {
		flex: 1;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 0;
	}

	.section-row {
		padding: 6px 8px;
		border-bottom: 1px solid var(--color-border);
	}

	/* Inputs and selects */
	.tf-input,
	.tf-select {
		width: 100%;
		padding: 5px 8px;
		font-size: 0.75rem;
		color: var(--color-text-primary);
		background-color: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		transition: border-color var(--duration-default) var(--ease-default);
		box-sizing: border-box;
	}

	.tf-input::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.6;
	}

	.tf-input:focus,
	.tf-select:focus {
		border-color: var(--color-accent);
	}

	.tf-select {
		appearance: none;
		background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%2386868b' stroke-width='2'%3E%3Cpolyline points='6 9 12 15 18 9'/%3E%3C/svg%3E");
		background-repeat: no-repeat;
		background-position: right 6px center;
		padding-right: 24px;
	}

	/* Workspace row */
	.workspace-row {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.workspace-select {
		flex: 1;
		min-width: 0;
	}

	.icon-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 5px;
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
		flex-shrink: 0;
	}

	.icon-btn:hover:not(:disabled) {
		color: var(--color-text-primary);
		border-color: var(--color-accent);
		background-color: rgba(255, 255, 255, 0.04);
	}

	.icon-btn.danger:hover:not(:disabled) {
		color: var(--color-danger);
		border-color: var(--color-danger);
		background-color: rgba(255, 69, 58, 0.08);
	}

	.icon-btn:disabled {
		opacity: 0.35;
		cursor: not-allowed;
	}

	/* Action buttons */
	.action-row {
		display: flex;
		gap: 4px;
	}

	.action-btn {
		flex: 1;
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

	.action-btn:hover:not(:disabled) {
		color: var(--color-text-primary);
		background-color: rgba(255, 255, 255, 0.04);
	}

	.action-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.action-btn.accent {
		color: var(--color-accent);
		border-color: var(--color-accent);
	}

	.action-btn.accent:hover:not(:disabled) {
		background-color: rgba(10, 132, 255, 0.12);
	}

	.action-btn.danger {
		color: var(--color-danger);
		border-color: var(--color-danger);
	}

	.action-btn.danger:hover:not(:disabled) {
		background-color: rgba(255, 69, 58, 0.12);
	}

	/* Output section */
	.output-section {
		display: flex;
		flex-direction: column;
		border-bottom: 1px solid var(--color-border);
	}

	.output-bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 4px 8px;
		border-bottom: 1px solid var(--color-border);
	}

	.output-bar-title {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.output-bar-actions {
		display: flex;
		align-items: center;
		gap: 4px;
		margin-left: auto;
	}

	.running-label {
		font-size: 0.6875rem;
		font-weight: 600;
		color: var(--color-accent);
		text-transform: uppercase;
		letter-spacing: 0.03em;
	}

	.cancel-btn {
		padding: 2px 8px;
		font-size: 0.625rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		color: var(--color-danger);
		background: transparent;
		border: 1px solid var(--color-danger);
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
	}

	.cancel-btn:hover {
		background-color: rgba(255, 69, 58, 0.12);
	}

	.output-console {
		margin: 0;
		padding: 8px;
		font-family: var(--font-mono);
		font-size: 0.6875rem;
		line-height: 1.5;
		color: var(--color-text-primary);
		background-color: var(--color-bg-primary);
		max-height: 300px;
		overflow-y: auto;
		white-space: pre-wrap;
		word-break: break-all;
	}

	/* Bottom tabs */
	.bottom-tabs {
		display: flex;
		gap: 1px;
		padding: 6px 8px;
		border-bottom: 1px solid var(--color-border);
	}

	.tab-btn {
		padding: 4px 10px;
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid transparent;
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition: all var(--duration-default) var(--ease-default);
	}

	.tab-btn:hover {
		color: var(--color-text-primary);
		background-color: rgba(255, 255, 255, 0.04);
	}

	.tab-btn.active {
		color: var(--color-accent);
		border-color: var(--color-accent);
		background-color: rgba(10, 132, 255, 0.08);
	}

	.bottom-content {
		flex: 1;
		overflow: hidden;
		min-height: 120px;
	}
</style>
