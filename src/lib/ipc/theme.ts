import { invoke } from '@tauri-apps/api/core';
import type { Theme } from '$lib/state/theme.svelte';

/** One row of the theme registry index. */
export interface ThemeEntry {
	id: string;
	name: string;
	author: string;
	version: string;
	appearance: 'dark' | 'light';
	description: string;
	url: string;
	sha256: string;
}

export async function themeFetchRegistry(url?: string): Promise<ThemeEntry[]> {
	return invoke<ThemeEntry[]>('theme_fetch_registry', { url: url ?? null });
}

/** Downloads, verifies the SHA-256, validates and stores the theme. */
export async function themeInstall(entry: ThemeEntry): Promise<Theme> {
	return invoke<Theme>('theme_install', { entry });
}

export async function themeListInstalled(): Promise<Theme[]> {
	return invoke<Theme[]>('theme_list_installed');
}

export async function themeUninstall(themeId: string): Promise<void> {
	return invoke('theme_uninstall', { themeId });
}

export async function themeDefaultRegistryUrl(): Promise<string> {
	return invoke<string>('theme_default_registry_url');
}
