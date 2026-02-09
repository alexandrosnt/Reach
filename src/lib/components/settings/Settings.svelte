<script lang="ts">
	import Modal from '$lib/components/shared/Modal.svelte';
	import GeneralTab from './GeneralTab.svelte';
	import AppearanceTab from './AppearanceTab.svelte';
	import SecurityTab from './SecurityTab.svelte';
	import SyncTab from './SyncTab.svelte';
	import BackupTab from './BackupTab.svelte';
	import AITab from './AITab.svelte';
	import PluginsTab from './PluginsTab.svelte';
	import { t } from '$lib/state/i18n.svelte';

	interface Props {
		open: boolean;
		onclose: () => void;
	}

	let { open, onclose }: Props = $props();

	type TabId = 'general' | 'appearance' | 'security' | 'sync' | 'backup' | 'ai' | 'plugins';

	let tabs = $derived([
		{ id: 'general' as TabId, label: t('settings.general'), icon: 'M12 15a3 3 0 100-6 3 3 0 000 6zM19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 01-2.83 2.83l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z' },
		{ id: 'appearance' as TabId, label: t('settings.appearance'), icon: 'M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z' },
		{ id: 'security' as TabId, label: t('settings.security'), icon: 'M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z' },
		{ id: 'ai' as TabId, label: t('settings.ai'), icon: 'M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z' },
		{ id: 'sync' as TabId, label: t('settings.sync'), icon: 'M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15' },
		{ id: 'backup' as TabId, label: t('settings.backup'), icon: 'M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4' },
		{ id: 'plugins' as TabId, label: t('settings.plugins'), icon: 'M13 2L3 14h9l-1 8 10-12h-9l1-8z' },
	]);

	let activeTabId = $state<TabId>('general');
</script>

<Modal {open} {onclose} title={t('settings.title')} maxWidth="680px">
	{#snippet children()}
		<div class="settings-layout">
			<nav class="settings-menu">
				{#each tabs as tab (tab.id)}
					<button
						class="menu-item"
						class:active={activeTabId === tab.id}
						onclick={() => (activeTabId = tab.id)}
					>
						<svg width="16" height="16" viewBox="0 0 24 24" fill="none">
							<path
								d={tab.icon}
								stroke="currentColor"
								stroke-width="1.8"
								stroke-linecap="round"
								stroke-linejoin="round"
							/>
						</svg>
						<span>{tab.label}</span>
					</button>
				{/each}
			</nav>

			<div class="settings-content">
				{#if activeTabId === 'general'}
					<GeneralTab />
				{:else if activeTabId === 'appearance'}
					<AppearanceTab />
				{:else if activeTabId === 'security'}
					<SecurityTab />
				{:else if activeTabId === 'sync'}
					<SyncTab />
				{:else if activeTabId === 'backup'}
					<BackupTab />
				{:else if activeTabId === 'ai'}
					<AITab />
				{:else if activeTabId === 'plugins'}
					<PluginsTab />
				{/if}
			</div>
		</div>
	{/snippet}
</Modal>

<style>
	.settings-layout {
		display: flex;
		gap: 0;
		min-height: 340px;
		margin: -20px;
	}

	.settings-menu {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 8px;
		width: 160px;
		flex-shrink: 0;
		border-right: 1px solid var(--color-border);
		background-color: var(--color-bg-secondary);
		border-radius: 0 0 0 var(--radius-modal, 12px);
	}

	.menu-item {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 7px 10px;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-secondary);
		background: transparent;
		border: none;
		border-radius: 6px;
		cursor: pointer;
		white-space: nowrap;
		transition:
			color var(--duration-default) var(--ease-default),
			background-color var(--duration-default) var(--ease-default);
	}

	.menu-item:hover {
		color: var(--color-text-primary);
		background-color: rgba(255, 255, 255, 0.06);
	}

	.menu-item.active {
		color: var(--color-text-primary);
		background-color: rgba(255, 255, 255, 0.1);
	}

	.menu-item svg {
		flex-shrink: 0;
		opacity: 0.7;
	}

	.menu-item.active svg {
		opacity: 1;
	}

	.settings-content {
		flex: 1;
		padding: 20px;
		overflow-y: auto;
		min-width: 0;
	}
</style>
