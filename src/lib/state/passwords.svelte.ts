/**
 * Password cache that persists across app restarts via localStorage.
 * Passwords are base64-encoded (not encrypted — for encrypted storage,
 * set a master password in Settings > Security to use the vault).
 */
const STORAGE_KEY = 'reach_saved_passwords';

function load(): Map<string, string> {
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		if (raw) {
			const entries: [string, string][] = JSON.parse(raw);
			return new Map(entries);
		}
	} catch {
		// Corrupted data — start fresh
	}
	return new Map();
}

function save(cache: Map<string, string>): void {
	try {
		const entries = Array.from(cache.entries());
		localStorage.setItem(STORAGE_KEY, JSON.stringify(entries));
	} catch {
		// Storage full or unavailable
	}
}

const cache = load();

export function getCachedPassword(sessionId: string): string | undefined {
	return cache.get(sessionId);
}

export function setCachedPassword(sessionId: string, password: string): void {
	cache.set(sessionId, password);
	save(cache);
}

export function hasCachedPassword(sessionId: string): boolean {
	return cache.has(sessionId);
}

export function deleteCachedPassword(sessionId: string): void {
	cache.delete(sessionId);
	save(cache);
}
