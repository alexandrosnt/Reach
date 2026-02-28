<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/state/i18n.svelte';
	import {
		getStateResources,
		isStateLoading,
		loadStateResources,
		stateShowResource,
		stateRemoveResource,
		stateMoveResource,
		stateImportResource,
		isCommandRunning
	} from '$lib/state/tofu.svelte';
	import type { TofuExecutionTarget } from '$lib/ipc/tofu';
	import Button from '$lib/components/shared/Button.svelte';
	import TofuStateImportModal from './TofuStateImportModal.svelte';

	interface Props {
		target: TofuExecutionTarget;
	}

	let { target }: Props = $props();

	let resources = $derived(getStateResources());
	let loading = $derived(isStateLoading());
	let running = $derived(isCommandRunning());
	let showImportModal = $state(false);
	let movingAddress = $state<string | null>(null);
	let moveDestination = $state('');

	onMount(() => {
		loadStateResources(target);
	});

	function handleRefresh() {
		loadStateResources(target);
	}

	function handleShow(address: string) {
		stateShowResource(target, address);
	}

	function handleRemove(address: string) {
		if (!confirm(t('tofu.state_remove_confirm'))) return;
		stateRemoveResource(target, address);
	}

	function handleMoveStart(address: string) {
		movingAddress = address;
		moveDestination = address;
	}

	function handleMoveConfirm() {
		if (!movingAddress || !moveDestination.trim()) return;
		stateMoveResource(target, movingAddress, moveDestination.trim());
		movingAddress = null;
		moveDestination = '';
	}

	function handleMoveCancel() {
		movingAddress = null;
		moveDestination = '';
	}

	function handleImport(address: string, id: string) {
		stateImportResource(target, address, id);
		showImportModal = false;
	}
</script>

<div class="state-panel">
	<header class="header">
		<h2 class="title">{t('tofu.state_title')}</h2>
		<div class="header-actions">
			<Button variant="secondary" size="sm" onclick={handleRefresh} disabled={loading || running}>
				{t('tofu.state_refresh')}
			</Button>
			<Button variant="primary" size="sm" onclick={() => showImportModal = true} disabled={running}>
				{t('tofu.state_import')}
			</Button>
		</div>
	</header>

	{#if loading}
		<div class="loading-state">
			<div class="spinner"></div>
			<p class="loading-text">{t('tofu.state_loading')}</p>
		</div>
	{:else if resources.length === 0}
		<div class="empty-state">
			<svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="empty-icon">
				<path
					d="M20 7H4a2 2 0 00-2 2v10a2 2 0 002 2h16a2 2 0 002-2V9a2 2 0 00-2-2z"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
				<path
					d="M16 7V5a2 2 0 00-2-2h-4a2 2 0 00-2 2v2"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
			<p class="empty-text">{t('tofu.state_empty')}</p>
		</div>
	{:else}
		<div class="resource-list">
			{#each resources as address (address)}
				<div class="resource-item">
					{#if movingAddress === address}
						<div class="move-form">
							<span class="move-label">{t('tofu.state_move_prompt')}</span>
							<div class="move-row">
								<input
									type="text"
									class="move-input"
									bind:value={moveDestination}
									onkeydown={(e) => { if (e.key === 'Enter') handleMoveConfirm(); if (e.key === 'Escape') handleMoveCancel(); }}
								/>
								<Button variant="primary" size="sm" onclick={handleMoveConfirm} disabled={!moveDestination.trim() || moveDestination.trim() === address}>
									{t('common.confirm')}
								</Button>
								<Button variant="secondary" size="sm" onclick={handleMoveCancel}>
									{t('common.cancel')}
								</Button>
							</div>
						</div>
					{:else}
						<span class="resource-address">{address}</span>
						<div class="resource-actions">
							<button type="button" class="action-btn show" onclick={() => handleShow(address)} disabled={running} title={t('tofu.state_show')}>
								<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
									<path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
									<circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="1.5"/>
								</svg>
								{t('tofu.state_show')}
							</button>
							<button type="button" class="action-btn move" onclick={() => handleMoveStart(address)} disabled={running} title={t('tofu.state_move')}>
								<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
									<path d="M5 12h14M12 5l7 7-7 7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
								</svg>
								{t('tofu.state_move')}
							</button>
							<button type="button" class="action-btn remove" onclick={() => handleRemove(address)} disabled={running} title={t('tofu.state_remove')}>
								<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
									<path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
								</svg>
								{t('tofu.state_remove')}
							</button>
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>

<TofuStateImportModal
	open={showImportModal}
	onclose={() => showImportModal = false}
	onimport={handleImport}
/>

<style>
	.state-panel {
		width: 100%;
		height: 100%;
		overflow-y: auto;
		background: var(--color-bg-primary);
		padding: 24px;
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

	.resource-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.resource-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 10px 16px;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		transition: border-color 0.12s ease;
	}

	.resource-item:hover {
		border-color: color-mix(in srgb, var(--color-border) 70%, var(--color-accent));
	}

	.resource-address {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
		font-family: monospace;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}

	.resource-actions {
		display: flex;
		gap: 4px;
		flex-shrink: 0;
	}

	.action-btn {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 4px 8px;
		font-size: 0.6875rem;
		font-weight: 500;
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition: background-color 0.12s ease, color 0.12s ease, border-color 0.12s ease;
		white-space: nowrap;
	}

	.action-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.action-btn.show {
		color: var(--color-accent);
	}

	.action-btn.show:hover:not(:disabled) {
		background: rgba(255, 255, 255, 0.04);
		border-color: var(--color-accent);
	}

	.action-btn.move {
		color: var(--color-text-secondary);
	}

	.action-btn.move:hover:not(:disabled) {
		background: rgba(255, 255, 255, 0.04);
		color: var(--color-text-primary);
	}

	.action-btn.remove {
		color: var(--color-danger, #ef4444);
	}

	.action-btn.remove:hover:not(:disabled) {
		background: rgba(239, 68, 68, 0.08);
		border-color: var(--color-danger, #ef4444);
	}

	.move-form {
		display: flex;
		flex-direction: column;
		gap: 8px;
		width: 100%;
	}

	.move-label {
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
	}

	.move-row {
		display: flex;
		gap: 8px;
		align-items: center;
	}

	.move-input {
		flex: 1;
		padding: 6px 10px;
		font-size: 0.8125rem;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		font-family: monospace;
	}

	.move-input:focus {
		border-color: var(--color-accent);
	}

	@media (max-width: 700px) {
		.state-panel {
			padding: 16px;
		}

		.resource-item {
			flex-wrap: wrap;
			padding: 10px 12px;
		}

		.resource-address {
			width: 100%;
			white-space: normal;
			word-break: break-all;
		}

		.resource-actions {
			width: 100%;
			justify-content: flex-end;
		}

		.move-row {
			flex-wrap: wrap;
		}

		.move-input {
			width: 100%;
		}
	}
</style>
