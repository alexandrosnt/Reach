<script lang="ts">
	import {
		getTabs, getActiveTab, createTab, closeTab, activateTab, getSplit, splitWith, unsplit, getDraggedTab, setDraggedTab, type Tab,
		getGroups, createGroup, addToGroup, removeFromGroup, renameGroup, setGroupColor, toggleGroup, groupHidden, ungroup, closeGroup,
		GROUP_COLORS, type GroupColor, type TabGroup
	} from '#lib/state/tabs.svelte.js';
	import DbMenu, { type MenuItem } from '#lib/components/db/DbMenu.svelte';
	import Modal from '#lib/components/shared/Modal.svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import { getActivePage, setActivePage, type Page } from '#lib/state/navigation.svelte.js';
	import { getEnabledDevopsTools } from '#lib/state/devops.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import DistroIcon from '#lib/components/sessions/DistroIcon.svelte';
	import * as mcp from '#lib/state/mcp.svelte.js';
	import { onMount } from 'svelte';

	let mcpStatus = $derived(mcp.getMcpStatus());

	// The share control is only rendered when the MCP server is running. A
	// button that cannot do anything is worse than no button: it invites a
	// click, fails, and teaches people the feature is broken.
	onMount(() => { mcp.refresh(); });

	/** The id the backend keys output on: the SSH connection, or the PTY id. */
	function shareIdOf(tab: { id: string; type: string; connectionId?: string }): string | null {
		if (tab.type === 'ssh') return tab.connectionId ?? null;
		if (tab.type === 'local') return tab.id;
		return null;
	}

	async function toggleShare(e: MouseEvent, tab: Tab): Promise<void> {
		e.stopPropagation();
		const id = shareIdOf(tab);
		if (!id) return;
		if (mcp.isShared(id)) {
			await mcp.unshare(id);
			return;
		}
		const isSsh = tab.type === 'ssh';
		await mcp.share(
			id,
			isSsh ? 'ssh' : 'local',
			tab.sshConnectParams?.host ?? 'localhost',
			tab.sshConnectParams?.username ?? ''
		);
	}

	let tabs = $derived(getTabs());
	let activeTab = $derived(getActiveTab());
	let activePage = $derived(getActivePage());

	function handleNewTab(): void {
		createTab('local');
	}

	function handleCloseTab(e: MouseEvent, id: string): void {
		e.stopPropagation();
		closeTab(id);
	}

	// Split view (#106): drag a tab onto the terminal, or use its menu.
	let split = $derived(getSplit());
	let dragged = $derived(getDraggedTab());
	let menu = $state<{ x: number; y: number; id: string } | null>(null);
	let pageMenu = $state<{ x: number; y: number } | null>(null);
	// When the page menu last closed: a press on its button closes it, and the
	// click that follows must not open it again.
	let pageMenuClosedAt = 0;
	/** A drag ended on this tab's own pointerup: that is not a click. */
	let swallowClick = false;
	const isMac = typeof navigator !== 'undefined' && /Mac/.test(navigator.platform);
	const mod = isMac ? '⌘⇧' : 'Ctrl+Shift+';

	function beginDrag(e: PointerEvent, id: string): void {
		if (e.button !== 0 || (e.target as HTMLElement).closest('button')) return;
		const x0 = e.clientX;
		const y0 = e.clientY;
		const move = (ev: PointerEvent) => {
			if (!getDraggedTab() && Math.hypot(ev.clientX - x0, ev.clientY - y0) > 6) {
				setDraggedTab(id);
				swallowClick = true;
			}
		};
		const up = () => {
			window.removeEventListener('pointermove', move);
			window.removeEventListener('pointerup', up);
		};
		window.addEventListener('pointermove', move);
		window.addEventListener('pointerup', up);
	}

	function onTabClick(id: string): void {
		if (swallowClick) {
			swallowClick = false;
			return;
		}
		activateTab(id);
	}

	function inSplit(id: string): boolean {
		return !!split && (split.first === id || split.second === id);
	}

	function menuItems(id: string): MenuItem[] {
		const items: MenuItem[] = [
			{ label: t('split.right'), hint: `${mod}E`, disabled: tabs.length < 2 && id !== activeTab?.id, action: () => splitWith(id, 'right') },
			{ label: t('split.down'), hint: `${mod}O`, disabled: tabs.length < 2 && id !== activeTab?.id, action: () => splitWith(id, 'down') }
		];
		if (split) items.push({ label: t('split.unsplit'), hint: `${mod}U`, action: unsplit });

		// Groups: a new one, any other there is, or out of the one it is in.
		const tab = tabs.find((x) => x.id === id);
		items.push({ label: '', separator: true }, { label: t('tabs.group_new'), icon: GROUP_ICON, action: () => startGroupFrom(id) });
		for (const g of groups) {
			if (g.id === tab?.groupId) continue;
			items.push({ label: t('tabs.group_add', { name: g.name }), icon: DOT_ICON, iconColor: groupColor(g.color), action: () => addToGroup(id, g.id) });
		}
		if (tab?.groupId) items.push({ label: t('tabs.group_remove'), action: () => removeFromGroup(id) });

		items.push({ label: '', separator: true }, { label: t('common.close'), action: () => closeTab(id) });
		return items;
	}

	// ---- Tab groups ---------------------------------------------------------

	let groups = $derived(getGroups());
	let groupMenu = $state<{ x: number; y: number; id: string } | null>(null);
	let renaming = $state<string | null>(null);
	let renameValue = $state('');
	let confirmClose = $state<TabGroup | null>(null);

	const GROUP_ICON = 'M4 6h7v5H4zM13 6h7v5h-7zM4 13h16v5H4z';
	const DOT_ICON = 'M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8z';
	const HEX: Record<GroupColor, string> = {
		blue: '#3b82f6',
		green: '#22c55e',
		yellow: '#eab308',
		red: '#ef4444',
		purple: '#a855f7',
		cyan: '#06b6d4',
		orange: '#f97316',
		pink: '#ec4899',
		grey: '#9ca3af'
	};

	function groupColor(c: GroupColor): string {
		return HEX[c];
	}

	/** First tab of its group in the bar: the group's label goes before it. */
	function startsGroup(i: number): boolean {
		const g = tabs[i].groupId;
		return !!g && (i === 0 || tabs[i - 1].groupId !== g);
	}

	function startGroupFrom(tabId: string): void {
		const g = createGroup(tabId);
		if (g) beginRename(g);
	}

	function beginRename(g: TabGroup): void {
		renaming = g.id;
		renameValue = g.name;
	}

	function finishRename(): void {
		if (renaming) renameGroup(renaming, renameValue);
		renaming = null;
	}

	function groupMenuItems(id: string): MenuItem[] {
		const g = groups.find((x) => x.id === id);
		if (!g) return [];
		const count = tabs.filter((x) => x.groupId === id).length;
		return [
			{ label: g.collapsed ? t('tabs.group_expand') : t('tabs.group_collapse'), action: () => toggleGroup(id) },
			{ label: t('tabs.group_rename'), action: () => beginRename(g) },
			{ label: '', separator: true },
			...GROUP_COLORS.map((c) => ({
				label: t(`tabs.color_${c}`),
				icon: DOT_ICON,
				iconColor: HEX[c],
				checked: g.color === c,
				action: () => setGroupColor(id, c)
			})),
			{ label: '', separator: true },
			{ label: t('tabs.group_ungroup'), action: () => ungroup(id) },
			{
				label: t('tabs.group_close'),
				danger: true,
				action: () => (count > 1 ? (confirmClose = g) : closeGroup(id))
			}
		];
	}

	/** Each page's icon, 24×24 and stroked, for the page button and menu. */
	const PAGE_ICONS: Record<string, string> = {
		terminal: 'M4 17l6-5-6-5M12 19h8',
		ansible: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM8.5 16.5 12 7l3.5 9.5M9.6 13.5h4.8',
		tofu: 'M12 2.5l8.2 4.75v9.5L12 21.5l-8.2-4.75v-9.5z',
		databases: 'M4 6c0-1.7 3.6-3 8-3s8 1.3 8 3-3.6 3-8 3-8-1.3-8-3zM4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3',
		containers: 'M3 7.5 12 3l9 4.5v9L12 21l-9-4.5zM3 7.5 12 12l9-4.5M12 12v9',
		kubernetes: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 7v3M12 14v3M7 12h3M14 12h3M8.5 8.5l2 2M13.5 13.5l2 2M15.5 8.5l-2 2M10.5 13.5l-2 2'
	};

	// The terminal always, then whichever DevOps tools are switched on.
	let pages = $derived<{ id: Page; label: () => string; icon: string }[]>([
		{ id: 'terminal', label: () => t('nav.terminal'), icon: PAGE_ICONS.terminal },
		...getEnabledDevopsTools().map((tool) => ({ id: tool.page, label: tool.label, icon: PAGE_ICONS[tool.page] })),
	]);
	let currentPage = $derived(pages.find((p) => p.id === activePage) ?? pages[0]);

	// SVG icon paths
	const terminalIcon = 'M4 17l6-5-6-5M12 19h8';
	/** A monitor: the tab shows a desktop, not a shell. */
	const rdpIcon = 'M4 5h16v11H4zM2 19h20M9 16v3M15 16v3';
	const sshIcon = 'M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z';
</script>

<nav class="tabbar">
	<!-- One button for the current page and a menu of the others: side by
	     side, Terminal, Ansible, OpenTofu and the rest took 550px, more than
	     a phone's screen and room the session tabs need on a desktop. -->
	<div class="page-switcher">
		{#if pages.length > 1}
			<button
				class="page-btn active page-pick"
				aria-haspopup="menu"
				aria-expanded={pageMenu !== null}
				onclick={(e) => {
					const r = e.currentTarget.getBoundingClientRect();
					if (performance.now() - pageMenuClosedAt < 400) return;
					pageMenu = { x: r.left, y: r.bottom + 4 };
				}}
			>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden="true">
					<path d={currentPage.icon} stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
				{currentPage.label()}
				<svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true"><path d="M2 4l3 3 3-3" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/></svg>
			</button>
		{:else}
			<button class="page-btn active" onclick={() => setActivePage('terminal')}>{t('nav.terminal')}</button>
		{/if}
	</div>

	{#if activePage === 'terminal'}
		<div class="divider"></div>

		<div class="tabs-scroll">
			{#each tabs as tab, i (tab.id)}
				{#if startsGroup(i)}
					{@const g = groups.find((x) => x.id === tab.groupId)}
					{#if g}
						{#if renaming === g.id}
							<!-- svelte-ignore a11y_autofocus -->
							<input
								class="group-chip group-rename"
								style:--group={groupColor(g.color)}
								bind:value={renameValue}
								aria-label={t('tabs.group_name')}
								autofocus
								onblur={finishRename}
								onkeydown={(e) => { if (e.key === 'Enter') finishRename(); else if (e.key === 'Escape') renaming = null; }}
							/>
						{:else}
							<button
								class="group-chip"
								class:collapsed={g.collapsed}
								style:--group={groupColor(g.color)}
								aria-expanded={!g.collapsed}
								title={g.collapsed ? t('tabs.group_expand') : t('tabs.group_collapse')}
								onclick={() => toggleGroup(g.id)}
								ondblclick={() => beginRename(g)}
								oncontextmenu={(e) => { e.preventDefault(); groupMenu = { x: e.clientX, y: e.clientY, id: g.id }; }}
							>
								<span class="group-dot"></span>
								<span class="group-name">{g.name}</span>
								{#if g.collapsed}<span class="group-count">{tabs.filter((x) => x.groupId === g.id).length}</span>{/if}
							</button>
						{/if}
					{/if}
				{/if}
				{#if !groupHidden(tab)}
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<div
					class="tab"
					class:grouped={!!tab.groupId}
					style:--group={tab.groupId ? groupColor(groups.find((x) => x.id === tab.groupId)?.color ?? 'grey') : undefined}
					class:active={tab.id === activeTab?.id}
					class:in-split={inSplit(tab.id)}
					class:dragging={dragged === tab.id}
					onclick={() => onTabClick(tab.id)}
					onpointerdown={(e) => beginDrag(e, tab.id)}
					oncontextmenu={(e) => {
						e.preventDefault();
						menu = { x: e.clientX, y: e.clientY, id: tab.id };
					}}
					onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') activateTab(tab.id); }}
					role="tab"
					tabindex="0"
					aria-selected={tab.id === activeTab?.id}
					title={tab.title}
				>
					{#if tab.type === 'ssh' && tab.detectedOs}
						<DistroIcon osId={tab.detectedOs} size={14} />
					{:else}
						<svg class="tab-icon" width="14" height="14" viewBox="0 0 24 24" fill="none">
							{#if tab.type === 'local'}
								<path d={terminalIcon} stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
							{:else if tab.type === 'rdp' || tab.type === 'vnc'}
								<path d={rdpIcon} stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
							{:else}
								<path d={sshIcon} fill="currentColor" />
							{/if}
						</svg>
					{/if}

					<span class="tab-title">{tab.sessionName || tab.title}</span>
					{#if tab.link}
						<!-- Seen from any tab: this session dropped, or is coming back. -->
						<span
							class="link-mark"
							class:reconnecting={tab.link === 'reconnecting'}
							role="img"
							aria-label={tab.link === 'reconnecting' ? t('terminal.reconnecting') : t('tabs.disconnected')}
							title={tab.link === 'reconnecting' ? t('terminal.reconnecting') : t('tabs.disconnected')}
						></span>
					{/if}

					{#if mcpStatus.enabled && shareIdOf(tab)}
						{@const sid = shareIdOf(tab) as string}
						{@const on = mcp.isShared(sid)}
						<button
							class="tab-share"
							class:shared={on}
							onclick={(e) => toggleShare(e, tab)}
							aria-pressed={on}
							title={on ? t('mcp.unshare_tab') : t('mcp.share_tab')}
							aria-label={on ? t('mcp.unshare_tab') : t('mcp.share_tab')}
						>
							<svg width="15" height="15" viewBox="0 0 24 24" fill="none" aria-hidden="true">
								{#if on}
									<!-- Live: a filled eye. Reads as "something is watching this" from
									     across the room, which is the whole point of this control. -->
									<path d="M2 12s3.6-6.5 10-6.5S22 12 22 12s-3.6 6.5-10 6.5S2 12 2 12z" fill="currentColor" opacity="0.9" />
									<circle cx="12" cy="12" r="2.6" fill="var(--color-bg-primary)" />
								{:else}
									<path d="M2 12s3.6-6.5 10-6.5S22 12 22 12s-3.6 6.5-10 6.5S2 12 2 12z" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" />
									<circle cx="12" cy="12" r="2.8" stroke="currentColor" stroke-width="1.8" />
								{/if}
							</svg>
						</button>
					{/if}

					<button
						class="tab-close"
						onclick={(e) => handleCloseTab(e, tab.id)}
						aria-label={t('common.close_tab', { title: tab.sessionName || tab.title })}
					>
						<svg width="8" height="8" viewBox="0 0 8 8" fill="none">
							<path d="M1 1l6 6M7 1L1 7" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
						</svg>
					</button>
				</div>
				{/if}
			{/each}
		</div>

		<button class="new-tab-btn" onclick={handleNewTab} aria-label={t('common.new_tab')}>
			<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
				<path d="M7 1v12M1 7h12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
			</svg>
		</button>
	{/if}
</nav>

{#if menu}
	<DbMenu x={menu.x} y={menu.y} items={menuItems(menu.id)} onclose={() => (menu = null)} />
{/if}
{#if pageMenu}
	<DbMenu
		x={pageMenu.x}
		y={pageMenu.y}
		items={pages.map((p) => ({ label: p.label(), icon: p.icon, checked: p.id === activePage, action: () => setActivePage(p.id) }))}
		onclose={() => {
			pageMenu = null;
			pageMenuClosedAt = performance.now();
		}}
	/>
{/if}
{#if groupMenu}
	<DbMenu x={groupMenu.x} y={groupMenu.y} items={groupMenuItems(groupMenu.id)} onclose={() => (groupMenu = null)} />
{/if}

<Modal open={confirmClose !== null} onclose={() => (confirmClose = null)} title={t('tabs.group_close')} maxWidth="420px">
	{#if confirmClose}
		<p class="confirm-text">
			{t('tabs.group_close_confirm', { name: confirmClose.name, count: String(tabs.filter((x) => x.groupId === confirmClose?.id).length) })}
		</p>
	{/if}
	{#snippet actions()}
		<Button variant="secondary" onclick={() => (confirmClose = null)}>{t('common.cancel')}</Button>
		<Button
			variant="danger"
			onclick={() => {
				if (confirmClose) closeGroup(confirmClose.id);
				confirmClose = null;
			}}>{t('tabs.group_close')}</Button
		>
	{/snippet}
</Modal>

<style>
	/* ---- Tab groups ---- */

	.group-chip {
		display: flex;
		align-items: center;
		gap: 6px;
		flex-shrink: 0;
		align-self: center;
		max-width: 140px;
		height: 22px;
		margin: 0 2px 0 6px;
		padding: 0 8px;
		border: none;
		border-radius: 999px;
		background: color-mix(in srgb, var(--group) 18%, transparent);
		color: var(--group);
		font-family: var(--font-sans);
		font-size: var(--text-2xs);
		font-weight: 600;
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.group-chip:hover {
		background: color-mix(in srgb, var(--group) 28%, transparent);
	}

	/* Folded, the chip stands for its tabs: solid, with how many. */
	.group-chip.collapsed {
		background: var(--group);
		color: var(--color-bg-primary);
	}

	.group-dot {
		flex-shrink: 0;
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: currentColor;
	}

	.group-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.group-count {
		font-variant-numeric: tabular-nums;
		opacity: 0.85;
	}

	.group-rename {
		width: 120px;
		outline: 1px solid var(--group);
		cursor: text;
	}

	/* A grouped tab carries the group's colour along its top edge; the bottom
	   one is the selected tab's. */
	.tab.grouped {
		box-shadow: inset 0 2px 0 color-mix(in srgb, var(--group) 80%, transparent);
		background-color: color-mix(in srgb, var(--group) 5%, transparent);
	}

	.confirm-text {
		margin: 0;
		font-size: var(--text-sm);
		line-height: 1.5;
		color: var(--color-text-primary);
	}

	.tabbar {
		display: flex;
		align-items: stretch;
		height: var(--tabbar-h, 36px);
		min-height: var(--tabbar-h, 36px);
		background-color: var(--color-bg-primary);
		border-bottom: 1px solid var(--color-border);
		user-select: none;
	}

	.page-switcher {
		display: flex;
		align-items: center;
		gap: 2px;
		padding: 0 8px;
		flex-shrink: 0;
	}

	.page-btn.page-pick {
		gap: 6px;
	}

	.page-btn {
		display: flex;
		align-items: center;
		padding: 4px 10px;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		font-weight: 500;
		cursor: pointer;
		white-space: nowrap;
		transition: background-color 150ms ease, color 150ms ease;
	}

	.page-btn:hover {
		background-color: var(--color-bg-secondary);
		color: var(--color-text-primary);
	}

	.page-btn.active {
		background-color: var(--color-bg-elevated, var(--color-bg-secondary));
		color: var(--color-text-primary);
	}

	.divider {
		width: 1px;
		margin: 8px 0;
		background: var(--color-border);
		flex-shrink: 0;
	}

	/* Scrolls its tabs: without min-width 0 it grew to fit them instead and
	   made the whole window wider than a phone's screen. */
	.tabs-scroll {
		display: flex;
		flex: 1;
		min-width: 0;
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
		transition: background-color 150ms ease, color 150ms ease, border-color 150ms ease;
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

	/* The other tab on screen in a split: shown, without the keyboard. */
	.tab.in-split:not(.active) {
		color: var(--color-text-primary);
		border-bottom-color: color-mix(in srgb, var(--color-accent) 40%, transparent);
	}

	.tab.dragging {
		opacity: 0.6;
		cursor: grabbing;
	}

	.tab-icon {
		flex-shrink: 0;
		opacity: 0.7;
	}

	.tab.active .tab-icon {
		opacity: 1;
	}

	/* Shape as well as colour: a solid red dot is a dropped connection, an
	   amber ring one being made again. */
	.link-mark {
		flex-shrink: 0;
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--color-danger);
	}

	.link-mark.reconnecting {
		background: transparent;
		box-shadow: inset 0 0 0 1.5px var(--color-warning);
		animation: link-pulse 1.2s ease-in-out infinite;
	}

	@keyframes link-pulse {
		50% {
			opacity: 0.35;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.link-mark.reconnecting {
			animation: none;
		}
	}

	.tab-title {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		text-align: left;
	}

	/* Only rendered while the MCP server runs. When a session is shared this
	   stays visible at all times, unlike .tab-close which is hover-only: an
	   AI being able to read a terminal is a state that must never be hidden.
	   Amber rather than green, for the same reason as the settings dot. */
	.tab-share {
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		width: 22px;
		height: 22px;
		padding: 0;
		border: none;
		border-radius: 50%;
		background: none;
		color: var(--color-text-tertiary);
		cursor: pointer;
		opacity: 0;
		position: relative;
	}

	.tab:hover .tab-share {
		opacity: 1;
	}

	/* Live. Deliberately loud: a filled amber eye on a tinted disc with a
	   pulsing ring, always visible and never hover-gated. Everything else on
	   a tab can afford to be subtle; a terminal an AI can read cannot be, and
	   the first attempt at 11px and hover-only was far too easy to miss. */
	.tab-share.shared {
		opacity: 1;
		color: var(--color-warning);
		background: color-mix(in srgb, var(--color-warning) 16%, transparent);
		box-shadow: 0 0 0 1px color-mix(in srgb, var(--color-warning) 45%, transparent);
	}

	.tab-share.shared::after {
		content: '';
		position: absolute;
		inset: -3px;
		border-radius: 50%;
		border: 1px solid var(--color-warning);
		animation: mcp-watch 2.4s var(--ease-default) infinite;
		pointer-events: none;
	}

	@keyframes mcp-watch {
		0% { transform: scale(0.85); opacity: 0.55; }
		70%, 100% { transform: scale(1.3); opacity: 0; }
	}

	/* The colour and ring carry the state on their own, so motion can go. */
	@media (prefers-reduced-motion: reduce) {
		.tab-share.shared::after { animation: none; opacity: 0.55; }
	}

	.tab-share:hover {
		background: var(--color-surface-hover);
		color: var(--color-text-primary);
	}

	.tab-share.shared:hover {
		color: var(--color-warning);
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
		transition: opacity 150ms ease, background-color 150ms ease;
	}

	.tab:hover .tab-close {
		opacity: 1;
	}

	.tab-close:hover {
		background-color: var(--color-surface-active);
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
		transition: background-color 150ms ease, color 150ms ease;
	}

	.new-tab-btn:hover {
		background-color: var(--color-bg-secondary);
		color: var(--color-text-primary);
	}

	/* 120px per tab overflows a phone at three tabs. Let them compress; the
	   label already truncates with an ellipsis. */
	@media (max-width: 600px) {
		.tab {
			min-width: 84px;
			max-width: 140px;
			padding: 0 6px 0 8px;
		}
	}

	@media (pointer: coarse) {
		.tab-close {
			width: 24px;
			height: 24px;
		}
	}
</style>
