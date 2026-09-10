<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
	import { open as shellOpen } from '@tauri-apps/plugin-shell';
	import Dropdown from '$lib/components/shared/Dropdown.svelte';
	import DiscordIcon from '$lib/components/shared/DiscordIcon.svelte';
	import { COMMUNITY } from '$lib/data/community';
	import LanguageSelect from '$lib/components/shared/LanguageSelect.svelte';
	import Toggle from '$lib/components/shared/Toggle.svelte';
	import { getSettings, updateSetting, syncTraySettings } from '$lib/state/settings.svelte';
	import { t, changeLocale } from '$lib/state/i18n.svelte';

	const settings = getSettings();

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
			<LanguageSelect value={settings.locale} onchange={onLanguageChange} />
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

	<!-- The permanent route to the community. The launch prompt can be
	     dismissed forever; this is what makes that safe to do. -->
	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('settings.community')}</span>
			<span class="setting-description">{t('settings.community_desc')}</span>
		</div>
		<div class="setting-control">
			<button class="discord-link" onclick={() => shellOpen(COMMUNITY.discordInvite)}>
				<DiscordIcon size={14} />
				<span>{COMMUNITY.discordLabel}</span>
			</button>
		</div>
	</div>

	<!-- Not a nag: a row that exists, once, where someone who wants to give
	     back can find it. No prompt, no badge, no reminder. -->
	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('settings.support')}</span>
			<span class="setting-description">{t('settings.support_desc')}</span>
		</div>
		<div class="setting-control">
			<button
				class="discord-link sponsor-link"
				title={COMMUNITY.sponsorLabel}
				onclick={() => shellOpen(COMMUNITY.sponsorUrl)}
			>
				<svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
					<path
						d="M4.25 2.5c-1.336 0-2.75 1.164-2.75 3 0 2.15 1.58 4.144 3.365 5.682A20.6 20.6 0 0 0 8 13.393a20.6 20.6 0 0 0 3.135-2.211C12.92 9.644 14.5 7.65 14.5 5.5c0-1.836-1.414-3-2.75-3-1.373 0-2.609.986-3.029 2.456a.75.75 0 0 1-1.442 0C6.859 3.486 5.623 2.5 4.25 2.5ZM8 14.25l-.345.666a.75.75 0 0 0 .69 0L8 14.25Z"
					/>
				</svg>
				<span>{t('settings.sponsor')}</span>
			</button>
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

	.discord-link {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		padding: 6px 12px;
		font-family: inherit;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		background: var(--color-surface-sunken, transparent);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		cursor: pointer;
		transition: color 0.15s, border-color 0.15s;
	}

	.discord-link:hover {
		color: #5865f2;
		border-color: color-mix(in srgb, #5865f2 50%, var(--color-border));
	}

	/* GitHub Sponsors' own pink, the way the Discord row takes blurple. */
	.sponsor-link:hover {
		color: #ea4aaa;
		border-color: color-mix(in srgb, #ea4aaa 50%, var(--color-border));
	}
</style>
