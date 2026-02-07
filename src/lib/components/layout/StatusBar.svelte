<script lang="ts">
	import { getActiveTab } from '$lib/state/tabs.svelte';
	import { t } from '$lib/state/i18n.svelte';

	let activeTab = $derived(getActiveTab());

	let currentTime = $state('');

	function formatTime(): string {
		const now = new Date();
		return now.toLocaleTimeString(undefined, {
			hour: '2-digit',
			minute: '2-digit',
			second: '2-digit'
		});
	}

	$effect(() => {
		currentTime = formatTime();

		const interval = setInterval(() => {
			currentTime = formatTime();
		}, 1000);

		return () => {
			clearInterval(interval);
		};
	});

	let connectionStatus = $derived.by(() => {
		if (!activeTab) return t('statusbar.no_session');
		if (activeTab.type === 'ssh' && activeTab.connectionId) {
			return t('statusbar.connected_to', { title: activeTab.title });
		}
		return t('statusbar.local');
	});
</script>

<footer class="statusbar">
	<div class="statusbar-left">
		<div class="status-indicator" class:connected={activeTab?.type === 'ssh'}></div>
		<span>{connectionStatus}</span>
	</div>

	<div class="statusbar-center">
		<span>{t('app.name')} v0.1.0</span>
	</div>

	<div class="statusbar-right">
		<span>{currentTime}</span>
	</div>
</footer>

<style>
	.statusbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 24px;
		min-height: 24px;
		padding: 0 12px;
		background-color: var(--color-bg-secondary);
		border-top: 1px solid var(--color-border);
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		user-select: none;
	}

	.statusbar-left {
		display: flex;
		align-items: center;
		gap: 6px;
		flex: 1;
	}

	.statusbar-center {
		flex-shrink: 0;
		text-align: center;
	}

	.statusbar-right {
		flex: 1;
		text-align: right;
		font-family: var(--font-mono);
		font-size: 0.625rem;
	}

	.status-indicator {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background-color: var(--color-text-secondary);
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.status-indicator.connected {
		background-color: var(--color-success);
	}
</style>
