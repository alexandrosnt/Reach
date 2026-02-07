<script lang="ts">
	import Modal from '$lib/components/shared/Modal.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import {
		playbookRun,
		playbookStop,
		type PlaybookRun,
		type PlaybookStepEvent
	} from '$lib/ipc/playbook';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { sendMessage, openAIPanel } from '$lib/state/ai-chat.svelte';
	import { getAISettings } from '$lib/state/ai.svelte';

	interface Props {
		open: boolean;
		onclose: () => void;
		connectionId: string;
		yaml?: string;
	}

	let { open, onclose, connectionId, yaml = '' }: Props = $props();

	let yamlInput = $state('');
	let currentRun = $state<PlaybookRun | null>(null);
	let steps = $state<PlaybookStepEvent[]>([]);
	let running = $state(false);
	let finished = $state(false);
	let expandedSteps = $state<Set<number>>(new Set());
	let unlistenStep: UnlistenFn | null = null;
	let unlistenComplete: UnlistenFn | null = null;

	$effect(() => {
		if (open) {
			yamlInput = yaml || '';
			currentRun = null;
			steps = [];
			running = false;
			finished = false;
			expandedSteps = new Set();
		}
	});

	async function handleRun(): Promise<void> {
		if (!yamlInput.trim()) {
			addToast(t('playbook.enter_yaml'), 'error');
			return;
		}

		if (!connectionId) {
			addToast(t('playbook.no_connection'), 'error');
			return;
		}

		try {
			running = true;
			finished = false;
			steps = [];
			expandedSteps = new Set();

			const run = await playbookRun(yamlInput, connectionId);
			currentRun = run;

			unlistenStep = await listen<PlaybookStepEvent>(
				`playbook-step-${run.id}`,
				(event) => {
					const stepEvent = event.payload;
					const existingIndex = steps.findIndex(
						(s) => s.step_index === stepEvent.step_index
					);
					if (existingIndex >= 0) {
						steps[existingIndex] = stepEvent;
					} else {
						steps.push(stepEvent);
					}
					expandedSteps.add(stepEvent.step_index);
					expandedSteps = expandedSteps;
				}
			);

			unlistenComplete = await listen<PlaybookRun>(
				`playbook-complete-${run.id}`,
				(event) => {
					currentRun = event.payload;
					running = false;
					finished = true;
					addToast(
						`Playbook "${event.payload.playbook_name}" ${event.payload.status.toLowerCase()}`,
						event.payload.status === 'Completed' ? 'success' : 'error'
					);
				}
			);
		} catch (err) {
			running = false;
			addToast(`Failed to run playbook: ${err}`, 'error');
		}
	}

	async function handleStop(): Promise<void> {
		if (!currentRun) return;

		try {
			await playbookStop(currentRun.id);
			running = false;
			finished = true;
			addToast(t('playbook.stopped_toast'), 'warning');
		} catch (err) {
			addToast(`Failed to stop playbook: ${err}`, 'error');
		}
	}

	function toggleStep(index: number): void {
		if (expandedSteps.has(index)) {
			expandedSteps.delete(index);
		} else {
			expandedSteps.add(index);
		}
		expandedSteps = expandedSteps;
	}

	function cleanup(): void {
		unlistenStep?.();
		unlistenComplete?.();
		unlistenStep = null;
		unlistenComplete = null;
	}

	function handleClose(): void {
		cleanup();
		onclose();
	}

	function stepStatusIcon(status: PlaybookStepEvent['status']): string {
		switch (status) {
			case 'running':
				return 'spinner';
			case 'completed':
				return 'check';
			case 'failed':
				return 'x';
			default:
				return 'pending';
		}
	}

	function stepStatusColor(status: PlaybookStepEvent['status']): string {
		switch (status) {
			case 'running':
				return 'var(--color-accent)';
			case 'completed':
				return 'var(--color-success)';
			case 'failed':
				return 'var(--color-danger)';
			default:
				return 'var(--color-text-secondary)';
		}
	}

	let summaryLabel = $derived(
		currentRun
			? `${currentRun.playbook_name} - ${currentRun.status}`
			: t('playbook.runner_title')
	);

	let completedCount = $derived(
		steps.filter((s) => s.status === 'completed').length
	);

	let failedCount = $derived(
		steps.filter((s) => s.status === 'failed').length
	);

	let aiSettings = $derived(getAISettings());
	let aiEnabled = $derived(aiSettings.enabled && aiSettings.apiKey && aiSettings.selectedModel);

	function handleAnalyzeWithAI(): void {
		if (!currentRun || steps.length === 0) return;

		const lines: string[] = [
			`Analyze the following playbook execution results for "${currentRun.playbook_name}" (status: ${currentRun.status}).`,
			''
		];

		for (const step of steps) {
			lines.push(`## Step ${step.step_index + 1}: ${step.step_name} [${step.status.toUpperCase()}]`);
			if (step.output.trim()) {
				lines.push('```');
				lines.push(step.output.trim());
				lines.push('```');
			} else {
				lines.push('(no output)');
			}
			lines.push('');
		}

		lines.push(`Summary: ${completedCount} completed, ${failedCount} failed out of ${steps.length} total steps.`);
		lines.push('');
		lines.push('Please analyze the results, identify any issues or errors, and suggest fixes if needed.');

		openAIPanel();
		sendMessage(lines.join('\n'));
		handleClose();
	}
</script>

<Modal open={open} onclose={handleClose} title={t('playbook.runner_title')}>
	{#snippet children()}
		<div class="runner-container">
			{#if !running && !finished}
				<div class="yaml-section">
					<label class="section-label" for="runner-yaml">{t('playbook.yaml_definition')}</label>
					<textarea
						id="runner-yaml"
						class="yaml-input"
						bind:value={yamlInput}
						placeholder={t('playbook.yaml_placeholder')}
						spellcheck="false"
					></textarea>
				</div>
			{/if}

			{#if running || finished}
				<div class="execution-section">
					{#if currentRun}
						<div class="run-header">
							<span class="run-title">{currentRun.playbook_name}</span>
							<span
								class="run-status"
								style:color={
									currentRun.status === 'Completed'
										? 'var(--color-success)'
										: currentRun.status === 'Failed'
											? 'var(--color-danger)'
											: currentRun.status === 'Running'
												? 'var(--color-accent)'
												: 'var(--color-warning)'
								}
							>
								{currentRun.status}
							</span>
						</div>

						{#if running}
							<div class="progress-bar">
								<div
									class="progress-fill"
									style:width="{currentRun.total_steps > 0
										? (currentRun.current_step / currentRun.total_steps) * 100
										: 0}%"
								></div>
							</div>
						{/if}
					{/if}

					<div class="steps-list">
						{#each steps as step (step.step_index)}
							<div class="step-item">
								<button
									class="step-header"
									onclick={() => toggleStep(step.step_index)}
								>
									<span class="step-icon" style:color={stepStatusColor(step.status)}>
										{#if step.status === 'running'}
											<span class="step-spinner"></span>
										{:else if step.status === 'completed'}
											<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
												<path d="M20 6L9 17l-5-5" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
											</svg>
										{:else if step.status === 'failed'}
											<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
												<path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
											</svg>
										{/if}
									</span>
									<span class="step-name">{step.step_name}</span>
									<svg
										class="step-chevron"
										class:expanded={expandedSteps.has(step.step_index)}
										width="12"
										height="12"
										viewBox="0 0 24 24"
										fill="none"
									>
										<path d="M6 9l6 6 6-6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
									</svg>
								</button>

								{#if expandedSteps.has(step.step_index) && step.output}
									<div class="step-output">
										<pre>{step.output}</pre>
									</div>
								{/if}
							</div>
						{/each}
					</div>

					{#if finished}
						<div class="summary">
							<span class="summary-label">{t('playbook.summary')}</span>
							<span class="summary-completed">{t('playbook.completed', { count: String(completedCount) })}</span>
							{#if failedCount > 0}
								<span class="summary-failed">{t('playbook.failed', { count: String(failedCount) })}</span>
							{/if}
						</div>
					{/if}
				</div>
			{/if}
		</div>
	{/snippet}

	{#snippet actions()}
		<Button variant="ghost" onclick={handleClose}>{t('common.close')}</Button>
		{#if finished && aiEnabled}
			<Button variant="secondary" onclick={handleAnalyzeWithAI}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
					<polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
				</svg>
				{t('playbook.analyze_ai')}
			</Button>
		{/if}
		{#if running}
			<Button variant="danger" onclick={handleStop}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
					<rect x="6" y="6" width="12" height="12" rx="1" />
				</svg>
				{t('playbook.stop')}
			</Button>
		{:else if !finished}
			<Button variant="primary" onclick={handleRun} disabled={!yamlInput.trim()}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
					<path d="M5 3l14 9-14 9V3z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
				{t('playbook.run_short')}
			</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	.runner-container {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.yaml-section {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.section-label {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
	}

	.yaml-input {
		width: 100%;
		min-height: 200px;
		padding: 14px;
		font-family: var(--font-mono, monospace);
		font-size: 0.8125rem;
		line-height: 1.6;
		color: var(--color-text-primary);
		background-color: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		resize: vertical;
		tab-size: 2;
		white-space: pre;
		overflow-wrap: normal;
		overflow-x: auto;
		box-sizing: border-box;
		transition: border-color var(--duration-default) var(--ease-default);
	}

	.yaml-input:focus {
		border-color: var(--color-accent);
	}

	.yaml-input::placeholder {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.execution-section {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.run-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.run-title {
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.run-status {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.progress-bar {
		height: 3px;
		background-color: rgba(255, 255, 255, 0.06);
		border-radius: 2px;
		overflow: hidden;
	}

	.progress-fill {
		height: 100%;
		background-color: var(--color-accent);
		border-radius: 2px;
		transition: width 0.3s var(--ease-default);
	}

	.steps-list {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.step-item {
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		overflow: hidden;
	}

	.step-header {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 8px 10px;
		border: none;
		background: transparent;
		color: inherit;
		cursor: pointer;
		font-family: var(--font-sans);
		text-align: left;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.step-header:hover {
		background-color: rgba(255, 255, 255, 0.03);
	}

	.step-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 16px;
		flex-shrink: 0;
	}

	.step-spinner {
		display: inline-block;
		width: 12px;
		height: 12px;
		border: 2px solid rgba(255, 255, 255, 0.15);
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	.step-name {
		flex: 1;
		font-size: 0.8125rem;
		color: var(--color-text-primary);
	}

	.step-chevron {
		flex-shrink: 0;
		color: var(--color-text-secondary);
		transition: transform var(--duration-default) var(--ease-default);
	}

	.step-chevron.expanded {
		transform: rotate(180deg);
	}

	.step-output {
		border-top: 1px solid var(--color-border);
		background-color: var(--color-bg-primary);
	}

	.step-output pre {
		margin: 0;
		padding: 10px 12px;
		font-family: var(--font-mono, monospace);
		font-size: 0.75rem;
		line-height: 1.5;
		color: var(--color-text-secondary);
		white-space: pre-wrap;
		word-break: break-all;
		overflow-x: auto;
	}

	.summary {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 10px 12px;
		background-color: rgba(255, 255, 255, 0.03);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
	}

	.summary-label {
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--color-text-secondary);
	}

	.summary-completed {
		font-size: 0.75rem;
		color: var(--color-success);
	}

	.summary-failed {
		font-size: 0.75rem;
		color: var(--color-danger);
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
