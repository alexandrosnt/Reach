<script lang="ts">
	import { getSettings, updateSetting } from '$lib/state/settings.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { onMount } from 'svelte';

	const settings = getSettings();
	let currentFont = $derived(settings.fontFamily || 'monospace');
	let currentSize = $derived(settings.fontSize || 14);
	let previewStyle = $derived(`font-family: '${currentFont}', monospace; font-size: ${currentSize}px`);

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


	// Google Fonts
	const MONOSPACE_FONTS = [
		'JetBrains Mono', 'Fira Code', 'Source Code Pro', 'Roboto Mono', 'Ubuntu Mono',
		'IBM Plex Mono', 'Space Mono', 'Inconsolata', 'Courier Prime', 'Anonymous Pro',
		'Share Tech Mono', 'Overpass Mono', 'Red Hat Mono', 'Martian Mono', 'Geist Mono',
		'DM Mono', 'Noto Sans Mono', 'B612 Mono', 'Azeret Mono', 'Major Mono Display',
		'Syne Mono', 'Xanh Mono', 'Cutive Mono', 'Nova Mono',
	];

	const SYSTEM_FONTS = [
		{ name: 'System Default', value: 'monospace' },
		{ name: 'SF Mono', value: 'SF Mono' },
		{ name: 'Cascadia Code', value: 'Cascadia Code' },
		{ name: 'Consolas', value: 'Consolas' },
		{ name: 'Menlo', value: 'Menlo' },
	];

	let pvFont = $state(settings.fontFamily || 'monospace');

	let fontSearch = $state('');
	let fontDropdownOpen = $state(false);
	let loadedFonts = $state<Set<string>>(new Set());

	let filteredFonts = $derived.by(() => {
		const q = fontSearch.toLowerCase();
		const google = MONOSPACE_FONTS.filter(f => f.toLowerCase().includes(q));
		const system = SYSTEM_FONTS.filter(f => f.name.toLowerCase().includes(q));
		return { google, system };
	});

	function loadGoogleFont(family: string): void {
		if (loadedFonts.has(family)) return;
		const id = `gf-${family.replace(/\s+/g, '-').toLowerCase()}`;
		if (document.getElementById(id)) {
			loadedFonts = new Set([...loadedFonts, family]);
			return;
		}
		const link = document.createElement('link');
		link.id = id;
		link.rel = 'stylesheet';
		link.href = `https://fonts.googleapis.com/css2?family=${encodeURIComponent(family)}:wght@400;700&display=swap`;
		document.head.appendChild(link);
		loadedFonts = new Set([...loadedFonts, family]);
	}

	function selectFont(family: string): void {
		loadGoogleFont(family);
		updateSetting('fontFamily', family);
		pvFont = family;
		fontDropdownOpen = false;
		fontSearch = '';
	}

	function selectSystemFont(value: string): void {
		updateSetting('fontFamily', value);
		pvFont = value;
		fontDropdownOpen = false;
		fontSearch = '';
	}

	// Preload visible fonts for preview
	function preloadVisibleFonts(): void {
		for (const f of MONOSPACE_FONTS.slice(0, 8)) {
			loadGoogleFont(f);
		}
	}

	// Load the currently selected font on mount
	onMount(() => {
		if (settings.fontFamily && !SYSTEM_FONTS.some(s => s.value === settings.fontFamily)) {
			loadGoogleFont(settings.fontFamily);
		}
	});

	// Load fonts when dropdown opens
	$effect(() => {
		if (fontDropdownOpen) {
			preloadVisibleFonts();
			// Load filtered fonts as user types
			for (const f of filteredFonts.google.slice(0, 10)) {
				loadGoogleFont(f);
			}
		}
	});
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
							{#if filteredFonts.system.length > 0}
								<div class="font-group-label">System Fonts</div>
								{#each filteredFonts.system as font (font.value)}
									<button
										class="font-option"
										class:active={currentFont === font.value}
										style:font-family="'{font.value}', monospace"
										onclick={() => selectSystemFont(font.value)}
									>
										{font.name}
									</button>
								{/each}
							{/if}
							{#if filteredFonts.google.length > 0}
								<div class="font-group-label">Google Fonts</div>
								{#each filteredFonts.google as font (font)}
									<button
										class="font-option"
										class:active={currentFont === font}
										style:font-family="'{font}', monospace"
										onclick={() => selectFont(font)}
									>
										{font}
									</button>
								{/each}
							{/if}
						</div>
					</div>
				{/if}
			</div>
		</div>
	</div>

	{#key `${pvFont}-${currentSize}`}
		<div class="font-preview-box">
			<span class="preview-label">Preview — {pvFont} @ {currentSize}px</span>
			<iframe
				title="Font Preview"
				class="preview-iframe"
				srcdoc={`<!DOCTYPE html><html><head><link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=${encodeURIComponent(pvFont)}:wght@400;700&display=swap"><style>*{margin:0;padding:0;background:#0a0a0a;color:#f5f5f7;}html,body{overflow:hidden;width:100%;height:100%;}pre{overflow:hidden;}</style></head><body><pre style="font-family:'${pvFont}',monospace;font-size:${currentSize}px;padding:12px;line-height:1.5;white-space:pre;">user@server:~$ ls -la\ntotal 42\ndrwxr-xr-x  2 root root 4096 Mar 20 08:00 .\n0123456789 ABCDEF abcdef</pre></body></html>`}
			></iframe>
		</div>
	{/key}
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

	.preview-iframe {
		width: 100%; height: 120px; border: none; display: block;
		border-radius: 0 0 8px 8px; overflow: hidden;
	}
</style>
