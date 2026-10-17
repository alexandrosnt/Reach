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
	wslDistro?: string
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
		title: title ?? (type === 'local' ? 'Local' : type === 'rdp' ? 'RDP' : type === 'vnc' ? 'VNC' : 'SSH'),
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

	if (after.focus) {
		activateTab(after.focus);
	} else if (wasActive && tabs.length > 0) {
		// If the closed tab was active and there are remaining tabs, activate an adjacent one
		const newIndex = Math.min(index, tabs.length - 1);
		tabs[newIndex].active = true;
	}
}

export function activateTab(id: string): void {
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
