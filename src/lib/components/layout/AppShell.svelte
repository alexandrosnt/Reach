<script lang="ts">
	import type { Snippet } from 'svelte';
	import { onMount, onDestroy } from 'svelte';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';
	import TitleBar from './TitleBar.svelte';
	import TabBar from './TabBar.svelte';
	import Sidebar from './Sidebar.svelte';
	import StatusBar from './StatusBar.svelte';
	import Toast from '$lib/components/shared/Toast.svelte';
	import UpdateBanner from '$lib/components/shared/UpdateBanner.svelte';
	import UpdateDialog from '$lib/components/shared/UpdateDialog.svelte';
	import ActiveSessionsDialog from '$lib/components/shared/ActiveSessionsDialog.svelte';
	import HostKeyDialog from '$lib/components/shared/HostKeyDialog.svelte';
	import { getUpdaterState, relaunchNow, postponeRelaunch } from '$lib/state/updater.svelte';
	import { getActiveTab, getTabs } from '$lib/state/tabs.svelte';
	import { getSettings } from '$lib/state/settings.svelte';
	import { sshListConnections } from '$lib/ipc/ssh';
	import AIPanel from '$lib/components/ai/AIPanel.svelte';

	const updater = getUpdaterState();

	interface Props {
		children: Snippet;
	}

	let { children }: Props = $props();

	let sidebarCollapsed = $state(false);
	let activeTab = $derived(getActiveTab());
	let activeConnectionId = $derived(activeTab?.connectionId);

	// --- Active-session guards for window close / app quit + update relaunch ---
	let closeOpen = $state(false);
	let closeCount = $state(0);
	let closeMode = $state<'window' | 'quit'>('window');
	let updateOpen = $state(false);
	let updateCount = $state(0);

	/** Count live SSH connections (backend truth; falls back to connected tabs). */
	async function countActiveConnections(): Promise<number> {
		try {
			return (await sshListConnections()).length;
		} catch {
			return getTabs().filter((tab) => tab.type === 'ssh' && !!tab.connectionId).length;
		}
	}

	/** Actually leave: quit the whole app (tray Quit) or just close the window. */
	async function doExit(mode: 'window' | 'quit'): Promise<void> {
		try {
			if (mode === 'quit') {
				await invoke('quit_app');
			} else {
				await getCurrentWindow().destroy();
			}
		} catch (e) {
			console.error('Exit failed:', e);
		}
	}

	/** Confirm first if SSH sessions are live; otherwise exit straight away. */
	async function requestExit(mode: 'window' | 'quit'): Promise<void> {
		const count = await countActiveConnections();
		if (count === 0) {
			await doExit(mode);
			return;
		}
		closeMode = mode;
		closeCount = count;
		closeOpen = true;
	}

	let unlistenClose: (() => void) | undefined;
	let unlistenQuit: (() => void) | undefined;
	onMount(async () => {
		try {
			// Window close (X / Alt+F4 / Cmd+Q). The frontend owns the decision
			// (single source of truth) — always prevent, then either hide to tray
			// or run the active-session guard. This avoids relying on the Rust
			// close-to-tray flag staying in sync (which caused X to close instead
			// of hiding to tray).
			unlistenClose = await getCurrentWindow().onCloseRequested(async (event) => {
				event.preventDefault(); // hold synchronously; we decide what to do
				if (getSettings().minimizeToTray) {
					await getCurrentWindow().hide(); // minimize to tray, not a termination
					return;
				}
				await requestExit('window');
			});
			// Tray "Quit" routes here (instead of a hard exit) so it warns about
			// active SSH sessions too.
			unlistenQuit = await listen('app-quit-requested', () => {
				void requestExit('quit');
			});
		} catch (e) {
			console.error('Failed to register exit handlers:', e);
		}
	});
	onDestroy(() => {
		unlistenClose?.();
		unlistenQuit?.();
	});

	async function confirmClose(): Promise<void> {
		closeOpen = false;
		await doExit(closeMode);
	}
	function cancelClose(): void {
		closeOpen = false;
	}

	// When an update finishes installing, confirm before relaunching if sessions
	// are live; otherwise relaunch immediately (e.g. startup with no sessions).
	let orchestrating = false;
	$effect(() => {
		if (!updater.readyToRelaunch) return;
		void orchestrateRelaunch();
	});

	async function orchestrateRelaunch(): Promise<void> {
		if (orchestrating) return;
		orchestrating = true;
		try {
			const count = await countActiveConnections();
			if (count === 0) {
				await relaunchNow();
				return;
			}
			updateCount = count;
			updateOpen = true;
		} finally {
			orchestrating = false;
		}
	}

	async function confirmUpdate(): Promise<void> {
		updateOpen = false;
		await relaunchNow();
	}
	function postponeUpdate(): void {
		updateOpen = false;
		postponeRelaunch();
	}
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

	<ActiveSessionsDialog
		open={closeOpen}
		variant="close"
		count={closeCount}
		onconfirm={confirmClose}
		oncancel={cancelClose}
	/>
	<ActiveSessionsDialog
		open={updateOpen}
		variant="update"
		count={updateCount}
		onconfirm={confirmUpdate}
		oncancel={postponeUpdate}
	/>
	<HostKeyDialog />
</div>

<style>
	.app-shell {
		display: grid;
		grid-template-rows: 38px 36px 1fr 24px;
		width: 100vw;
		height: 100vh;
		/* Keep the title/status bars clear of the device status & navigation
		   bars on mobile. `env(safe-area-inset-*)` is 0 on desktop, so this is
		   a no-op there (requires viewport-fit=cover, set in app.html). */
		padding-top: env(safe-area-inset-top);
		padding-bottom: env(safe-area-inset-bottom);
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
