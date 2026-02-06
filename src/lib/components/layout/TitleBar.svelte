<script lang="ts">
	import Settings from '$lib/components/settings/Settings.svelte';
	import { registerSettingsOpener } from '$lib/state/shortcuts.svelte';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { getAISettings } from '$lib/state/ai.svelte';
	import { __APP_VERSION__ } from '$lib/version';
	import { toggleAIPanel, getAIChatState } from '$lib/state/ai-chat.svelte';

	let aiSettings = $derived(getAISettings());
	let aiConfigured = $derived(aiSettings.enabled && !!aiSettings.apiKey && !!aiSettings.selectedModel);
	let aiPanelOpen = $derived(getAIChatState().panelOpen);

	let settingsOpen = $state(false);

	function openSettings() {
		settingsOpen = true;
	}

	function closeSettings() {
		settingsOpen = false;
	}

	registerSettingsOpener(openSettings);

	function minimize() {
		getCurrentWindow().minimize();
	}

	function toggleMaximize() {
		getCurrentWindow().toggleMaximize();
	}

	function close() {
		getCurrentWindow().close();
	}
</script>

<header class="titlebar" data-tauri-drag-region>
	<div class="titlebar-left" data-tauri-drag-region>
		<img src="/app-icon.png" alt="" class="app-icon" draggable="false" />
		<span class="app-name">Reach</span>
		<span class="app-version">v{__APP_VERSION__}</span>
		<button class="settings-btn" onclick={openSettings} aria-label="Open settings">
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
				<circle cx="12" cy="12" r="3" />
				<path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
			</svg>
		</button>
		{#if aiConfigured}
			<button class="ai-btn" class:ai-active={aiPanelOpen} onclick={toggleAIPanel} aria-label="AI Assistant" title="AI Assistant (Ctrl+Shift+A)">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
					<path d="M12 2l2.09 6.26L20.18 10l-6.09 1.74L12 18l-2.09-6.26L3.82 10l6.09-1.74L12 2z" />
					<path d="M20 16l.88 2.64L23.52 20l-2.64.76L20 23.4l-.88-2.64L16.48 20l2.64-.76L20 16z" />
				</svg>
			</button>
		{/if}
	</div>

	<div class="titlebar-right">
		<button class="window-btn" onclick={minimize} aria-label="Minimize">
			<svg width="10" height="10" viewBox="0 0 10 10" fill="none">
				<path d="M1 5h8" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
			</svg>
		</button>

		<button class="window-btn" onclick={toggleMaximize} aria-label="Maximize">
			<svg width="10" height="10" viewBox="0 0 10 10" fill="none">
				<rect x="1" y="1" width="8" height="8" rx="1.5" stroke="currentColor" stroke-width="1.2" />
			</svg>
		</button>

		<button class="window-btn window-btn-close" onclick={close} aria-label="Close">
			<svg width="10" height="10" viewBox="0 0 10 10" fill="none">
				<path d="M1.5 1.5l7 7M8.5 1.5l-7 7" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
			</svg>
		</button>
	</div>
</header>

<Settings open={settingsOpen} onclose={closeSettings} />

<style>
	.titlebar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 38px;
		min-height: 38px;
		padding: 0 12px;
		background-color: var(--color-bg-secondary);
		border-bottom: 1px solid var(--color-border);
		user-select: none;
		-webkit-app-region: drag;
	}

	.titlebar-left {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.app-icon {
		width: 18px;
		height: 18px;
		-webkit-app-region: no-drag;
		pointer-events: none;
	}

	.app-name {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
		letter-spacing: 0.02em;
	}

	.app-version {
		font-size: 0.6875rem;
		font-weight: 400;
		color: var(--color-text-secondary);
		letter-spacing: 0.01em;
	}

	.settings-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		border: none;
		border-radius: var(--radius-btn);
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		-webkit-app-region: no-drag;
		transition:
			background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.settings-btn:hover {
		background-color: rgba(255, 255, 255, 0.06);
		color: var(--color-text-primary);
	}

	.ai-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		border: none;
		border-radius: var(--radius-btn);
		background: transparent;
		color: var(--color-accent);
		cursor: pointer;
		-webkit-app-region: no-drag;
		transition:
			background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default),
			box-shadow var(--duration-default) var(--ease-default);
	}

	.ai-btn:hover {
		background-color: rgba(10, 132, 255, 0.12);
		color: var(--color-accent);
	}

	.ai-btn.ai-active {
		background-color: rgba(10, 132, 255, 0.15);
		color: var(--color-accent);
		box-shadow: 0 0 8px rgba(10, 132, 255, 0.25);
	}

	.titlebar-right {
		display: flex;
		align-items: center;
		gap: 2px;
		-webkit-app-region: no-drag;
	}

	.window-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		border: none;
		border-radius: var(--radius-btn);
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.window-btn:hover {
		background-color: rgba(255, 255, 255, 0.06);
		color: var(--color-text-primary);
	}

	.window-btn-close:hover {
		background-color: var(--color-danger);
		color: #fff;
	}
</style>
