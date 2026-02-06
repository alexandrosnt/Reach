import { invoke } from '@tauri-apps/api/core';

// Types
export interface VaultInfo {
	id: string;
	name: string;
	vaultType: 'private' | 'shared';
	memberCount?: number;
	secretCount: number;
	lastSync?: number;
}

export interface SecretMetadata {
	id: string;
	name: string;
	category: string;
	createdAt: number;
	updatedAt: number;
}

export interface MemberInfo {
	userUuid: string;
	publicKey: string;
	role: string;
	addedAt: number;
}

export interface InviteInfo {
	vaultId: string;
	syncUrl: string;
	token: string;
}

// ==================== IDENTITY ====================

export async function initIdentity(password: string): Promise<string> {
	return invoke<string>('vault_init_identity', { password });
}

export async function unlock(password: string): Promise<boolean> {
	return invoke<boolean>('vault_unlock', { password });
}

export async function lock(): Promise<void> {
	return invoke('vault_lock');
}

export async function isLocked(): Promise<boolean> {
	return invoke<boolean>('vault_is_locked');
}

export async function hasIdentity(): Promise<boolean> {
	return invoke<boolean>('vault_has_identity');
}

/** Auto-unlock using OS keychain (TLS-style, no password needed). */
export async function autoUnlock(): Promise<boolean> {
	return invoke<boolean>('vault_auto_unlock');
}

/** Export identity for backup/multi-device (returns base64 secret key).
 * WARNING: This is SENSITIVE! Protect this value! */
export async function exportIdentity(): Promise<string> {
	return invoke<string>('vault_export_identity');
}

/** Import identity from backup (for new device). */
export async function importIdentity(secretKey: string): Promise<string> {
	return invoke<string>('vault_import_identity', { secret_key: secretKey });
}

export async function getPublicKey(): Promise<string | null> {
	return invoke<string | null>('vault_get_public_key');
}

export async function getUserUuid(): Promise<string | null> {
	return invoke<string | null>('vault_get_user_uuid');
}

// ==================== VAULT MANAGEMENT ====================

export async function createVault(
	name: string,
	vaultType: 'private' | 'shared',
	syncUrl?: string,
	syncToken?: string
): Promise<VaultInfo> {
	return invoke<VaultInfo>('vault_create', {
		name,
		vault_type: vaultType,
		sync_url: syncUrl,
		sync_token: syncToken
	});
}

export async function openVault(
	vaultId: string,
	syncUrl?: string,
	token?: string
): Promise<VaultInfo> {
	return invoke<VaultInfo>('vault_open', { vault_id: vaultId, sync_url: syncUrl, token });
}

export async function closeVault(vaultId: string): Promise<void> {
	return invoke('vault_close', { vault_id: vaultId });
}

export async function deleteVault(vaultId: string): Promise<void> {
	return invoke('vault_delete', { vault_id: vaultId });
}

export async function listVaults(): Promise<VaultInfo[]> {
	return invoke<VaultInfo[]>('vault_list');
}

export async function unlockVault(vaultId: string): Promise<void> {
	return invoke('vault_unlock_vault', { vault_id: vaultId });
}

export async function lockVault(vaultId: string): Promise<void> {
	return invoke('vault_lock_vault', { vault_id: vaultId });
}

export async function syncVault(vaultId: string): Promise<void> {
	return invoke('vault_sync', { vault_id: vaultId });
}

// ==================== SECRETS ====================

export async function createSecret(
	vaultId: string,
	name: string,
	category: string,
	value: string
): Promise<string> {
	return invoke<string>('vault_secret_create', { vault_id: vaultId, name, category, value });
}

export async function readSecret(vaultId: string, secretId: string): Promise<string> {
	return invoke<string>('vault_secret_read', { vault_id: vaultId, secret_id: secretId });
}

export async function updateSecret(vaultId: string, secretId: string, value: string): Promise<void> {
	return invoke('vault_secret_update', { vault_id: vaultId, secret_id: secretId, value });
}

export async function deleteSecret(vaultId: string, secretId: string): Promise<void> {
	return invoke('vault_secret_delete', { vault_id: vaultId, secret_id: secretId });
}

export async function listSecrets(vaultId: string): Promise<SecretMetadata[]> {
	return invoke<SecretMetadata[]>('vault_secret_list', { vault_id: vaultId });
}

// ==================== SHARING ====================

export async function inviteMember(
	vaultId: string,
	inviteePublicKey: string,
	inviteeUuid: string,
	role: string
): Promise<InviteInfo> {
	return invoke<InviteInfo>('vault_invite_member', {
		vault_id: vaultId,
		invitee_public_key: inviteePublicKey,
		invitee_uuid: inviteeUuid,
		role
	});
}

export async function acceptInvite(syncUrl: string, token: string): Promise<VaultInfo> {
	return invoke<VaultInfo>('vault_accept_invite', { sync_url: syncUrl, token });
}

export async function removeMember(vaultId: string, userUuid: string): Promise<void> {
	return invoke('vault_remove_member', { vault_id: vaultId, user_uuid: userUuid });
}

export async function listMembers(vaultId: string): Promise<MemberInfo[]> {
	return invoke<MemberInfo[]>('vault_list_members', { vault_id: vaultId });
}

// ==================== SHARE INDIVIDUAL ITEMS ====================

export interface SharedItemInfo {
	id: string;
	secretId: string;
	recipientUuid: string;
	expiresAt?: number;
	createdAt: number;
}

export interface ShareItemResult {
	shareId: string;
	secretId: string;
	recipientUuid: string;
	syncUrl?: string;
	expiresAt?: number;
}

export interface ReceivedShare {
	shareId: string;
	secretId: string;
	secretName: string;
	category: string;
	sharerUuid: string;
	receivedAt: number;
}

/** Share a specific secret (session/credential) with another user. */
export async function shareItem(
	vaultId: string,
	secretId: string,
	recipientUuid: string,
	recipientPublicKey: string,
	expiresInHours?: number
): Promise<ShareItemResult> {
	return invoke<ShareItemResult>('vault_share_item', {
		vault_id: vaultId,
		secret_id: secretId,
		recipient_uuid: recipientUuid,
		recipient_public_key: recipientPublicKey,
		expires_in_hours: expiresInHours
	});
}

/** List items shared from a vault. */
export async function listSharedItems(vaultId: string): Promise<SharedItemInfo[]> {
	return invoke<SharedItemInfo[]>('vault_list_shared_items', { vault_id: vaultId });
}

/** Revoke a shared item. */
export async function revokeSharedItem(vaultId: string, shareId: string): Promise<void> {
	return invoke('vault_revoke_shared_item', { vault_id: vaultId, share_id: shareId });
}

/** Accept a shared item (copy to local vault). */
export async function acceptSharedItem(
	sourceVaultId: string,
	shareId: string,
	targetVaultId: string
): Promise<string> {
	return invoke<string>('vault_accept_shared_item', {
		source_vault_id: sourceVaultId,
		share_id: shareId,
		target_vault_id: targetVaultId
	});
}

/** List items shared with me. */
export async function listReceivedShares(): Promise<ReceivedShare[]> {
	return invoke<ReceivedShare[]>('vault_list_received_shares');
}

// ==================== APP SETTINGS (ENCRYPTED) ====================

export interface AppSettings {
	tursoOrg?: string;
	tursoApiToken?: string;
	tursoGroup?: string;
	personalDbUrl?: string;
	personalDbToken?: string;
	syncEnabled: boolean;
	theme?: string;
	[key: string]: unknown;
}

// ==================== TURSO PLATFORM API ====================

export interface TursoDbInfo {
	dbId: string;
	hostname: string;
	name: string;
}

/** Create a new database in Turso (for shared vaults). */
export async function createTursoDatabase(dbName: string): Promise<TursoDbInfo> {
	return invoke<TursoDbInfo>('turso_create_database', { db_name: dbName });
}

/** Create an auth token for a Turso database. */
export async function createTursoDatabaseToken(dbName: string): Promise<string> {
	return invoke<string>('turso_create_database_token', { db_name: dbName });
}

/** Get app settings. */
export async function getSettings(): Promise<AppSettings> {
	return invoke<AppSettings>('vault_get_settings');
}

/** Save app settings. */
export async function saveSettings(settings: AppSettings): Promise<void> {
	return invoke('vault_save_settings', { settings });
}

/** Get Turso config. */
export async function getTursoConfig(): Promise<[string | null, string | null]> {
	return invoke<[string | null, string | null]>('vault_get_turso_config');
}

/** Set Turso config. */
export async function setTursoConfig(org?: string, token?: string): Promise<void> {
	return invoke('vault_set_turso_config', { org, token });
}

// ==================== FULL BACKUP ====================

export interface BackupPreview {
	version: number;
	exportedAt: number;
	userUuid: string;
	vaultCount: number;
	secretCount: number;
	hasSyncConfig: boolean;
}

/** Export a full encrypted backup to a file. */
export async function exportBackup(exportPassword: string, filePath: string): Promise<void> {
	return invoke('vault_export_backup', { export_password: exportPassword, file_path: filePath });
}

/** Preview a backup file (validate and return metadata). */
export async function previewBackup(filePath: string, exportPassword: string): Promise<BackupPreview> {
	return invoke<BackupPreview>('vault_preview_backup', { file_path: filePath, export_password: exportPassword });
}

/** Import a full encrypted backup from a file. */
export async function importBackup(filePath: string, exportPassword: string, masterPassword: string): Promise<string> {
	return invoke<string>('vault_import_backup', { file_path: filePath, export_password: exportPassword, master_password: masterPassword });
}

// ==================== PERSONAL SYNC CONFIG ====================

/** Set personal sync config (for cloud backup of ALL user data). */
export async function setPersonalSync(syncUrl?: string, syncToken?: string): Promise<void> {
	return invoke('vault_set_personal_sync', { sync_url: syncUrl, sync_token: syncToken });
}

/** Get personal sync config. */
export async function getPersonalSync(): Promise<[string | null, string | null]> {
	return invoke<[string | null, string | null]>('vault_get_personal_sync');
}
