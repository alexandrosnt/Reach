import { invoke } from '@tauri-apps/api/core';
import * as settingsIpc from '$lib/ipc/settings';
import type { AppSettings } from '$lib/ipc/settings';

export interface Settings {
	theme: 'dark' | 'light' | 'system';
	fontSize: number;
	fontFamily: string;
	defaultShell: string;
	openLastSession: boolean;
	locale: string;
	minimizeToTray: boolean;
	startWithSystem: boolean;
	setupComplete: boolean;
	pendingTursoOrg: string;
	pendingTursoApiToken: string;
}

/** Secure settings stored encrypted in vault (API keys etc.) */
export interface SecureSettings {
	openrouterApiKey: string | null;
	openrouterUrl: string | null;
	defaultAiModel: string | null;
}

const STORAGE_KEY = 'reach-settings';

const defaults: Settings = {
	theme: 'dark',
	fontSize: 14,
	fontFamily: 'monospace',
	defaultShell: '/bin/bash',
	openLastSession: false,
	locale: 'en',
	minimizeToTray: false,
	startWithSystem: false,
	setupComplete: false,
	pendingTursoOrg: '',
	pendingTursoApiToken: ''
};

let settings = $state<Settings>({ ...defaults });

// Secure settings (API keys) - stored encrypted in vault
const secureDefaults: SecureSettings = {
	openrouterApiKey: null,
	openrouterUrl: null,
	defaultAiModel: null
};
let secureSettings = $state<SecureSettings>({ ...secureDefaults });
let secureLoading = $state(false);
let secureError = $state<string | null>(null);

export function getSettings(): Settings {
	return settings;
}

export function updateSetting<K extends keyof Settings>(key: K, value: Settings[K]): void {
	settings[key] = value;
	saveSettings();
}

export function loadSettings(): void {
	if (typeof localStorage === 'undefined') return;

	try {
		const stored = localStorage.getItem(STORAGE_KEY);
		if (stored) {
			const parsed = JSON.parse(stored) as Partial<Settings>;
			settings.theme = parsed.theme ?? defaults.theme;
			settings.fontSize = parsed.fontSize ?? defaults.fontSize;
			settings.fontFamily = parsed.fontFamily ?? defaults.fontFamily;
			settings.defaultShell = parsed.defaultShell ?? defaults.defaultShell;
			settings.openLastSession = parsed.openLastSession ?? defaults.openLastSession;
			settings.locale = parsed.locale ?? defaults.locale;
			settings.minimizeToTray = parsed.minimizeToTray ?? defaults.minimizeToTray;
			settings.startWithSystem = parsed.startWithSystem ?? defaults.startWithSystem;
			settings.pendingTursoOrg = parsed.pendingTursoOrg ?? defaults.pendingTursoOrg;
			settings.pendingTursoApiToken = parsed.pendingTursoApiToken ?? defaults.pendingTursoApiToken;
			// Migration: existing users who already have localStorage data get setupComplete: true
			settings.setupComplete = parsed.setupComplete ?? true;
		}
	} catch {
		// If parsing fails, keep defaults
	}
}

export function saveSettings(): void {
	if (typeof localStorage === 'undefined') return;

	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
	} catch {
		// Storage might be full or unavailable
	}
}

// --- Secure Settings (API keys in encrypted vault) ---

/** Get secure settings (read-only). */
export function getSecureSettings(): SecureSettings {
	return secureSettings;
}

/** Check if secure settings are loading. */
export function isSecureLoading(): boolean {
	return secureLoading;
}

/** Get secure settings error. */
export function getSecureError(): string | null {
	return secureError;
}

/** Check if API key is configured. */
export function hasApiKey(): boolean {
	return !!secureSettings.openrouterApiKey;
}

/** Get OpenRouter API key. */
export function getOpenRouterApiKey(): string | null {
	return secureSettings.openrouterApiKey;
}

/** Get OpenRouter URL (with default). */
export function getOpenRouterUrl(): string {
	return secureSettings.openrouterUrl ?? 'https://openrouter.ai/api/v1/chat/completions';
}

/** Get default AI model (with default). */
export function getDefaultAiModel(): string {
	return secureSettings.defaultAiModel ?? 'anthropic/claude-3.5-sonnet';
}

/** Load secure settings from vault. Call after vault unlock. */
export async function loadSecureSettings(): Promise<void> {
	secureLoading = true;
	secureError = null;

	try {
		const loaded = await settingsIpc.getAll();
		secureSettings.openrouterApiKey = loaded.openrouterApiKey;
		secureSettings.openrouterUrl = loaded.openrouterUrl;
		secureSettings.defaultAiModel = loaded.defaultAiModel;
	} catch (e) {
		secureError = e instanceof Error ? e.message : String(e);
	} finally {
		secureLoading = false;
	}
}

/** Set OpenRouter API key. O(1). */
export async function setOpenRouterApiKey(apiKey: string): Promise<void> {
	await settingsIpc.set(settingsIpc.SETTING_KEYS.OPENROUTER_API_KEY, apiKey);
	secureSettings.openrouterApiKey = apiKey;
}

/** Set OpenRouter URL. O(1). */
export async function setOpenRouterUrl(url: string): Promise<void> {
	await settingsIpc.set(settingsIpc.SETTING_KEYS.OPENROUTER_URL, url);
	secureSettings.openrouterUrl = url;
}

/** Set default AI model. O(1). */
export async function setDefaultAiModel(model: string): Promise<void> {
	await settingsIpc.set(settingsIpc.SETTING_KEYS.DEFAULT_AI_MODEL, model);
	secureSettings.defaultAiModel = model;
}

/** Clear secure settings (on vault lock). */
export function clearSecureSettings(): void {
	secureSettings.openrouterApiKey = null;
	secureSettings.openrouterUrl = null;
	secureSettings.defaultAiModel = null;
}

/** Sync minimizeToTray setting to Rust backend AtomicBool. Call after loadSettings(). */
export async function syncTraySettings(): Promise<void> {
	try {
		await invoke('set_close_to_tray', { enabled: settings.minimizeToTray });
	} catch {
		// Backend not ready yet
	}
}

/** Restore local settings from vault AppSettings (after backup import + relaunch). */
export async function restoreLocalSettingsFromVault(): Promise<void> {
	try {
		const vaultSettings = await invoke<Record<string, unknown>>('vault_get_settings');
		let changed = false;
		if (typeof vaultSettings.minimizeToTray === 'boolean') {
			settings.minimizeToTray = vaultSettings.minimizeToTray;
			changed = true;
		}
		if (typeof vaultSettings.startWithSystem === 'boolean') {
			settings.startWithSystem = vaultSettings.startWithSystem;
			changed = true;
		}
		if (typeof vaultSettings.defaultShell === 'string') {
			settings.defaultShell = vaultSettings.defaultShell;
			changed = true;
		}
		if (typeof vaultSettings.openLastSession === 'boolean') {
			settings.openLastSession = vaultSettings.openLastSession;
			changed = true;
		}
		if (typeof vaultSettings.fontSize === 'number') {
			settings.fontSize = vaultSettings.fontSize;
			changed = true;
		}
		if (typeof vaultSettings.fontFamily === 'string') {
			settings.fontFamily = vaultSettings.fontFamily;
			changed = true;
		}
		if (typeof vaultSettings.locale === 'string') {
			settings.locale = vaultSettings.locale;
			changed = true;
		}
		if (changed) {
			saveSettings();
			await syncTraySettings();
		}
	} catch {
		// Vault not unlocked or settings not available
	}
}
