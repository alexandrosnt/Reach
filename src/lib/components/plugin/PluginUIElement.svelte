<script lang="ts">
	import type { UiElement } from '$lib/ipc/plugin';
	import Self from './PluginUIElement.svelte';

	interface Props {
		element: UiElement;
		onAction: (action: string) => void;
	}

	let { element, onAction }: Props = $props();
</script>

{#if element.type === 'text'}
	<p class="ui-text" class:muted={element.muted}>{element.content}</p>

{:else if element.type === 'heading'}
	{#if (element.level ?? 2) === 2}
		<h2 class="ui-heading">{element.content}</h2>
	{:else if element.level === 3}
		<h3 class="ui-heading h3">{element.content}</h3>
	{:else}
		<h4 class="ui-heading h4">{element.content}</h4>
	{/if}

{:else if element.type === 'button'}
	<button
		class="ui-btn"
		class:primary={element.variant === 'primary'}
		class:danger={element.variant === 'danger'}
		onclick={() => onAction(element.action)}
	>
		{element.label}
	</button>

{:else if element.type === 'input'}
	<label class="ui-field">
		<span class="ui-label">{element.label}</span>
		<input
			type="text"
			class="ui-input"
			value={element.value ?? ''}
			placeholder={element.placeholder ?? ''}
		/>
	</label>

{:else if element.type === 'toggle'}
	<label class="ui-toggle">
		<input type="checkbox" checked={element.checked ?? false} />
		<span class="toggle-track"></span>
		<span class="ui-label">{element.label}</span>
	</label>

{:else if element.type === 'select'}
	<label class="ui-field">
		<span class="ui-label">{element.label}</span>
		<select class="ui-select" value={element.selected ?? ''}>
			{#each element.options as option (option)}
				<option value={option}>{option}</option>
			{/each}
		</select>
	</label>

{:else if element.type === 'table'}
	<div class="ui-table-wrap">
		<table class="ui-table">
			<thead>
				<tr>
					{#each element.headers as header, i (i)}
						<th>{header}</th>
					{/each}
				</tr>
			</thead>
			<tbody>
				{#each element.rows as row, ri (ri)}
					<tr>
						{#each row as cell, ci (ci)}
							<td>{cell}</td>
						{/each}
					</tr>
				{/each}
			</tbody>
		</table>
	</div>

{:else if element.type === 'code'}
	<pre class="ui-code"><code>{element.content}</code></pre>

{:else if element.type === 'divider'}
	<hr class="ui-divider" />

{:else if element.type === 'spacer'}
	<div class="ui-spacer"></div>

{:else if element.type === 'row'}
	<div class="ui-row">
		{#each element.children as child, i (i)}
			<Self element={child} {onAction} />
		{/each}
	</div>

{:else if element.type === 'column'}
	<div class="ui-column">
		{#each element.children as child, i (i)}
			<Self element={child} {onAction} />
		{/each}
	</div>

{:else if element.type === 'alert'}
	<div
		class="ui-alert"
		class:info={!element.level || element.level === 'info'}
		class:warning={element.level === 'warning'}
		class:error={element.level === 'error'}
		class:success={element.level === 'success'}
	>
		{element.content}
	</div>

{:else if element.type === 'progress'}
	<div class="ui-progress">
		{#if element.label}
			<span class="ui-label">{element.label}</span>
		{/if}
		<div class="progress-track">
			<div class="progress-fill" style="width: {Math.min(100, Math.max(0, element.value))}%"></div>
		</div>
	</div>
{/if}

<style>
	/* Text */
	.ui-text {
		margin: 0;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		line-height: 1.5;
		color: var(--color-text-primary);
	}

	.ui-text.muted {
		color: var(--color-text-secondary);
	}

	/* Headings */
	.ui-heading {
		margin: 0 0 4px;
		font-family: var(--font-sans);
		font-size: 1rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.ui-heading.h3 {
		font-size: 0.9375rem;
	}

	.ui-heading.h4 {
		font-size: 0.8125rem;
		font-weight: 500;
	}

	/* Button */
	.ui-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 6px 14px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background: var(--color-bg-elevated);
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: 0.75rem;
		cursor: pointer;
		transition:
			background-color var(--duration-default) var(--ease-default),
			border-color var(--duration-default) var(--ease-default);
	}

	.ui-btn:hover {
		background: rgba(255, 255, 255, 0.08);
	}

	.ui-btn.primary {
		background: var(--color-accent);
		border-color: var(--color-accent);
		color: #fff;
	}

	.ui-btn.primary:hover {
		filter: brightness(1.1);
	}

	.ui-btn.danger {
		background: var(--color-danger);
		border-color: var(--color-danger);
		color: #fff;
	}

	.ui-btn.danger:hover {
		filter: brightness(1.1);
	}

	/* Input */
	.ui-field {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.ui-label {
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.ui-input {
		padding: 6px 10px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background: var(--color-bg-secondary);
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		outline: none;
		transition: border-color var(--duration-default) var(--ease-default);
	}

	.ui-input:focus {
		border-color: var(--color-accent);
	}

	/* Toggle */
	.ui-toggle {
		display: flex;
		align-items: center;
		gap: 8px;
		cursor: pointer;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
	}

	.ui-toggle input {
		position: absolute;
		opacity: 0;
		width: 0;
		height: 0;
	}

	.toggle-track {
		position: relative;
		display: inline-block;
		width: 32px;
		height: 18px;
		background: rgba(255, 255, 255, 0.1);
		border-radius: 9px;
		flex-shrink: 0;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.toggle-track::after {
		content: '';
		position: absolute;
		top: 3px;
		left: 3px;
		width: 12px;
		height: 12px;
		border-radius: 50%;
		background: var(--color-text-secondary);
		transition:
			transform var(--duration-default) var(--ease-default),
			background-color var(--duration-default) var(--ease-default);
	}

	.ui-toggle input:checked + .toggle-track {
		background: var(--color-accent);
	}

	.ui-toggle input:checked + .toggle-track::after {
		transform: translateX(14px);
		background: #fff;
	}

	/* Select */
	.ui-select {
		padding: 6px 10px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background: var(--color-bg-secondary);
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		outline: none;
		cursor: pointer;
		transition: border-color var(--duration-default) var(--ease-default);
	}

	.ui-select:focus {
		border-color: var(--color-accent);
	}

	/* Table */
	.ui-table-wrap {
		overflow-x: auto;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
	}

	.ui-table {
		width: 100%;
		border-collapse: collapse;
		font-family: var(--font-sans);
		font-size: 0.75rem;
	}

	.ui-table th {
		padding: 6px 10px;
		text-align: left;
		font-weight: 600;
		color: var(--color-text-secondary);
		background: var(--color-bg-secondary);
		border-bottom: 1px solid var(--color-border);
	}

	.ui-table td {
		padding: 6px 10px;
		color: var(--color-text-primary);
		border-bottom: 1px solid var(--color-border);
	}

	.ui-table tbody tr:last-child td {
		border-bottom: none;
	}

	/* Code */
	.ui-code {
		margin: 0;
		padding: 10px 12px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background: var(--color-bg-secondary);
		color: var(--color-text-primary);
		font-family: var(--font-mono);
		font-size: 0.75rem;
		line-height: 1.5;
		overflow-x: auto;
		white-space: pre;
	}

	/* Divider */
	.ui-divider {
		border: none;
		border-top: 1px solid var(--color-border);
		margin: 4px 0;
	}

	/* Spacer */
	.ui-spacer {
		height: 12px;
	}

	/* Row / Column */
	.ui-row {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-start;
		gap: 8px;
	}

	.ui-column {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	/* Alert */
	.ui-alert {
		padding: 8px 12px;
		border-radius: var(--radius-btn);
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		line-height: 1.4;
	}

	.ui-alert.info {
		background: rgba(59, 130, 246, 0.12);
		border: 1px solid rgba(59, 130, 246, 0.3);
		color: var(--color-accent);
	}

	.ui-alert.warning {
		background: rgba(245, 158, 11, 0.12);
		border: 1px solid rgba(245, 158, 11, 0.3);
		color: var(--color-warning);
	}

	.ui-alert.error {
		background: rgba(239, 68, 68, 0.12);
		border: 1px solid rgba(239, 68, 68, 0.3);
		color: var(--color-danger);
	}

	.ui-alert.success {
		background: rgba(34, 197, 94, 0.12);
		border: 1px solid rgba(34, 197, 94, 0.3);
		color: var(--color-success);
	}

	/* Progress */
	.ui-progress {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.progress-track {
		height: 6px;
		border-radius: 3px;
		background: var(--color-bg-secondary);
		overflow: hidden;
	}

	.progress-fill {
		height: 100%;
		border-radius: 3px;
		background: var(--color-accent);
		transition: width var(--duration-default) var(--ease-default);
	}
</style>
