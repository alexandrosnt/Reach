<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
	import { open as shellOpen } from '@tauri-apps/plugin-shell';
	import FaIcon from '$lib/components/shared/FaIcon.svelte';
	import { faCopy, faUpRightFromSquare } from '@fortawesome/free-solid-svg-icons';
	import { addToast } from '$lib/state/toasts.svelte';
	import Dropdown from '$lib/components/shared/Dropdown.svelte';
	import DiscordIcon from '$lib/components/shared/DiscordIcon.svelte';
	import { COMMUNITY } from '$lib/data/community';
	import LanguageSelect from '$lib/components/shared/LanguageSelect.svelte';
	import Toggle from '$lib/components/shared/Toggle.svelte';
	import { getSettings, updateSetting, syncTraySettings } from '$lib/state/settings.svelte';
	import { effectiveThreshold, MIN_PASTE_THRESHOLD } from '$lib/terminal/paste';
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

	function onPasteWarningChange(checked: boolean) {
		updateSetting('warnOnMultilinePaste', checked);
	}

	/**
	 * A threshold below the minimum would stop a one-word paste, which
	 * reads as a bug rather than a setting, so the field settles to the
	 * lowest value that means anything. Turning the warning off is what the
	 * toggle above is for.
	 */
	function onPasteThresholdChange(e: Event & { currentTarget: HTMLInputElement }) {
		const typed = Number(e.currentTarget.value);
		const next = effectiveThreshold(typed);
		e.currentTarget.value = String(next);
		updateSetting('multilinePasteThreshold', next);
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
				hideLabel
				checked={settings.openLastSession}
				label={t('settings.open_last_session')}
				onchange={onLastSessionChange}
			/>
		</div>
	</div>

	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('settings.paste_warning')}</span>
			<span class="setting-description">{t('settings.paste_warning_desc')}</span>
		</div>
		<div class="setting-control">
			<Toggle
				hideLabel
				checked={settings.warnOnMultilinePaste}
				label={t('settings.paste_warning')}
				onchange={onPasteWarningChange}
			/>
		</div>
	</div>

	{#if settings.warnOnMultilinePaste}
		<div class="setting-row">
			<div class="setting-info">
				<span class="setting-label">{t('settings.paste_threshold')}</span>
				<span class="setting-description">{t('settings.paste_threshold_desc')}</span>
			</div>
			<div class="setting-control">
				<input
					class="threshold-input"
					type="number"
					min={MIN_PASTE_THRESHOLD}
					step="1"
					value={settings.multilinePasteThreshold}
					onchange={onPasteThresholdChange}
				/>
			</div>
		</div>
	{/if}

	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">{t('settings.minimize_to_tray')}</span>
			<span class="setting-description">{t('settings.tray_desc')}</span>
		</div>
		<div class="setting-control">
			<Toggle
				hideLabel
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
				hideLabel
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
			<!-- One control, two ways out of it: the body opens the invite, the
			     glyph on the right copies it. A second button beside the first
			     read as two half-width controls where every other row has one. -->
			<button
				class="discord-link"
				title={COMMUNITY.discordInvite}
				onclick={() => shellOpen(COMMUNITY.discordInvite)}
			>
				<DiscordIcon size={14} />
				<span>{t('settings.open_discord')}</span>
				<span class="ext"><FaIcon icon={faUpRightFromSquare} size={10} /></span>
				<!-- Nested interactive content is invalid inside a <button>, so the
				     copy affordance is a span with the keyboard behaviour written
				     out: Enter and Space, and a tab stop of its own. -->
				<span
					class="copy"
					role="button"
					tabindex="0"
					title={t('common.copy')}
					aria-label={t('common.copy')}
					onclick={async (e) => {
						// Without this the click reaches the parent and the browser
						// opens as well as the clipboard being written.
						e.stopPropagation();
						await navigator.clipboard.writeText(COMMUNITY.discordInvite);
						addToast(t('vault.copied_toast'), 'success');
					}}
					onkeydown={async (e) => {
						if (e.key !== 'Enter' && e.key !== ' ') return;
						e.preventDefault();
						e.stopPropagation();
						await navigator.clipboard.writeText(COMMUNITY.discordInvite);
						addToast(t('vault.copied_toast'), 'success');
					}}
				>
					<FaIcon icon={faCopy} size={12} />
				</span>
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
				<span class="ext"><FaIcon icon={faUpRightFromSquare} size={10} /></span>
			</button>
		</div>
	</div>
</div>

<style>
	.threshold-input {
		width: 100%;
		padding: 7px 10px;
		border-radius: var(--radius-btn);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: inherit;
		font-size: 0.8125rem;
	}

	.threshold-input:focus {
		outline: none;
		border-color: var(--color-accent);
	}


	/* The label takes the space; the two glyphs sit at the far edge. */
	.discord-link > span:not(.ext):not(.copy) {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	/* The glyph that says "this leaves the app". */
	.ext {
		display: inline-flex;
		margin-left: auto;
		color: var(--color-text-tertiary);
	}

	.discord-link:hover .ext {
		color: inherit;
	}

	/* Spacing, not a rule. A hairline here drew the eye to the divider rather
	   than to either glyph, which is the thing it was meant to stop doing. */
	.copy {
		display: inline-flex;
		align-items: center;
		padding: 3px;
		margin-left: 6px;
		border-radius: 3px;
		color: var(--color-text-tertiary);
		transition:
			color 0.15s,
			background-color 0.15s;
	}

	/* The hover tint is what separates it now: nothing until you reach for it. */
	.copy:hover {
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
	}

	.copy:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 1px;
	}

	/* Full column width, like the fields above: one edge for every control. */
	.discord-link {
		display: inline-flex;
		align-items: center;
		width: 100%;
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
