<script lang="ts">
	import Toggle from '$lib/components/shared/Toggle.svelte';
	import { themeState, applyTheme, loadInstalledThemes, DARK, LIGHT } from '$lib/state/theme.svelte';
	import { onMount } from 'svelte';
	import { themeFetchRegistry, themeInstall, themeUninstall, type ThemeEntry } from '$lib/ipc/theme';
	import { validateTheme } from '$lib/state/theme.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { getSettings, updateSetting } from '$lib/state/settings.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { matchesQuery } from '$lib/utils/search';

	const settings = getSettings();
	let currentFont = $derived(settings.fontFamily || 'monospace');
	let currentSize = $derived(settings.fontSize || 14);

	let registry = $state<ThemeEntry[]>([]);
	let registryError = $state<string | null>(null);
	let loadingRegistry = $state(false);
	let installingId = $state<string | null>(null);

	let installedIds = $derived(new Set(themeState.installed.map((t) => t.id)));

	/**
	 * The registry is a public, PR-gated index with no size limit, so this list
	 * has to stay bounded: rendering every entry builds one DOM row per theme
	 * and would stall the settings panel long before the registry got
	 * interesting. Search narrows, and the list grows a page at a time on
	 * request — cheaper and far less fragile than virtualising rows whose
	 * height varies with whether a theme carries a description.
	 */
	const PAGE = 40;

	let themeQuery = $state('');
	let shown = $state(PAGE);

	let matching = $derived(
		registry.filter((e) => matchesQuery(themeQuery, [e.name, e.description, e.author, e.appearance]))
	);
	let visible = $derived(matching.slice(0, shown));

	// A new search starts from the top of its own results.
	$effect(() => {
		themeQuery;
		shown = PAGE;
	});

	async function loadRegistry(): Promise<void> {
		loadingRegistry = true;
		registryError = null;
		try {
			registry = await themeFetchRegistry();
		} catch (err) {
			registryError = String(err);
		} finally {
			loadingRegistry = false;
		}
	}

	async function install(entry: ThemeEntry): Promise<void> {
		installingId = entry.id;
		try {
			const doc = await themeInstall(entry);
			const result = validateTheme(doc);
			if (!result.ok) {
				addToast(result.error, 'error');
				return;
			}
			themeState.installed = [
				...themeState.installed.filter((t) => t.id !== result.theme.id),
				result.theme
			];
			addToast(t('themes.installed'), 'info');
		} catch (err) {
			addToast(String(err), 'error');
		} finally {
			installingId = null;
		}
	}

	async function remove(id: string): Promise<void> {
		try {
			await themeUninstall(id);
			themeState.installed = themeState.installed.filter((th) => th.id !== id);
			// Fall back if the theme in use was the one removed.
			if (settings.themeId === id) selectTheme(SYSTEM);
		} catch (err) {
			addToast(String(err), 'error');
		}
	}

	onMount(() => {
		// Re-read from disk in case a theme was installed or edited since startup.
		loadInstalledThemes();
		loadRegistry();
	});

	/** "System" follows the OS; every other entry is a real theme. */
	const SYSTEM = '__system__';

	let choices = $derived([
		{ id: SYSTEM, name: t('settings.theme_system'), swatch: null },
		...themeState.all.map((th) => ({ id: th.id, name: th.name, swatch: th }))
	]);

	let selectedId = $derived(settings.themeId ?? (settings.theme === 'system' ? SYSTEM : null));

	function selectTheme(id: string) {
		if (id === SYSTEM) {
			updateSetting('themeId', undefined);
			updateSetting('theme', 'system');
			const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
			applyTheme(prefersDark ? DARK : LIGHT);
			return;
		}
		const theme = themeState.all.find((t2) => t2.id === id);
		if (!theme) return;
		updateSetting('themeId', theme.id);
		// Keep `theme` consistent so anything still reading it, and the fallback
		// path when a themeId no longer resolves, stay sensible.
		updateSetting('theme', theme.appearance);
		applyTheme(theme);
	}


	// Local + system monospace fonts only — NO network (Google Fonts) fetches.
	// `JetBrains Mono` is bundled (app.css @font-face); the rest resolve to
	// whatever the OS has installed, falling back to `monospace`. Any other
	// installed font can be entered as a custom family.
	const SYSTEM_FONTS = [
		{ name: 'JetBrains Mono (bundled)', value: 'JetBrains Mono' },
		{ name: 'System Monospace', value: 'monospace' },
		{ name: 'SF Mono', value: 'SF Mono' },
		{ name: 'Menlo', value: 'Menlo' },
		{ name: 'Monaco', value: 'Monaco' },
		{ name: 'Cascadia Code', value: 'Cascadia Code' },
		{ name: 'Cascadia Mono', value: 'Cascadia Mono' },
		{ name: 'Consolas', value: 'Consolas' },
		{ name: 'Courier New', value: 'Courier New' },
		{ name: 'DejaVu Sans Mono', value: 'DejaVu Sans Mono' },
		{ name: 'Liberation Mono', value: 'Liberation Mono' },
		{ name: 'Ubuntu Mono', value: 'Ubuntu Mono' },
		{ name: 'Noto Sans Mono', value: 'Noto Sans Mono' },
		{ name: 'Source Code Pro', value: 'Source Code Pro' },
	];

	// Mixes ASCII, box-drawing, halfwidth kana, fullwidth latin and CJK so the
	// user can check fullwidth/halfwidth column alignment of the chosen font.
	const PREVIEW_TEXT =
		'user@server:~$ ls -la\n' +
		'0123456789  abcXYZ  {}[]()<>=>|\n' +
		'CJK 日本語 中文 한국어   Kana ｱｲｳﾊﾝｶｸ\n' +
		'全角ＡＢＣ 漢字  ┌─┬─┐│x│y│└─┴─┘';

	let fontSearch = $state('');
	let fontDropdownOpen = $state(false);

	let filteredFonts = $derived(
		SYSTEM_FONTS.filter((f) => f.name.toLowerCase().includes(fontSearch.toLowerCase()))
	);

	// Full configurability: if the query matches no known entry, offer it as a
	// custom font family (any font installed on the user's system).
	let customFont = $derived.by(() => {
		const q = fontSearch.trim();
		if (!q) return null;
		const known = SYSTEM_FONTS.some(
			(f) => f.name.toLowerCase() === q.toLowerCase() || f.value.toLowerCase() === q.toLowerCase()
		);
		return known ? null : q;
	});

	function selectFont(value: string): void {
		updateSetting('fontFamily', value);
		fontDropdownOpen = false;
		fontSearch = '';
	}
</script>

<div class="tab-content">
	<div class="setting-section">
		<span class="section-label">{t('settings.theme')}</span>
		<div class="theme-cards">
			{#each choices as choice (choice.id)}
				<button
					class="theme-card"
					class:active={selectedId === choice.id}
					onclick={() => selectTheme(choice.id)}
					title={choice.name}
				>
					{#if choice.swatch}
						<!-- Preview drawn from the theme's own tokens, so an installed
						     theme shows what it actually looks like. -->
						<div
							class="theme-preview"
							style="background: {choice.swatch.colors['bg-primary']}; border-color: {choice.swatch.colors.border};"
						>
							<span class="swatch-bar" style="background: {choice.swatch.colors['bg-secondary']};"></span>
							<span class="swatch-dot" style="background: {choice.swatch.colors.accent};"></span>
						</div>
					{:else}
						<div class="theme-preview preview-system">
							<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
								<rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
								<line x1="8" y1="21" x2="16" y2="21" /><line x1="12" y1="17" x2="12" y2="21" />
							</svg>
						</div>
					{/if}
					<span class="theme-label">{choice.name}</span>
				</button>
			{/each}
		</div>
	</div>

	<div class="setting-section">
		<div class="section-head">
			<span class="section-label">{t('themes.marketplace')}</span>
			<button class="link-btn" onclick={loadRegistry} disabled={loadingRegistry}>
				{t('themes.refresh')}
			</button>
		</div>

		{#if registryError}
			<p class="registry-msg">{t('themes.error')}</p>
			<p class="registry-detail">{registryError}</p>
		{:else if registry.length === 0}
			<p class="registry-msg">{t('themes.empty')}</p>
		{:else}
			{#if registry.length > PAGE}
				<input
					bind:value={themeQuery}
					class="theme-search"
					type="text"
					autocomplete="off"
					spellcheck="false"
					placeholder={t('themes.search')}
					aria-label={t('themes.search')}
				/>
			{/if}

			{#if matching.length === 0}
				<p class="registry-msg">{t('themes.no_match')}</p>
			{/if}

			<div class="theme-rows">
				{#each visible as entry (entry.id)}
					{@const isInstalled = installedIds.has(entry.id)}
					<div class="theme-row">
						<div class="row-info">
							<span class="row-name">{entry.name}</span>
							{#if entry.description}
								<span class="row-desc">{entry.description}</span>
							{/if}
						</div>
						{#if isInstalled}
							<button class="row-btn" onclick={() => remove(entry.id)}>{t('themes.remove')}</button>
						{:else}
							<button
								class="row-btn primary"
								onclick={() => install(entry)}
								disabled={installingId !== null}
							>
								{installingId === entry.id ? t('themes.installing') : t('themes.install')}
							</button>
						{/if}
					</div>
				{/each}
			</div>

			{#if matching.length > visible.length}
				<button class="show-more" onclick={() => (shown += PAGE)}>
					{t('themes.show_more', {
						shown: visible.length,
						total: matching.length
					})}
				</button>
			{/if}
		{/if}
	</div>

	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('settings.font_size')}</span>
			<span class="setting-description">Current: {settings.fontSize}px — Use Ctrl + Mouse Wheel in terminal to adjust</span>
		</div>
		<div class="setting-control">
			<span class="font-size-badge">{settings.fontSize}px</span>
		</div>
	</div>

	<div class="setting-row font-row">
		<div class="setting-info">
			<span class="setting-label">{t('settings.terminal_font')}</span>
			<span class="setting-description">{t('settings.font_desc')}</span>
		</div>
		<div class="setting-control">
			<div class="font-picker">
				<button
					class="font-picker-btn"
					style="font-family: '{currentFont}', monospace"
					onclick={() => (fontDropdownOpen = !fontDropdownOpen)}
				>
					{currentFont}
					<svg width="10" height="10" viewBox="0 0 10 10" fill="none" class="chevron" class:open={fontDropdownOpen}>
						<path d="M2 4l3 3 3-3" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
				</button>

				{#if fontDropdownOpen}
					<div class="font-dropdown">
						<input
							class="font-search"
							type="text"
							placeholder="Search fonts..."
							bind:value={fontSearch}
						/>
						<div class="font-list">
							{#if filteredFonts.length > 0}
								<div class="font-group-label">{t('settings.system_fonts')}</div>
								{#each filteredFonts as font (font.value)}
									<button
										class="font-option"
										class:active={currentFont === font.value}
										style:font-family="'{font.value}', monospace"
										onclick={() => selectFont(font.value)}
									>
										{font.name}
									</button>
								{/each}
							{/if}
							{#if customFont}
								<div class="font-group-label">{t('settings.custom_font')}</div>
								<button
									class="font-option"
									style:font-family="'{customFont}', monospace"
									onclick={() => selectFont(customFont)}
								>
									{t('settings.use_font', { font: customFont })}
								</button>
							{/if}
							{#if filteredFonts.length === 0 && !customFont}
								<div class="font-empty">{t('settings.no_fonts')}</div>
							{/if}
						</div>
					</div>
				{/if}
			</div>
		</div>
	</div>

	<div class="font-preview-box">
		<span class="preview-label">{t('settings.preview')} — {currentFont} @ {currentSize}px</span>
		<pre class="preview-pre" style="font-family: '{currentFont}', monospace; font-size: {currentSize}px;">{PREVIEW_TEXT}</pre>
	</div>

	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('settings.shell_colors')}</span>
			<span class="setting-description">{t('settings.shell_colors_desc')}</span>
		</div>
		<div class="setting-control">
			<Toggle
				hideLabel
				checked={settings.injectShellColors}
				label={t('settings.shell_colors')}
				onchange={(v) => updateSetting('injectShellColors', v)}
			/>
		</div>
	</div>
</div>

<style>

	.setting-section {
		padding: 12px 0;
		border-bottom: 1px solid var(--color-border);
	}

	.section-label {
		display: block;
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--color-text-primary);
		margin-bottom: 12px;
	}

	.swatch-bar {
		position: absolute;
		left: 0;
		top: 0;
		bottom: 0;
		width: 34%;
	}

	.swatch-dot {
		position: absolute;
		right: 7px;
		bottom: 7px;
		width: 8px;
		height: 8px;
		border-radius: 50%;
	}

	.section-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}

	.link-btn {
		padding: 0;
		font-family: var(--font-sans);
		font-size: var(--text-xs);
		color: var(--color-accent);
		background: none;
		border: none;
		cursor: pointer;
	}

	.link-btn:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.registry-msg {
		margin: var(--space-2) 0 0;
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
	}

	.registry-detail {
		margin: var(--space-1) 0 0;
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
	}

	.theme-search {
		width: 100%;
		padding: 7px var(--space-2);
		margin-bottom: var(--space-2);
		font-size: var(--text-sm);
		font-family: inherit;
		color: var(--color-text-primary);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		outline: none;
	}

	.theme-search:focus {
		border-color: var(--color-accent);
	}

	.theme-search::placeholder {
		color: var(--color-text-tertiary);
	}

	.show-more {
		width: 100%;
		margin-top: var(--space-2);
		padding: 8px;
		font-size: var(--text-xs);
		font-family: inherit;
		color: var(--color-text-secondary);
		background: none;
		border: 1px dashed var(--color-border);
		border-radius: var(--radius-sm);
		cursor: pointer;
	}

	.show-more:hover {
		background: var(--color-surface-hover);
		color: var(--color-text-primary);
	}

	.theme-rows {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin-top: var(--space-2);
	}

	.theme-row {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-2) var(--space-3);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background: var(--color-bg-secondary);
	}

	.row-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
		flex: 1;
		min-width: 0;
	}

	.row-name {
		font-size: var(--text-sm);
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.row-desc {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.row-btn {
		flex: none;
		padding: var(--space-1) var(--space-3);
		font-family: var(--font-sans);
		font-size: var(--text-xs);
		font-weight: 500;
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
		border: none;
		border-radius: var(--radius-sm);
		cursor: pointer;
	}

	.row-btn.primary {
		color: #fff;
		background: var(--color-accent);
	}

	.row-btn:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.theme-cards {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 10px;
	}

	.theme-card {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
		padding: 12px;
		background: transparent;
		border: 2px solid var(--color-border);
		border-radius: var(--radius-card);
		cursor: pointer;
		transition: border-color var(--duration-default) var(--ease-default), background-color var(--duration-default) var(--ease-default);
		font-family: var(--font-sans);
	}

	.theme-card:hover { background-color: var(--color-surface-hover); }
	.theme-card.active { border-color: var(--color-accent); background-color: rgba(10, 132, 255, 0.08); }

	.theme-preview {
		position: relative;
		overflow: hidden;
		width: 48px; height: 48px; border-radius: 10px;
		display: flex; align-items: center; justify-content: center;
	}

	.preview-system { background: linear-gradient(135deg, var(--color-bg-elevated) 50%, var(--color-text-primary) 50%); color: var(--color-text-primary); }

	.theme-label { font-size: 0.75rem; font-weight: 500; color: var(--color-text-primary); }

	.font-row { align-items: flex-start; }

	.font-size-badge {
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--color-accent);
		font-variant-numeric: tabular-nums;
	}

	/* Font picker */
	.font-picker { position: relative; }

	.font-picker-btn {
		display: flex; align-items: center; justify-content: space-between; gap: 8px;
		width: 100%; padding: 7px 10px;
		background: var(--color-bg-primary); border: 1px solid var(--color-border);
		border-radius: 6px; color: var(--color-text-primary);
		font-size: 0.8125rem; cursor: pointer; text-align: left;
		transition: border-color 0.15s ease;
	}
	.font-picker-btn:hover { border-color: var(--color-accent); }

	.chevron { transition: transform 0.15s ease; flex-shrink: 0; color: var(--color-text-secondary); }
	.chevron.open { transform: rotate(180deg); }

	.font-dropdown {
		position: absolute; top: calc(100% + 4px); right: 0; width: 260px;
		background: var(--color-bg-elevated); border: 1px solid var(--color-border);
		border-radius: 8px; box-shadow: var(--shadow-elevated);
		z-index: 100; overflow: hidden;
	}

	.font-search {
		width: 100%; padding: 8px 10px; border: none;
		border-bottom: 1px solid var(--color-border);
		background: transparent; color: var(--color-text-primary);
		font-family: var(--font-sans); font-size: 0.75rem; outline: none;
		box-sizing: border-box;
	}
	.font-search::placeholder { color: var(--color-text-secondary); opacity: 0.5; }

	.font-list {
		max-height: 280px; overflow-y: auto; padding: 4px 0;
		scrollbar-width: thin; scrollbar-color: var(--color-surface-active) transparent;
	}

	.font-group-label {
		padding: 6px 12px 3px; font-size: 0.625rem; font-weight: 600;
		text-transform: uppercase; letter-spacing: 0.05em;
		color: var(--color-text-secondary); opacity: 0.6;
	}

	.font-option {
		display: block; width: 100%; padding: 6px 12px; border: none;
		background: transparent; color: var(--color-text-primary);
		font-size: 0.8125rem; cursor: pointer; text-align: left;
		transition: background-color 0.1s ease;
	}
	.font-option:hover { background-color: var(--color-surface-hover); }
	.font-option.active { background-color: rgba(10, 132, 255, 0.12); color: var(--color-accent); }

	/* Preview box */
	.font-preview-box {
		margin-top: 12px;
		border: 1px solid var(--color-border);
		border-radius: 8px;
		overflow: hidden;
	}

	.preview-label {
		display: block; font-size: 0.625rem; font-weight: 600;
		text-transform: uppercase; letter-spacing: 0.05em;
		color: var(--color-text-secondary); padding: 8px 12px 0;
		font-family: var(--font-sans);
	}

	.preview-pre {
		margin: 0;
		padding: 12px;
		min-height: 96px;
		line-height: 1.6;
		white-space: pre;
		overflow-x: auto;
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border-radius: 0 0 8px 8px;
	}

	.font-empty {
		padding: 10px 12px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		font-family: var(--font-sans);
	}

</style>
