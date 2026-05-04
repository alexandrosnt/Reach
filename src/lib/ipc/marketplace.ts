import { invoke } from '@tauri-apps/api/core';
import type { PluginInfo, PluginPermission } from './plugin';

export interface MarketplaceEntry {
	id: string;
	name: string;
	version: string;
	description: string;
	author: string;
	repo: string;
	downloadUrl: string;
	sha256: string;
	permissions: PluginPermission[];
}

export async function marketplaceFetch(): Promise<MarketplaceEntry[]> {
	return invoke<MarketplaceEntry[]>('marketplace_fetch');
}

export async function marketplaceInstall(entry: MarketplaceEntry): Promise<PluginInfo> {
	return invoke<PluginInfo>('marketplace_install', { entry });
}

export async function marketplaceUninstall(pluginId: string): Promise<void> {
	return invoke('marketplace_uninstall', { pluginId });
}

export async function marketplaceGetUrl(): Promise<string> {
	return invoke<string>('marketplace_get_url');
}

export async function marketplaceSetUrl(url: string): Promise<void> {
	return invoke('marketplace_set_url', { url });
}
