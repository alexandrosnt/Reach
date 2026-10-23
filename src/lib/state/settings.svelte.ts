import { DEFAULT_PASTE_THRESHOLD } from '$lib/terminal/paste';
import { invoke } from '@tauri-apps/api/core';
import * as settingsIpc from '$lib/ipc/settings';
import type { AppSettings } from '$lib/ipc/settings';
import type { SessionLogConfig } from '$lib/ipc/ssh';
import { NEW_FEATURE_IDS } from '$lib/data/whats-new';

export interface Settings {
	theme: 'dark' | 'light' | 'system';
	/** Id of an installed theme. Overrides `theme` when set. */
	themeId?: string;
	fontSize: number;
	fontFamily: string;
	defaultShell: string;
	openLastSession: boolean;
	locale: string;
	minimizeToTray: boolean;
	startWithSystem: boolean;
	/** Type a colour and prompt setup into the remote shell after login. Off
	 *  unless the user turns it on: it is typed, so the shell records it in
	 *  its history, and servers colour themselves for an xterm-256color
	 *  terminal anyway, as they do for PuTTY. Saved under a new name so the
	 *  old default of "on" is not carried over. */
	typeShellSetup: boolean;
	/** Keep the server's login message (MOTD) on screen after connecting. */
	showLoginMessage: boolean;
	/** Write SSH sessions to a text file, as PuTTY's Logging panel does.
	 *  Off unless turned on: a log holds whatever the server printed. */
	sessionLogMode: 'off' | 'printable' | 'all';
	/** Folder for session logs; empty means the default (Documents/Reach Logs). */
	sessionLogFolder: string;
	/** File name with PuTTY's placeholders: &H host, &P port, &Y &M &D date, &T time. */
	sessionLogName: string;
	/** Add to an existing file rather than replace it. */
	sessionLogAppend: boolean;
	/** Start each log with a date line. */
	sessionLogHeader: boolean;
	/** Remote desktop: paint through WebGL (the GPU) rather than the 2D canvas. */
	rdpHardwareRendering: boolean;
	/** Remote desktop: advertise the graphics pipeline, for hosts that encode H.264. */
	rdpGraphicsPipeline: boolean;

	/**
	 * Confirm a multi-line paste before it reaches the shell.
	 *
	 * A newline runs what precedes it the moment it arrives, so a stray copy
	 * can execute on a production host before anyone has read it. On by
	 * default; off for people who paste blocks all day and have already
	 * answered the question.
	 */
	warnOnMultilinePaste: boolean;
	/** Warn once a paste reaches this many lines. See $lib/terminal/paste. */
	multilinePasteThreshold: number;

	setupComplete: boolean;
	pendingTursoOrg: string;
	/**
	 * Held in memory only, between the setup wizard and the Sync tab, which
	 * moves it into the encrypted vault. Never written to localStorage: a
	 * token there sits on disk in the clear.
	 */
	pendingTursoApiToken: string;

	/**
	 * State for the one-time nudge towards the Discord community.
	 *
	 * Three fields rather than one boolean, because a single "dismissed" flag
	 * can only express "never ask" or "ask on every single launch", and both
	 * are wrong. `launches` holds it back until someone has actually used the
	 * app, `communityPromptLastShown` turns "Not now" into a real deferral,
	 * and `communityPromptDismissed` is the checkbox that ends it for good.
	 */
	launches: number;
	communityPromptDismissed: boolean;
	/** Epoch ms of the last time the prompt was shown. 0 means never. */
	communityPromptLastShown: number;

	/**
	 * Ids from `$lib/data/whats-new` this user has already been shown.
	 *
	 * A fresh install starts with all of them: someone meeting Reach for the
	 * first time should not be told which three corners of it are recent.
	 */
	seenFeatures: string[];

	/**
	 * DevOps workspaces switched on, by id from `$lib/state/devops`.
	 *
	 * Per device rather than synced: the tools themselves are installed per
	 * machine, and a laptop without Ansible has no use for its workspace.
	 */
	devopsTools: string[];

	/** Lock after this many minutes without keyboard or mouse use; 0 never. */
	autoLockMinutes: number;
	/** Lock when the computer's own session locks or goes to sleep. */
	lockOnSystemLock: boolean;
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
	themeId: undefined,
	fontSize: 14,
	fontFamily: 'monospace',
	defaultShell: '/bin/bash',
	openLastSession: false,
	locale: 'en',
	minimizeToTray: false,
	startWithSystem: false,
	typeShellSetup: false,
	showLoginMessage: true,
	sessionLogMode: 'off',
	sessionLogFolder: '',
	sessionLogName: '&H-&Y&M&D-&T.log',
	sessionLogAppend: true,
	sessionLogHeader: true,
	rdpHardwareRendering: true,
	rdpGraphicsPipeline: false,
	warnOnMultilinePaste: true,
	multilinePasteThreshold: DEFAULT_PASTE_THRESHOLD,
	setupComplete: false,
	pendingTursoOrg: '',
	pendingTursoApiToken: '',
	launches: 0,
	communityPromptDismissed: false,
	communityPromptLastShown: 0,
	seenFeatures: [],
	devopsTools: [],
	autoLockMinutes: 0,
	lockOnSystemLock: false
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

/** The session log to ask `ssh_connect` for, or null when logging is off. */
export function sessionLogConfig(): SessionLogConfig | null {
	if (settings.sessionLogMode === 'off') return null;
	return {
		mode: settings.sessionLogMode,
		folder: settings.sessionLogFolder.trim(),
		name: settings.sessionLogName.trim(),
		append: settings.sessionLogAppend,
		header: settings.sessionLogHeader
	};
}

export function updateSetting<K extends keyof Settings>(key: K, value: Settings[K]): void {
	settings[key] = value;
	saveSettings();
}

export function loadSettings(): void {
	if (typeof localStorage === 'undefined') return;

	try {
		// Reach 0.1.0 kept the passwords "Remember password" saved here, in
		// the clear. 0.1.1 moved them to the vault and stopped reading this
		// key, but left what was in it; nothing uses it, so it goes.
		localStorage.removeItem('reach_saved_passwords');

		const stored = localStorage.getItem(STORAGE_KEY);

		// No stored settings at all means this is a first run. Everything in the
		// what's-new list is marked seen, because all of it is new to someone
		// who has never opened Reach, and highlighting an arbitrary three of it
		// is noise rather than news.
		if (!stored) {
			settings.seenFeatures = [...NEW_FEATURE_IDS];
			saveSettings();
			return;
		}

		if (stored) {
			const parsed = JSON.parse(stored) as Partial<Settings>;
			// Earlier versions wrote the pending Turso token here in the clear; the
			// next save leaves it out, so save once now to take it off the disk.
			if (parsed.pendingTursoApiToken) queueMicrotask(saveSettings);
			settings.theme = parsed.theme ?? defaults.theme;
			settings.fontSize = parsed.fontSize ?? defaults.fontSize;
			settings.fontFamily = parsed.fontFamily ?? defaults.fontFamily;
			settings.defaultShell = parsed.defaultShell ?? defaults.defaultShell;
			settings.openLastSession = parsed.openLastSession ?? defaults.openLastSession;
			settings.locale = parsed.locale ?? defaults.locale;
			settings.minimizeToTray = parsed.minimizeToTray ?? defaults.minimizeToTray;
			settings.startWithSystem = parsed.startWithSystem ?? defaults.startWithSystem;
			settings.typeShellSetup = parsed.typeShellSetup ?? defaults.typeShellSetup;
			settings.showLoginMessage = parsed.showLoginMessage ?? defaults.showLoginMessage;
			settings.sessionLogMode = parsed.sessionLogMode ?? defaults.sessionLogMode;
			settings.sessionLogFolder = parsed.sessionLogFolder ?? defaults.sessionLogFolder;
			settings.sessionLogName = parsed.sessionLogName ?? defaults.sessionLogName;
			settings.sessionLogAppend = parsed.sessionLogAppend ?? defaults.sessionLogAppend;
			settings.sessionLogHeader = parsed.sessionLogHeader ?? defaults.sessionLogHeader;
			settings.rdpHardwareRendering = parsed.rdpHardwareRendering ?? defaults.rdpHardwareRendering;
			settings.rdpGraphicsPipeline = parsed.rdpGraphicsPipeline ?? defaults.rdpGraphicsPipeline;
			settings.pendingTursoOrg = parsed.pendingTursoOrg ?? defaults.pendingTursoOrg;
			settings.launches = parsed.launches ?? defaults.launches;
			settings.communityPromptDismissed =
				parsed.communityPromptDismissed ?? defaults.communityPromptDismissed;
			settings.communityPromptLastShown =
				parsed.communityPromptLastShown ?? defaults.communityPromptLastShown;
			// Absent for anyone upgrading, which is exactly who should see the
			// badges — so it stays empty rather than being seeded.
			settings.seenFeatures = parsed.seenFeatures ?? defaults.seenFeatures;
			// A fresh install starts with every DevOps tool off, so Reach opens
			// as a remote-access app. Anyone upgrading has had Ansible and
			// OpenTofu in the tab bar all along, and an update should not take
			// them away, so they keep both until they choose otherwise.
			settings.devopsTools = parsed.devopsTools ?? ['ansible', 'tofu'];
			settings.autoLockMinutes = parsed.autoLockMinutes ?? defaults.autoLockMinutes;
			settings.lockOnSystemLock = parsed.lockOnSystemLock ?? defaults.lockOnSystemLock;
			// Migration: existing users who already have localStorage data get setupComplete: true
			settings.setupComplete = parsed.setupComplete ?? true;
		}
	} catch {
		// If parsing fails, keep defaults
	}
}

/**
 * Record that the user has been shown one or more new features.
 *
 * Additive and idempotent: marking something already seen is a no-op, so a
 * component can call this on every render of the thing without churning
 * localStorage.
 */
export function markFeaturesSeen(ids: string[]): void {
	const fresh = ids.filter((id) => !settings.seenFeatures.includes(id));
	if (fresh.length === 0) return;
	settings.seenFeatures = [...settings.seenFeatures, ...fresh];
	saveSettings();
}

/** Launches before the community prompt is allowed to appear at all. */
const PROMPT_AFTER_LAUNCHES = 3;

/** How long "Not now" buys, in days. */
const PROMPT_SNOOZE_DAYS = 21;

/**
 * Count this launch. Called once, from the root layout.
 *
 * Separate from the prompt check so the count keeps rising even when the
 * prompt can never show — otherwise turning the prompt back on would start
 * someone from zero again.
 */
export function recordLaunch(): void {
	settings.launches += 1;
	saveSettings();
}

/**
 * Whether to show the community prompt on this launch.
 *
 * Deliberately not on first run. Someone who has opened Reach once has not
 * decided whether they like it yet, and asking them to join a chat server is
 * the fastest way to make the answer no.
 */
export function shouldShowCommunityPrompt(): boolean {
	if (settings.communityPromptDismissed) return false;
	if (!settings.setupComplete) return false;
	if (settings.launches < PROMPT_AFTER_LAUNCHES) return false;

	const snooze = PROMPT_SNOOZE_DAYS * 24 * 60 * 60 * 1000;
	return Date.now() - settings.communityPromptLastShown > snooze;
}

/** "Not now" — defer, do not silence. */
export function snoozeCommunityPrompt(): void {
	settings.communityPromptLastShown = Date.now();
	saveSettings();
}

/** The checkbox, and also what joining does: neither wants asking again. */
export function dismissCommunityPrompt(): void {
	settings.communityPromptDismissed = true;
	settings.communityPromptLastShown = Date.now();
	saveSettings();
}

export function saveSettings(): void {
	if (typeof localStorage === 'undefined') return;

	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...settings, pendingTursoApiToken: undefined }));
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
