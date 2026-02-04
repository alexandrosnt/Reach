<script lang="ts">
	import Dropdown from '$lib/components/shared/Dropdown.svelte';
	import { getSettings, updateSetting } from '$lib/state/settings.svelte';

	const settings = getSettings();

	type ThemeValue = 'dark' | 'light' | 'system';

	const themes: { label: string; value: ThemeValue; icon: string }[] = [
		{ label: 'Dark', value: 'dark', icon: 'moon' },
		{ label: 'Light', value: 'light', icon: 'sun' },
		{ label: 'System', value: 'system', icon: 'monitor' }
	];

	const fontOptions = [
		{ label: 'JetBrains Mono', value: 'JetBrains Mono' },
		{ label: 'SF Mono', value: 'SF Mono' },
		{ label: 'Cascadia Code', value: 'Cascadia Code' },
		{ label: 'Fira Code', value: 'Fira Code' },
		{ label: 'monospace', value: 'monospace' }
	];

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

	function onFontSizeInput(e: Event) {
		const target = e.target as HTMLInputElement;
		updateSetting('fontSize', parseInt(target.value, 10));
	}

	function onFontChange(value: string) {
		updateSetting('fontFamily', value);
	}
</script>

<div class="tab-content">
	<div class="setting-section">
		<span class="section-label">Theme</span>
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
								<line x1="12" y1="1" x2="12" y2="3" />
								<line x1="12" y1="21" x2="12" y2="23" />
								<line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
								<line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
								<line x1="1" y1="12" x2="3" y2="12" />
								<line x1="21" y1="12" x2="23" y2="12" />
								<line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
								<line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
							</svg>
						{:else}
							<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
								<rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
								<line x1="8" y1="21" x2="16" y2="21" />
								<line x1="12" y1="17" x2="12" y2="21" />
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
			<span class="setting-label">Font Size</span>
			<span class="setting-description">Terminal text size ({settings.fontSize}px)</span>
		</div>
		<div class="setting-control slider-control">
			<span class="range-value">{settings.fontSize}px</span>
			<input
				type="range"
				class="range-slider"
				min="10"
				max="24"
				step="1"
				value={settings.fontSize}
				oninput={onFontSizeInput}
			/>
		</div>
	</div>

	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">Terminal Font</span>
			<span class="setting-description">Font used in the terminal emulator</span>
		</div>
		<div class="setting-control">
			<Dropdown
				options={fontOptions}
				selected={settings.fontFamily}
				onchange={onFontChange}
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
		transition:
			border-color var(--duration-default) var(--ease-default),
			background-color var(--duration-default) var(--ease-default);
		font-family: var(--font-sans);
	}

	.theme-card:hover {
		background-color: rgba(255, 255, 255, 0.04);
	}

	.theme-card.active {
		border-color: var(--color-accent);
		background-color: rgba(10, 132, 255, 0.08);
	}

	.theme-preview {
		width: 48px;
		height: 48px;
		border-radius: 10px;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.preview-dark {
		background-color: #1c1c1e;
		color: #f5f5f7;
	}

	.preview-light {
		background-color: #f5f5f7;
		color: #1d1d1f;
	}

	.preview-system {
		background: linear-gradient(135deg, #1c1c1e 50%, #f5f5f7 50%);
		color: var(--color-text-primary);
	}

	.theme-label {
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.setting-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 12px 0;
		border-bottom: 1px solid var(--color-border);
		gap: 24px;
	}

	.setting-row:last-child {
		border-bottom: none;
	}

	.setting-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.setting-label {
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.setting-description {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.setting-control {
		flex-shrink: 0;
		min-width: 180px;
	}

	.slider-control {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.range-value {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-accent);
		min-width: 36px;
		text-align: right;
		font-variant-numeric: tabular-nums;
	}

	.range-slider {
		-webkit-appearance: none;
		appearance: none;
		width: 130px;
		height: 4px;
		border-radius: 2px;
		background: var(--color-border);
		outline: none;
		cursor: pointer;
	}

	.range-slider::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		width: 16px;
		height: 16px;
		border-radius: 50%;
		background: var(--color-accent);
		border: 2px solid #fff;
		box-shadow: 0 1px 4px rgba(0, 0, 0, 0.3);
		cursor: pointer;
		transition: transform var(--duration-default) var(--ease-default);
	}

	.range-slider::-webkit-slider-thumb:hover {
		transform: scale(1.15);
	}

	.range-slider::-moz-range-thumb {
		width: 16px;
		height: 16px;
		border-radius: 50%;
		background: var(--color-accent);
		border: 2px solid #fff;
		box-shadow: 0 1px 4px rgba(0, 0, 0, 0.3);
		cursor: pointer;
	}
</style>
