use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use secrecy::SecretBox;
use tauri::State;

use crate::state::AppState;
use crate::vault::{
    AppSettings, InviteInfo, MemberInfo, MemberRole, ReceivedShare, SecretCategory, SecretMetadata,
    ShareItemResult, SharedItemInfo, VaultInfo, VaultType,
};

// ==================== IDENTITY ====================

#[tauri::command]
#[tracing::instrument(skip(password, state))]
pub async fn vault_init_identity(
    password: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let mut manager = state.vault_manager.lock().await;
    manager.init_identity(&password).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(password, state))]
pub async fn vault_unlock(
    password: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let mut manager = state.vault_manager.lock().await;
    manager.unlock(&password).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_lock(state: State<'_, AppState>) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    manager.lock();
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_is_locked(state: State<'_, AppState>) -> Result<bool, String> {
    let manager = state.vault_manager.lock().await;
    Ok(manager.is_locked())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_has_identity(state: State<'_, AppState>) -> Result<bool, String> {
    let manager = state.vault_manager.lock().await;
    Ok(manager.has_identity().await)
}

/// Auto-unlock using OS keychain (TLS-style, no password needed).
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_auto_unlock(state: State<'_, AppState>) -> Result<bool, String> {
    let mut manager = state.vault_manager.lock().await;
    manager.auto_unlock().await.map_err(|e| e.to_string())
}

/// Reset vault - delete all local data and start fresh.
/// WARNING: This is destructive! All encrypted data will be lost.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_reset(state: State<'_, AppState>) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    manager.reset().await.map_err(|e| e.to_string())
}

/// Export identity for backup/multi-device (returns base64 secret key).
/// WARNING: This is sensitive! User must protect this value.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_export_identity(state: State<'_, AppState>) -> Result<String, String> {
    let manager = state.vault_manager.lock().await;
    manager.export_identity().map_err(|e| e.to_string())
}

/// Import identity from backup (for new device).
#[tauri::command]
#[tracing::instrument(skip(state, secret_key))]
pub async fn vault_import_identity(
    secret_key: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let mut manager = state.vault_manager.lock().await;
    manager.import_identity(&secret_key).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_get_public_key(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let manager = state.vault_manager.lock().await;
    Ok(manager.get_public_key())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_get_user_uuid(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let manager = state.vault_manager.lock().await;
    Ok(manager.get_user_uuid())
}

// ==================== VAULT MANAGEMENT ====================

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state, sync_token))]
pub async fn vault_create(
    name: String,
    vault_type: String,
    sync_url: Option<String>,
    sync_token: Option<String>,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let mut manager = state.vault_manager.lock().await;

    let vt = match vault_type.as_str() {
        "private" => VaultType::Private,
        "shared" => VaultType::Shared { members: vec![] },
        _ => return Err("Invalid vault type".to_string()),
    };

    manager.create_vault(&name, vt, sync_url.as_deref(), sync_token.as_deref()).await.map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_open(
    vault_id: String,
    sync_url: Option<String>,
    token: Option<String>,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let mut manager = state.vault_manager.lock().await;
    manager
        .open_vault(&vault_id, sync_url.as_deref(), token.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_close(
    vault_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    manager.close_vault(&vault_id).await.map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_delete(
    vault_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    manager.delete_vault(&vault_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_list(state: State<'_, AppState>) -> Result<Vec<VaultInfo>, String> {
    let manager = state.vault_manager.lock().await;
    manager.list_vaults().await.map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_unlock_vault(
    vault_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    manager.unlock_vault(&vault_id).await.map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_lock_vault(
    vault_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    manager.lock_vault(&vault_id);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_sync(
    vault_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    manager.sync_vault(&vault_id).await.map_err(|e| e.to_string())
}

// ==================== SECRETS ====================

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(value, state))]
pub async fn vault_secret_create(
    vault_id: String,
    name: String,
    category: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let manager = state.vault_manager.lock().await;

    let cat: SecretCategory = category.parse().map_err(|e: String| e)?;
    let plaintext = SecretBox::new(Box::new(value.into_bytes()));

    manager
        .create_secret(&vault_id, &name, cat, plaintext)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_secret_read(
    vault_id: String,
    secret_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let manager = state.vault_manager.lock().await;

    let plaintext = manager
        .read_secret(&vault_id, &secret_id)
        .await
        .map_err(|e| e.to_string())?;

    use secrecy::ExposeSecret;
    String::from_utf8(plaintext.expose_secret().clone())
        .map_err(|e| format!("Invalid UTF-8: {}", e))
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(value, state))]
pub async fn vault_secret_update(
    vault_id: String,
    secret_id: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;

    let plaintext = SecretBox::new(Box::new(value.into_bytes()));

    manager
        .update_secret(&vault_id, &secret_id, plaintext)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_secret_delete(
    vault_id: String,
    secret_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;
    manager
        .delete_secret(&vault_id, &secret_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_secret_list(
    vault_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<SecretMetadata>, String> {
    let manager = state.vault_manager.lock().await;
    manager.list_secrets(&vault_id).await.map_err(|e| e.to_string())
}

// ==================== SHARING ====================

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_invite_member(
    vault_id: String,
    invitee_public_key: String,
    invitee_uuid: String,
    role: String,
    state: State<'_, AppState>,
) -> Result<InviteInfo, String> {
    let manager = state.vault_manager.lock().await;

    let pk_bytes = BASE64
        .decode(&invitee_public_key)
        .map_err(|e| format!("Invalid public key: {}", e))?;

    if pk_bytes.len() != 32 {
        return Err(format!("Invalid public key length: expected 32, got {}", pk_bytes.len()));
    }

    let mut public_key = [0u8; 32];
    public_key.copy_from_slice(&pk_bytes);

    let member_role: MemberRole = role.parse().map_err(|e: String| e)?;

    manager
        .invite_member(&vault_id, &public_key, &invitee_uuid, member_role)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(token, state))]
pub async fn vault_accept_invite(
    sync_url: String,
    token: String,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let mut manager = state.vault_manager.lock().await;
    manager
        .accept_invite(&sync_url, &token)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_remove_member(
    vault_id: String,
    user_uuid: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;
    manager
        .remove_member(&vault_id, &user_uuid)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_list_members(
    vault_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<MemberInfo>, String> {
    let manager = state.vault_manager.lock().await;
    manager.list_members(&vault_id).await.map_err(|e| e.to_string())
}

// ==================== SHARE INDIVIDUAL ITEMS ====================

/// Share a specific secret (session/credential) with another user.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_share_item(
    vault_id: String,
    secret_id: String,
    recipient_uuid: String,
    recipient_public_key: String,
    expires_in_hours: Option<u64>,
    state: State<'_, AppState>,
) -> Result<ShareItemResult, String> {
    let manager = state.vault_manager.lock().await;

    let pk_bytes = BASE64
        .decode(&recipient_public_key)
        .map_err(|e| format!("Invalid public key: {}", e))?;

    if pk_bytes.len() != 32 {
        return Err(format!("Invalid public key length: expected 32, got {}", pk_bytes.len()));
    }

    let mut public_key = [0u8; 32];
    public_key.copy_from_slice(&pk_bytes);

    manager
        .share_item(&vault_id, &secret_id, &recipient_uuid, &public_key, expires_in_hours)
        .await
        .map_err(|e| e.to_string())
}

/// List items shared from a vault.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_list_shared_items(
    vault_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<SharedItemInfo>, String> {
    let manager = state.vault_manager.lock().await;
    manager.list_shared_items(&vault_id).await.map_err(|e| e.to_string())
}

/// Revoke a shared item.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_revoke_shared_item(
    vault_id: String,
    share_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;
    manager.revoke_shared_item(&vault_id, &share_id).await.map_err(|e| e.to_string())
}

/// Accept a shared item (copy to local vault).
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn vault_accept_shared_item(
    source_vault_id: String,
    share_id: String,
    target_vault_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let manager = state.vault_manager.lock().await;
    manager
        .accept_shared_item(&source_vault_id, &share_id, &target_vault_id)
        .await
        .map_err(|e| e.to_string())
}

/// List items shared with me.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_list_received_shares(
    state: State<'_, AppState>,
) -> Result<Vec<ReceivedShare>, String> {
    let manager = state.vault_manager.lock().await;
    manager.list_received_shares().await.map_err(|e| e.to_string())
}

// ==================== APP SETTINGS (ENCRYPTED) ====================

/// Get app settings.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_get_settings(
    state: State<'_, AppState>,
) -> Result<AppSettings, String> {
    let manager = state.vault_manager.lock().await;
    manager.get_settings().await.map_err(|e| e.to_string())
}

/// Save app settings.
#[tauri::command]
#[tracing::instrument(skip(state, settings))]
pub async fn vault_save_settings(
    settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;
    manager.save_settings(&settings).await.map_err(|e| e.to_string())
}

/// Get Turso config.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_get_turso_config(
    state: State<'_, AppState>,
) -> Result<(Option<String>, Option<String>), String> {
    let manager = state.vault_manager.lock().await;
    manager.get_turso_config().await.map_err(|e| e.to_string())
}

/// Set Turso config.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state, token))]
pub async fn vault_set_turso_config(
    org: Option<String>,
    token: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;
    manager.set_turso_config(org, token).await.map_err(|e| e.to_string())
}

// ==================== TURSO PLATFORM API ====================

use crate::vault::{turso_api, TursoDbInfo};

/// Create a new database in Turso (for shared vaults).
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn turso_create_database(
    db_name: String,
    state: State<'_, AppState>,
) -> Result<TursoDbInfo, String> {
    let manager = state.vault_manager.lock().await;
    let settings = manager.get_settings().await.map_err(|e| e.to_string())?;

    let org = settings.turso_org.ok_or("Turso organization not configured")?;
    let api_token = settings.turso_api_token.ok_or("Turso API token not configured")?;
    let group = settings.turso_group.unwrap_or_else(|| "default".to_string());

    turso_api::create_database(&org, &api_token, &db_name, &group)
        .await
        .map_err(|e| e.to_string())
}

/// Create an auth token for a Turso database.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state))]
pub async fn turso_create_database_token(
    db_name: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let manager = state.vault_manager.lock().await;
    let settings = manager.get_settings().await.map_err(|e| e.to_string())?;

    let org = settings.turso_org.ok_or("Turso organization not configured")?;
    let api_token = settings.turso_api_token.ok_or("Turso API token not configured")?;

    turso_api::create_database_token(&org, &api_token, &db_name)
        .await
        .map_err(|e| e.to_string())
}

// ==================== FULL BACKUP ====================

use crate::vault::export::BackupPreview;

/// Export a full encrypted backup to a file.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state, export_password))]
pub async fn vault_export_backup(
    export_password: String,
    file_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;
    manager
        .export_full_backup(&export_password, &file_path)
        .await
        .map_err(|e| e.to_string())
}

/// Preview a backup file (validate and return metadata).
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state, export_password))]
pub async fn vault_preview_backup(
    file_path: String,
    export_password: String,
    state: State<'_, AppState>,
) -> Result<BackupPreview, String> {
    let manager = state.vault_manager.lock().await;
    manager
        .preview_backup(&file_path, &export_password)
        .await
        .map_err(|e| e.to_string())
}

/// Import a full encrypted backup from a file.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state, export_password, master_password))]
pub async fn vault_import_backup(
    file_path: String,
    export_password: String,
    master_password: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let mut manager = state.vault_manager.lock().await;
    manager
        .import_full_backup(&file_path, &export_password, &master_password)
        .await
        .map_err(|e| e.to_string())
}

// ==================== PERSONAL SYNC CONFIG ====================

/// Set personal sync config (for cloud backup of ALL user data).
/// This stores the sync URL and token in the identity file.
#[tauri::command(rename_all = "snake_case")]
#[tracing::instrument(skip(state, sync_token))]
pub async fn vault_set_personal_sync(
    sync_url: Option<String>,
    sync_token: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut manager = state.vault_manager.lock().await;
    manager.set_personal_sync_config(sync_url, sync_token).await.map_err(|e| e.to_string())
}

/// Get personal sync config.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn vault_get_personal_sync(
    state: State<'_, AppState>,
) -> Result<(Option<String>, Option<String>), String> {
    let manager = state.vault_manager.lock().await;
    Ok(manager.get_personal_sync_config())
}
