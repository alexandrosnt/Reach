<script lang="ts">
	import { getSettings, updateSetting } from '$lib/state/settings.svelte';
	import { t } from '$lib/state/i18n.svelte';

	const settings = getSettings();
	let currentFont = $derived(settings.fontFamily || 'monospace');
	let currentSize = $derived(settings.fontSize || 14);

	type ThemeValue = 'dark' | 'light' | 'system';

	let themes = $derived([
		{ label: t('settings.theme_dark'), value: 'dark' as ThemeValue, icon: 'moon' },
		{ label: t('settings.theme_light'), value: 'light' as ThemeValue, icon: 'sun' },
		{ label: t('settings.theme_system'), value: 'system' as ThemeValue, icon: 'monitor' },
	]);

	function selectTheme(value: ThemeValue) {
		updateSetting('theme', value);
		applyTheme(value);
	}

	function applyTheme(theme: ThemeValue) {
		const root = document.documentElement;
		root.classList.remove('dark', 'light');

		if (theme === 'system') {
			const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
			root.classList.add(prefersDark ? 'dark' : 'light');
		} else {
			root.classList.add(theme);
		}
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
			{#each themes as theme (theme.value)}
				<button
					class="theme-card"
					class:active={settings.theme === theme.value}
					onclick={() => selectTheme(theme.value)}
				>
					<div class="theme-preview" class:preview-dark={theme.value === 'dark'} class:preview-light={theme.value === 'light'} class:preview-system={theme.value === 'system'}>
						{#if theme.icon === 'moon'}
							<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
								<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
							</svg>
						{:else if theme.icon === 'sun'}
							<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
								<circle cx="12" cy="12" r="5" />
								<line x1="12" y1="1" x2="12" y2="3" /><line x1="12" y1="21" x2="12" y2="23" />
								<line x1="4.22" y1="4.22" x2="5.64" y2="5.64" /><line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
								<line x1="1" y1="12" x2="3" y2="12" /><line x1="21" y1="12" x2="23" y2="12" />
								<line x1="4.22" y1="19.78" x2="5.64" y2="18.36" /><line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
							</svg>
						{:else}
							<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
								<rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
								<line x1="8" y1="21" x2="16" y2="21" /><line x1="12" y1="17" x2="12" y2="21" />
							</svg>
						{/if}
					</div>
					<span class="theme-label">{theme.label}</span>
				</button>
			{/each}
		</div>
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
			<input
				type="checkbox"
				class="toggle-checkbox"
				checked={settings.injectShellColors}
				onchange={(e) => updateSetting('injectShellColors', e.currentTarget.checked)}
			/>
		</div>
	</div>
</div>

<style>
	.tab-content {
		display: flex;
		flex-direction: column;
	}

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

	.theme-card:hover { background-color: rgba(255, 255, 255, 0.04); }
	.theme-card.active { border-color: var(--color-accent); background-color: rgba(10, 132, 255, 0.08); }

	.theme-preview {
		width: 48px; height: 48px; border-radius: 10px;
		display: flex; align-items: center; justify-content: center;
	}

	.preview-dark { background-color: #1c1c1e; color: #f5f5f7; }
	.preview-light { background-color: #f5f5f7; color: #1d1d1f; }
	.preview-system { background: linear-gradient(135deg, #1c1c1e 50%, #f5f5f7 50%); color: var(--color-text-primary); }

	.theme-label { font-size: 0.75rem; font-weight: 500; color: var(--color-text-primary); }

	.setting-row {
		display: flex; justify-content: space-between; align-items: center;
		padding: 12px 0; border-bottom: 1px solid var(--color-border); gap: 24px;
	}
	.setting-row:last-child { border-bottom: none; }

	.font-row { align-items: flex-start; }

	.setting-info { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
	.setting-label { font-size: 0.875rem; font-weight: 500; color: var(--color-text-primary); }
	.setting-description { font-size: 0.75rem; color: var(--color-text-secondary); }
	.setting-control { flex-shrink: 0; min-width: 180px; }


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
		border-radius: 8px; box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
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
		scrollbar-width: thin; scrollbar-color: rgba(255,255,255,0.1) transparent;
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
	.font-option:hover { background-color: rgba(255, 255, 255, 0.06); }
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
		background: #0a0a0a;
		border-radius: 0 0 8px 8px;
	}

	.font-empty {
		padding: 10px 12px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		font-family: var(--font-sans);
	}

	.toggle-checkbox {
		width: 18px;
		height: 18px;
		accent-color: var(--color-accent);
		cursor: pointer;
	}
</style>
