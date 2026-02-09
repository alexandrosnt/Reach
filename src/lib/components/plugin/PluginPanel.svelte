<script lang="ts">
	import PluginListItem from './PluginListItem.svelte';
	import PluginView from './PluginView.svelte';
	import {
		pluginDiscover,
		pluginLoad,
		pluginUnload,
		pluginList,
		pluginSetConfig,
		type PluginInfo,
		type PluginConfig
	} from '$lib/ipc/plugin';
	import {
		getPlugins,
		setPlugins,
		getDiscoveredManifests,
		setDiscoveredManifests,
		getActivePluginId,
		setActivePluginId,
		getLoading,
		setLoading,
		getError,
		setError
	} from '$lib/state/plugin.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';

	interface Props {
		connectionId: string | undefined;
	}

	let { connectionId }: Props = $props();

	let discovering = $state(false);

	let plugins = $derived(getPlugins());
	let activePluginId = $derived(getActivePluginId());
	let loading = $derived(getLoading());

	let selectedPlugin = $derived(
		activePluginId ? plugins.find((p) => p.manifest.id === activePluginId) : undefined
	);

	async function discoverPlugins(): Promise<void> {
		discovering = true;
		setLoading(true);
		setError(null);

		try {
			const manifests = await pluginDiscover();
			setDiscoveredManifests(manifests);

			for (const manifest of manifests) {
				try {
					await pluginLoad(manifest.id);
				} catch (err) {
					console.error(`Failed to load plugin ${manifest.id}:`, err);
				}
			}

			await refreshPlugins();
		} catch (err) {
			setError(String(err));
			addToast(String(err), 'error');
		} finally {
			setLoading(false);
			discovering = false;
		}
	}

	async function refreshPlugins(): Promise<void> {
		try {
			const list = await pluginList();
			setPlugins(list);
		} catch (err) {
			setError(String(err));
			addToast(String(err), 'error');
		}
	}

	async function handleToggle(pluginId: string, enabled: boolean): Promise<void> {
		const plugin = plugins.find((p) => p.manifest.id === pluginId);
		if (!plugin) return;

		const config: PluginConfig = {
			id: pluginId,
			enabled,
			grantedPermissions: plugin.manifest.permissions
		};

		try {
			await pluginSetConfig(config);

			if (enabled) {
				await pluginLoad(pluginId);
			} else {
				await pluginUnload(pluginId);
			}

			await refreshPlugins();
		} catch (err) {
			addToast(String(err), 'error');
		}
	}

	function handlePluginClick(pluginId: string): void {
		if (activePluginId === pluginId) {
			setActivePluginId(null);
		} else {
			setActivePluginId(pluginId);
		}
	}

	onMount(() => {
		let unlisten: UnlistenFn | undefined;

		// Silently load current plugin list without loading flash
		pluginList()
			.then((list) => setPlugins(list))
			.catch(() => {});

		listen<{ pluginId: string; message: string; level?: string }>('plugin-notify', (event) => {
			const { message, level } = event.payload;
			const toastType = level === 'error' ? 'error' : level === 'warning' ? 'warning' : 'info';
			addToast(message, toastType);
		}).then((fn) => {
			unlisten = fn;
		});

		return () => {
			unlisten?.();
		};
	});
</script>

<div class="plugin-panel">
	<div class="toolbar">
		<span class="toolbar-title">{t('plugin.title')}</span>
		<div class="toolbar-actions">
			<button
				class="toolbar-btn"
				onclick={discoverPlugins}
				disabled={discovering}
				title={t('plugin.reload')}
			>
				<svg class:spinning={discovering} width="14" height="14" viewBox="0 0 24 24" fill="none">
					<path
						d="M23 4v6h-6M1 20v-6h6"
						stroke="currentColor"
						stroke-width="1.5"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
					<path
						d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"
						stroke="currentColor"
						stroke-width="1.5"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
			</button>
		</div>
	</div>

	{#if loading}
		<div class="loading-indicator">
			<span class="loading-text">{t('plugin.loading')}</span>
		</div>
	{/if}

	{#if plugins.length === 0 && !loading}
		<div class="empty-state">
			<p class="empty-text">{t('plugin.no_plugins')}</p>
			<p class="empty-hint">{t('plugin.no_plugins_hint')}</p>
		</div>
	{:else}
		<div class="plugin-list">
			{#each plugins as plugin (plugin.manifest.id)}
				<PluginListItem
					{plugin}
					active={activePluginId === plugin.manifest.id}
					onclick={() => handlePluginClick(plugin.manifest.id)}
					onToggle={(enabled) => handleToggle(plugin.manifest.id, enabled)}
				/>
			{/each}
		</div>
	{/if}

	{#if selectedPlugin?.hasUi && activePluginId}
		<div class="divider"></div>
		<div class="plugin-view-container">
			<PluginView pluginId={activePluginId} {connectionId} />
		</div>
	{/if}
</div>

<style>
	.plugin-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	.toolbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 10px;
		border-bottom: 1px solid var(--color-border);
	}

	.toolbar-title {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
	}

	.toolbar-actions {
		display: flex;
		align-items: center;
		gap: 2px;
	}

	.toolbar-btn {
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
		transition: all var(--duration-default) var(--ease-default);
	}

	.toolbar-btn:hover:not(:disabled) {
		background-color: rgba(255, 255, 255, 0.08);
		color: var(--color-text-primary);
	}

	.toolbar-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}

	.spinning {
		animation: spin 0.8s linear infinite;
	}

	.loading-indicator {
		padding: 6px 10px;
	}

	.loading-text {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 32px 16px;
		text-align: center;
	}

	.empty-text {
		margin: 0 0 4px;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
	}

	.empty-hint {
		margin: 0;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
		line-height: 1.5;
	}

	.plugin-list {
		flex: 1;
		overflow-y: auto;
		padding: 4px 8px;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.divider {
		height: 1px;
		background-color: var(--color-border);
		margin: 0 10px;
		flex-shrink: 0;
	}

	.plugin-view-container {
		flex: 1;
		min-height: 0;
		overflow: hidden;
	}
</style>
