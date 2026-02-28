<script lang="ts">
	import type { TofuExecutionTarget, TofuOutput } from '$lib/ipc/tofu';
	import { t } from '$lib/state/i18n.svelte';
	import {
		getActiveOutputs,
		saveOutputs,
		getOutputValues,
		isOutputsLoading,
		loadOutputValues
	} from '$lib/state/tofu.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import TofuOutputEditor from './TofuOutputEditor.svelte';

	interface Props {
		target: TofuExecutionTarget;
	}

	let { target }: Props = $props();

	let showEditor = $state(false);
	let editingOutput = $state<TofuOutput | null>(null);

	let outputs = $derived(getActiveOutputs());
	let liveValues = $derived(getOutputValues());
	let loadingValues = $derived(isOutputsLoading());
	let existingNames = $derived(outputs.map((o) => o.name));

	function handleAdd() {
		editingOutput = null;
		showEditor = true;
	}

	function handleEdit(output: TofuOutput) {
		editingOutput = output;
		showEditor = true;
	}

	function handleDelete(name: string) {
		const updated = outputs.filter((o) => o.name !== name);
		saveOutputs(updated);
	}

	function handleEditorClose() {
		showEditor = false;
		editingOutput = null;
	}

	function handleEditorSave(output: TofuOutput) {
		let updated: TofuOutput[];
		if (editingOutput) {
			updated = outputs.map((o) => (o.name === editingOutput!.name ? output : o));
		} else {
			updated = [...outputs, output];
		}
		saveOutputs(updated);
		showEditor = false;
		editingOutput = null;
	}

	function handleRefreshValues() {
		loadOutputValues(target);
	}

	function formatValue(value: unknown): string {
		if (typeof value === 'string') return value;
		return JSON.stringify(value);
	}
</script>

<div class="output-panel">
	<!-- Section 1: Output Definitions -->
	<section class="section">
		<header class="header">
			<h2 class="title">{t('tofu.outputs_title')}</h2>
			<div class="header-actions">
				<Button variant="primary" size="sm" onclick={handleAdd}>
					{t('tofu.add_output')}
				</Button>
			</div>
		</header>

		{#if outputs.length === 0}
			<div class="empty-state">
				<svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="empty-icon">
					<path
						d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8l-6-6z"
						stroke="currentColor"
						stroke-width="1.5"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
					<path
						d="M14 2v6h6M16 13H8M16 17H8M10 9H8"
						stroke="currentColor"
						stroke-width="1.5"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
				<p class="empty-text">{t('tofu.no_outputs')}</p>
			</div>
		{:else}
			<div class="output-list">
				{#each outputs as output (output.name)}
					<div class="output-item">
						<div class="output-info">
							<div class="output-name-row">
								<span class="output-name">{output.name}</span>
								{#if output.sensitive}
									<span class="badge sensitive">{t('tofu.output_sensitive')}</span>
								{/if}
							</div>
							<span class="output-value-expr">{output.value}</span>
							{#if output.description}
								<span class="output-description">{output.description}</span>
							{/if}
						</div>
						<div class="output-actions">
							<button type="button" class="action-btn edit" onclick={() => handleEdit(output)} title={t('common.edit')}>
								<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
									<path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
									<path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
								</svg>
							</button>
							<button type="button" class="action-btn remove" onclick={() => handleDelete(output.name)} title={t('common.delete')}>
								<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
									<path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
								</svg>
							</button>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</section>

	<!-- Section 2: Live Values -->
	<section class="section">
		<header class="header">
			<h2 class="title">{t('tofu.live_values')}</h2>
			<div class="header-actions">
				<Button variant="secondary" size="sm" onclick={handleRefreshValues} disabled={loadingValues}>
					{t('tofu.state_refresh')}
				</Button>
			</div>
		</header>

		{#if loadingValues}
			<div class="loading-state">
				<div class="spinner"></div>
				<p class="loading-text">{t('tofu.state_loading')}</p>
			</div>
		{:else if liveValues.length === 0}
			<div class="empty-state">
				<svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="empty-icon">
					<path
						d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"
						stroke="currentColor"
						stroke-width="1.5"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
					<polyline
						points="7 10 12 15 17 10"
						stroke="currentColor"
						stroke-width="1.5"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
					<line
						x1="12" y1="15" x2="12" y2="3"
						stroke="currentColor"
						stroke-width="1.5"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
				<p class="empty-text">{t('tofu.live_values_empty')}</p>
			</div>
		{:else}
			<div class="values-list">
				{#each liveValues as val (val.name)}
					<div class="value-item">
						<div class="value-name">{val.name}</div>
						<div class="value-type">{val.outputType}</div>
						<div class="value-content">
							{#if val.sensitive}
								<span class="sensitive-placeholder">[sensitive]</span>
							{:else}
								<code class="value-code">{formatValue(val.value)}</code>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</section>
</div>

<TofuOutputEditor
	open={showEditor}
	output={editingOutput}
	{existingNames}
	onclose={handleEditorClose}
	onsave={handleEditorSave}
/>

<style>
	.output-panel {
		width: 100%;
		height: 100%;
		overflow-y: auto;
		background: var(--color-bg-primary);
		padding: 24px;
		display: flex;
		flex-direction: column;
		gap: 32px;
	}

	.section {
		display: flex;
		flex-direction: column;
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

	/* Output Definitions List */
	.output-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.output-item {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 12px;
		padding: 12px 16px;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		transition: border-color 0.12s ease;
	}

	.output-item:hover {
		border-color: color-mix(in srgb, var(--color-border) 70%, var(--color-accent));
	}

	.output-info {
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 0;
		overflow: hidden;
	}

	.output-name-row {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.output-name {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
		font-family: monospace;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.badge.sensitive {
		display: inline-flex;
		align-items: center;
		padding: 1px 6px;
		font-size: 0.625rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		background: rgba(239, 68, 68, 0.12);
		color: var(--color-danger, #ef4444);
		border-radius: 4px;
		white-space: nowrap;
	}

	.output-value-expr {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		font-family: monospace;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.output-description {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		opacity: 0.8;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.output-actions {
		display: flex;
		gap: 4px;
		flex-shrink: 0;
		margin-top: 2px;
	}

	.action-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 4px;
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition: background-color 0.12s ease, color 0.12s ease, border-color 0.12s ease;
	}

	.action-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.action-btn.edit {
		color: var(--color-accent);
	}

	.action-btn.edit:hover:not(:disabled) {
		background: rgba(255, 255, 255, 0.04);
		border-color: var(--color-accent);
	}

	.action-btn.remove {
		color: var(--color-danger, #ef4444);
	}

	.action-btn.remove:hover:not(:disabled) {
		background: rgba(239, 68, 68, 0.08);
		border-color: var(--color-danger, #ef4444);
	}

	/* Live Values List */
	.values-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.value-item {
		display: flex;
		align-items: center;
		gap: 16px;
		padding: 10px 16px;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		transition: border-color 0.12s ease;
	}

	.value-item:hover {
		border-color: color-mix(in srgb, var(--color-border) 70%, var(--color-accent));
	}

	.value-name {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
		font-family: monospace;
		white-space: nowrap;
		min-width: 120px;
	}

	.value-type {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		background: rgba(255, 255, 255, 0.04);
		padding: 2px 8px;
		border-radius: 4px;
		white-space: nowrap;
		flex-shrink: 0;
	}

	.value-content {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		text-align: right;
	}

	.value-code {
		font-size: 0.75rem;
		font-family: monospace;
		color: var(--color-text-primary);
		background: rgba(255, 255, 255, 0.03);
		padding: 2px 6px;
		border-radius: 3px;
	}

	.sensitive-placeholder {
		font-size: 0.75rem;
		font-style: italic;
		color: var(--color-text-secondary);
		opacity: 0.6;
	}

	@media (max-width: 700px) {
		.value-item {
			flex-wrap: wrap;
			gap: 8px;
			padding: 10px 12px;
		}

		.value-name {
			min-width: unset;
			width: 100%;
		}

		.value-content {
			text-align: left;
			width: 100%;
		}
	}
</style>
