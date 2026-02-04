<script lang="ts">
	import {
		tunnelCreate,
		tunnelStart,
		tunnelStop,
		tunnelList,
		type TunnelConfig
	} from '$lib/ipc/tunnel';
	import { addToast } from '$lib/state/toasts.svelte';
	import TunnelCard from './TunnelCard.svelte';
	import Input from '$lib/components/shared/Input.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import { untrack } from 'svelte';

	interface Props {
		connectionId?: string;
	}

	let { connectionId }: Props = $props();

	let tunnels = $state<TunnelConfig[]>([]);
	let loading = $state(true);
	let showForm = $state(false);

	let formType = $state<'Local' | 'Remote' | 'Dynamic'>('Local');
	let formLocalPort = $state('8080');
	let formRemoteHost = $state('localhost');
	let formRemotePort = $state('80');
	let creating = $state(false);

	async function loadTunnels(): Promise<void> {
		try {
			tunnels = await tunnelList();
		} catch (err) {
			console.error('Failed to load tunnels:', err);
		} finally {
			loading = false;
		}
	}

	function handleNewTunnel(): void {
		formType = 'Local';
		formLocalPort = '8080';
		formRemoteHost = 'localhost';
		formRemotePort = '80';
		showForm = true;
	}

	function handleCancelForm(): void {
		showForm = false;
	}

	async function handleCreateTunnel(): Promise<void> {
		if (!connectionId) {
			addToast('No active connection selected', 'error');
			return;
		}

		const localPort = parseInt(formLocalPort, 10);
		const remotePort = parseInt(formRemotePort, 10);

		if (isNaN(localPort) || localPort < 1 || localPort > 65535) {
			addToast('Invalid local port (1-65535)', 'error');
			return;
		}

		if (isNaN(remotePort) || remotePort < 1 || remotePort > 65535) {
			addToast('Invalid remote port (1-65535)', 'error');
			return;
		}

		if (!formRemoteHost.trim()) {
			addToast('Remote host is required', 'error');
			return;
		}

		try {
			creating = true;
			const tunnel = await tunnelCreate(
				formType,
				localPort,
				formRemoteHost.trim(),
				remotePort,
				connectionId
			);
			tunnels.push(tunnel);
			tunnels = tunnels;
			showForm = false;
			addToast(`Tunnel created on port ${localPort}`, 'success');
		} catch (err) {
			addToast(`Failed to create tunnel: ${err}`, 'error');
		} finally {
			creating = false;
		}
	}

	async function handleStart(tunnel: TunnelConfig): Promise<void> {
		try {
			await tunnelStart(tunnel.id);
			const idx = tunnels.findIndex((t) => t.id === tunnel.id);
			if (idx >= 0) {
				tunnels[idx] = { ...tunnels[idx], active: true };
			}
			addToast(`Tunnel on port ${tunnel.local_port} started`, 'success');
		} catch (err) {
			addToast(`Failed to start tunnel: ${err}`, 'error');
		}
	}

	async function handleStop(tunnel: TunnelConfig): Promise<void> {
		try {
			await tunnelStop(tunnel.id);
			const idx = tunnels.findIndex((t) => t.id === tunnel.id);
			if (idx >= 0) {
				tunnels[idx] = { ...tunnels[idx], active: false };
			}
			addToast(`Tunnel on port ${tunnel.local_port} stopped`, 'info');
		} catch (err) {
			addToast(`Failed to stop tunnel: ${err}`, 'error');
		}
	}

	function handleDelete(tunnel: TunnelConfig): void {
		tunnels = tunnels.filter((t) => t.id !== tunnel.id);
		addToast('Tunnel removed', 'info');
	}

	$effect(() => {
		untrack(() => loadTunnels());
	});
</script>

<div class="tunnel-manager">
	<div class="actions-row">
		<button class="new-tunnel-btn" onclick={handleNewTunnel}>
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none">
				<path
					d="M12 5v14M5 12h14"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
				/>
			</svg>
			New Tunnel
		</button>
	</div>

	{#if showForm}
		<div class="create-form">
			<div class="form-header">
				<span class="form-title">Create Tunnel</span>
				<button class="form-close" onclick={handleCancelForm} aria-label="Close form">
					<svg width="12" height="12" viewBox="0 0 14 14" fill="none">
						<path d="M1 1L13 13M13 1L1 13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
					</svg>
				</button>
			</div>

			<div class="form-body">
				<div class="type-selector">
					{#each ['Local', 'Remote', 'Dynamic'] as tunnelType}
						<button
							class="type-option"
							class:selected={formType === tunnelType}
							onclick={() => (formType = tunnelType as 'Local' | 'Remote' | 'Dynamic')}
						>
							{tunnelType}
						</button>
					{/each}
				</div>

				<Input label="Local Port" bind:value={formLocalPort} type="number" placeholder="8080" />
				<Input label="Remote Host" bind:value={formRemoteHost} placeholder="localhost" />

				{#if formType !== 'Dynamic'}
					<Input label="Remote Port" bind:value={formRemotePort} type="number" placeholder="80" />
				{/if}

				<div class="form-actions">
					<Button variant="ghost" size="sm" onclick={handleCancelForm}>Cancel</Button>
					<Button variant="primary" size="sm" onclick={handleCreateTunnel} disabled={creating}>
						{creating ? 'Creating...' : 'Create'}
					</Button>
				</div>
			</div>
		</div>
	{/if}

	{#if loading}
		<div class="loading-state">
			<span class="spinner"></span>
			<span class="loading-text">Loading tunnels...</span>
		</div>
	{:else if tunnels.length === 0 && !showForm}
		<p class="empty-state">No tunnels configured. Create one to get started.</p>
	{:else}
		{#if tunnels.length > 0}
			<div class="divider"></div>
			<div class="tunnels-scroll">
				{#each tunnels as tunnel (tunnel.id)}
					<div class="tunnel-row">
						<TunnelCard
							{tunnel}
							onstart={() => handleStart(tunnel)}
							onstop={() => handleStop(tunnel)}
						/>
						<button
							class="delete-btn"
							onclick={() => handleDelete(tunnel)}
							title="Remove tunnel"
							aria-label="Remove tunnel"
						>
							<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
								<path d="M3 6h18" />
								<path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
								<path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
							</svg>
						</button>
					</div>
				{/each}
			</div>
		{/if}
	{/if}
</div>

<style>
	.tunnel-manager {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 4px 0;
	}

	.actions-row {
		display: flex;
		gap: 6px;
	}

	.new-tunnel-btn {
		display: flex;
		align-items: center;
		gap: 6px;
		flex: 1;
		padding: 8px 10px;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		font-weight: 500;
		border-radius: var(--radius-btn);
		cursor: pointer;
		color: var(--color-accent);
		background: transparent;
		border: 1px solid var(--color-accent);
		transition:
			background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.new-tunnel-btn:hover {
		background-color: rgba(0, 122, 255, 0.1);
	}

	.new-tunnel-btn:active {
		transform: scale(0.98);
	}

	.create-form {
		border: 1px solid var(--color-border);
		border-radius: var(--radius-card, 8px);
		overflow: hidden;
	}

	.form-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 10px;
		border-bottom: 1px solid var(--color-border);
		background-color: rgba(255, 255, 255, 0.02);
	}

	.form-title {
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.form-close {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 20px;
		padding: 0;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.form-close:hover {
		background-color: rgba(255, 255, 255, 0.08);
		color: var(--color-text-primary);
	}

	.form-body {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 10px;
	}

	.type-selector {
		display: flex;
		gap: 4px;
		background-color: rgba(255, 255, 255, 0.03);
		border-radius: var(--radius-btn);
		padding: 3px;
	}

	.type-option {
		flex: 1;
		padding: 5px 8px;
		font-family: var(--font-sans);
		font-size: 0.75rem;
		font-weight: 500;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition:
			background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.type-option:hover {
		color: var(--color-text-primary);
	}

	.type-option.selected {
		background-color: var(--color-accent);
		color: #fff;
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: 6px;
		padding-top: 4px;
	}

	.divider {
		height: 1px;
		background-color: var(--color-border);
		opacity: 0.5;
		margin: 2px 0;
	}

	.tunnels-scroll {
		display: flex;
		flex-direction: column;
		gap: 2px;
		overflow-y: auto;
	}

	.tunnel-row {
		display: flex;
		align-items: center;
		gap: 2px;
	}

	.tunnel-row :global(.tunnel-card) {
		flex: 1;
	}

	.delete-btn {
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
		opacity: 0;
		transition:
			background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default),
			opacity var(--duration-default) var(--ease-default);
	}

	.tunnel-row:hover .delete-btn {
		opacity: 1;
	}

	.delete-btn:hover {
		background-color: rgba(255, 255, 255, 0.08);
		color: var(--color-danger);
	}

	.delete-btn:active {
		transform: scale(0.92);
	}

	.loading-state {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		padding: 16px 0;
	}

	.loading-text {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.empty-state {
		margin: 0;
		padding: 12px 4px;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
		text-align: center;
	}

	.spinner {
		display: inline-block;
		width: 14px;
		height: 14px;
		border: 2px solid rgba(255, 255, 255, 0.15);
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
