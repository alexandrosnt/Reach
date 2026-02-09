import { invoke } from '@tauri-apps/api/core';

// --- Types ---

export interface PluginManifest {
	id: string;
	name: string;
	version: string;
	description: string;
	author: string;
	entry: string;
	permissions: PluginPermission[];
	hooks: string[];
}

export type PluginPermission =
	| 'ssh_exec'
	| 'ssh_list_connections'
	| 'sftp_list'
	| 'sftp_read'
	| 'sftp_write'
	| 'vault_read'
	| 'vault_write'
	| 'tunnel_manage'
	| 'http'
	| 'notify'
	| 'ui';

export type PluginStatus =
	| { status: 'Loaded' }
	| { status: 'Running' }
	| { status: 'Error'; message: string }
	| { status: 'Disabled' };

export interface PluginInfo {
	manifest: PluginManifest;
	status: PluginStatus;
	grantedPermissions: PluginPermission[];
	hasUi: boolean;
}

export interface PluginConfig {
	id: string;
	enabled: boolean;
	grantedPermissions: PluginPermission[];
}

export type UiElement =
	| { type: 'text'; content: string; muted?: boolean }
	| { type: 'heading'; content: string; level?: number }
	| { type: 'button'; label: string; action: string; variant?: string }
	| { type: 'input'; label: string; key: string; value?: string; placeholder?: string }
	| { type: 'toggle'; label: string; key: string; checked?: boolean }
	| { type: 'select'; label: string; key: string; options: string[]; selected?: string }
	| { type: 'table'; headers: string[]; rows: string[][] }
	| { type: 'code'; content: string; language?: string }
	| { type: 'divider' }
	| { type: 'spacer' }
	| { type: 'row'; children: UiElement[] }
	| { type: 'column'; children: UiElement[] }
	| { type: 'alert'; content: string; level?: string }
	| { type: 'progress'; value: number; label?: string };

export interface PluginUiState {
	pluginId: string;
	title: string;
	elements: UiElement[];
}

// --- IPC Wrappers ---

export async function pluginDiscover(): Promise<PluginManifest[]> {
	return invoke<PluginManifest[]>('plugin_discover');
}

export async function pluginLoad(pluginId: string): Promise<PluginInfo> {
	return invoke<PluginInfo>('plugin_load', { pluginId });
}

export async function pluginUnload(pluginId: string): Promise<void> {
	return invoke('plugin_unload', { pluginId });
}

export async function pluginReload(pluginId: string): Promise<PluginInfo> {
	return invoke<PluginInfo>('plugin_reload', { pluginId });
}

export async function pluginList(): Promise<PluginInfo[]> {
	return invoke<PluginInfo[]>('plugin_list');
}

export async function pluginCallAction(
	pluginId: string,
	action: string,
	params: Record<string, unknown> = {}
): Promise<PluginUiState | null> {
	return invoke<PluginUiState | null>('plugin_call_action', { pluginId, action, params });
}

export async function pluginGetUi(pluginId: string): Promise<PluginUiState | null> {
	return invoke<PluginUiState | null>('plugin_get_ui', { pluginId });
}

export async function pluginGetConfig(pluginId: string): Promise<PluginConfig | null> {
	return invoke<PluginConfig | null>('plugin_get_config', { pluginId });
}

export async function pluginSetConfig(config: PluginConfig): Promise<void> {
	return invoke('plugin_set_config', { config });
}

export async function pluginGetDir(): Promise<string> {
	return invoke<string>('plugin_get_dir');
}

export async function pluginSetDir(dir: string): Promise<void> {
	return invoke('plugin_set_dir', { dir });
}

export async function pluginDispatchHook(
	eventName: string,
	data: Record<string, unknown> = {}
): Promise<void> {
	return invoke('plugin_dispatch_hook', { eventName, data });
}
