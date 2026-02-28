<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { checkTool, isToolInstalled, getToolVersion } from '$lib/state/tofu.svelte';
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

		unlisten = await listen<ToolInstallEvent>('toolchain-install-tofu', (event) => {
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
			await toolchainInstall('tofu');
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
	{#if isToolInstalled()}
		<div class="status-row installed">
			<svg class="check-icon" width="20" height="20" viewBox="0 0 20 20" fill="none">
				<circle cx="10" cy="10" r="9" stroke="currentColor" stroke-width="1.5" />
				<path d="M6 10l3 3 5-6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
			</svg>
			<div class="status-text">
				<span class="label">{t('tofu.installed')}</span>
				{#if getToolVersion()}
					<span class="version">{t('tofu.version', { version: getToolVersion()! })}</span>
				{/if}
			</div>
		</div>
	{:else if installLogs.length > 0}
		<div class="install-progress">
			<h3 class="progress-title">{t('tofu.install_progress')}</h3>
			<div class="log-container" bind:this={logContainer}>
				{#each installLogs as line, i (i)}
					<div class="log-line">{line}</div>
				{/each}
			</div>
			{#if installDone}
				<div class="progress-actions">
					<Button variant="primary" size="sm" onclick={handleContinue}>
						{t('tofu.continue')}
					</Button>
				</div>
			{/if}
		</div>
	{:else}
		<div class="not-installed">
			<svg class="warn-icon" width="20" height="20" viewBox="0 0 20 20" fill="none">
				<circle cx="10" cy="10" r="9" stroke="currentColor" stroke-width="1.5" />
				<path d="M10 6v5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
				<circle cx="10" cy="14" r="1" fill="currentColor" />
			</svg>
			<span class="not-installed-text">{t('tofu.tool_not_installed')}</span>
			<Button
				variant="primary"
				size="sm"
				onclick={handleInstall}
				disabled={installing}
			>
				{installing ? t('tofu.installing') : t('tofu.install')}
			</Button>
		</div>
	{/if}
</div>

<style>
	.toolchain-setup {
		padding: 16px;
		border-radius: var(--radius-btn);
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
	}

	.status-row {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.status-row.installed {
		color: #4ade80;
	}

	.check-icon {
		flex-shrink: 0;
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
