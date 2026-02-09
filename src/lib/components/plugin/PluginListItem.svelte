<script lang="ts">
	import type { PluginInfo } from '$lib/ipc/plugin';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		plugin: PluginInfo;
		active: boolean;
		onclick: () => void;
		onToggle: (enabled: boolean) => void;
	}

	let { plugin, active, onclick, onToggle }: Props = $props();

	let statusLabel = $derived(
		plugin.status.status === 'Running'
			? t('plugin.status.running')
			: plugin.status.status === 'Loaded'
				? t('plugin.status.loaded')
				: plugin.status.status === 'Error'
					? t('plugin.status.error')
					: t('plugin.status.disabled')
	);

	let isEnabled = $derived(plugin.status.status !== 'Disabled');

	function handleToggle(e: Event): void {
		e.stopPropagation();
		onToggle(!isEnabled);
	}
</script>

<button class="plugin-item" class:active onclick={onclick} title={plugin.manifest.description}>
	<div class="plugin-info">
		<div class="plugin-header">
			<span class="status-dot {plugin.status.status.toLowerCase()}" title={statusLabel}></span>
			<span class="plugin-name" class:bold={active}>{plugin.manifest.name}</span>
			<span class="plugin-version">v{plugin.manifest.version}</span>
		</div>
	</div>

	<div class="toggle-wrapper" role="switch" aria-checked={isEnabled} tabindex={0} onclick={handleToggle} onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handleToggle(e); } }}>
		<label class="toggle" aria-label={t('plugin.toggle')}>
			<input type="checkbox" checked={isEnabled} onchange={handleToggle} tabindex={-1} />
			<span class="toggle-track"></span>
		</label>
	</div>
</button>

<style>
	.plugin-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		width: 100%;
		padding: 6px 8px;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		cursor: pointer;
		text-align: left;
		transition:
			background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.plugin-item:hover {
		background-color: rgba(255, 255, 255, 0.06);
	}

	.plugin-item.active {
		background-color: rgba(255, 255, 255, 0.08);
	}

	.plugin-info {
		flex: 1;
		min-width: 0;
		overflow: hidden;
	}

	.plugin-header {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.status-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		flex-shrink: 0;
	}

	.status-dot.running {
		background-color: var(--color-success);
		box-shadow: 0 0 4px var(--color-success);
	}

	.status-dot.loaded {
		background-color: var(--color-warning);
	}

	.status-dot.error {
		background-color: var(--color-danger);
	}

	.status-dot.disabled {
		background-color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.plugin-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--color-text-primary);
	}

	.plugin-name.bold {
		font-weight: 600;
	}

	.plugin-version {
		color: var(--color-text-secondary);
		font-size: 0.625rem;
		flex-shrink: 0;
	}

	.toggle-wrapper {
		flex-shrink: 0;
	}

	.toggle {
		position: relative;
		display: inline-block;
		width: 28px;
		height: 16px;
		cursor: pointer;
	}

	.toggle input {
		opacity: 0;
		width: 0;
		height: 0;
		position: absolute;
	}

	.toggle-track {
		position: absolute;
		inset: 0;
		background-color: rgba(255, 255, 255, 0.1);
		border-radius: 8px;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.toggle-track::after {
		content: '';
		position: absolute;
		top: 2px;
		left: 2px;
		width: 12px;
		height: 12px;
		border-radius: 50%;
		background-color: var(--color-text-secondary);
		transition:
			transform var(--duration-default) var(--ease-default),
			background-color var(--duration-default) var(--ease-default);
	}

	.toggle input:checked + .toggle-track {
		background-color: var(--color-accent);
	}

	.toggle input:checked + .toggle-track::after {
		transform: translateX(12px);
		background-color: #fff;
	}
</style>
