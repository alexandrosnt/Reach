import { SvelteMap } from 'svelte/reactivity';
import * as vaultIpc from '$lib/ipc/vault';
import { restoreLocalSettingsFromVault } from '$lib/state/settings.svelte';
import type {
	VaultInfo,
	SecretMetadata,
	MemberInfo,
	InviteInfo,
	SharedItemInfo,
	ShareItemResult,
	ReceivedShare,
	AppSettings,
	BackupPreview
} from '$lib/ipc/vault';

// Re-export types
export type {
	VaultInfo,
	SecretMetadata,
	MemberInfo,
	InviteInfo,
	SharedItemInfo,
	ShareItemResult,
	ReceivedShare,
	AppSettings,
	BackupPreview
};

// Reactive state object - use this for Svelte 5 reactivity
class VaultState {
	locked = $state(true);
	hasIdentity = $state(false);
	keychainError = $state(false); // True when identity exists but keychain access failed
	vaults = $state(new SvelteMap<string, VaultInfo>());
	activeVaultId = $state<string | null>(null);
	secrets = $state(new SvelteMap<string, SecretMetadata>());
	userUuid = $state<string | null>(null);
	publicKey = $state<string | null>(null);

	// Derived
	get vaultList() {
		return Array.from(this.vaults.values());
	}
	get secretList() {
		return Array.from(this.secrets.values());
	}
	get activeVault() {
		return this.activeVaultId ? this.vaults.get(this.activeVaultId) ?? null : null;
	}
}

// Export singleton instance for reactivity
export const vaultState = new VaultState();

// Legacy getters for backward compatibility
export function isLocked(): boolean {
	return vaultState.locked;
}

export function getHasIdentity(): boolean {
	return vaultState.hasIdentity;
}

export function getVaults(): SvelteMap<string, VaultInfo> {
	return vaultState.vaults;
}

export function getVaultList(): VaultInfo[] {
	return vaultState.vaultList;
}

export function getActiveVaultId(): string | null {
	return vaultState.activeVaultId;
}

export function getActiveVault(): VaultInfo | null {
	return vaultState.activeVault;
}

export function getSecrets(): SvelteMap<string, SecretMetadata> {
	return vaultState.secrets;
}

export function getSecretList(): SecretMetadata[] {
	return vaultState.secretList;
}

export function getUserUuid(): string | null {
	return vaultState.userUuid;
}

export function getPublicKey(): string | null {
	return vaultState.publicKey;
}

// ==================== IDENTITY ====================

export async function initIdentity(password: string): Promise<string> {
	const uuid = await vaultIpc.initIdentity(password);
	vaultState.userUuid = uuid;
	vaultState.locked = false;
	vaultState.hasIdentity = true;
	vaultState.publicKey = await vaultIpc.getPublicKey();
	await refreshVaults();
	return uuid;
}

export async function unlock(password: string): Promise<boolean> {
	const success = await vaultIpc.unlock(password);
	if (success) {
		vaultState.locked = false;
		vaultState.userUuid = await vaultIpc.getUserUuid();
		vaultState.publicKey = await vaultIpc.getPublicKey();
		await refreshVaults();
		restoreLocalSettingsFromVault();
	}
	return success;
}

/** Auto-unlock using OS keychain (TLS-style, no password). */
export async function autoUnlock(): Promise<boolean> {
	const success = await vaultIpc.autoUnlock();
	if (success) {
		vaultState.locked = false;
		vaultState.userUuid = await vaultIpc.getUserUuid();
		vaultState.publicKey = await vaultIpc.getPublicKey();
		await refreshVaults();
		restoreLocalSettingsFromVault();
	}
	return success;
}

/** Export identity for backup/multi-device. */
export async function exportIdentity(): Promise<string> {
	return vaultIpc.exportIdentity();
}

/** Import identity from backup (new device). */
export async function importIdentity(secretKey: string): Promise<string> {
	const uuid = await vaultIpc.importIdentity(secretKey);
	vaultState.locked = false;
	vaultState.hasIdentity = true;
	vaultState.userUuid = uuid;
	vaultState.publicKey = await vaultIpc.getPublicKey();
	await refreshVaults();
	return uuid;
}

export async function lock(): Promise<void> {
	await vaultIpc.lock();
	vaultState.locked = true;
	vaultState.secrets.clear();
	vaultState.activeVaultId = null;
}

export async function checkState(): Promise<void> {
	vaultState.hasIdentity = await vaultIpc.hasIdentity();
	vaultState.locked = await vaultIpc.isLocked();
	vaultState.keychainError = false;

	// TLS-style: auto-unlock using OS keychain if identity exists
	if (vaultState.hasIdentity && vaultState.locked) {
		try {
			const success = await vaultIpc.autoUnlock();
			if (success) {
				vaultState.locked = false;
			} else {
				// Keychain key missing - data exists but can't decrypt
				// DO NOT set hasIdentity = false (that would show Initialize button)
				vaultState.keychainError = true;
			}
		} catch {
			// Keychain access failed - data exists but can't decrypt
			vaultState.keychainError = true;
		}
	}

	if (!vaultState.locked) {
		vaultState.userUuid = await vaultIpc.getUserUuid();
		vaultState.publicKey = await vaultIpc.getPublicKey();
		await refreshVaults();
		restoreLocalSettingsFromVault();
	}
}

// ==================== VAULT MANAGEMENT ====================

export async function refreshVaults(): Promise<void> {
	const list = await vaultIpc.listVaults();
	vaultState.vaults.clear();
	for (const v of list) {
		vaultState.vaults.set(v.id, v);
	}
}

export async function createVault(
	name: string,
	type: 'private' | 'shared',
	syncUrl?: string,
	syncToken?: string
): Promise<VaultInfo> {
	// For shared vaults, auto-create Turso database if no sync config provided
	if (type === 'shared' && !syncUrl) {
		// Generate unique DB name from vault name
		const dbName = `vault-${name.toLowerCase().replace(/[^a-z0-9]/g, '-')}-${Date.now()}`;

		// Create database via Turso API
		const dbInfo = await vaultIpc.createTursoDatabase(dbName);

		// Create token for this database
		const token = await vaultIpc.createTursoDatabaseToken(dbInfo.name);

		// Use the hostname as sync URL
		syncUrl = `libsql://${dbInfo.hostname}`;
		syncToken = token;
	}

	// Create vault with sync config
	const vault = await vaultIpc.createVault(name, type, syncUrl, syncToken);
	vaultState.vaults.set(vault.id, vault);
	return vault;
}

export async function openVault(vaultId: string): Promise<void> {
	const vault = await vaultIpc.openVault(vaultId);
	vaultState.vaults.set(vault.id, vault);
	await vaultIpc.unlockVault(vaultId);
	vaultState.activeVaultId = vaultId;
	await refreshSecrets();
}

export async function closeVault(): Promise<void> {
	if (vaultState.activeVaultId) {
		await vaultIpc.closeVault(vaultState.activeVaultId);
		vaultState.activeVaultId = null;
		vaultState.secrets.clear();
	}
}

export async function deleteVault(vaultId: string): Promise<void> {
	await vaultIpc.deleteVault(vaultId);
	vaultState.vaults.delete(vaultId);
	if (vaultState.activeVaultId === vaultId) {
		vaultState.activeVaultId = null;
		vaultState.secrets.clear();
	}
}

export async function setActiveVault(vaultId: string | null): Promise<void> {
	if (vaultId && vaultId !== vaultState.activeVaultId) {
		const vault = vaultState.vaults.get(vaultId);
		if (vault) {
			await vaultIpc.openVault(vaultId);
			await vaultIpc.unlockVault(vaultId);
			vaultState.activeVaultId = vaultId;
			await refreshSecrets();
		}
	} else if (!vaultId) {
		vaultState.activeVaultId = null;
		vaultState.secrets.clear();
	}
}

export async function syncVault(vaultId?: string): Promise<void> {
	const id = vaultId ?? vaultState.activeVaultId;
	if (id) {
		await vaultIpc.syncVault(id);
	}
}

// ==================== SECRETS ====================

async function refreshSecrets(): Promise<void> {
	if (!vaultState.activeVaultId) return;
	const list = await vaultIpc.listSecrets(vaultState.activeVaultId);
	vaultState.secrets.clear();
	for (const s of list) {
		vaultState.secrets.set(s.id, s);
	}
}

export async function createSecret(
	name: string,
	category: string,
	value: string
): Promise<string> {
	if (!vaultState.activeVaultId) throw new Error('No active vault');
	const secretId = await vaultIpc.createSecret(vaultState.activeVaultId, name, category, value);
	await refreshSecrets();
	return secretId;
}

export async function readSecret(secretId: string): Promise<string> {
	if (!vaultState.activeVaultId) throw new Error('No active vault');
	return vaultIpc.readSecret(vaultState.activeVaultId, secretId);
}

export async function updateSecret(secretId: string, value: string): Promise<void> {
	if (!vaultState.activeVaultId) throw new Error('No active vault');
	await vaultIpc.updateSecret(vaultState.activeVaultId, secretId, value);
	await refreshSecrets();
}

export async function deleteSecret(secretId: string): Promise<void> {
	if (!vaultState.activeVaultId) throw new Error('No active vault');
	await vaultIpc.deleteSecret(vaultState.activeVaultId, secretId);
	vaultState.secrets.delete(secretId);
}

// ==================== SHARING ====================

export async function inviteMember(
	inviteePublicKey: string,
	inviteeUuid: string,
	role: string
): Promise<InviteInfo> {
	if (!vaultState.activeVaultId) throw new Error('No active vault');
	return vaultIpc.inviteMember(vaultState.activeVaultId, inviteePublicKey, inviteeUuid, role);
}

export async function acceptInvite(syncUrl: string, token: string): Promise<VaultInfo> {
	const vault = await vaultIpc.acceptInvite(syncUrl, token);
	vaultState.vaults.set(vault.id, vault);
	return vault;
}

export async function removeMember(memberUuid: string): Promise<void> {
	if (!vaultState.activeVaultId) throw new Error('No active vault');
	await vaultIpc.removeMember(vaultState.activeVaultId, memberUuid);
}

export async function listMembers(): Promise<MemberInfo[]> {
	if (!vaultState.activeVaultId) throw new Error('No active vault');
	return vaultIpc.listMembers(vaultState.activeVaultId);
}

// ==================== SHARE INDIVIDUAL ITEMS ====================

export async function shareItem(
	secretId: string,
	recipientUuid: string,
	recipientPublicKey: string,
	expiresInHours?: number
): Promise<ShareItemResult> {
	if (!vaultState.activeVaultId) throw new Error('No active vault');
	return vaultIpc.shareItem(vaultState.activeVaultId, secretId, recipientUuid, recipientPublicKey, expiresInHours);
}

export async function listSharedItems(): Promise<SharedItemInfo[]> {
	if (!vaultState.activeVaultId) throw new Error('No active vault');
	return vaultIpc.listSharedItems(vaultState.activeVaultId);
}

export async function revokeSharedItem(shareId: string): Promise<void> {
	if (!vaultState.activeVaultId) throw new Error('No active vault');
	return vaultIpc.revokeSharedItem(vaultState.activeVaultId, shareId);
}

export async function acceptSharedItem(
	sourceVaultId: string,
	shareId: string,
	targetVaultId?: string
): Promise<string> {
	const target = targetVaultId ?? vaultState.activeVaultId;
	if (!target) throw new Error('No target vault');
	return vaultIpc.acceptSharedItem(sourceVaultId, shareId, target);
}

export async function listReceivedShares(): Promise<ReceivedShare[]> {
	return vaultIpc.listReceivedShares();
}

// ==================== APP SETTINGS (ENCRYPTED) ====================

export async function getSettings(): Promise<AppSettings> {
	return vaultIpc.getSettings();
}

export async function saveSettings(settings: AppSettings): Promise<void> {
	return vaultIpc.saveSettings(settings);
}

export async function getTursoConfig(): Promise<{ org: string | null; token: string | null }> {
	const [org, token] = await vaultIpc.getTursoConfig();
	return { org, token };
}

export async function setTursoConfig(org?: string, token?: string): Promise<void> {
	return vaultIpc.setTursoConfig(org, token);
}

// ==================== FULL BACKUP ====================

/** Export a full encrypted backup. Uses Tauri save dialog for file path. */
export async function exportBackup(exportPassword: string, filePath: string): Promise<void> {
	return vaultIpc.exportBackup(exportPassword, filePath);
}

/** Preview a backup file — validate and return metadata. */
export async function previewBackup(filePath: string, exportPassword: string): Promise<BackupPreview> {
	return vaultIpc.previewBackup(filePath, exportPassword);
}

/** Import a full encrypted backup. Resets local state and restores. */
export async function importBackup(filePath: string, exportPassword: string, masterPassword: string): Promise<string> {
	const uuid = await vaultIpc.importBackup(filePath, exportPassword, masterPassword);
	// Re-check state after import
	await checkState();
	return uuid;
}
