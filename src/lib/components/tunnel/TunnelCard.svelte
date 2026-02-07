<script lang="ts">
	import { t } from '$lib/state/i18n.svelte';
	import type { TunnelConfig } from '$lib/ipc/tunnel';

	interface Props {
		tunnel: TunnelConfig;
		onstart: () => void;
		onstop: () => void;
	}

	let { tunnel, onstart, onstop }: Props = $props();

	let typeLabel = $derived(
		tunnel.tunnel_type === 'Local'
			? 'L'
			: tunnel.tunnel_type === 'Remote'
				? 'R'
				: 'D'
	);

	let typeColor = $derived(
		tunnel.tunnel_type === 'Local'
			? 'var(--color-accent)'
			: tunnel.tunnel_type === 'Remote'
				? 'var(--color-warning)'
				: 'var(--color-success)'
	);
</script>

<div class="tunnel-card">
	<div class="card-main">
		<span
			class="type-badge"
			style:color={typeColor}
			style:border-color={typeColor}
			title={t('tunnel.type_title', { type: tunnel.tunnel_type })}
		>
			{typeLabel}
		</span>

		<div class="tunnel-info">
			<span class="tunnel-mapping">
				localhost:{tunnel.local_port}
				<svg class="arrow-icon" width="14" height="14" viewBox="0 0 24 24" fill="none">
					<path d="M5 12h14M12 5l7 7-7 7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
				{tunnel.remote_host}:{tunnel.remote_port}
			</span>
			<span class="tunnel-type">{t('tunnel.type_tunnel', { type: tunnel.tunnel_type })}</span>
		</div>

		<div class="tunnel-status">
			<span
				class="status-dot"
				class:active={tunnel.active}
				title={tunnel.active ? t('tunnel.active') : t('tunnel.inactive')}
			></span>
		</div>
	</div>

	<div class="tunnel-actions">
		{#if tunnel.active}
			<button class="action-btn stop-btn" onclick={onstop} title={t('tunnel.stop_tunnel')} aria-label={t('tunnel.stop_tunnel')}>
				<svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
					<rect x="6" y="6" width="12" height="12" rx="1" />
				</svg>
			</button>
		{:else}
			<button class="action-btn start-btn" onclick={onstart} title={t('tunnel.start_tunnel')} aria-label={t('tunnel.start_tunnel')}>
				<svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
					<path d="M8 5v14l11-7z" />
				</svg>
			</button>
		{/if}
	</div>
</div>

<style>
	.tunnel-card {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 6px 8px;
		border-radius: var(--radius-card, 8px);
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.tunnel-card:hover {
		background-color: rgba(255, 255, 255, 0.04);
	}

	.card-main {
		flex: 1;
		min-width: 0;
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.type-badge {
		flex-shrink: 0;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 22px;
		height: 22px;
		font-size: 0.625rem;
		font-weight: 700;
		text-transform: uppercase;
		border: 1px solid;
		border-radius: 4px;
	}

	.tunnel-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.tunnel-mapping {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		font-size: 0.75rem;
		font-family: var(--font-mono, monospace);
		color: var(--color-text-primary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.arrow-icon {
		flex-shrink: 0;
		color: var(--color-text-secondary);
		opacity: 0.6;
	}

	.tunnel-type {
		font-size: 0.625rem;
		color: var(--color-text-secondary);
	}

	.tunnel-status {
		flex-shrink: 0;
		padding: 0 4px;
	}

	.status-dot {
		display: block;
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background-color: var(--color-text-secondary);
		opacity: 0.4;
		transition: background-color var(--duration-default) var(--ease-default),
			opacity var(--duration-default) var(--ease-default);
	}

	.status-dot.active {
		background-color: var(--color-success);
		opacity: 1;
		box-shadow: 0 0 6px rgba(52, 199, 89, 0.4);
	}

	.tunnel-actions {
		display: flex;
		align-items: center;
		gap: 2px;
		opacity: 0;
		transition: opacity var(--duration-default) var(--ease-default);
	}

	.tunnel-card:hover .tunnel-actions {
		opacity: 1;
	}

	.action-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		padding: 0;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition:
			background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.action-btn:hover {
		background-color: rgba(255, 255, 255, 0.08);
		color: var(--color-text-primary);
	}

	.action-btn:active {
		transform: scale(0.92);
	}

	.start-btn:hover {
		color: var(--color-success);
	}

	.stop-btn:hover {
		color: var(--color-danger);
	}
</style>
