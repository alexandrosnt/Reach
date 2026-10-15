import { invoke } from '@tauri-apps/api/core';

/**
 * External password managers (src-tauri/src/pwmanager).
 *
 * A saved session or database connection can point a secret field at an entry
 * in a password manager instead of keeping the secret in Reach's vault. The
 * backend resolves the reference at connect time; no command here ever returns
 * a secret to the webview. Secrets configured for the provider itself (the
 * Passbolt private key, its passphrase, a TOTP secret) are write-only: the view
 * only says whether one is stored.
 */

/** A pointer to an entry in a password manager. Not secret. */
export interface SecretRef {
	/** Provider id, e.g. "passbolt". */
	provider: string;
	/** The provider's id for the entry (Passbolt: the resource UUID). */
	entry_id: string;
	/** The entry's name when it was picked; for display only, never trusted. */
	label?: string | null;
	/** Signed approvals, one per person who allowed this entry for the session. Kept as they came: an editor must send them back. */
	approvals?: string[];
	/** Read only: the current user approved this entry for the session as it now is. Ignored when sent. */
	trusted?: boolean;
	/** Set by the editor on an entry the user picked in this edit, so saving approves it. Never comes back. */
	confirm?: boolean;
}

/** An entry's metadata, as listed by a provider. Never holds a secret. */
export interface EntrySummary {
	id: string;
	name: string;
	username?: string | null;
	uri?: string | null;
	folder_id?: string | null;
}

export interface ProviderStatus {
	id: string;
	name: string;
	enabled: boolean;
	/** The provider can run here (its CLI was found, the platform supports it). */
	available: boolean;
	/** Enough is configured to log in. */
	configured: boolean;
	version?: string | null;
	detail?: string | null;
}

export type PassboltCredentials = 'reach' | 'cli_config';
export type PassboltMfaMode = 'none' | 'totp_secret';
/** Ask for the passphrase when a sign-in needs it (kept in memory only), or keep it in Reach's vault. */
export type PassboltPassphraseMode = 'ask' | 'stored';

/** "Remember for" choices in Settings, in seconds; 0 = ask every time. The backend allows up to 8 hours. */
export const REMEMBER_CHOICES = [0, 300, 900, 3600, 14400] as const;
export const REMEMBER_DEFAULT = 900;

export interface PassboltView {
	/**
	 * The `passbolt` binary; null = found on PATH. Stored on this device only,
	 * never in the synced vault. Must be named passbolt / passbolt.exe.
	 */
	cli_path: string | null;
	credentials: PassboltCredentials;
	server_address: string;
	mfa_mode: PassboltMfaMode;
	timeout_secs: number;
	has_private_key: boolean;
	has_passphrase: boolean;
	has_totp_secret: boolean;
	passphrase_mode: PassboltPassphraseMode;
	/** How long an asked passphrase is kept in memory; 0 = not kept. */
	passphrase_remember_secs: number;
	/** An asked passphrase is in memory right now ("Forget now" has something to forget). */
	passphrase_cached: boolean;
}

export interface PasswordManagersView {
	enabled: boolean;
	passbolt: PassboltView;
}

export interface PassboltConfigInput {
	/** Saved on this device only; checked on disk when it changes and the feature is on. */
	cli_path: string | null;
	credentials: PassboltCredentials;
	server_address: string;
	mfa_mode: PassboltMfaMode;
	timeout_secs: number;
	/** "ask" removes a stored passphrase on save; a passphrase sent with it is ignored. */
	passphrase_mode: PassboltPassphraseMode;
	passphrase_remember_secs: number;
	/** Absent or null keeps what is stored. */
	private_key?: string | null;
	/** A local file (no network share) the backend reads the armored private key from; at most 32,000 bytes. */
	private_key_file?: string | null;
	passphrase?: string | null;
	totp_secret?: string | null;
	clear_private_key?: boolean;
	clear_passphrase?: boolean;
	clear_totp_secret?: boolean;
}

export interface PasswordManagersConfigInput {
	enabled: boolean;
	passbolt: PassboltConfigInput;
}

export interface TestResult {
	ok: boolean;
	entries: number;
	message: string;
}

export function pwmanagerGetConfig(): Promise<PasswordManagersView> {
	return invoke<PasswordManagersView>('pwmanager_get_config');
}

export function pwmanagerSaveConfig(config: PasswordManagersConfigInput): Promise<PasswordManagersView> {
	return invoke<PasswordManagersView>('pwmanager_save_config', { config });
}

export function pwmanagerStatus(): Promise<ProviderStatus[]> {
	return invoke<ProviderStatus[]>('pwmanager_status');
}

export function pwmanagerTest(provider: string): Promise<TestResult> {
	return invoke<TestResult>('pwmanager_test', { provider });
}

/**
 * Entries, metadata only. The backend caches the full listing per account for
 * a few minutes and runs one sign-in at a time; callers should still not send
 * a new request while one is out (see SecretRefPicker).
 */
export function pwmanagerList(provider: string, query?: string | null, refresh?: boolean): Promise<EntrySummary[]> {
	return invoke<EntrySummary[]>('pwmanager_list', {
		provider,
		query: query?.trim() ? query.trim() : null,
		refresh: refresh ?? null,
	});
}

/** The non-secret part of a view, as the input for a save that changes no secret. */
export function configInputFromView(view: PasswordManagersView): PasswordManagersConfigInput {
	const p = view.passbolt;
	return {
		enabled: view.enabled,
		passbolt: {
			cli_path: p.cli_path,
			credentials: p.credentials,
			server_address: p.server_address,
			mfa_mode: p.mfa_mode,
			timeout_secs: p.timeout_secs,
			passphrase_mode: p.passphrase_mode,
			passphrase_remember_secs: p.passphrase_remember_secs,
		},
	};
}

/**
 * The backend asks for a password manager's passphrase (Passbolt, with
 * "Ask when needed"). One question per account at a time; answered with
 * `pwmanagerPassphraseResponse`, withdrawn with PASSPHRASE_PROMPT_CLOSED_EVENT.
 */
export const PASSPHRASE_PROMPT_EVENT = 'pwmanager-passphrase-prompt';
/** Payload: the promptId of a question that is no longer open. */
export const PASSPHRASE_PROMPT_CLOSED_EVENT = 'pwmanager-passphrase-prompt-closed';

export interface PassphrasePrompt {
	promptId: string;
	/** "passbolt" */
	provider: string;
	/** "Passbolt" */
	providerName: string;
	reason: 'connect' | 'list' | 'test' | 'other';
	/** What is being connected to, for "connect". */
	target: string | null;
	/** The previous passphrase was not accepted. */
	retry: boolean;
	/** Why, in the backend's words. */
	error: string | null;
	/** How long the answer is kept in memory; 0 = not kept. */
	rememberSecs: number;
}

/** Answer a passphrase question; null cancels. Rejected (question stays open) if the passphrase cannot be taken. */
export function pwmanagerPassphraseResponse(promptId: string, passphrase: string | null): Promise<void> {
	return invoke('pwmanager_passphrase_response', { promptId, passphrase });
}

/** Drop any passphrase kept in memory. */
export function pwmanagerForgetPassphrase(): Promise<void> {
	return invoke('pwmanager_forget_passphrase');
}
