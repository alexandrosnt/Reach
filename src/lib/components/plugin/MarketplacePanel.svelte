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
	/** plugin id -> installed manifest version, for update detection. */
	let installedVersions = $state<Map<string, string>>(new Map());
	let loading = $state(false);
	let installingId = $state<string | null>(null);
	let search = $state('');
	let error = $state<string | null>(null);
	let registryUrl = $state('');
	let registryInput = $state('');
	let showRegistry = $state(false);

	/**
	 * Compare two semver-ish strings. Returns >0 if a is newer than b, 0 if
	 * equal, <0 if older. Build metadata is ignored; a prerelease sorts before
	 * its own release (1.0.0-rc1 < 1.0.0), matching semver precedence.
	 * Unparseable parts compare as 0 so a malformed version never claims to be
	 * an upgrade.
	 */
	function compareVersions(a: string, b: string): number {
		const split = (v: string) => {
			const [core, pre] = v.trim().split('+')[0].split('-', 2);
			const nums = core.split('.').map((n) => Number.parseInt(n, 10) || 0);
			return { nums, pre: pre ?? '' };
		};
		const va = split(a);
		const vb = split(b);
		const len = Math.max(va.nums.length, vb.nums.length);
		for (let i = 0; i < len; i++) {
			const d = (va.nums[i] ?? 0) - (vb.nums[i] ?? 0);
			if (d !== 0) return d;
		}
		if (va.pre === vb.pre) return 0;
		// A version with no prerelease tag outranks one that has it.
		if (va.pre === '') return 1;
		if (vb.pre === '') return -1;
		return va.pre < vb.pre ? -1 : 1;
	}

	/** True when the registry offers a strictly newer version than installed. */
	function hasUpdate(entry: MarketplaceEntry): boolean {
		const current = installedVersions.get(entry.id);
		if (current === undefined) return false;
		return compareVersions(entry.version, current) > 0;
	}

	const visibleEntries = $derived.by(() => {
		const q = search.trim().toLowerCase();
		if (!q) return entries;
		return entries.filter((e) =>
			[e.name, e.description, e.author, e.id, ...(e.keywords ?? [])]
				.filter(Boolean)
				.some((field) => field.toLowerCase().includes(q))
		);
	});

	function indexInstalled(plugins: { manifest: { id: string; version: string } }[]): void {
		installedVersions = new Map(plugins.map((p) => [p.manifest.id, p.manifest.version]));
	}

	async function refresh(): Promise<void> {
		loading = true;
		error = null;
		try {
			const [list, plugins] = await Promise.all([marketplaceFetch(), pluginList()]);
			entries = list;
			indexInstalled(plugins);
			setPlugins(plugins);
		} catch (err) {
			error = String(err);
		} finally {
			loading = false;
		}
	}

	/**
	 * Install or update. The backend replaces the plugin directory wholesale,
	 * so the same call serves both; only the toast differs.
	 */
	async function install(entry: MarketplaceEntry): Promise<void> {
		const updating = hasUpdate(entry);
		installingId = entry.id;
		try {
			await marketplaceInstall(entry);
			addToast(
				updating
					? t('marketplace.updated', { name: entry.name, version: entry.version })
					: t('marketplace.installed', { name: entry.name }),
				'info'
			);
			const plugins = await pluginList();
			setPlugins(plugins);
			indexInstalled(plugins);
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

	{#if entries.length > 0}
		<div class="search-row">
			<svg
				class="search-icon"
				width="14"
				height="14"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="1.5"
				stroke-linecap="round"
				stroke-linejoin="round"
				aria-hidden="true"
			>
				<circle cx="11" cy="11" r="7" />
				<path d="M21 21l-4.35-4.35" />
			</svg>
			<input
				class="search-input"
				type="text"
				bind:value={search}
				placeholder={t('marketplace.search')}
				spellcheck="false"
				aria-label={t('marketplace.search')}
			/>
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
	{:else if visibleEntries.length === 0}
		<div class="empty-state">
			<p class="empty-text">{t('marketplace.no_results')}</p>
			<p class="empty-hint">{t('marketplace.no_results_hint', { query: search.trim() })}</p>
		</div>
	{:else}
		<div class="entry-list">
			{#each visibleEntries as entry (entry.id)}
				{@const installedVersion = installedVersions.get(entry.id)}
				{@const installed = installedVersion !== undefined}
				{@const updatable = hasUpdate(entry)}
				<div class="entry">
					<div class="entry-row">
						<span class="entry-name">{entry.name}</span>
						<span class="entry-version">v{entry.version}</span>
						{#if updatable}
							<span class="update-badge">{t('marketplace.update')}</span>
						{/if}
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
							class:installed={installed && !updatable}
							class:updatable
							onclick={() => install(entry)}
							disabled={(installed && !updatable) || installingId !== null}
						>
							{#if installingId === entry.id}
								{updatable ? t('marketplace.updating') : t('marketplace.installing')}
							{:else if updatable}
								{t('marketplace.update')}
							{:else if installed}
								{t('marketplace.installed_label')}
							{:else}
								{t('marketplace.install')}
							{/if}
						</button>
						{#if installed && installedVersion}
							<span class="installed-version">v{installedVersion}</span>
						{/if}
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

	.install-btn.updatable {
		background-color: var(--color-warning, #d98322);
	}

	.search-row {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 0 8px;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		background: var(--color-bg-secondary);
	}

	.search-icon {
		flex: none;
		color: var(--color-text-secondary);
	}

	.search-input {
		flex: 1 1 auto;
		min-width: 0;
		padding: 6px 0;
		font-size: 0.75rem;
		font-family: inherit;
		color: var(--color-text-primary);
		background: transparent;
		border: none;
		outline: none;
	}

	.search-input::placeholder {
		color: var(--color-text-secondary);
	}

	.update-badge {
		padding: 1px 5px;
		font-size: 0.5625rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: #fff;
		background-color: var(--color-warning, #d98322);
		border-radius: 3px;
	}

	.installed-version {
		font-size: 0.625rem;
		color: var(--color-text-secondary);
	}
</style>
