/**
 * Whether a password manager can be picked from right now.
 *
 * Secret fields (SecretRefField) only offer "From password manager" when one
 * is turned on, found and configured. Finding out runs the provider's CLI
 * once (`--version`, no login), so it is asked lazily, the first time a field
 * needs it, and again after Settings → Password managers saves.
 */

import { pwmanagerStatus, type ProviderStatus } from '$lib/ipc/pwmanager';
import { t } from '$lib/state/i18n.svelte';

let providers = $state<ProviderStatus[]>([]);
let loaded = $state(false);
let lastError = $state<string | null>(null);
let inflight: Promise<void> | null = null;
/** A newer question makes the answer to an older one irrelevant. */
let seq = 0;
let askedAt = 0;
/** How soon a "nothing usable" answer may be asked again. */
const RECHECK_MS = 3000;

/**
 * Ask once while the answer is "usable"; later calls are free until
 * `refreshPwManagers`. An answer of "nothing usable" is asked again (at most
 * every few seconds): it may predate a CLI install or a save made since, and
 * a stale "no" would hide the picker for the rest of the run.
 */
export function ensurePwManagers(): Promise<void> {
	if (inflight) return inflight;
	if (loaded && (pwManagersUsable() || Date.now() - askedAt < RECHECK_MS)) return Promise.resolve();
	return refreshPwManagers();
}

/** Ask the backend again: after a save, or once a CLI was installed. */
export function refreshPwManagers(): Promise<void> {
	const mine = ++seq;
	askedAt = Date.now();
	const run = pwmanagerStatus()
		.then((list) => {
			if (mine !== seq) return;
			providers = list;
			lastError = null;
		})
		.catch((e) => {
			// An older backend, or a platform without providers: nothing to offer.
			if (mine !== seq) return;
			providers = [];
			lastError = String(e);
		})
		.finally(() => {
			if (mine !== seq) return;
			loaded = true;
			inflight = null;
		});
	inflight = run;
	return run;
}

/** The first answer has come (until then nothing is known to be missing). */
export function pwManagersLoaded(): boolean {
	return loaded;
}

/** Why the last status question failed, if it did. */
export function pwManagerStatusError(): string | null {
	return lastError;
}

/** Every provider the backend knows, usable or not (for Settings). */
export function pwManagerProviders(): ProviderStatus[] {
	return providers;
}

/** Providers a secret can be picked from. */
export function usableProviders(): ProviderStatus[] {
	return providers.filter((p) => p.enabled && p.available && p.configured);
}

export function pwManagersUsable(): boolean {
	return usableProviders().length > 0;
}

/** A provider's display name ("Passbolt"), known or not. */
export function providerName(id: string): string {
	const known = providers.find((p) => p.id === id);
	if (known) return known.name;
	return id ? id.charAt(0).toUpperCase() + id.slice(1) : id;
}

/**
 * How long an asked passphrase is kept, in words: "5 minutes", "1 hour", or
 * "Ask every time" for 0. Settings offers fixed choices; any other stored
 * value is shown in minutes.
 */
export function rememberLabel(secs: number): string {
	if ([0, 300, 900, 3600, 14400].includes(secs)) return t(`pwmanager.remember_${secs}`);
	return t('pwmanager.remember_custom', { minutes: String(Math.max(1, Math.round(secs / 60))) });
}
