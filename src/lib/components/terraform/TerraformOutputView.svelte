<script lang="ts">
	import {
		getOutputs,
		setOutputs,
		getWorkingDir,
		getExecMode,
		getSelectedConnectionId
	} from '$lib/state/terraform.svelte';
	import { terraformOutput } from '$lib/ipc/terraform';
	import { t } from '$lib/state/i18n.svelte';
	import { addToast } from '$lib/state/toasts.svelte';

	let loading = $state(false);

	let outputs = $derived(getOutputs());
	let outputEntries = $derived(Object.entries(outputs));

	async function refreshOutputs(): Promise<void> {
		const dir = getWorkingDir();
		if (!dir) {
			addToast(t('terraform.output_error'), 'error');
			return;
		}

		loading = true;
		try {
			const result = await terraformOutput(
				dir,
				getExecMode(),
				getSelectedConnectionId() ?? undefined
			);
			setOutputs(result);
		} catch (err) {
			addToast(t('terraform.output_error') + ': ' + String(err), 'error');
		} finally {
			loading = false;
		}
	}

	function formatValue(val: unknown): string {
		if (val === null || val === undefined) return '';
		if (typeof val === 'string') return val;
		return JSON.stringify(val, null, 2);
	}
</script>

<div class="output-view">
	<div class="toolbar">
		<button class="refresh-btn" onclick={refreshOutputs} disabled={loading}>
			{#if loading}
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
			{t('terraform.refresh_outputs')}
		</button>
	</div>

	<div class="output-list">
		{#if loading}
			<div class="empty-state">{t('terraform.loading')}</div>
		{:else if outputEntries.length === 0}
			<div class="empty-state">{t('terraform.no_outputs')}</div>
		{:else}
			{#each outputEntries as [key, entry] (key)}
				{@const output = entry as Record<string, unknown>}
				{@const isSensitive = output.sensitive === true}
				<div class="output-entry">
					<div class="output-header">
						<span class="output-name">{key}</span>
						{#if output.type}
							<span class="output-type">{output.type}</span>
						{/if}
					</div>
					<div class="output-value" class:sensitive={isSensitive}>
						{#if isSensitive}
							{t('terraform.sensitive_value')}
						{:else}
							<pre class="value-pre">{formatValue(output.value)}</pre>
						{/if}
					</div>
				</div>
			{/each}
		{/if}
	</div>
</div>

<style>
	.output-view {
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

	.output-list {
		flex: 1;
		overflow-y: auto;
		padding: 6px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.empty-state {
		padding: 16px 8px;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		text-align: center;
		opacity: 0.6;
	}

	.output-entry {
		padding: 6px 10px;
		background-color: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
	}

	.output-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		margin-bottom: 3px;
	}

	.output-name {
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--color-text-primary);
		font-family: var(--font-mono, 'JetBrains Mono', monospace);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}

	.output-type {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		font-family: var(--font-mono, 'JetBrains Mono', monospace);
		white-space: nowrap;
		flex-shrink: 0;
		opacity: 0.7;
	}

	.output-value {
		font-size: 0.75rem;
		color: var(--color-text-primary);
	}

	.output-value.sensitive {
		font-size: 0.6875rem;
		font-style: italic;
		color: var(--color-text-secondary);
		opacity: 0.7;
	}

	.value-pre {
		margin: 0;
		font-family: var(--font-mono, 'JetBrains Mono', monospace);
		font-size: 0.75rem;
		line-height: 1.4;
		color: var(--color-accent);
		white-space: pre-wrap;
		word-break: break-all;
	}
</style>
