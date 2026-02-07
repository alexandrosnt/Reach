<script lang="ts">
	import { getTabs, getActiveTab, updateTabTitle } from '$lib/state/tabs.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { ptySpawn, ptyClose } from '$lib/ipc/pty';
	import { sshDisconnect } from '$lib/ipc/ssh';
	import { monitoringStart, monitoringStop, monitoringGetStats } from '$lib/ipc/monitoring';
	import { updateStats, removeStats } from '$lib/state/monitoring.svelte';
	import { getEditorFile } from '$lib/state/editor.svelte';
	import Terminal from '$lib/components/terminal/Terminal.svelte';
	import MonitoringBar from '$lib/components/terminal/MonitoringBar.svelte';
	import FileEditorOverlay from '$lib/components/editor/FileEditorOverlay.svelte';

	/**
	 * Handle terminal title changes from OSC 2 escape sequences.
	 * When the shell sends a title like "root@hostname: /root",
	 * extract the username and update the tab title to reflect the
	 * effective user (e.g. after sudo su -).
	 */
	function handleTerminalTitleChange(tabId: string, terminalTitle: string): void {
		const tab = getTabs().find((t) => t.id === tabId);
		if (!tab || tab.type !== 'ssh') return;

		// Terminal titles from bash are typically: "user@hostname: dir"
		// Extract the username from before the first @
		const atIndex = terminalTitle.indexOf('@');
		if (atIndex <= 0) return;

		const effectiveUser = terminalTitle.substring(0, atIndex).trim();
		if (!effectiveUser) return;

		// Get the original host (IP/hostname) from the current tab title
		const currentAtIndex = tab.title.indexOf('@');
		const originalHost = currentAtIndex >= 0 ? tab.title.substring(currentAtIndex + 1) : '';
		if (!originalHost) return;

		const newTitle = `${effectiveUser}@${originalHost}`;
		if (newTitle !== tab.title) {
			updateTabTitle(tabId, newTitle);
		}
	}

	let tabs = $derived(getTabs());
	let activeTab = $derived(getActiveTab());
	let editorFile = $derived(getEditorFile());

	let spawnedPtys = $state(new Set<string>());
	let connectedSsh = $state(new Set<string>());
	let monitoredConnections = $state(new Set<string>());
	let pollIntervals = $state(new Map<string, ReturnType<typeof setInterval>>());

	// Spawn PTY for new local tabs
	$effect(() => {
		for (const tab of tabs) {
			if (tab.type === 'local' && !spawnedPtys.has(tab.id)) {
				spawnedPtys.add(tab.id);
				ptySpawn(tab.id).catch((err) => {
					console.error(`Failed to spawn PTY for tab ${tab.id}:`, err);
				});
			}
			if (tab.type === 'ssh' && tab.connectionId) {
				connectedSsh.add(tab.connectionId);
			}
		}
	});

	// Start monitoring for new SSH connections (poll-based)
	$effect(() => {
		for (const tab of tabs) {
			if (tab.type === 'ssh' && tab.connectionId && !monitoredConnections.has(tab.connectionId)) {
				const connId = tab.connectionId;
				monitoredConnections.add(connId);

				monitoringStart(connId).catch((err) => {
					console.error(`Failed to start monitoring for ${connId}:`, err);
				});

				// Poll for stats every 3 seconds
				const poll = async () => {
					try {
						const stats = await monitoringGetStats(connId);
						updateStats(connId, stats);
					} catch {
						// Stats not available yet — ignore
					}
				};
				// Initial fetch after a short delay
				setTimeout(poll, 1500);
				const interval = setInterval(poll, 3000);
				pollIntervals.set(connId, interval);
			}
		}
	});

	// Clean up PTYs, SSH connections, and monitoring for closed tabs
	$effect(() => {
		const tabIds = new Set(tabs.map((t) => t.id));
		const activeConnectionIds = new Set(
			tabs.filter((t) => t.connectionId).map((t) => t.connectionId!)
		);

		for (const id of spawnedPtys) {
			if (!tabIds.has(id)) {
				spawnedPtys.delete(id);
				ptyClose(id).catch((err) => {
					console.error(`Failed to close PTY ${id}:`, err);
				});
			}
		}

		for (const connId of connectedSsh) {
			if (!activeConnectionIds.has(connId)) {
				connectedSsh.delete(connId);
				sshDisconnect(connId).catch((err) => {
					console.error(`Failed to disconnect SSH ${connId}:`, err);
				});
			}
		}

		for (const connId of monitoredConnections) {
			if (!activeConnectionIds.has(connId)) {
				monitoredConnections.delete(connId);

				monitoringStop(connId).catch((err) => {
					console.error(`Failed to stop monitoring for ${connId}:`, err);
				});

				const interval = pollIntervals.get(connId);
				if (interval) {
					clearInterval(interval);
					pollIntervals.delete(connId);
				}

				removeStats(connId);
			}
		}
	});
</script>

<div class="page-container">
	{#if tabs.length === 0}
		<div class="empty-state">
			<svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="empty-icon">
				<path
					d="M4 17l6-5-6-5"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
				<path
					d="M12 19h8"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
				/>
			</svg>
			<h2 class="empty-title">{t('terminal.title')}</h2>
			<p class="empty-subtitle">{t('terminal.empty_hint')}</p>
		</div>
	{:else}
		<div class="terminal-area">
			{#each tabs as tab (tab.id)}
				<div class="terminal-wrapper" class:active={tab.id === activeTab?.id}>
					<Terminal
						ptyId={tab.id}
						type={tab.type}
						connectionId={tab.connectionId}
						active={tab.id === activeTab?.id}
						onTitleChange={(title) => handleTerminalTitleChange(tab.id, title)}
					/>
				</div>
			{/each}
		</div>
		<MonitoringBar connectionId={activeTab?.connectionId} sshUser={activeTab?.title?.split('@')[0]} />
	{/if}

	{#if editorFile}
		<FileEditorOverlay />
	{/if}
</div>

<style>
	.page-container {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		overflow: hidden;
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 12px;
		padding: 32px;
		flex: 1;
	}

	.empty-icon {
		color: var(--color-text-secondary);
		opacity: 0.4;
	}

	.empty-title {
		margin: 0;
		font-size: 1.5rem;
		font-weight: 500;
		color: var(--color-text-secondary);
		letter-spacing: -0.01em;
	}

	.empty-subtitle {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
		opacity: 0.6;
	}

	.terminal-area {
		flex: 1;
		position: relative;
		overflow: hidden;
	}

	.terminal-wrapper {
		position: absolute;
		inset: 0;
		display: none;
	}

	.terminal-wrapper.active {
		display: block;
	}
</style>
