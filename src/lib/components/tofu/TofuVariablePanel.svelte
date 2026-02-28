<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import { getActiveVariables, saveVariables } from '$lib/state/tofu.svelte';
	import type { TofuVariable } from '$lib/ipc/tofu';
	import Button from '$lib/components/shared/Button.svelte';
	import TofuVariableEditor from './TofuVariableEditor.svelte';

	let variables = $derived(getActiveVariables());

	let showEditor = $state(false);
	let editingVariable = $state<TofuVariable | null>(null);

	function handleAdd() {
		editingVariable = null;
		showEditor = true;
	}

	function handleEdit(variable: TofuVariable) {
		editingVariable = variable;
		showEditor = true;
	}

	async function handleDelete(variableName: string) {
		const newList = variables.filter((v) => v.name !== variableName);
		await saveVariables(newList);
	}

	async function handleSave(variable: TofuVariable) {
		let newList: TofuVariable[];
		if (editingVariable) {
			newList = variables.map((v) => (v.name === editingVariable!.name ? variable : v));
		} else {
			newList = [...variables, variable];
		}
		await saveVariables(newList);
		showEditor = false;
		editingVariable = null;
	}

	function handleClose() {
		showEditor = false;
		editingVariable = null;
	}
</script>

<div class="variable-panel">
	<div class="panel-header">
		<Button variant="primary" size="sm" onclick={handleAdd}>
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
				<path
					d="M12 5v14M5 12h14"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
			{t('tofu.add_variable')}
		</Button>
	</div>

	{#if variables.length === 0}
		<div class="empty-state">
			<p class="empty-message">{t('tofu.no_variables')}</p>
		</div>
	{:else}
		<div class="table-wrapper">
			<table class="variable-table">
				<thead>
					<tr>
						<th class="col-name">{t('tofu.var_name')}</th>
						<th class="col-type">{t('tofu.var_type')}</th>
						<th class="col-default">{t('tofu.var_default')}</th>
						<th class="col-sensitive">{t('tofu.var_sensitive')}</th>
						<th class="col-desc">{t('tofu.var_description')}</th>
						<th class="col-actions">{t('tofu.var_actions')}</th>
					</tr>
				</thead>
				<tbody>
					{#each variables as variable (variable.name)}
						<tr class="variable-row">
							<td class="cell-name">
								<span class="mono">{variable.name}</span>
							</td>
							<td class="cell-type">
								<span class="type-badge">{variable.varType}</span>
							</td>
							<td class="cell-default">
								{#if variable.defaultValue !== null}
									{#if variable.sensitive}
										<span class="masked-value">********</span>
									{:else}
										<span class="default-value">{variable.defaultValue}</span>
									{/if}
								{:else}
									<span class="no-value">--</span>
								{/if}
							</td>
							<td class="cell-sensitive">
								{#if variable.sensitive}
									<span class="sensitive-badge">
										<svg width="12" height="12" viewBox="0 0 24 24" fill="none">
											<rect x="3" y="11" width="18" height="11" rx="2" stroke="currentColor" stroke-width="1.5" />
											<path d="M7 11V7a5 5 0 0110 0v4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
										</svg>
										{t('tofu.sensitive_yes')}
									</span>
								{:else}
									<span class="not-sensitive">--</span>
								{/if}
							</td>
							<td class="cell-desc">
								{#if variable.description}
									<span class="description-text">{variable.description}</span>
								{:else}
									<span class="no-value">--</span>
								{/if}
							</td>
							<td class="cell-actions">
								<button
									type="button"
									class="action-btn edit-btn"
									title={t('tofu.edit_variable')}
									onclick={() => handleEdit(variable)}
								>
									<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
										<path
											d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"
											stroke="currentColor"
											stroke-width="1.5"
											stroke-linecap="round"
											stroke-linejoin="round"
										/>
										<path
											d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"
											stroke="currentColor"
											stroke-width="1.5"
											stroke-linecap="round"
											stroke-linejoin="round"
										/>
									</svg>
								</button>
								<button
									type="button"
									class="action-btn delete-btn"
									title={t('tofu.delete_variable')}
									onclick={() => handleDelete(variable.name)}
								>
									<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
										<path
											d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2m3 0v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6h14z"
											stroke="currentColor"
											stroke-width="1.5"
											stroke-linecap="round"
											stroke-linejoin="round"
										/>
									</svg>
								</button>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</div>

{#if showEditor}
	<TofuVariableEditor
		variable={editingVariable}
		existingNames={variables.map((v) => v.name)}
		onsave={handleSave}
		onclose={handleClose}
	/>
{/if}

<style>
	.variable-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	.panel-header {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		padding: 12px 16px;
		border-bottom: 1px solid var(--color-border);
	}

	/* Empty state */
	.empty-state {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 32px 16px;
	}

	.empty-message {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		font-style: italic;
		opacity: 0.6;
	}

	/* Table */
	.table-wrapper {
		flex: 1;
		overflow: auto;
	}

	.variable-table {
		width: 100%;
		min-width: 700px;
		border-collapse: collapse;
		font-size: 0.8125rem;
	}

	.variable-table thead {
		position: sticky;
		top: 0;
		z-index: 1;
	}

	.variable-table th {
		padding: 10px 12px;
		text-align: left;
		font-size: 0.6875rem;
		font-weight: 600;
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		background: var(--color-bg-elevated);
		border-bottom: 1px solid var(--color-border);
		white-space: nowrap;
	}

	.variable-table td {
		padding: 10px 12px;
		color: var(--color-text-primary);
		border-bottom: 1px solid var(--color-border);
		vertical-align: middle;
	}

	.variable-row {
		transition: background-color 0.12s ease;
	}

	.variable-row:hover {
		background: rgba(255, 255, 255, 0.03);
	}

	/* Column widths */
	.col-name {
		width: 18%;
	}
	.col-type {
		width: 10%;
	}
	.col-default {
		width: 18%;
	}
	.col-sensitive {
		width: 10%;
	}
	.col-desc {
		width: 32%;
	}
	.col-actions {
		width: 12%;
		text-align: right !important;
	}

	/* Cell content */
	.mono {
		font-family: monospace;
		font-size: 0.8125rem;
		color: var(--color-accent);
	}

	.type-badge {
		display: inline-block;
		padding: 2px 8px;
		font-size: 0.6875rem;
		font-weight: 600;
		color: var(--color-text-primary);
		background: rgba(255, 255, 255, 0.08);
		border-radius: 9999px;
		text-transform: lowercase;
	}

	.default-value {
		font-family: monospace;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		word-break: break-all;
	}

	.masked-value {
		font-family: monospace;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		opacity: 0.5;
		letter-spacing: 0.05em;
	}

	.no-value {
		color: var(--color-text-secondary);
		opacity: 0.4;
	}

	.sensitive-badge {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 2px 8px;
		font-size: 0.6875rem;
		font-weight: 600;
		color: #f59e0b;
		background: rgba(245, 158, 11, 0.1);
		border-radius: 9999px;
	}

	.not-sensitive {
		color: var(--color-text-secondary);
		opacity: 0.4;
	}

	.description-text {
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		display: -webkit-box;
		-webkit-line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}

	/* Action buttons */
	.cell-actions {
		text-align: right;
		white-space: nowrap;
	}

	.action-btn {
		display: inline-flex;
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
		transition:
			color 0.12s ease,
			background-color 0.12s ease;
	}

	.action-btn:hover {
		background: rgba(255, 255, 255, 0.06);
	}

	.edit-btn:hover {
		color: var(--color-accent);
	}

	.delete-btn:hover {
		color: var(--color-danger);
	}
</style>
