<script lang="ts">
	import Dropdown from '$lib/components/shared/Dropdown.svelte';
	import Toggle from '$lib/components/shared/Toggle.svelte';
	import { getSettings, updateSetting } from '$lib/state/settings.svelte';

	const settings = getSettings();

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
</script>

<div class="tab-content">
	<div class="setting-row">
		<div class="setting-info">
			<span class="setting-label">Default Shell</span>
			<span class="setting-description">Shell used when opening new local terminals</span>
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
			<span class="setting-label">Startup Behavior</span>
			<span class="setting-description">Restore tabs from your previous session on launch</span>
		</div>
		<div class="setting-control">
			<Toggle
				checked={settings.openLastSession}
				label="Open with last session"
				onchange={onLastSessionChange}
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
