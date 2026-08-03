<script lang="ts">
	import {
		marketplaceFetch,
		marketplaceInstall,
		marketplaceGetUrl,
		marketplaceLoadUrl,
		marketplaceResetUrl,
		marketplaceSetUrl,
		type MarketplaceEntry
	} from '$lib/ipc/marketplace';
	import { pluginList } from '$lib/ipc/plugin';
	import { setPlugins } from '$lib/state/plugin.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { onMount } from 'svelte';

	let entries = $state<MarketplaceEntry[]>([]);
	let installedIds = $state<Set<string>>(new Set());
	let loading = $state(false);
	let installingId = $state<string | null>(null);
	let error = $state<string | null>(null);
	let registryUrl = $state('');
	let registryInput = $state('');
	let showRegistry = $state(false);

	async function refresh(): Promise<void> {
		loading = true;
		error = null;
		try {
			const [list, plugins] = await Promise.all([marketplaceFetch(), pluginList()]);
			entries = list;
			installedIds = new Set(plugins.map((p) => p.manifest.id));
			setPlugins(plugins);
		} catch (err) {
			error = String(err);
		} finally {
			loading = false;
		}
	}

	async function install(entry: MarketplaceEntry): Promise<void> {
		installingId = entry.id;
		try {
			await marketplaceInstall(entry);
			addToast(t('marketplace.installed', { name: entry.name }), 'info');
			const plugins = await pluginList();
			setPlugins(plugins);
			installedIds = new Set(plugins.map((p) => p.manifest.id));
		} catch (err) {
			addToast(String(err), 'error');
		} finally {
			installingId = null;
		}
	}

	async function saveRegistry(): Promise<void> {
		const url = registryInput.trim();
		if (!url) return;
		try {
			await marketplaceSetUrl(url);
			registryUrl = url;
			addToast(t('marketplace.registry_saved'), 'info');
			await refresh();
		} catch (err) {
			addToast(String(err), 'error');
		}
	}

	async function resetRegistry(): Promise<void> {
		try {
			await marketplaceResetUrl();
			registryUrl = await marketplaceGetUrl();
			registryInput = registryUrl;
			addToast(t('marketplace.registry_reset_done'), 'info');
			await refresh();
		} catch (err) {
			addToast(String(err), 'error');
		}
	}

	onMount(async () => {
		try {
			// Apply any persisted registry override before the first fetch.
			registryUrl = await marketplaceLoadUrl();
		} catch {
			registryUrl = await marketplaceGetUrl().catch(() => '');
		}
		registryInput = registryUrl;
		refresh();
	});
</script>

<div class="marketplace">
	<div class="toolbar">
		<span class="toolbar-title">{t('marketplace.title')}</span>
		<button class="toolbar-btn" onclick={refresh} disabled={loading} title={t('marketplace.refresh')}>
			<svg
				class:spinning={loading}
				width="14"
				height="14"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="1.5"
				stroke-linecap="round"
				stroke-linejoin="round"
			>
				<path d="M23 4v6h-6M1 20v-6h6" />
				<path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15" />
			</svg>
		</button>
		<button
			class="toolbar-btn"
			onclick={() => (showRegistry = !showRegistry)}
			title={t('marketplace.registry_label')}
		>
			<svg
				width="14"
				height="14"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="1.5"
				stroke-linecap="round"
				stroke-linejoin="round"
			>
				<circle cx="12" cy="12" r="3" />
				<path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 11-2.83 2.83l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 11-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 11-2.83-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 110-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 112.83-2.83l.06.06a1.65 1.65 0 001.82.33H9a1.65 1.65 0 001-1.51V3a2 2 0 114 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 112.83 2.83l-.06.06a1.65 1.65 0 00-.33 1.82V9a1.65 1.65 0 001.51 1H21a2 2 0 110 4h-.09a1.65 1.65 0 00-1.51 1z" />
			</svg>
		</button>
	</div>

	{#if showRegistry}
		<div class="registry-row">
			<input
				class="registry-input"
				type="text"
				bind:value={registryInput}
				placeholder="https://…/plugins.json"
				spellcheck="false"
			/>
			<button class="registry-btn" onclick={saveRegistry}>{t('marketplace.registry_save')}</button>
			<button class="registry-btn" onclick={resetRegistry}>{t('marketplace.registry_reset')}</button>
			<p class="registry-hint">{t('marketplace.registry_locked_hint')}</p>
		</div>
	{/if}

	{#if loading && entries.length === 0}
		<div class="empty-state">
			<p class="empty-text">{t('marketplace.loading')}</p>
		</div>
	{:else if error}
		<div class="empty-state">
			<p class="empty-text">{t('marketplace.error')}</p>
			<p class="empty-hint">{error}</p>
		</div>
	{:else if entries.length === 0}
		<div class="empty-state">
			<p class="empty-text">{t('marketplace.no_entries')}</p>
			<p class="empty-hint">{t('marketplace.no_entries_hint')}</p>
		</div>
	{:else}
		<div class="entry-list">
			{#each entries as entry (entry.id)}
				{@const installed = installedIds.has(entry.id)}
				<div class="entry">
					<div class="entry-row">
						<span class="entry-name">{entry.name}</span>
						<span class="entry-version">v{entry.version}</span>
					</div>
					{#if entry.author}
						<span class="entry-author">{entry.author}</span>
					{/if}
					{#if entry.description}
						<p class="entry-desc">{entry.description}</p>
					{/if}
					{#if entry.permissions.length > 0}
						<div class="entry-perms">
							{#each entry.permissions as perm (perm)}
								<span class="perm-chip">{perm}</span>
							{/each}
						</div>
					{/if}
					<div class="entry-actions">
						<button
							class="install-btn"
							class:installed
							onclick={() => install(entry)}
							disabled={installed || installingId !== null}
						>
							{#if installingId === entry.id}
								{t('marketplace.installing')}
							{:else if installed}
								{t('marketplace.installed_label')}
							{:else}
								{t('marketplace.install')}
							{/if}
						</button>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.marketplace {
		display: flex;
		flex-direction: column;
		gap: 8px;
		min-height: 0;
	}

	.toolbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 4px 8px;
	}

	.registry-row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		padding: 6px 8px;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		background: var(--color-bg-secondary);
	}

	.registry-input {
		flex: 1 1 240px;
		min-width: 0;
		padding: 5px 8px;
		font-size: 0.75rem;
		font-family: inherit;
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: 4px;
	}

	.registry-btn {
		padding: 5px 10px;
		font-size: 0.6875rem;
		font-weight: 600;
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: 4px;
		cursor: pointer;
	}

	.registry-btn:hover {
		background: var(--color-bg-secondary);
	}

	.registry-hint {
		flex-basis: 100%;
		margin: 0;
		font-size: 0.625rem;
		color: var(--color-text-secondary);
	}

	.toolbar-title {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
	}

	.toolbar-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 4px;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.toolbar-btn:hover:not(:disabled) {
		color: var(--color-text-primary);
		background-color: rgba(255, 255, 255, 0.06);
	}

	.toolbar-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.spinning {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.empty-state {
		padding: 24px 12px;
		text-align: center;
	}

	.empty-text {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-primary);
	}

	.empty-hint {
		margin: 6px 0 0;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.entry-list {
		display: flex;
		flex-direction: column;
		gap: 6px;
		overflow-y: auto;
		padding: 0 4px;
	}

	.entry {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 8px;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		background: rgba(255, 255, 255, 0.02);
	}

	.entry-row {
		display: flex;
		align-items: baseline;
		gap: 6px;
	}

	.entry-name {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.entry-version {
		font-size: 0.625rem;
		color: var(--color-text-secondary);
	}

	.entry-author {
		font-size: 0.625rem;
		color: var(--color-text-secondary);
	}

	.entry-desc {
		margin: 2px 0 0;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.entry-perms {
		display: flex;
		flex-wrap: wrap;
		gap: 3px;
	}

	.perm-chip {
		padding: 1px 5px;
		font-size: 0.5625rem;
		font-family: var(--font-mono, monospace);
		color: var(--color-text-secondary);
		background: rgba(255, 255, 255, 0.04);
		border: 1px solid var(--color-border);
		border-radius: 3px;
	}

	.entry-actions {
		display: flex;
		justify-content: flex-end;
		margin-top: 4px;
	}

	.install-btn {
		padding: 4px 10px;
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		font-weight: 500;
		color: #fff;
		background-color: var(--color-accent);
		border: none;
		border-radius: 4px;
		cursor: pointer;
	}

	.install-btn:hover:not(:disabled) {
		background-color: var(--color-accent-hover);
	}

	.install-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.install-btn.installed {
		background-color: rgba(255, 255, 255, 0.06);
		color: var(--color-text-secondary);
	}
</style>
