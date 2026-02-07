<script lang="ts">
	import {
		getTransfers,
		removeTransfer,
		clearCompleted,
		type Transfer
	} from '$lib/state/transfers.svelte';
	import { t } from '$lib/state/i18n.svelte';

	let transfers = $derived(getTransfers());
	let hasCompleted = $derived(transfers.some((t) => t.status !== 'uploading'));

	function formatSize(bytes: number): string {
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
		return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
	}
</script>

<div class="transfer-queue">
	{#if transfers.length === 0}
		<div class="empty-transfers">
			<svg width="20" height="20" viewBox="0 0 24 24" fill="none" class="transfer-icon">
				<path
					d="M12 5v14M5 12l7-7 7 7"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
			<span class="transfer-text">{t('transfer.no_active')}</span>
		</div>
	{:else}
		{#if hasCompleted}
			<button class="clear-btn" onclick={clearCompleted} type="button">{t('transfer.clear_finished')}</button>
		{/if}
		<div class="transfer-list">
			{#each transfers as transfer (transfer.id)}
				<div class="transfer-item" class:completed={transfer.status === 'completed'} class:error={transfer.status === 'error'}>
					<div class="transfer-row">
						<span class="transfer-filename" title={transfer.filename}>{transfer.filename}</span>
						{#if transfer.status === 'uploading'}
							<span class="transfer-percent">{Math.round(transfer.percent)}%</span>
						{:else if transfer.status === 'completed'}
							<button class="dismiss-btn" onclick={() => removeTransfer(transfer.id)} type="button" aria-label={t('transfer.dismiss')}>
								<svg width="10" height="10" viewBox="0 0 24 24" fill="none">
									<path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
								</svg>
							</button>
						{:else}
							<button class="dismiss-btn" onclick={() => removeTransfer(transfer.id)} type="button" aria-label={t('transfer.dismiss')}>
								<svg width="10" height="10" viewBox="0 0 24 24" fill="none">
									<path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
								</svg>
							</button>
						{/if}
					</div>
					<div class="transfer-detail">
						{#if transfer.status === 'uploading'}
							<span class="transfer-size">{formatSize(transfer.bytesTransferred)} / {formatSize(transfer.totalBytes)}</span>
						{:else if transfer.status === 'completed'}
							<span class="transfer-done">{t('transfer.completed')}</span>
						{:else}
							<span class="transfer-err">{transfer.error ?? t('transfer.failed')}</span>
						{/if}
					</div>
					{#if transfer.status === 'uploading'}
						<div class="bar-track">
							<div class="bar-fill" style:width="{transfer.percent}%"></div>
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.transfer-queue {
		padding: 4px 0;
	}

	.empty-transfers {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
		padding: 16px 8px;
	}

	.transfer-icon {
		color: var(--color-text-secondary);
		opacity: 0.4;
	}

	.transfer-text {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.6;
	}

	.clear-btn {
		display: block;
		margin: 0 8px 4px;
		padding: 2px 6px;
		border: none;
		border-radius: var(--radius-btn);
		background: transparent;
		color: var(--color-accent, #007aff);
		font-family: var(--font-sans);
		font-size: 0.625rem;
		cursor: pointer;
		transition: background-color 0.15s;
	}

	.clear-btn:hover {
		background-color: rgba(255, 255, 255, 0.06);
	}

	.transfer-list {
		display: flex;
		flex-direction: column;
	}

	.transfer-item {
		padding: 5px 8px;
		border-bottom: 1px solid var(--color-border);
	}

	.transfer-item.completed {
		opacity: 0.6;
	}

	.transfer-item.error {
		background-color: rgba(255, 69, 58, 0.06);
	}

	.transfer-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 6px;
	}

	.transfer-filename {
		font-size: 0.6875rem;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
		flex: 1;
	}

	.transfer-percent {
		font-size: 0.625rem;
		color: var(--color-text-secondary);
		font-variant-numeric: tabular-nums;
		flex-shrink: 0;
	}

	.dismiss-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 16px;
		border: none;
		border-radius: 50%;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		flex-shrink: 0;
		transition: background-color 0.15s;
	}

	.dismiss-btn:hover {
		background-color: rgba(255, 255, 255, 0.1);
	}

	.transfer-detail {
		margin-top: 1px;
	}

	.transfer-size {
		font-size: 0.5625rem;
		color: var(--color-text-secondary);
		font-variant-numeric: tabular-nums;
	}

	.transfer-done {
		font-size: 0.5625rem;
		color: #30d158;
	}

	.transfer-err {
		font-size: 0.5625rem;
		color: #ff453a;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.bar-track {
		height: 2px;
		background: rgba(255, 255, 255, 0.08);
		border-radius: 1px;
		margin-top: 4px;
		overflow: hidden;
	}

	.bar-fill {
		height: 100%;
		background: var(--color-accent, #007aff);
		border-radius: 1px;
		transition: width 0.3s ease;
	}
</style>
