<script lang="ts">
	import type { Snippet } from 'svelte';
	import TitleBar from './TitleBar.svelte';
	import TabBar from './TabBar.svelte';
	import Sidebar from './Sidebar.svelte';
	import StatusBar from './StatusBar.svelte';
	import Toast from '$lib/components/shared/Toast.svelte';
	import UpdateBanner from '$lib/components/shared/UpdateBanner.svelte';
	import UpdateDialog from '$lib/components/shared/UpdateDialog.svelte';
	import { getUpdaterState } from '$lib/state/updater.svelte';
	import { getActiveTab } from '$lib/state/tabs.svelte';
	import AIPanel from '$lib/components/ai/AIPanel.svelte';

	const updater = getUpdaterState();

	interface Props {
		children: Snippet;
	}

	let { children }: Props = $props();

	let sidebarCollapsed = $state(false);
	let activeTab = $derived(getActiveTab());
	let activeConnectionId = $derived(activeTab?.connectionId);
</script>

<div class="app-shell">
	<TitleBar />
	<TabBar />

	<div class="app-body">
		<Sidebar bind:collapsed={sidebarCollapsed} connectionId={activeConnectionId} />
		<main class="main-content">
			{@render children()}
		</main>
		<AIPanel connectionId={activeConnectionId} activeTabId={activeTab?.id} activeTabType={activeTab?.type} />
	</div>

	<StatusBar />
	<Toast />
	<UpdateBanner />
	<UpdateDialog open={updater.startupBlocking} />
</div>

<style>
	.app-shell {
		display: grid;
		grid-template-rows: 38px 36px 1fr 24px;
		width: 100vw;
		height: 100vh;
		overflow: hidden;
		background-color: var(--color-bg-primary);
	}

	.app-body {
		display: flex;
		overflow: hidden;
	}

	.main-content {
		flex: 1;
		overflow: auto;
		background-color: var(--color-bg-primary);
	}
</style>
