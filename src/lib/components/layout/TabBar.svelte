<script lang="ts">
	import { getTabs, getActiveTab, createTab, closeTab, activateTab } from '$lib/state/tabs.svelte';

	let tabs = $derived(getTabs());
	let activeTab = $derived(getActiveTab());

	function handleNewTab(): void {
		createTab('local');
	}

	function handleCloseTab(e: MouseEvent, id: string): void {
		e.stopPropagation();
		closeTab(id);
	}

	// SVG icon paths
	const terminalIcon = 'M4 17l6-5-6-5M12 19h8';
	const sshIcon = 'M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z';
</script>

<nav class="tabbar">
	<div class="tabs-scroll">
		{#each tabs as tab (tab.id)}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="tab"
				class:active={tab.id === activeTab?.id}
				onclick={() => activateTab(tab.id)}
				onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') activateTab(tab.id); }}
				role="tab"
				tabindex="0"
				aria-selected={tab.id === activeTab?.id}
				title={tab.title}
			>
				<svg class="tab-icon" width="14" height="14" viewBox="0 0 24 24" fill="none">
					{#if tab.type === 'local'}
						<path d={terminalIcon} stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
					{:else}
						<path d={sshIcon} fill="currentColor" />
					{/if}
				</svg>

				<span class="tab-title">{tab.title}</span>

				<button
					class="tab-close"
					onclick={(e) => handleCloseTab(e, tab.id)}
					aria-label="Close tab {tab.title}"
				>
					<svg width="8" height="8" viewBox="0 0 8 8" fill="none">
						<path d="M1 1l6 6M7 1L1 7" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
					</svg>
				</button>
			</div>
		{/each}
	</div>

	<button class="new-tab-btn" onclick={handleNewTab} aria-label="New tab">
		<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
			<path d="M7 1v12M1 7h12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
		</svg>
	</button>
</nav>

<style>
	.tabbar {
		display: flex;
		align-items: stretch;
		height: 36px;
		min-height: 36px;
		background-color: var(--color-bg-primary);
		border-bottom: 1px solid var(--color-border);
		user-select: none;
	}

	.tabs-scroll {
		display: flex;
		flex: 1;
		overflow-x: auto;
		overflow-y: hidden;
		scrollbar-width: none;
	}

	.tabs-scroll::-webkit-scrollbar {
		display: none;
	}

	.tab {
		display: flex;
		align-items: center;
		gap: 6px;
		min-width: 120px;
		max-width: 200px;
		padding: 0 8px 0 12px;
		border: none;
		border-bottom: 2px solid transparent;
		background: transparent;
		color: var(--color-text-secondary);
		font-family: var(--font-sans);
		font-size: 0.75rem;
		cursor: pointer;
		white-space: nowrap;
		flex-shrink: 0;
		transition: background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default),
			border-color var(--duration-default) var(--ease-default);
	}

	.tab:hover {
		background-color: var(--color-bg-secondary);
		color: var(--color-text-primary);
	}

	.tab.active {
		background-color: var(--color-bg-secondary);
		color: var(--color-text-primary);
		border-bottom-color: var(--color-accent);
	}

	.tab-icon {
		flex-shrink: 0;
		opacity: 0.7;
	}

	.tab.active .tab-icon {
		opacity: 1;
	}

	.tab-title {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		text-align: left;
	}

	.tab-close {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 18px;
		height: 18px;
		border: none;
		border-radius: 4px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		flex-shrink: 0;
		opacity: 0;
		transition: opacity var(--duration-default) var(--ease-default),
			background-color var(--duration-default) var(--ease-default);
	}

	.tab:hover .tab-close {
		opacity: 1;
	}

	.tab-close:hover {
		background-color: rgba(255, 255, 255, 0.1);
		color: var(--color-text-primary);
	}

	.new-tab-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 36px;
		min-width: 36px;
		border: none;
		border-left: 1px solid var(--color-border);
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.new-tab-btn:hover {
		background-color: var(--color-bg-secondary);
		color: var(--color-text-primary);
	}
</style>
