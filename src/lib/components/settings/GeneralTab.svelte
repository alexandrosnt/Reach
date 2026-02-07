<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
	import Dropdown from '$lib/components/shared/Dropdown.svelte';
	import Toggle from '$lib/components/shared/Toggle.svelte';
	import { getSettings, updateSetting, syncTraySettings } from '$lib/state/settings.svelte';
	import { t, changeLocale } from '$lib/state/i18n.svelte';

	const settings = getSettings();

	const languageOptions = [
		{ label: 'English', value: 'en' },
		{ label: 'Deutsch', value: 'de' },
		{ label: 'Français', value: 'fr' },
		{ label: 'Ελληνικά', value: 'el' },
		{ label: 'Italiano', value: 'it' }
	];

	function onLanguageChange(value: string) {
		changeLocale(value);
		updateSetting('locale', value);
	}

	const shellOptions = [
		{ label: 'Bash', value: '/bin/bash' },
		{ label: 'Zsh', value: '/bin/zsh' },
		{ label: 'PowerShell', value: 'powershell' },
		{ label: 'CMD', value: 'cmd' }
	];

	function onShellChange(value: string) {
		updateSetting('defaultShell', value);
	}

	function onLastSessionChange(checked: boolean) {
		updateSetting('openLastSession', checked);
	}

	async function onMinimizeToTrayChange(checked: boolean) {
		await invoke('set_close_to_tray', { enabled: checked });
		updateSetting('minimizeToTray', checked);
	}

	async function onStartWithSystemChange(checked: boolean) {
		if (checked) {
			await enable();
		} else {
			await disable();
		}
		updateSetting('startWithSystem', checked);
	}
</script>

<div class="tab-content">
	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('settings.language')}</span>
		</div>
		<div class="setting-control">
			<Dropdown
				options={languageOptions}
				selected={settings.locale}
				onchange={onLanguageChange}
			/>
		</div>
	</div>

	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('settings.default_shell')}</span>
			<span class="setting-description">{t('settings.shell_desc')}</span>
		</div>
		<div class="setting-control">
			<Dropdown
				options={shellOptions}
				selected={settings.defaultShell}
				onchange={onShellChange}
			/>
		</div>
	</div>

	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('settings.startup_behavior')}</span>
			<span class="setting-description">{t('settings.restore_tabs_desc')}</span>
		</div>
		<div class="setting-control">
			<Toggle
				checked={settings.openLastSession}
				label={t('settings.open_last_session')}
				onchange={onLastSessionChange}
			/>
		</div>
	</div>

	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('settings.minimize_to_tray')}</span>
			<span class="setting-description">{t('settings.tray_desc')}</span>
		</div>
		<div class="setting-control">
			<Toggle
				checked={settings.minimizeToTray}
				label={t('settings.minimize_to_tray')}
				onchange={onMinimizeToTrayChange}
			/>
		</div>
	</div>

	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('settings.start_with_system')}</span>
			<span class="setting-description">{t('settings.system_startup_desc')}</span>
		</div>
		<div class="setting-control">
			<Toggle
				checked={settings.startWithSystem}
				label={t('settings.start_with_system')}
				onchange={onStartWithSystemChange}
			/>
		</div>
	</div>
</div>

<style>
	.tab-content {
		display: flex;
		flex-direction: column;
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
</style>
