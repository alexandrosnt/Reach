<script lang="ts">
	import Modal from '$lib/components/shared/Modal.svelte';
	import GeneralTab from './GeneralTab.svelte';
	import AppearanceTab from './AppearanceTab.svelte';
	import SecurityTab from './SecurityTab.svelte';
	import AITab from './AITab.svelte';

	interface Props {
		open: boolean;
		onclose: () => void;
	}

	let { open, onclose }: Props = $props();

	type TabId = 'general' | 'appearance' | 'security' | 'ai';

	const tabs: { id: TabId; label: string }[] = [
		{ id: 'general', label: 'General' },
		{ id: 'appearance', label: 'Appearance' },
		{ id: 'security', label: 'Security' },
		{ id: 'ai', label: 'AI' }
	];

	let activeTabId = $state<TabId>('general');
</script>

<Modal {open} {onclose} title="Settings">
	{#snippet children()}
		<div class="settings-tabs">
			{#each tabs as tab (tab.id)}
				<button
					class="tab-btn"
					class:active={activeTabId === tab.id}
					onclick={() => (activeTabId = tab.id)}
				>
					{tab.label}
				</button>
			{/each}
		</div>

		<div class="settings-content">
			{#if activeTabId === 'general'}
				<GeneralTab />
			{:else if activeTabId === 'appearance'}
				<AppearanceTab />
			{:else if activeTabId === 'security'}
				<SecurityTab />
			{:else if activeTabId === 'ai'}
				<AITab />
			{/if}
		</div>
	{/snippet}
</Modal>

<style>
	.settings-tabs {
		display: flex;
		gap: 2px;
		padding: 4px;
		background-color: var(--color-bg-secondary);
		border-radius: 10px;
		margin-bottom: 20px;
	}

	.tab-btn {
		flex: 1;
		padding: 8px 16px;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-secondary);
		background: transparent;
		border: none;
		border-radius: 8px;
		cursor: pointer;
		transition:
			color var(--duration-default) var(--ease-default),
			background-color var(--duration-default) var(--ease-default);
	}

	.tab-btn:hover {
		color: var(--color-text-primary);
	}

	.tab-btn.active {
		color: var(--color-text-primary);
		background-color: var(--color-bg-elevated);
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12);
	}

	.settings-content {
		min-height: 260px;
	}
</style>
