/**
 * Settings IPC wrappers.
 * All operations are O(1) - settings stored encrypted in vault.
 */

import { invoke } from '@tauri-apps/api/core';

/** App settings structure */
export interface AppSettings {
	openrouterApiKey: string | null;
	openrouterUrl: string | null;
	defaultAiModel: string | null;
}

/** Setting keys for O(1) lookup */
export const SETTING_KEYS = {
	OPENROUTER_API_KEY: 'openrouter_api_key',
	OPENROUTER_URL: 'openrouter_url',
	DEFAULT_AI_MODEL: 'default_ai_model'
} as const;

/** Get all app settings. O(1) per setting. */
export async function getAll(): Promise<AppSettings> {
	return invoke<AppSettings>('settings_get_all');
}

/** Get a single setting by key. O(1). */
export async function get(key: string): Promise<string | null> {
	return invoke<string | null>('settings_get', { key });
}

/** Set a setting value. O(1). */
export async function set(key: string, value: string): Promise<void> {
	return invoke('settings_set', { key, value });
}

/** Delete a setting. O(1). */
export async function remove(key: string): Promise<void> {
	return invoke('settings_delete', { key });
}

/** Save all app settings at once. */
export async function saveAll(settings: Partial<AppSettings>): Promise<void> {
	return invoke('settings_save_all', { settings });
}
