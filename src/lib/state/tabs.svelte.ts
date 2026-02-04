export type TabType = 'local' | 'ssh';

export interface Tab {
	id: string;
	title: string;
	type: TabType;
	connectionId?: string;
	active: boolean;
}

let tabs = $state<Tab[]>([]);

let activeTab = $derived<Tab | undefined>(tabs.find((t) => t.active));

export function getTabs(): Tab[] {
	return tabs;
}

export function getActiveTab(): Tab | undefined {
	return activeTab;
}

export function createTab(type: TabType, title?: string, connectionId?: string): Tab {
	const id = crypto.randomUUID();

	// Deactivate all existing tabs
	for (const tab of tabs) {
		tab.active = false;
	}

	const tab: Tab = {
		id,
		title: title ?? (type === 'local' ? 'Local' : 'SSH'),
		type,
		connectionId,
		active: true
	};

	tabs.push(tab);
	return tab;
}

export function closeTab(id: string): void {
	const index = tabs.findIndex((t) => t.id === id);
	if (index === -1) return;

	const wasActive = tabs[index].active;

	tabs.splice(index, 1);

	// If the closed tab was active and there are remaining tabs, activate an adjacent one
	if (wasActive && tabs.length > 0) {
		const newIndex = Math.min(index, tabs.length - 1);
		tabs[newIndex].active = true;
	}
}

export function activateTab(id: string): void {
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
