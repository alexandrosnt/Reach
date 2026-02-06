import { invoke } from '@tauri-apps/api/core';
import {
	getOpenRouterApiKey,
	setOpenRouterApiKey,
	getDefaultAiModel,
	setDefaultAiModel,
	loadSecureSettings
} from './settings.svelte';

export interface AISettings {
	enabled: boolean;
	apiKey: string;
	selectedModel: string;
}

export interface OpenRouterModel {
	id: string;
	name: string;
	description: string;
	context_length: number;
	pricing: { prompt: string; completion: string };
}

const STORAGE_KEY = 'reach-ai-settings';

let aiSettings = $state<AISettings>({ enabled: false, apiKey: '', selectedModel: '' });
let models = $state<OpenRouterModel[]>([]);
let modelsLoading = $state(false);
let modelsError = $state<string | undefined>();
let secureLoaded = $state(false);

export function getAISettings(): AISettings {
	return aiSettings;
}

/** Update AI setting. API key is stored encrypted in vault, others in localStorage. */
export async function updateAISetting<K extends keyof AISettings>(
	key: K,
	value: AISettings[K]
): Promise<void> {
	aiSettings[key] = value;

	if (key === 'apiKey') {
		// Store API key encrypted in vault (O(1))
		await setOpenRouterApiKey(value as string);
	} else if (key === 'selectedModel') {
		// Store model in vault too for encryption
		await setDefaultAiModel(value as string);
		saveLocalSettings();
	} else {
		saveLocalSettings();
	}
}

/** Load AI settings: localStorage for enabled, vault for API key/model. */
export function loadAISettings(): void {
	if (typeof localStorage === 'undefined') return;

	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		if (raw) {
			const parsed = JSON.parse(raw) as Partial<{ enabled: boolean; selectedModel: string }>;
			aiSettings.enabled = parsed.enabled ?? false;
			// selectedModel loaded from vault, but fallback to localStorage for migration
			if (!aiSettings.selectedModel && parsed.selectedModel) {
				aiSettings.selectedModel = parsed.selectedModel;
			}
		}
	} catch {
		// Corrupted data — keep defaults
	}
}

/** Load secure settings from vault (call after vault unlock). */
export async function loadSecureAISettings(): Promise<void> {
	if (secureLoaded) return;

	try {
		await loadSecureSettings();
		const apiKey = getOpenRouterApiKey();
		const model = getDefaultAiModel();

		if (apiKey) aiSettings.apiKey = apiKey;
		if (model) aiSettings.selectedModel = model;

		secureLoaded = true;
	} catch {
		// Vault not unlocked yet
	}
}

/** Clear secure settings (on vault lock). */
export function clearSecureAISettings(): void {
	aiSettings.apiKey = '';
	secureLoaded = false;
}

/** Save non-sensitive settings to localStorage. */
function saveLocalSettings(): void {
	if (typeof localStorage === 'undefined') return;
	try {
		localStorage.setItem(
			STORAGE_KEY,
			JSON.stringify({
				enabled: aiSettings.enabled,
				selectedModel: aiSettings.selectedModel
			})
		);
	} catch {
		// Storage full or unavailable
	}
}

/** @deprecated Use updateAISetting which handles async vault storage. */
export function saveAISettings(): void {
	saveLocalSettings();
}

export function getModels(): OpenRouterModel[] {
	return models;
}

export function getModelsLoading(): boolean {
	return modelsLoading;
}

export function getModelsError(): string | undefined {
	return modelsError;
}

export async function fetchModels(): Promise<void> {
	if (!aiSettings.apiKey) return;
	modelsLoading = true;
	modelsError = undefined;
	try {
		const result = await invoke<Array<{ id: string; name: string; description: string; context_length: number; pricing: { prompt: string; completion: string } }>>('ai_fetch_models', {
			request: { apiKey: aiSettings.apiKey }
		});
		models = result.map((m) => ({
			id: m.id,
			name: m.name,
			description: m.description,
			context_length: m.context_length,
			pricing: {
				prompt: m.pricing.prompt,
				completion: m.pricing.completion
			}
		}));
	} catch (e) {
		modelsError = String(e);
	} finally {
		modelsLoading = false;
	}
}

export function getSelectedModelInfo(): OpenRouterModel | undefined {
	return models.find((m) => m.id === aiSettings.selectedModel);
}
