import { invoke } from '@tauri-apps/api/core';

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

function encodeKey(key: string): string {
	if (!key) return '';
	try {
		return btoa(key);
	} catch {
		return key;
	}
}

function decodeKey(encoded: string): string {
	if (!encoded) return '';
	try {
		return atob(encoded);
	} catch {
		return encoded;
	}
}

export function getAISettings(): AISettings {
	return aiSettings;
}

export function updateAISetting<K extends keyof AISettings>(key: K, value: AISettings[K]): void {
	aiSettings[key] = value;
	saveAISettings();
}

export function loadAISettings(): void {
	if (typeof localStorage === 'undefined') return;
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		if (raw) {
			const parsed = JSON.parse(raw) as Partial<{ enabled: boolean; apiKey: string; selectedModel: string }>;
			aiSettings.enabled = parsed.enabled ?? false;
			aiSettings.apiKey = decodeKey(parsed.apiKey ?? '');
			aiSettings.selectedModel = parsed.selectedModel ?? '';
		}
	} catch {
		// Corrupted data — keep defaults
	}
}

export function saveAISettings(): void {
	if (typeof localStorage === 'undefined') return;
	try {
		localStorage.setItem(
			STORAGE_KEY,
			JSON.stringify({
				enabled: aiSettings.enabled,
				apiKey: encodeKey(aiSettings.apiKey),
				selectedModel: aiSettings.selectedModel
			})
		);
	} catch {
		// Storage full or unavailable
	}
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
