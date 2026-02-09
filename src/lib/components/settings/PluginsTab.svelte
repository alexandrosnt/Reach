<script lang="ts">
	import {
		pluginGetDir,
		pluginSetDir,
		pluginDiscover,
		pluginList,
		pluginLoad,
		pluginUnload,
		pluginSetConfig,
		type PluginInfo,
		type PluginConfig
	} from '$lib/ipc/plugin';
	import { getPlugins, setPlugins } from '$lib/state/plugin.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { open } from '@tauri-apps/plugin-dialog';
	import Toggle from '$lib/components/shared/Toggle.svelte';
	import Button from '$lib/components/shared/Button.svelte';
	import { onMount } from 'svelte';

	let pluginsDir = $state('');
	let loading = $state(false);
	let discovering = $state(false);

	let plugins = $derived(getPlugins());

	onMount(() => {
		loadInitialData();
	});

	async function loadInitialData() {
		loading = true;
		try {
			pluginsDir = await pluginGetDir();
			const list = await pluginList();
			setPlugins(list);
		} catch {
			// IPC not available in dev
		}
		loading = false;
	}

	async function browsePath() {
		try {
			const selected = await open({ directory: true });
			if (selected) {
				pluginsDir = selected as string;
				await pluginSetDir(pluginsDir);
				addToast(t('settings.plugins_browse_toast'), 'success');
			}
		} catch (e) {
			addToast(
				e instanceof Error ? e.message : 'Failed to set plugins directory',
				'error'
			);
		}
	}

	async function handleDiscover() {
		discovering = true;
		try {
			const manifests = await pluginDiscover();

			for (const manifest of manifests) {
				const alreadyLoaded = plugins.some(
					(p) => p.manifest.id === manifest.id
				);
				if (!alreadyLoaded) {
					// Grant manifest permissions before loading so APIs are injected
					const config: PluginConfig = {
						id: manifest.id,
						enabled: true,
						grantedPermissions: manifest.permissions ?? []
					};
					await pluginSetConfig(config);
					await pluginLoad(manifest.id);
				}
			}

			const list = await pluginList();
			setPlugins(list);
			addToast(
				t('plugin.discovered_toast', { count: String(manifests.length) }),
				'success'
			);
		} catch (e) {
			addToast(
				e instanceof Error ? e.message : 'Failed to discover plugins',
				'error'
			);
		}
		discovering = false;
	}

	async function handleReloadAll() {
		loading = true;
		try {
			for (const plugin of plugins) {
				if (plugin.status.status !== 'Disabled') {
					await pluginUnload(plugin.manifest.id);
					await pluginLoad(plugin.manifest.id);
				}
			}
			const list = await pluginList();
			setPlugins(list);
			addToast(t('plugin.reloaded_toast'), 'success');
		} catch (e) {
			addToast(
				e instanceof Error ? e.message : 'Failed to reload plugins',
				'error'
			);
		}
		loading = false;
	}

	async function handleToggle(plugin: PluginInfo, enabled: boolean) {
		try {
			if (enabled) {
				await pluginLoad(plugin.manifest.id);
				const config: PluginConfig = {
					id: plugin.manifest.id,
					enabled: true,
					grantedPermissions: plugin.manifest.permissions
				};
				await pluginSetConfig(config);
			} else {
				await pluginUnload(plugin.manifest.id);
				const config: PluginConfig = {
					id: plugin.manifest.id,
					enabled: false,
					grantedPermissions: []
				};
				await pluginSetConfig(config);
			}
			const list = await pluginList();
			setPlugins(list);
		} catch (e) {
			addToast(
				e instanceof Error ? e.message : 'Failed to toggle plugin',
				'error'
			);
		}
	}

	function getStatusLabel(status: PluginInfo['status']): string {
		switch (status.status) {
			case 'Loaded':
				return 'Loaded';
			case 'Running':
				return 'Running';
			case 'Disabled':
				return 'Disabled';
			case 'Error':
				return 'Error';
			default:
				return 'Unknown';
		}
	}

	function isPluginEnabled(plugin: PluginInfo): boolean {
		return plugin.status.status !== 'Disabled';
	}
</script>

<div class="tab-content">
	<!-- Plugins Directory -->
	<div class="section">
		<h3 class="section-title">{t('settings.plugins_dir')}</h3>
		<p class="section-desc">{t('settings.plugins_dir_desc')}</p>

		<div class="dir-row">
			<input
				type="text"
				class="dir-input"
				value={pluginsDir}
				readonly
				placeholder={t('settings.plugins_dir')}
			/>
			<Button variant="secondary" size="sm" onclick={browsePath}>
				{t('settings.plugins_browse')}
			</Button>
		</div>
	</div>

	<!-- Action Buttons -->
	<div class="action-row">
		<Button
			variant="primary"
			size="sm"
			onclick={handleDiscover}
			disabled={discovering}
		>
			{discovering ? t('plugin.discovering') : t('settings.plugins_discover')}
		</Button>
		<Button
			variant="secondary"
			size="sm"
			onclick={handleReloadAll}
			disabled={loading}
		>
			{t('plugin.reload_all')}
		</Button>
	</div>

	<!-- Installed Plugins -->
	<div class="section">
		<h3 class="section-title">{t('settings.plugins_installed')}</h3>

		{#if loading}
			<p class="empty-state">{t('plugin.loading')}</p>
		{:else if plugins.length === 0}
			<div class="empty-state">
				<p class="empty-text">{t('settings.plugins_no_installed')}</p>
				<p class="empty-hint">{t('settings.plugins_no_installed_desc')}</p>
			</div>
		{:else}
			<div class="plugin-list">
				{#each plugins as plugin (plugin.manifest.id)}
					<div class="plugin-row">
						<div class="plugin-info">
							<div class="plugin-header">
								<span class="plugin-name">{plugin.manifest.name}</span>
								<span class="plugin-version">v{plugin.manifest.version}</span>
								<span
									class="status-badge"
									class:running={plugin.status.status === 'Running'}
									class:loaded={plugin.status.status === 'Loaded'}
									class:error={plugin.status.status === 'Error'}
									class:disabled={plugin.status.status === 'Disabled'}
								>
									{getStatusLabel(plugin.status)}
								</span>
							</div>
							{#if plugin.manifest.description}
								<span class="plugin-desc">{plugin.manifest.description}</span>
							{/if}
							<span class="plugin-author">{plugin.manifest.author}</span>
						</div>
						<div class="plugin-control">
							<Toggle
								checked={isPluginEnabled(plugin)}
								onchange={(checked) => handleToggle(plugin, checked)}
							/>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>

<style>
	.tab-content {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.section {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.section-title {
		margin: 0;
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.section-desc {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.dir-row {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.dir-input {
		flex: 1;
		padding: 6px 10px;
		font-size: 0.8125rem;
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		background-color: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-input, 6px);
		outline: none;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.dir-input:focus {
		border-color: var(--color-accent);
	}

	.action-row {
		display: flex;
		gap: 8px;
		padding-top: 4px;
	}

	.plugin-list {
		display: flex;
		flex-direction: column;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-card, 8px);
		overflow: hidden;
	}

	.plugin-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 10px 12px;
		gap: 16px;
		border-bottom: 1px solid var(--color-border);
	}

	.plugin-row:last-child {
		border-bottom: none;
	}

	.plugin-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
		flex: 1;
	}

	.plugin-header {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}

	.plugin-name {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.plugin-version {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		font-family: var(--font-mono);
	}

	.plugin-desc {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.plugin-author {
		font-size: 0.6875rem;
		color: var(--color-text-tertiary, var(--color-text-secondary));
		opacity: 0.7;
	}

	.plugin-control {
		flex-shrink: 0;
	}

	.status-badge {
		display: inline-flex;
		align-items: center;
		padding: 2px 8px;
		border-radius: 6px;
		font-size: 0.6875rem;
		font-weight: 500;
	}

	.status-badge.running {
		background-color: rgba(48, 209, 88, 0.12);
		color: var(--color-success);
	}

	.status-badge.loaded {
		background-color: rgba(10, 132, 255, 0.12);
		color: var(--color-accent);
	}

	.status-badge.error {
		background-color: rgba(255, 69, 58, 0.12);
		color: var(--color-danger);
	}

	.status-badge.disabled {
		background-color: rgba(255, 255, 255, 0.06);
		color: var(--color-text-secondary);
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 4px;
		padding: 24px 16px;
		text-align: center;
	}

	.empty-text {
		margin: 0;
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-secondary);
	}

	.empty-hint {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
	}
</style>
