<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { checkTool, isToolInstalled, getToolVersion, isLocalUnsupported, isWsl } from '$lib/state/ansible.svelte';
	import { toolchainInstall, type ToolInstallEvent } from '$lib/ipc/toolchain';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import Button from '$lib/components/shared/Button.svelte';

	let installing = $state(false);
	let installLogs = $state<string[]>([]);
	let installDone = $state(false);
	let installSuccess = $state(false);

	let logContainer: HTMLDivElement | undefined = $state(undefined);

	onMount(() => {
		checkTool();
	});

	// Auto-scroll log container when new logs arrive
	$effect(() => {
		if (installLogs.length > 0 && logContainer) {
			logContainer.scrollTop = logContainer.scrollHeight;
		}
	});

	async function handleInstall() {
		installing = true;
		installLogs = [];
		installDone = false;
		installSuccess = false;

		let unlisten: UnlistenFn | null = null;

		unlisten = await listen<ToolInstallEvent>('toolchain-install-ansible', (event) => {
			const data = event.payload;
			if (data.message) {
				installLogs = [...installLogs, data.message];
			}

			if (data.done) {
				installDone = true;
				installSuccess = data.success;
				installing = false;
				if (unlisten) {
					unlisten();
					unlisten = null;
				}
			}
		});

		try {
			await toolchainInstall('ansible');
			await checkTool();
		} catch {
			installing = false;
			installDone = true;
			installSuccess = false;
		}
	}

	function handleContinue() {
		installLogs = [];
		installDone = false;
	}
</script>

<div class="toolchain-setup">
	{#if isWsl()}
		<!-- WSL detected on Windows: show two-row checklist -->
		<div class="checklist">
			<div class="checklist-row">
				<svg class="check-icon success" width="20" height="20" viewBox="0 0 20 20" fill="none">
					<circle cx="10" cy="10" r="9" stroke="currentColor" stroke-width="1.5" />
					<path d="M6 10l3 3 5-6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
				<div class="checklist-info">
					<span class="checklist-label">Windows Subsystem for Linux</span>
					<span class="checklist-detail detected">{t('ansible.wsl_mode')}</span>
				</div>
			</div>

			{#if isToolInstalled()}
				<div class="checklist-row">
					<svg class="check-icon success" width="20" height="20" viewBox="0 0 20 20" fill="none">
						<circle cx="10" cy="10" r="9" stroke="currentColor" stroke-width="1.5" />
						<path d="M6 10l3 3 5-6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
					<div class="checklist-info">
						<span class="checklist-label">{t('ansible.installed')}</span>
						{#if getToolVersion()}
							<span class="checklist-detail">{t('ansible.version', { version: getToolVersion()! })}</span>
						{/if}
					</div>
				</div>
			{:else if installLogs.length === 0}
				<div class="checklist-row">
					<svg class="x-icon" width="20" height="20" viewBox="0 0 20 20" fill="none">
						<circle cx="10" cy="10" r="9" stroke="currentColor" stroke-width="1.5" />
						<path d="M7 7l6 6M13 7l-6 6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
					</svg>
					<span class="checklist-label missing">{t('ansible.tool_not_installed')}</span>
					<Button
						variant="primary"
						size="sm"
						onclick={handleInstall}
						disabled={installing}
					>
						{installing ? t('ansible.installing') : t('ansible.install')}
					</Button>
				</div>
			{/if}
		</div>
	{:else if isLocalUnsupported()}
		<!-- Windows without WSL: warning banner -->
		<div class="warning-banner">
			<svg class="warn-icon" width="18" height="18" viewBox="0 0 20 20" fill="none">
				<path d="M10 2L1 18h18L10 2z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" />
				<path d="M10 8v4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
				<circle cx="10" cy="15" r="1" fill="currentColor" />
			</svg>
			<span class="warning-text">{t('ansible.local_unsupported')}</span>
		</div>
	{:else}
		<!-- Linux/macOS: standard ansible status -->
		{#if isToolInstalled()}
			<div class="status-row installed">
				<svg class="check-icon success" width="20" height="20" viewBox="0 0 20 20" fill="none">
					<circle cx="10" cy="10" r="9" stroke="currentColor" stroke-width="1.5" />
					<path d="M6 10l3 3 5-6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
				<div class="status-text">
					<span class="label">{t('ansible.installed')}</span>
					{#if getToolVersion()}
						<span class="version">{t('ansible.version', { version: getToolVersion()! })}</span>
					{/if}
				</div>
			</div>
		{:else if installLogs.length === 0}
			<div class="not-installed">
				<svg class="warn-icon" width="20" height="20" viewBox="0 0 20 20" fill="none">
					<circle cx="10" cy="10" r="9" stroke="currentColor" stroke-width="1.5" />
					<path d="M10 6v5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
					<circle cx="10" cy="14" r="1" fill="currentColor" />
				</svg>
				<span class="not-installed-text">{t('ansible.tool_not_installed')}</span>
				<Button
					variant="primary"
					size="sm"
					onclick={handleInstall}
					disabled={installing}
				>
					{installing ? t('ansible.installing') : t('ansible.install')}
				</Button>
			</div>
		{/if}
	{/if}

	<!-- Install progress section (shared across all platforms) -->
	{#if installLogs.length > 0}
		<div class="install-progress">
			<h3 class="progress-title">{t('ansible.install_progress')}</h3>
			<div class="log-container" bind:this={logContainer}>
				{#each installLogs as line, i (i)}
					<div class="log-line">{line}</div>
				{/each}
			</div>
			{#if installDone}
				<div class="progress-actions">
					<Button variant="primary" size="sm" onclick={handleContinue}>
						{t('ansible.continue')}
					</Button>
				</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	.toolchain-setup {
		padding: 16px;
		border-radius: var(--radius-btn);
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	/* Two-row checklist for WSL mode */
	.checklist {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.checklist-row {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.checklist-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.checklist-label {
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.checklist-label.missing {
		flex: 1;
		color: var(--color-text-secondary);
	}

	.checklist-detail {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.checklist-detail.detected {
		color: var(--color-text-secondary);
	}

	/* Status icons */
	.check-icon {
		flex-shrink: 0;
	}

	.check-icon.success {
		color: #4ade80;
	}

	.x-icon {
		flex-shrink: 0;
		color: #f87171;
	}

	/* Warning banner (no WSL on Windows) */
	.warning-banner {
		display: flex;
		align-items: flex-start;
		gap: 10px;
		padding: 10px 12px;
		border-radius: var(--radius-btn);
		background-color: rgba(251, 191, 36, 0.08);
		border: 1px solid rgba(251, 191, 36, 0.25);
		color: #fbbf24;
	}

	.warning-text {
		font-size: 0.8125rem;
		line-height: 1.5;
		color: var(--color-text-secondary);
	}

	/* Standard status row (Linux/macOS) */
	.status-row {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.status-row.installed {
		color: #4ade80;
	}

	.status-text {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.label {
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.version {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	/* Not installed row */
	.not-installed {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.warn-icon {
		flex-shrink: 0;
		color: var(--color-text-secondary);
	}

	.not-installed-text {
		flex: 1;
		font-size: 0.875rem;
		color: var(--color-text-secondary);
	}

	/* Install progress */
	.install-progress {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.progress-title {
		margin: 0;
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.log-container {
		max-height: 200px;
		overflow-y: auto;
		padding: 8px;
		border-radius: var(--radius-btn);
		background-color: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		font-family: monospace;
		font-size: 0.75rem;
		line-height: 1.5;
	}

	.log-line {
		color: var(--color-text-secondary);
		white-space: pre-wrap;
		word-break: break-all;
	}

	.progress-actions {
		display: flex;
		justify-content: flex-end;
	}
</style>
