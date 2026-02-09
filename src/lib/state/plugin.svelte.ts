import type { PluginInfo, PluginManifest, PluginUiState } from '$lib/ipc/plugin';

let plugins = $state<PluginInfo[]>([]);
let discoveredManifests = $state<PluginManifest[]>([]);
let activePluginId = $state<string | null>(null);
let pluginUiStates = $state<Map<string, PluginUiState>>(new Map());
let loading = $state(false);
let error = $state<string | null>(null);

export function getPlugins(): PluginInfo[] {
	return plugins;
}
export function setPlugins(list: PluginInfo[]): void {
	plugins = list;
}

export function getDiscoveredManifests(): PluginManifest[] {
	return discoveredManifests;
}
export function setDiscoveredManifests(list: PluginManifest[]): void {
	discoveredManifests = list;
}

export function getActivePluginId(): string | null {
	return activePluginId;
}
export function setActivePluginId(id: string | null): void {
	activePluginId = id;
}

export function getPluginUiStates(): Map<string, PluginUiState> {
	return pluginUiStates;
}
export function setPluginUiState(pluginId: string, state: PluginUiState): void {
	pluginUiStates = new Map(pluginUiStates).set(pluginId, state);
}
export function removePluginUiState(pluginId: string): void {
	const next = new Map(pluginUiStates);
	next.delete(pluginId);
	pluginUiStates = next;
}

export function getLoading(): boolean {
	return loading;
}
export function setLoading(val: boolean): void {
	loading = val;
}

export function getError(): string | null {
	return error;
}
export function setError(msg: string | null): void {
	error = msg;
}
