<script lang="ts">
	import { getTabs, getActiveTab, updateTabTitle, updateTabConnection, createTab, activateTab, getSplit, setSplitRatio, getDraggedTab, setDraggedTab, splitWith, unsplit } from '$lib/state/tabs.svelte';
	import { paneOf, paneStyle, dividerStyle, dropZone, neighbour, type Direction } from '$lib/terminal/split';
	import { getActivePage } from '$lib/state/navigation.svelte';
	import { isDevopsEnabled } from '$lib/state/devops.svelte';
	import { t } from '$lib/state/i18n.svelte';
	import { ptySpawn, ptyClose } from '$lib/ipc/pty';
	import { sshDisconnect } from '$lib/ipc/ssh';
	import { monitoringStart, monitoringStop, monitoringGetStats, monitoringStartWsl } from '$lib/ipc/monitoring';
	import { updateStats, removeStats } from '$lib/state/monitoring.svelte';
	import Terminal from '$lib/components/terminal/Terminal.svelte';
	import RdpPanel from '$lib/components/rdp/RdpPanel.svelte';
	import MonitoringBar from '$lib/components/terminal/MonitoringBar.svelte';
	import RemoteTerminal from '$lib/components/share/RemoteTerminal.svelte';
	import * as share from '$lib/state/share.svelte';
	import AnsiblePage from '$lib/components/ansible/AnsiblePage.svelte';
	import TofuPage from '$lib/components/tofu/TofuPage.svelte';
	import DbPage from '$lib/components/db/DbPage.svelte';
	import ContainersPage from '$lib/components/containers/ContainersPage.svelte';
	import K8sPage from '$lib/components/k8s/K8sPage.svelte';
	import EditorWindow from '$lib/components/editor/EditorWindow.svelte';

	const isEditorWindow = typeof window !== 'undefined' && new URLSearchParams(window.location.search).has('editor');

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
	let activePage = $derived(getActivePage());

	// Two tabs side by side (#106); the rules are in $lib/terminal/split.
	let areaEl = $state<HTMLDivElement>();
	let innerWidth = $state(1200);
	/** Below this width only the focused pane is shown; the split comes back with the room. */
	const SPLIT_MIN_WIDTH = 900;
	let split = $derived(getSplit());
	let shownSplit = $derived(
		split && innerWidth >= SPLIT_MIN_WIDTH && tabs.some((x) => x.id === split.first) && tabs.some((x) => x.id === split.second) ? split : null
	);
	let draggedTab = $derived(getDraggedTab());
	let dropAt = $state<Direction | null>(null);

	function startResize(e: PointerEvent): void {
		if (!areaEl || !shownSplit) return;
		e.preventDefault();
		const handle = e.currentTarget as HTMLElement;
		handle.setPointerCapture(e.pointerId);
		const rect = areaEl.getBoundingClientRect();
		const across = shownSplit.direction === 'right';
		const move = (ev: PointerEvent) => setSplitRatio(across ? (ev.clientX - rect.left) / rect.width : (ev.clientY - rect.top) / rect.height);
		const up = () => {
			handle.removeEventListener('pointermove', move);
			handle.removeEventListener('pointerup', up);
			handle.removeEventListener('pointercancel', up);
		};
		handle.addEventListener('pointermove', move);
		handle.addEventListener('pointerup', up);
		handle.addEventListener('pointercancel', up);
	}

	function onDividerKey(e: KeyboardEvent): void {
		if (!shownSplit) return;
		const step = e.shiftKey ? 0.1 : 0.02;
		const [back, fwd] = shownSplit.direction === 'right' ? ['ArrowLeft', 'ArrowRight'] : ['ArrowUp', 'ArrowDown'];
		if (e.key === back) setSplitRatio(shownSplit.ratio - step);
		else if (e.key === fwd) setSplitRatio(shownSplit.ratio + step);
		else if (e.key === 'Home') setSplitRatio(0);
		else if (e.key === 'End') setSplitRatio(1);
		else return;
		e.preventDefault();
	}

	// A tab dragged from the bar: where it would land, shown while it moves.
	$effect(() => {
		const id = draggedTab;
		if (!id) {
			dropAt = null;
			return;
		}
		const move = (ev: PointerEvent) => {
			const r = areaEl?.getBoundingClientRect();
			if (!r) return;
			const inside = ev.clientX >= r.left && ev.clientX <= r.right && ev.clientY >= r.top && ev.clientY <= r.bottom;
			dropAt = inside && innerWidth >= SPLIT_MIN_WIDTH ? dropZone(ev.clientX - r.left, ev.clientY - r.top, r.width, r.height) : null;
		};
		const up = () => {
			const at = dropAt;
			setDraggedTab(null);
			if (at) splitWith(id, at);
		};
		const key = (ev: KeyboardEvent) => {
			if (ev.key === 'Escape') setDraggedTab(null);
		};
		window.addEventListener('pointermove', move);
		window.addEventListener('pointerup', up);
		window.addEventListener('keydown', key);
		return () => {
			window.removeEventListener('pointermove', move);
			window.removeEventListener('pointerup', up);
			window.removeEventListener('keydown', key);
		};
	});

	// Ctrl+Shift+E splits right, Ctrl+Shift+O down (as Terminator), Ctrl+Shift+U
	// back to one. Caught before the terminal sees them: no shell binds them.
	$effect(() => {
		const onKey = (e: KeyboardEvent) => {
			if (activePage !== 'terminal' || !(e.ctrlKey || e.metaKey) || !e.shiftKey || e.altKey) return;
			const k = e.key.toLowerCase();
			if (k === 'e' || k === 'o') {
				const focused = activeTab?.id;
				if (!focused) return;
				e.preventDefault();
				e.stopPropagation();
				const other = neighbour(tabs.map((x) => x.id), focused);
				splitWith(other ?? focused, k === 'e' ? 'right' : 'down');
			} else if (k === 'u' && split) {
				e.preventDefault();
				e.stopPropagation();
				unsplit();
			}
		};
		window.addEventListener('keydown', onKey, { capture: true });
		return () => window.removeEventListener('keydown', onKey, { capture: true });
	});

	let dbVisited = $state(false);
	$effect(() => {
		if (activePage === 'databases') dbVisited = true;
	});
	let dbMounted = $derived(dbVisited && isDevopsEnabled('databases'));
	let containersVisited = $state(false);
	$effect(() => {
		if (activePage === 'containers') containersVisited = true;
	});
	let containersMounted = $derived(containersVisited && isDevopsEnabled('containers'));
	let k8sVisited = $state(false);
	$effect(() => {
		if (activePage === 'kubernetes') k8sVisited = true;
	});
	let k8sMounted = $derived(k8sVisited && isDevopsEnabled('kubernetes'));

	let spawnedPtys = $state(new Set<string>());
	let connectedSsh = $state(new Set<string>());
	let monitoredConnections = $state(new Set<string>());
	let pollIntervals = $state(new Map<string, ReturnType<typeof setInterval>>());

	// A local tab's PTY starts once its terminal listens, so the first
	// output (a WSL distro's banner, a prompt) is not lost.
	function handleTerminalReady(tab: (typeof tabs)[number]) {
		if (tab.type === 'local' && !spawnedPtys.has(tab.id)) {
			spawnedPtys.add(tab.id);
			ptySpawn(tab.id, tab.localCommand?.program, undefined, undefined, tab.localCommand?.args).catch((err) => {
				console.error(`Failed to spawn PTY for tab ${tab.id}:`, err);
			});
		}
	}

	// Track SSH connections opened in tabs
	$effect(() => {
		for (const tab of tabs) {
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

	// WSL monitoring: only poll while the WSL tab is in view to let the WSL VM go idle
	$effect(() => {
		const isWslInView = activePage === 'terminal' && activeTab?.wslDistro !== undefined && activeTab?.connectionId;
		if (isWslInView) {
			const connId = activeTab!.connectionId!;
			const distro = activeTab!.wslDistro!;

			monitoringStartWsl(connId, distro).catch((err) => {
				console.error(`Failed to start WSL monitoring for ${connId}:`, err);
			});

			const poll = async () => {
				try {
					const stats = await monitoringGetStats(connId);
					updateStats(connId, stats);
				} catch {
					// Stats not available yet — ignore
				}
			};

			const initialTimer = setTimeout(poll, 1500);
			const interval = setInterval(poll, 3000);

			return () => {
				clearTimeout(initialTimer);
				clearInterval(interval);
				monitoringStop(connId).catch((err) => {
					console.error(`Failed to stop WSL monitoring for ${connId}:`, err);
				});
			};
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

<svelte:window bind:innerWidth />

{#if isEditorWindow}
	<EditorWindow />
{:else}
	<div class="page-container">
		<div class="page-view" class:active={activePage === 'terminal'}>
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
					<button class="empty-action" onclick={() => createTab('local')}>
						{t('terminal.new_tab')}
					</button>
				</div>
			{:else}
				<!-- Side by side with the other person's terminal while a share is
				     connected and they are sharing one. Left is always mine. -->
				<div class="terminal-split" class:sharing={share.isShowingRemote()}>
				<div class="terminal-area" bind:this={areaEl}>
					{#each tabs as tab (tab.id)}
						{@const pane = shownSplit ? paneOf(shownSplit, tab.id) : null}
						{@const focused = tab.id === activeTab?.id}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<div
							class="terminal-wrapper"
							class:active={focused || pane !== null}
							class:pane={pane !== null}
							class:focused={pane !== null && focused}
							style={pane && shownSplit ? paneStyle(shownSplit, pane) : undefined}
							onpointerdowncapture={() => {
								if (pane && !focused) activateTab(tab.id);
							}}
						>
							{#if tab.type === 'rdp' && tab.rdpConnectParams}
								<RdpPanel id={tab.id} params={tab.rdpConnectParams} active={focused} />
							{:else if tab.type === 'vnc' && tab.vncConnectParams}
								<RdpPanel id={tab.id} params={tab.vncConnectParams} protocol="vnc" active={focused} />
							{:else if tab.type !== 'rdp' && tab.type !== 'vnc'}
								<Terminal
									ptyId={tab.id}
									type={tab.type}
									connectionId={tab.connectionId}
									active={focused}
									visible={focused || pane !== null}
									onTitleChange={(title) => handleTerminalTitleChange(tab.id, title)}
									sshConnectParams={tab.sshConnectParams}
									onReconnected={(newId) => updateTabConnection(tab.id, newId)}
									onReady={() => handleTerminalReady(tab)}/>
							{/if}
						</div>
					{/each}
					{#if shownSplit}
						<!-- A focusable separator is ARIA's window splitter: drag it, or move it with the arrow keys. -->
						<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
						<div
							class="split-divider"
							class:down={shownSplit.direction === 'down'}
							style={dividerStyle(shownSplit)}
							role="separator"
							tabindex="0"
							aria-orientation={shownSplit.direction === 'right' ? 'vertical' : 'horizontal'}
							aria-label={t('split.divider')}
							aria-valuemin={20}
							aria-valuemax={80}
							aria-valuenow={Math.round(shownSplit.ratio * 100)}
							onpointerdown={startResize}
							onkeydown={onDividerKey}
							ondblclick={() => setSplitRatio(0.5)}
						></div>
					{/if}
					{#if draggedTab && dropAt}
						<div class="drop-zone" class:down={dropAt === 'down'} aria-hidden="true">
							<span>{dropAt === 'right' ? t('split.drop_right') : t('split.drop_down')}</span>
						</div>
					{/if}
				</div>
				{#if share.isShowingRemote()}
					<RemoteTerminal />
				{/if}
				</div>
				<!-- A remote desktop has no server stats to show: under one, the bar
				     only said "Not connected" about a desktop that was. -->
				{#if activeTab?.type !== 'rdp' && activeTab?.type !== 'vnc'}
					<MonitoringBar
						connectionId={activeTab?.connectionId}
						sshUser={activeTab?.title?.split('@')[0]}
						sessionId={activeTab?.type === 'ssh' ? activeTab?.connectionId : activeTab?.id}
					/>
				{/if}
			{/if}
		</div>

		<!-- Mounted from the first visit until the tool is switched off, so a
		     query's text and results survive a trip to the terminal. -->
		{#if dbMounted}
			<div class="page-view" class:active={activePage === 'databases'}>
				<DbPage />
			</div>
		{/if}

		<!-- Kept mounted from the first visit, as Databases is, so an open log
		     and the chosen host survive a trip to the terminal. -->
		{#if containersMounted}
			<div class="page-view" class:active={activePage === 'containers'}>
				<ContainersPage />
			</div>
		{/if}

		{#if k8sMounted}
			<div class="page-view" class:active={activePage === 'kubernetes'}>
				<K8sPage />
			</div>
		{/if}

		{#if activePage === 'ansible' && isDevopsEnabled('ansible')}
			<AnsiblePage />
		{:else if activePage === 'tofu' && isDevopsEnabled('tofu')}
			<TofuPage />
		{/if}
	</div>
{/if}

<style>

	.page-container {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		overflow: hidden;
		position: relative;
	}

	.page-view {
		position: absolute;
		inset: 0;
		display: none;
		flex-direction: column;
	}

	.page-view.active {
		display: flex;
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--space-3);
		padding: var(--space-6);
		flex: 1;
	}

	.empty-icon {
		color: var(--color-text-tertiary);
	}

	/* Was --color-text-secondary at 40%/60% opacity: a grey heading on a grey
	   ground, which reads as disabled rather than as a title. */
	.empty-title {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
		color: var(--color-text-primary);
		letter-spacing: -0.01em;
	}

	.empty-subtitle {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
		text-align: center;
	}

	.empty-action {
		margin-top: var(--space-1);
		padding: var(--space-2) var(--space-4);
		font-family: var(--font-sans);
		font-size: var(--text-sm);
		font-weight: 500;
		color: #fff;
		background-color: var(--color-accent);
		border: none;
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.empty-action:hover {
		background-color: var(--color-accent-hover);
	}

	/* A 48px icon plus a 20px heading is most of a phone's terminal pane. Shrink
	   the decoration and keep the action. */
	@media (max-width: 600px), (max-height: 480px) {
		.empty-state {
			gap: var(--space-2);
			padding: var(--space-4);
		}

		.empty-icon {
			width: 32px;
			height: 32px;
		}

		.empty-title {
			font-size: var(--text-lg);
		}
	}

	.terminal-area {
		flex: 1;
		position: relative;
		/* Not hidden: that can still be scrolled, and a phone keyboard
		   opening scrolled the terminal's input into view, leaving the
		   terminal half off the top and a gap under it (#127). */
		overflow: clip;
	}

	.terminal-wrapper {
		position: absolute;
		inset: 0;
		display: none;
	}

	.terminal-wrapper.active {
		display: block;
	}

	/* Split (#106): which pane has the keyboard is drawn, faintly, so typing
	   never goes to the wrong server. */
	/* Drawn above the pane's content (a terminal paints its whole box), and
	   never in the way of a click. */
	.terminal-wrapper.pane::after {
		content: '';
		position: absolute;
		inset: 0;
		z-index: 5;
		pointer-events: none;
		border: 1px solid var(--color-border);
	}

	.terminal-wrapper.pane.focused::after {
		border: 2px solid var(--color-accent);
	}

	.split-divider {
		position: absolute;
		z-index: 2;
		cursor: col-resize;
		background: transparent;
		touch-action: none;
	}

	.split-divider.down {
		cursor: row-resize;
	}

	.split-divider::after {
		content: '';
		position: absolute;
		inset: 0;
		margin: auto;
		background: var(--color-border);
	}

	.split-divider:not(.down)::after {
		width: 1px;
	}

	.split-divider.down::after {
		height: 1px;
	}

	.split-divider:hover::after,
	.split-divider:focus-visible::after {
		background: var(--color-accent);
	}

	.split-divider:not(.down):hover::after,
	.split-divider:not(.down):focus-visible::after {
		width: 3px;
	}

	.split-divider.down:hover::after,
	.split-divider.down:focus-visible::after {
		height: 3px;
	}

	.split-divider:focus-visible {
		outline: none;
	}

	/* Where a tab dragged from the bar would land. */
	.drop-zone {
		position: absolute;
		z-index: 3;
		top: 0;
		bottom: 0;
		right: 0;
		width: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--color-accent) 16%, transparent);
		border: 2px dashed color-mix(in srgb, var(--color-accent) 70%, transparent);
		border-radius: 6px;
		pointer-events: none;
	}

	.drop-zone.down {
		top: auto;
		left: 0;
		width: auto;
		height: 50%;
	}

	.drop-zone span {
		padding: 6px 12px;
		border-radius: 999px;
		background: var(--color-accent);
		color: #fff;
		font-size: var(--text-sm);
		font-weight: 600;
	}
	.terminal-split {
		flex: 1;
		display: flex;
		min-height: 0;
		min-width: 0;
	}

	.terminal-split > .terminal-area {
		flex: 1;
		min-width: 0;
	}

	/* Half each, and a hairline between: theirs is a mirror, not a tab. */
	.terminal-split.sharing > :global(.remote-pane) {
		flex: 1;
		min-width: 0;
		border-left: 1px solid var(--color-border);
	}
</style>
