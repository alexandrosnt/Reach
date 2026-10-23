import type { SshConnectParams } from '$lib/ipc/ssh';
import { rdpDisconnect, type RdpConnectParams } from '$lib/ipc/rdp';
import { vncDisconnect, type VncConnectParams } from '$lib/ipc/vnc';
import * as splits from '$lib/terminal/split';
import type { Direction, Split } from '$lib/terminal/split';

export type TabType = 'local' | 'ssh' | 'rdp' | 'vnc';

/** A tab that shows a remote desktop rather than a terminal. */
export function isDesktopTab(tab: Pick<Tab, 'type'> | undefined): boolean {
	return tab?.type === 'rdp' || tab?.type === 'vnc';
}

export interface Tab {
	id: string;
	title: string;
	type: TabType;
	connectionId?: string;
	active: boolean;
	/** Session display name (e.g. "MiniPC") */
	sessionName?: string;
	/** Detected OS ID for distro icon (e.g. "ubuntu", "debian") */
	detectedOs?: string | null;
	/** SSH connect params for reconnection */
	sshConnectParams?: SshConnectParams;
	/** What an RDP tab connects to; the panel connects itself once it knows its size. */
	rdpConnectParams?: RdpConnectParams;
	/** What a VNC tab connects to. */
	vncConnectParams?: VncConnectParams;
	/** A local tab that runs this program instead of the default shell
	 *  (a shell inside a container on this computer, say). */
	localCommand?: { program: string; args: string[] };
	/** The WSL distribution name if this tab is a WSL session. */
	wslDistro?: string;
	/** The tab group it is in, if any (see TabGroup). */
	groupId?: string;
	/** An SSH tab whose connection dropped, or is being made again: shown on
	 *  the tab, so a dead session is seen from any tab, not only its own. */
	link?: 'down' | 'reconnecting';
}

let tabs = $state<Tab[]>([]);

let activeTab = $derived<Tab | undefined>(tabs.find((t) => t.active));

/** Two tabs side by side (#106); see $lib/terminal/split. */
let split = $state<Split | null>(null);
/** A tab being dragged from the tab bar, for the terminal area to drop beside. */
let draggedTab = $state<string | null>(null);

export function getSplit(): Split | null {
	return split;
}

/**
 * Show `tabId` beside the focused tab, which keeps its place. The focused
 * tab itself (dragged out, say) goes beside its neighbour in the bar; with no
 * tab to pair it with, a new local terminal opens beside it.
 */
export function splitWith(tabId: string, direction: Direction): void {
	let first = activeTab?.id;
	if (first === tabId) first = splits.neighbour(tabs.map((t) => t.id), tabId) ?? undefined;
	if (!first) {
		const fresh = createTab('local');
		split = splits.open(tabId, fresh.id, direction, split?.ratio ?? 0.5);
		return;
	}
	const next = splits.open(first, tabId, direction, split?.ratio ?? 0.5);
	if (!next) return;
	split = next;
	activateTab(tabId);
}

export function unsplit(): void {
	split = null;
}

export function setSplitRatio(ratio: number): void {
	if (split) split.ratio = splits.clampRatio(ratio);
}

export function getDraggedTab(): string | null {
	return draggedTab;
}

export function setDraggedTab(id: string | null): void {
	draggedTab = id;
}

export function getTabs(): Tab[] {
	return tabs;
}

export function getActiveTab(): Tab | undefined {
	return activeTab;
}

/** "Local", then "Local 2", "Local 3": three tabs all called Local could not
 *  be told apart in the tab bar, the palette or the status bar. */
function numbered(base: string): string {
	const taken = new Set(tabs.map((t) => t.title));
	if (!taken.has(base)) return base;
	for (let n = 2; ; n++) {
		if (!taken.has(`${base} ${n}`)) return `${base} ${n}`;
	}
}

export function createTab(type: TabType, title?: string, connectionId?: string, sessionName?: string, detectedOs?: string | null, localCommand?: { program: string; args: string[] }, wslDistro?: string): Tab {
	const id = crypto.randomUUID();

	// In a split, a new tab takes the focused pane, as choosing a tab does.
	split = splits.activate(split, activeTab?.id, id);

	// Deactivate all existing tabs
	for (const tab of tabs) {
		tab.active = false;
	}

	const tab: Tab = {
		id,
		title: title ?? numbered(type === 'local' ? 'Local' : type === 'rdp' ? 'RDP' : type === 'vnc' ? 'VNC' : 'SSH'),
		type,
		connectionId,
		active: true,
		sessionName,
		detectedOs,
		localCommand,
		wslDistro,
	};

	tabs.push(tab);
	return tab;
}

export function closeTab(id: string): void {
	const index = tabs.findIndex((t) => t.id === id);
	if (index === -1) return;

	// A desktop's session lives in the backend and outlives its panel; the
	// tab going is what ends it.
	if (tabs[index].type === 'rdp') {
		void rdpDisconnect(id).catch(() => {});
	} else if (tabs[index].type === 'vnc') {
		void vncDisconnect(id).catch(() => {});
	}

	const wasActive = tabs[index].active;
	// One of a split's two closing ends the split; the other keeps focus.
	const after = splits.close(split, id);
	split = after.split;

	tabs.splice(index, 1);
	dropEmptyGroups();

	if (after.focus) {
		activateTab(after.focus);
	} else if (wasActive && tabs.length > 0) {
		// If the closed tab was active and there are remaining tabs, activate an adjacent one
		const newIndex = Math.min(index, tabs.length - 1);
		tabs[newIndex].active = true;
	}
}

export function activateTab(id: string): void {
	// A tab shown is never hidden in a folded group.
	const g = groupOfTab(id);
	if (g?.collapsed) g.collapsed = false;
	// In a split, a tab other than the two takes the focused pane.
	split = splits.activate(split, activeTab?.id, id);
	for (const tab of tabs) {
		tab.active = tab.id === id;
	}
}

export function updateTabTitle(id: string, title: string): void {
	const tab = tabs.find((t) => t.id === id);
	if (tab) {
		tab.title = title;
	}
}

/** Set by the tab's terminal; undefined when the connection is up. */
export function setTabLink(id: string, link: Tab['link']): void {
	const tab = tabs.find((t) => t.id === id);
	if (tab && tab.link !== link) tab.link = link;
}

export function updateTabOs(connectionId: string, os: string): void {
	const tab = tabs.find((t) => t.connectionId === connectionId);
	if (tab) {
		tab.detectedOs = os;
	}
}

export function updateTabConnection(tabId: string, newConnectionId: string): void {
	const tab = tabs.find((t) => t.id === tabId);
	if (tab) {
		tab.connectionId = newConnectionId;
	}
}

export function reorderTab(fromIndex: number, toIndex: number): void {
	if (
		fromIndex < 0 ||
		fromIndex >= tabs.length ||
		toIndex < 0 ||
		toIndex >= tabs.length ||
		fromIndex === toIndex
	) {
		return;
	}

	const [moved] = tabs.splice(fromIndex, 1);
	tabs.splice(toIndex, 0, moved);
}

// ---- Tab groups ---------------------------------------------------------
//
// A named, coloured run of tabs in the bar, as in a browser: its tabs stay
// side by side, and folding the group hides them (their sessions keep
// running). Groups last while their tabs are open; nothing is saved.

// Blue last: it is the selected tab's colour, and a group must not look selected.
export const GROUP_COLORS = ['green', 'purple', 'orange', 'cyan', 'pink', 'yellow', 'red', 'grey', 'blue'] as const;
export type GroupColor = (typeof GROUP_COLORS)[number];

export interface TabGroup {
	id: string;
	name: string;
	color: GroupColor;
	collapsed: boolean;
}

let groups = $state<TabGroup[]>([]);

export function getGroups(): TabGroup[] {
	return groups;
}

export function groupOfTab(tabId: string): TabGroup | undefined {
	const id = tabs.find((t) => t.id === tabId)?.groupId;
	return id ? groups.find((g) => g.id === id) : undefined;
}

/** Index just past the last tab of `groupId`, or -1 if it has none. */
function groupEnd(groupId: string): number {
	let end = -1;
	tabs.forEach((t, i) => {
		if (t.groupId === groupId) end = i + 1;
	});
	return end;
}

/** Move a tab to `to` (an index into the bar as it is before the move). */
function moveTab(tabId: string, to: number): void {
	const from = tabs.findIndex((t) => t.id === tabId);
	if (from === -1) return;
	const [tab] = tabs.splice(from, 1);
	tabs.splice(from < to ? to - 1 : to, 0, tab);
}

/** A new group holding `tabId`, named after it, in the next unused colour. */
export function createGroup(tabId: string, name?: string): TabGroup | undefined {
	const tab = tabs.find((t) => t.id === tabId);
	if (!tab) return undefined;
	if (tab.groupId) removeFromGroup(tabId);
	const used = new Set(groups.map((g) => g.color));
	const color = GROUP_COLORS.find((c) => !used.has(c)) ?? GROUP_COLORS[groups.length % GROUP_COLORS.length];
	const group: TabGroup = { id: crypto.randomUUID(), name: name ?? (tab.sessionName || tab.title), color, collapsed: false };
	groups.push(group);
	tab.groupId = group.id;
	return groups[groups.length - 1];
}

/** Put a tab in a group, after the group's last tab so its tabs stay together. */
export function addToGroup(tabId: string, groupId: string): void {
	const tab = tabs.find((t) => t.id === tabId);
	if (!tab || !groups.some((g) => g.id === groupId) || tab.groupId === groupId) return;
	const end = groupEnd(groupId);
	tab.groupId = groupId;
	if (end !== -1) moveTab(tabId, end);
	dropEmptyGroups();
}

/** Take a tab out of its group, placing it just after the group. */
export function removeFromGroup(tabId: string): void {
	const tab = tabs.find((t) => t.id === tabId);
	const groupId = tab?.groupId;
	if (!tab || !groupId) return;
	tab.groupId = undefined;
	const end = groupEnd(groupId);
	if (end !== -1) moveTab(tabId, end);
	dropEmptyGroups();
}

export function renameGroup(groupId: string, name: string): void {
	const g = groups.find((x) => x.id === groupId);
	if (g && name.trim()) g.name = name.trim();
}

export function setGroupColor(groupId: string, color: GroupColor): void {
	const g = groups.find((x) => x.id === groupId);
	if (g) g.color = color;
}

/** Fold or unfold a group. Folding the group of the tab in front shows the
 *  nearest tab outside it instead, if there is one. */
export function toggleGroup(groupId: string): void {
	const g = groups.find((x) => x.id === groupId);
	if (!g) return;
	g.collapsed = !g.collapsed;
	if (g.collapsed && activeTab?.groupId === groupId) {
		const outside = tabs.find((t) => t.groupId !== groupId && !groupHidden(t));
		if (outside) activateTab(outside.id);
	}
}

/** Whether a tab is hidden in a folded group. */
export function groupHidden(tab: Tab): boolean {
	if (!tab.groupId) return false;
	return groups.find((g) => g.id === tab.groupId)?.collapsed ?? false;
}

/** Undo a group; its tabs stay open where they are. */
export function ungroup(groupId: string): void {
	for (const t of tabs) if (t.groupId === groupId) t.groupId = undefined;
	groups = groups.filter((g) => g.id !== groupId);
}

/** Close every tab in a group. */
export function closeGroup(groupId: string): void {
	for (const t of tabs.filter((x) => x.groupId === groupId)) closeTab(t.id);
	groups = groups.filter((g) => g.id !== groupId);
}

function dropEmptyGroups(): void {
	const used = new Set(tabs.map((t) => t.groupId).filter(Boolean));
	if (groups.some((g) => !used.has(g.id))) groups = groups.filter((g) => used.has(g.id));
}
