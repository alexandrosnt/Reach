<script lang="ts">
	import { rdpDisconnectAll } from '$lib/ipc/rdp';
	import { vncDisconnectAll } from '$lib/ipc/vnc';
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
	import AuthPromptDialog from '$lib/components/shared/AuthPromptDialog.svelte';
	import PassphrasePrompt from '$lib/components/pwmanager/PassphrasePrompt.svelte';
	import DuplicateVaultsDialog from '$lib/components/vault/DuplicateVaultsDialog.svelte';
	import { addToast } from '$lib/state/toasts.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import McpConfirmDialog from '$lib/components/shared/McpConfirmDialog.svelte';
	import { getUpdaterState, relaunchNow, postponeRelaunch } from '$lib/state/updater.svelte';
	import { getActiveTab, getTabs, isDesktopTab } from '$lib/state/tabs.svelte';
	import { getActivePage } from '$lib/state/navigation.svelte';
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

	// On a phone the sidebar is a full-screen drawer over the terminal: once a
	// tab opens or another is picked, close it so that tab is what you see.
	let lastTabId: string | undefined;
	$effect(() => {
		const id = activeTab?.id;
		if (id && id !== lastTabId && window.matchMedia('(max-width: 700px)').matches) {
			sidebarCollapsed = true;
		}
		lastTabId = id;
	});

	// The same for a workspace (Containers, Kubernetes, Databases…): picked
	// from the tab bar, it is what you came to see, not the session drawer.
	let lastPage: string | undefined;
	$effect(() => {
		const page = getActivePage();
		if (page !== lastPage && page !== 'terminal' && window.matchMedia('(max-width: 700px)').matches) {
			sidebarCollapsed = true;
		}
		lastPage = page;
	});

	// --- Active-session guards for window close / app quit + update relaunch ---
	let closeOpen = $state(false);
	let closeCount = $state(0);
	let closeMode = $state<'window' | 'quit'>('window');
	let updateOpen = $state(false);
	let updateCount = $state(0);

	/** Count live SSH connections (backend truth; falls back to connected tabs), plus open desktops. */
	async function countActiveConnections(): Promise<number> {
		const desktops = getTabs().filter(isDesktopTab).length;
		try {
			return (await sshListConnections()).length + desktops;
		} catch {
			return getTabs().filter((tab) => tab.type === 'ssh' && !!tab.connectionId).length + desktops;
		}
	}

	/** Actually leave: quit the whole app (tray Quit) or just close the window. */
	async function doExit(mode: 'window' | 'quit'): Promise<void> {
		// Desktops first, so each server gets a disconnect rather than a
		// dropped socket, and no session thread outlives the window.
		try {
			await Promise.all([rdpDisconnectAll(), vncDisconnectAll()]);
		} catch {
			// Nothing open, or the backend is already gone: leave anyway.
		}
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
	// A login that got in with the SSH agent or a password after the server
	// refused the session's own key. It works here, and fails on any device
	// without that agent (a phone): said now, rather than found out there.
	onMount(() => {
		// A session log that could not be opened: the session runs, but the
		// user is told at once that it is not being logged.
		const stopLogError = listen<{ host: string; message: string }>('ssh-log-error', (e) => {
			addToast(t('ssh.log_failed', { host: e.payload.host, message: e.payload.message }), 'error', 20000);
		});
		// A host key refused under ssh_config settings: the connection error
		// only says the key was not accepted, this says why.
		const stopHostKey = listen<{ host: string; port: number; reason: string }>('ssh-hostkey-refused', (e) => {
			addToast(t('hostkey.refused', { host: `${e.payload.host}:${e.payload.port}`, reason: e.payload.reason }), 'error', 20000);
		});
		const stopWeakKex = listen<{ host: string; port: number; kex: string }>('ssh-weak-kex', (e) => {
			addToast(t('ssh.weak_kex', { host: `${e.payload.host}:${e.payload.port}`, kex: e.payload.kex }), 'warning', 12000);
		});
		const unlisten = listen<{ host: string; fingerprint: string; via: 'agent' | 'password' }>('ssh-key-refused-notice', (e) => {
			const { host, fingerprint, via } = e.payload;
			addToast(t(via === 'agent' ? 'ssh.key_refused_agent' : 'ssh.key_refused_password', { host, fingerprint }), 'warning', 20000);
		});
		return () => {
			void unlisten.then((stop) => stop());
			void stopLogError.then((stop) => stop());
			void stopHostKey.then((stop) => stop());
			void stopWeakKex.then((stop) => stop());
		};
	});

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
		<!-- The assistant works on terminals. A desktop tab has no shell for it
		     to read or type into, so it is told there is no tab. -->
		<AIPanel
			connectionId={activeConnectionId}
			activeTabId={isDesktopTab(activeTab) ? undefined : activeTab?.id}
			activeTabType={activeTab?.type === 'rdp' || activeTab?.type === 'vnc' ? undefined : activeTab?.type}
		/>
	</div>

	<StatusBar />
	<Toast />
	<UpdateBanner />
	<UpdateDialog open={updater.startupBlocking} />
	<DuplicateVaultsDialog />

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
	<AuthPromptDialog />
	<PassphrasePrompt />
	<McpConfirmDialog />
</div>

<style>
	.app-shell {
		display: grid;
		grid-template-rows: var(--titlebar-h, 38px) var(--tabbar-h, 36px) 1fr var(--statusbar-h, 24px);
		width: 100vw;
		height: 100vh;
		/* dvh tracks the *visible* viewport, so the status bar is not pushed
		   under the on-screen keyboard or the mobile browser chrome. Declared
		   after the vh fallback for engines that do not support it. */
		height: 100dvh;
		/* Keep the title/status bars clear of the device status & navigation
		   bars on mobile. `env(safe-area-inset-*)` is 0 on desktop, so this is
		   a no-op there (requires viewport-fit=cover, set in app.html). */
		padding-top: env(safe-area-inset-top);
		padding-bottom: env(safe-area-inset-bottom);
		overflow: hidden;
		background-color: var(--color-bg-primary);
	}

	.app-body {
		position: relative;
		display: flex;
		overflow: hidden;
	}

	.main-content {
		flex: 1;
		min-width: 0;
		overflow: auto;
		background-color: var(--color-bg-primary);
	}

	/* Phones in landscape, and short desktop windows, have very little vertical
	   room. Trim the fixed chrome so the terminal keeps a usable number of rows. */
	@media (max-height: 480px) {
		.app-shell {
			--titlebar-h: 32px;
			--tabbar-h: 30px;
			--statusbar-h: 20px;
		}
	}
</style>
